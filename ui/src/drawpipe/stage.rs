// SPDX-License-Identifier: GPL-2.0-or-later

use super::feed::batch_view;
use super::*;
use crate::claim_timing::Sampler;
use crate::picture::deposit::count::CountFormat;
use crate::picture::deposit::{kernel, pool::Pool, standard, DiffBand, Tap, HIT_LIMIT};

#[cfg(test)]
mod tests;

struct Prepared {
    index: usize,
    decoded_dots: bool,
    plan: Option<standard::FastFrame>,
    codes: Option<CodeLayout>,
    folds: crate::framestats::Folds,
    measured: bool,
}

type Arena = Arc<Vec<Option<Box<Prepared>>>>;
const SLICE: usize = 64;

impl View {
    pub(super) fn record_hit_bound(&self) -> u64 {
        let window = self.m.time_per_div() as f64 * HDIV as f64;
        let dt = window / crate::sparse::POINTS as f64;
        standard::record_hit_bound(self.w, window / dt, self.m.delay as f64 / window)
    }
}

enum Job {
    Place(Arc<Batch>, Arc<View>, Box<Prepared>),
    Tile(Arc<Batch>, Arena, usize, DiffBand),
}

fn run(scratch: &mut Frame, job: &mut Job) {
    match job {
        Job::Place(b, v, p) => place(b, v, p, scratch),
        Job::Tile(b, placed, count, band) => {
            for p in placed[..*count].iter().flatten() {
                if p.decoded_dots {
                    continue;
                }
                let Some(plan) = &p.plan else { continue };
                let samples = p.codes.map_or(kernel::SamplesRef::Volts(&[]), |layout| {
                    kernel::SamplesRef::Codes(layout.channel(chunk(b, p.index), band.channel))
                });
                paint_band(band, samples, &plan.plan);
            }
        }
    }
}

fn paint_band(band: &mut DiffBand, samples: kernel::SamplesRef<'_>, plan: &standard::FastPlan) {
    let (a, z) = plan.rows(band.channel);
    if a < z {
        band.touch(a..z);
        band.paint(samples, plan);
    }
}

fn newest_at(b: &Batch, index: usize) -> Option<&Frame> {
    b.newest.as_deref().filter(|_| index + 1 == b.frames)
}

fn chunk(b: &Batch, i: usize) -> &[u8] {
    &b.lease.raw.as_ref().expect("batch owns its buffer until every job returns").bytes()[i * b.fb..(i + 1) * b.fb]
}

fn place(b: &Batch, v: &View, p: &mut Prepared, scratch: &mut Frame) {
    let previous = p.plan.take().and_then(|frame| frame.plan.into_raw());
    let f = placement_frame(b, v, p, scratch);
    let spacing = f.dt / v.m.time_per_div() as f64 * v.w as f64 / HDIV as f64;
    let pairs = crate::compress::live(f) && spacing <= 0.5;
    p.decoded_dots = false;
    if !pairs && (spacing > 0.5 || f.envelope || f.roll || f.scan || crate::deepmeas::deep_raw_frame(f)) {
        let rec = crate::sparse::record_with(&v.m, f, v.interp).unwrap_or_else(|| {
            let window = v.m.time_per_div() as f64 * HDIV as f64;
            crate::board::display_record(f, &v.m, crate::sparse::POINTS, window, v.m.delay as f64 - window / 2.0)
        });
        p.plan = Some(standard::FastFrame::record(rec, standard::projection(&v.m, v.w, v.h, spacing), [None; NCH]));
        return;
    }
    let words = p.codes.map(|layout| (chunk(b, p.index), layout));
    p.plan = Some(standard::batch_plan(&v.m, (v.w, v.h), f, words, previous));
    p.decoded_dots = v.m.dots_at(spacing) && p.codes.is_none() && !pairs;
}

fn placement_frame<'a>(b: &'a Batch, v: &View, p: &mut Prepared, scratch: &'a mut Frame) -> &'a Frame {
    if let Some(newest) = newest_at(b, p.index) {
        p.codes = None;
        p.measured = false;
        return newest;
    }
    let bytes = chunk(b, p.index);
    p.codes = b.codes.as_ref().and_then(|codes| codes(bytes, scratch));
    let spacing = scratch.dt / v.m.time_per_div() as f64 * v.w as f64 / HDIV as f64;
    if p.codes.is_none() || spacing > 0.5 || scratch.envelope || scratch.roll || scratch.scan || !p.folds.is_empty() {
        b.decode.decode(p.index, bytes, scratch);
        p.codes = None;
    }
    b.header.tag(scratch);
    p.measured = crate::framestats::fold_frame(scratch, &v.m, &mut p.folds);
    scratch
}

pub(super) fn work(
    rx: Receiver<Message>,
    view: &Mutex<ViewState>,
    slot: &Handoff,
    n: usize,
    little: u64,
    counters: &'static Counters,
) {
    let mut painter = Painter::new(n, little);
    let mut current: Option<Deposit> = None;
    let mut statistics = None;
    while let Ok(message) = rx.recv() {
        let state = oos_hal::sync::lock(view);
        let reset_before = slot.reset_before();
        if let Some(deposit) = current.take_if(|d| d.predates(reset_before)) {
            deposit.reject(state.identity, counters);
        }
        let b = match message {
            Message::Batch(b) => b,
            Message::Wake => {
                if let Some(d) = current.take() {
                    current = slot.put(d).err();
                }
                continue;
            }
            #[cfg(test)]
            Message::Barrier(reply) => {
                let _ = reply.send(());
                continue;
            }
        };
        let v = batch_view(&b, &state, reset_before);
        let identity = state.identity;
        let reader = v.is_none().then(|| state.view.clone()).flatten();
        drop(state);
        let Some(v) = v else {
            if same_pair(b.header.identity(), identity) {
                let _ = painter.send_tap(slot, &b, TapProjection::Unpainted, reader.as_deref(), identity);
            }
            continue;
        };
        let key = (v.gen, b.header.identity().map(|id| id.stats_gen()));
        if statistics != Some(key) {
            for p in Arc::get_mut(&mut painter.arena).expect("all previous batch jobs returned").iter_mut().flatten() {
                p.folds = crate::framestats::folds_for(&v.m);
            }
            statistics = Some(key);
        }
        let Some(deposit) = painter.batch(current.take(), slot, &b, &v, identity) else {
            if slot.closed() {
                break;
            }
            continue;
        };
        current = slot.put(deposit).err();
    }
    if let Some(deposit) = current {
        deposit.reject(oos_hal::sync::lock(view).identity, counters);
    }
}

fn prepare(
    mut current: Option<Deposit>,
    slot: &Handoff,
    b: &Batch,
    v: &View,
    identity: Option<oos_core::change::Identity>,
    (hits, format): (u64, CountFormat),
) -> Option<Deposit> {
    if current.as_ref().is_some_and(|d| !d.matches(v.gen, v.w, v.h) || !same_pair(d.identity, b.header.identity())) {
        let old = current.take().expect("the preceding check found a deposit");
        old.reject(identity, b.counters);
        slot.recycle(old);
    }
    if current.as_ref().is_some_and(|d| d.format() != format || d.extent.hits() + hits > format.hit_limit()) {
        let full = current.take().expect("the weight limit applies to an existing deposit");
        if let Err(full) = slot.send(full) {
            full.reject(identity, b.counters);
            return None;
        }
    }
    let deposit = current.get_or_insert_with(|| {
        let spare = slot.spare();
        match spare {
            Some(mut d) if d.matches(v.gen, v.w, v.h) => {
                d.reset(b.header.identity());
                d
            }
            other => {
                drop(other);
                Deposit::new(v.gen, (v.w, v.h), b.header.identity(), v.m.ch.each_ref().map(|c| c.on))
            }
        }
    });
    deposit.set_format(format);
    if b.header.identity().map(|id| id.stats_gen()) > deposit.identity.map(|id| id.stats_gen()) {
        Counters::add(&b.counters.losses.folds, deposit.folds.iter().map(|(_, f)| f.n).sum());
        deposit.folds.clear();
        deposit.identity = b.header.identity();
    }
    current
}

struct Painter {
    pool: Pool<Job, Frame>,
    arena: Arena,
    jobs: Vec<Job>,
    sampler: Sampler,
    tap_folds: crate::framestats::Folds,
}

impl Painter {
    fn new(workers: usize, little: u64) -> Self {
        Self {
            pool: Pool::new(workers, little, run),
            arena: Arena::default(),
            jobs: Vec::new(),
            sampler: Sampler::default(),
            tap_folds: Vec::new(),
        }
    }

    fn send_tap(
        &mut self,
        slot: &Handoff,
        b: &Batch,
        projection: TapProjection,
        view: Option<&View>,
        identity: Option<oos_core::change::Identity>,
    ) -> Option<u64> {
        let frame = b.newest.clone()?;
        let reader = view.map(|v| (v, identity)).or_else(|| b.header.reader.as_ref().map(|(v, id)| (&**v, *id)));
        let folds = reader.and_then(|(v, id)| {
            crate::framestats::TapFolds::compute(&frame, &v.m, id.map(|id| id.stats_gen()), &mut self.tap_folds)
        });
        let tap = Tap { frame, identity: b.header.identity(), projection, folds };
        slot.publish_tap(tap, b.view_gen_at_submit, b.header.tap.as_ref(), b.header.order.as_ref())
    }

    fn batch(
        &mut self,
        current: Option<Deposit>,
        slot: &Handoff,
        b: &Arc<Batch>,
        v: &Arc<View>,
        identity: Option<oos_core::change::Identity>,
    ) -> Option<Deposit> {
        let start = b.next.load(Ordering::Relaxed);
        if start == b.frames {
            let _ = self.send_tap(slot, b, TapProjection::Unpainted, Some(v), identity);
            return current;
        }
        let slices = (start..b.frames).step_by(SLICE);
        let sampled = b.counters.diagnostics_enabled();
        let prior_hits = current.as_ref().map_or(0, |d| d.extent.hits());
        let frames = b.frames - start;
        // Lines use one span per column or a fallback resampled to POINTS, including any newest replacement.
        // The latter bounds every tile of either route for this claimed view.
        let line_hits = || v.record_hit_bound().saturating_mul(frames as u64);
        let preflight = frames <= SLICE || v.m.dots_at(0.0) || line_hits() > HIT_LIMIT.saturating_sub(prior_hits);
        let started = (sampled && preflight && frames > 0).then(|| self.sampler.start()).flatten();
        let bound = if preflight { self.plan(b, v, slices.clone()) } else { (0, CountFormat::Narrow) };
        let placement = started.map(|t| t.elapsed());
        if bound.0 > bound.1.hit_limit() {
            let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
            b.counters.discard(Lost::Budget, records);
            let _ = self.send_tap(slot, b, TapProjection::Unpainted, Some(v), identity);
            return current;
        }
        let tap = self.send_tap(slot, b, TapProjection::Projected { gen: v.gen, size: (v.w, v.h) }, Some(v), identity);
        let mut d = prepare(current, slot, b, v, identity, bound)?;
        if let Some(number) = tap {
            d.include_tap(number);
        }
        let before = d.records;
        for first in slices {
            let started = if first == start && preflight {
                placement.map(|_| std::time::Instant::now())
            } else {
                sampled.then(|| self.sampler.start()).flatten()
            };
            let count =
                if first == start && preflight { (b.frames - first).min(SLICE) } else { self.place_slice(b, v, first) };
            if crate::pacing::is_stale(b.header.cfg()) {
                d.progress.stale += count as u64;
            }
            draw_tiles(b, &mut d.planes, &mut self.pool, &self.arena, count, &mut self.jobs);
            account(b, &mut d, &self.arena[..count]);
            if let Some(started) = started {
                let elapsed = started.elapsed() + placement.filter(|_| first == start).unwrap_or_default();
                d.progress.sample = Some((elapsed, count as u64));
            }
        }
        b.next.store(b.frames, Ordering::Relaxed);
        d.extent.include(b.fed);
        b.counters.publish(&d.progress, d.records - before);
        d.progress.measured = 0;
        Some(d)
    }

    fn plan(
        &mut self,
        b: &Arc<Batch>,
        v: &Arc<View>,
        slices: impl DoubleEndedIterator<Item = usize>,
    ) -> (u64, CountFormat) {
        // Keep at most one slice of plans. Reuse the first slice when painting starts.
        let mut hits = 0u64;
        let mut format = CountFormat::Narrow;
        for first in slices.rev() {
            let count = self.place_slice(b, v, first);
            for p in self.arena[..count].iter().flatten() {
                if let Some(frame) = &p.plan {
                    hits = hits.saturating_add(frame.plan.hit_bound());
                    if frame.plan.format() == CountFormat::Wide {
                        format = CountFormat::Wide;
                    }
                }
            }
        }
        (hits, format)
    }

    fn place_slice(&mut self, b: &Arc<Batch>, v: &Arc<View>, first: usize) -> usize {
        let count = (b.frames - first).min(SLICE);
        let placed = Arc::get_mut(&mut self.arena).expect("all tile jobs returned their arena references");
        placed.resize_with(placed.len().max(count), || None);
        for (index, slot) in (first..first + count).zip(&mut placed[..count]) {
            let mut p = slot.take().unwrap_or_else(|| {
                Box::new(Prepared {
                    index,
                    decoded_dots: false,
                    plan: None,
                    codes: None,
                    folds: crate::framestats::folds_for(&v.m),
                    measured: false,
                })
            });
            p.index = index;
            for (_, fold) in &mut p.folds {
                *fold = Default::default();
            }
            self.jobs.push(Job::Place(b.clone(), v.clone(), p));
        }
        self.pool.map(&mut self.jobs);
        for (slot, job) in placed[..count].iter_mut().zip(self.jobs.drain(..)) {
            if let Job::Place(_, _, p) = job {
                *slot = Some(p);
            }
        }
        count
    }
}

fn draw_tiles(
    b: &Arc<Batch>,
    planes: &mut Vec<DiffBand>,
    pool: &mut Pool<Job, Frame>,
    arena: &Arena,
    count: usize,
    jobs: &mut Vec<Job>,
) {
    for band in planes.drain(..) {
        jobs.push(Job::Tile(b.clone(), arena.clone(), count, band));
    }
    pool.map(jobs);
    for job in jobs.drain(..) {
        if let Job::Tile(.., band) = job {
            planes.push(band);
        }
    }
    // Keep only one decoded record in scratch while painting its dots across the bands.
    let scratch = pool.scratch();
    for p in arena[..count].iter().flatten().filter(|p| p.decoded_dots) {
        let Some(plan) = &p.plan else { continue };
        let frame = newest_at(b, p.index).unwrap_or_else(|| {
            b.decode.decode(p.index, chunk(b, p.index), scratch);
            scratch
        });
        for band in &mut *planes {
            let samples = kernel::SamplesRef::Volts(&frame.ch[band.channel]);
            paint_band(band, samples, &plan.plan);
        }
    }
}

fn account(b: &Batch, d: &mut Deposit, placed: &[Option<Box<Prepared>>]) {
    for p in placed.iter().flatten() {
        let Some(plan) = &p.plan else { continue };
        d.records += 1;
        d.extent.add_hits(plan.plan.hit_bound());
        d.progress.measured += u64::from(p.measured);
        d.triggered += u64::from(plan.triggered);
        if let Some(outcome) = plan.plan.outcome() {
            Stats { edges: &mut d.edges, off: &mut d.off }.record(outcome);
        }
        if d.identity == b.header.identity() {
            crate::framestats::add_folds(&mut d.folds, &p.folds);
        } else {
            Counters::add(&b.counters.losses.folds, p.folds.iter().map(|(_, f)| f.n).sum());
        }
    }
}
