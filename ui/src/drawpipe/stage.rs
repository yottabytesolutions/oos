// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::claim_timing::Sampler;
use crate::picture::deposit::{kernel, pool::Pool, standard, DiffBand};

#[cfg(test)]
mod tests;

struct Prepared {
    index: usize,
    samples: Option<Frame>,
    plan: Option<standard::FastFrame>,
    codes: Option<CodeLayout>,
    folds: crate::framestats::Folds,
    measured: bool,
}

type Arena = Arc<Vec<Option<Box<Prepared>>>>;

enum Job {
    Place(Arc<Batch>, Arc<View>, Box<Prepared>),
    Tile(Arc<Batch>, Arena, usize, DiffBand),
}

fn run(scratch: &mut Frame, job: &mut Job) {
    match job {
        Job::Place(b, v, p) => place(b, v, p, scratch),
        Job::Tile(b, placed, count, band) => {
            for p in placed[..*count].iter().flatten() {
                let Some(plan) = &p.plan else { continue };
                let (a, z) = plan.plan.rows(band.channel);
                if a >= z {
                    continue;
                }
                band.touch(a..z);
                let samples = match p.codes {
                    Some(layout) => kernel::SamplesRef::Codes(layout.channel(chunk(b, p.index), band.channel)),
                    None => {
                        kernel::SamplesRef::Volts(p.samples.as_ref().map_or(&[], |f| f.ch[band.channel].as_slice()))
                    }
                };
                standard::add_sample_band(band.channel, &mut band.plane.d, band.first, samples, &plan.plan);
            }
        }
    }
}

fn chunk(b: &Batch, i: usize) -> &[u8] {
    &b.lease.raw.as_ref().expect("batch owns its buffer until every job returns").bytes()[i * b.fb..(i + 1) * b.fb]
}

fn place(b: &Batch, v: &View, p: &mut Prepared, scratch: &mut Frame) {
    let previous = p.plan.take().and_then(|frame| frame.plan.into_raw());
    decode(b, v, p, scratch);
    let bytes = chunk(b, p.index);
    let f = &*scratch;
    let spacing = f.dt / v.m.time_per_div() as f64 * v.w as f64 / HDIV as f64;
    let pairs = crate::compress::live(f) && spacing <= 0.5;
    if !pairs && (spacing > 0.5 || f.envelope || f.roll || f.scan || crate::deepmeas::deep_raw_frame(f)) {
        let rec = crate::sparse::record_with(&v.m, f, v.interp).unwrap_or_else(|| {
            let window = v.m.time_per_div() as f64 * HDIV as f64;
            crate::board::display_record(f, &v.m, 3600, window, v.m.delay as f64 - window / 2.0)
        });
        p.plan = Some(standard::FastFrame::record(rec, standard::projection(&v.m, v.w, v.h, spacing), [None; NCH]));
        return;
    }
    p.plan = Some(standard::batch_plan(&v.m, (v.w, v.h), f, p.codes.map(|layout| (bytes, layout)), previous));
    if v.m.dots_at(spacing) && p.codes.is_none() && !pairs {
        p.samples = Some(std::mem::take(scratch));
    }
}

fn decode(b: &Batch, v: &View, p: &mut Prepared, f: &mut Frame) {
    let bytes = chunk(b, p.index);
    if let Some(samples) = p.samples.take() {
        *f = samples;
    }
    p.codes = b.codes.as_ref().and_then(|codes| codes(bytes, f));
    let spacing = f.dt / v.m.time_per_div() as f64 * v.w as f64 / HDIV as f64;
    if p.codes.is_none() || spacing > 0.5 || f.envelope || f.roll || f.scan || !p.folds.is_empty() {
        b.decode.decode(p.index, bytes, f);
        p.codes = None;
    }
    if p.index + 1 == b.frames {
        if let Some(newest) = &b.newest {
            f.clone_from(newest);
            p.codes = None;
        }
    }
    b.header.tag(f);
    p.measured =
        (p.index + 1 != b.frames || b.newest.is_none()) && crate::framestats::fold_frame(f, &v.m, &mut p.folds);
}

pub(super) fn work(
    rx: Receiver<Message>,
    view: &Mutex<ViewState>,
    slot: &Handoff,
    n: usize,
    little: u64,
    counters: &'static Counters,
) {
    let mut pool = Pool::new(n, little, run);
    let mut current: Option<Deposit> = None;
    let mut arena = Arena::default();
    let mut jobs = Vec::new();
    let mut statistics = None;
    let mut sampler = Sampler::default();
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
                if let Some(d) = current.take() {
                    current = slot.put(d).err();
                }
                let _ = reply.send(());
                continue;
            }
        };
        let Some(v) = batch_view(&b, &state, reset_before) else { continue };
        let identity = state.identity;
        drop(state);
        let key = (v.gen, b.header.identity().map(|id| id.stats_gen()));
        if statistics != Some(key) {
            for p in Arc::get_mut(&mut arena).expect("all previous batch jobs returned").iter_mut().flatten() {
                p.folds = crate::framestats::folds_for(&v.m);
            }
            statistics = Some(key);
        }
        let Some(mut deposit) = prepare(current.take(), slot, &b, &v, identity) else {
            if slot.closed() {
                break;
            }
            continue;
        };
        draw(&b, &v, &mut deposit, &mut pool, &mut arena, &mut jobs, &mut sampler);
        current = slot.put(deposit).err();
    }
    if let Some(deposit) = current {
        deposit.reject(oos_hal::sync::lock(view).identity, counters);
    }
}

fn batch_view(b: &Batch, state: &ViewState, reset_before: u64) -> Option<Arc<View>> {
    let v = state
        .view
        .clone()
        .filter(|_| b.view_gen_at_submit >= reset_before && accepts(b, feed::legacy_stamp(state), state.identity));
    if v.is_none() {
        let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
        if !same_pair(b.header.identity(), state.identity) {
            Counters::add(&b.counters.losses.pipe, records);
        }
        b.counters.discard(Lost::Settings, records);
    }
    v
}

fn prepare(
    mut current: Option<Deposit>,
    slot: &Handoff,
    b: &Batch,
    v: &View,
    identity: Option<oos_core::change::Identity>,
) -> Option<Deposit> {
    let mut waveform = None;
    if current.as_ref().is_some_and(|d| !d.matches(v.gen, v.w, v.h) || !same_pair(d.identity, b.header.identity())) {
        let mut old = current.take().expect("the preceding check found a deposit");
        old.reject(identity, b.counters);
        if b.newest.is_none() && same_pair(old.identity, identity) {
            waveform = old.newest.take();
        }
        slot.recycle(old);
    }
    let records = b.frames.saturating_sub(b.next.load(Ordering::Relaxed)) as u64;
    if current.as_ref().is_some_and(|d| d.records + records > u16::MAX as u64 / 24) {
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
            _ => Deposit::new(v.gen, (v.w, v.h), b.header.identity(), v.m.ch.each_ref().map(|c| c.on)),
        }
    });
    if waveform.is_some() {
        deposit.newest = waveform;
    }
    // Only the final waveform is measured by UI readers. Earlier coalesced taps belong in the pool folds.
    if b.newest.is_some() {
        if let Some(prior) = &deposit.newest {
            if crate::framestats::fold_frame(prior, &v.m, &mut deposit.folds) {
                deposit.progress.measured += 1;
            }
        }
    }
    if b.header.identity().map(|id| id.stats_gen()) > deposit.identity.map(|id| id.stats_gen()) {
        Counters::add(&b.counters.losses.folds, deposit.folds.iter().map(|(_, f)| f.n).sum());
        deposit.folds.clear();
        deposit.identity = b.header.identity();
    }
    current
}

fn draw(
    b: &Arc<Batch>,
    v: &Arc<View>,
    d: &mut Deposit,
    pool: &mut Pool<Job, Frame>,
    arena: &mut Arena,
    jobs: &mut Vec<Job>,
    sampler: &mut Sampler,
) {
    let start = b.next.load(Ordering::Relaxed);
    if start == b.frames {
        return;
    }
    let before = d.records;
    for first in (start..b.frames).step_by(64) {
        let count = (b.frames - first).min(64);
        let started = b.counters.diagnostics_enabled().then(|| sampler.start()).flatten();
        if crate::pacing::is_stale(b.header.cfg()) {
            d.progress.stale += count as u64;
        }
        let placed = Arc::get_mut(arena).expect("all tile jobs returned their arena references");
        placed.resize_with(placed.len().max(count), || None);
        for (index, slot) in (first..first + count).zip(&mut placed[..count]) {
            let mut p = slot.take().unwrap_or_else(|| {
                Box::new(Prepared {
                    index,
                    samples: None,
                    plan: None,
                    codes: None,
                    folds: crate::framestats::folds_for(&v.m),
                    measured: false,
                })
            });
            p.index = index;
            jobs.push(Job::Place(b.clone(), v.clone(), p));
        }
        pool.map(jobs);
        for (slot, job) in placed[..count].iter_mut().zip(jobs.drain(..)) {
            if let Job::Place(_, _, p) = job {
                *slot = Some(p);
            }
        }
        draw_tiles(b, &mut d.planes, pool, arena, count, jobs);
        account(b, d, &arena[..count]);
        let placed = Arc::get_mut(arena).expect("tile jobs released the arena");
        for p in placed[..count].iter_mut().flatten() {
            for (_, fold) in &mut p.folds {
                *fold = Default::default();
            }
        }
        if let Some(started) = started {
            d.progress.sample = Some((started.elapsed(), count as u64));
        }
    }
    b.next.store(b.frames, Ordering::Relaxed);
    d.newest = b.newest.clone().or_else(|| d.newest.take());
    d.read_at = Some(b.fed);
    b.counters.publish(&d.progress, d.records - before);
    d.progress.measured = 0;
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
}

fn account(b: &Batch, d: &mut Deposit, placed: &[Option<Box<Prepared>>]) {
    for p in placed.iter().flatten() {
        let Some(plan) = &p.plan else { continue };
        d.records += 1;
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
