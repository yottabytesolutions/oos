// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::{kernel, pool::Pool, standard, DiffBand};

#[cfg(test)]
mod tests;

struct Prepared {
    index: usize,
    samples: Option<Frame>,
    plan: Option<standard::FastFrame>,
    codes: Option<CodeLayout>,
    folds: crate::framestats::Folds,
}

enum Job {
    Place(Arc<Batch>, Arc<View>, Box<Prepared>),
    Tile(Arc<Batch>, Arc<Vec<Box<Prepared>>>, DiffBand),
}

fn run(scratch: &mut Frame, job: &mut Job) {
    match job {
        Job::Place(b, v, p) if p.index < b.frames => place(b, v, p, scratch),
        Job::Place(..) => {}
        Job::Tile(b, placed, band) => {
            for p in placed.iter() {
                if p.index >= b.frames {
                    continue;
                }
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
    if !(p.index + 1 == b.frames && b.newest.is_some()) && crate::framestats::fold_frame(f, &v.m, &mut p.folds) {
        Counters::add(&b.counters.measured, 1);
    }
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
    let mut arena = Arc::new(Vec::<Box<Prepared>>::new());
    let mut jobs = Vec::new();
    let mut statistics = None;
    while let Ok(message) = rx.recv() {
        let b = match message {
            Message::Batch(b) => b,
            Message::Wake => {
                if let Some(d) = current.take() {
                    current = slot.put(d);
                }
                continue;
            }
            #[cfg(test)]
            Message::Barrier(reply) => {
                if let Some(d) = current.take() {
                    current = slot.put(d);
                }
                let _ = reply.send(());
                continue;
            }
        };
        let Some((v, identity)) = batch_view(&b, view) else { continue };
        let key = (v.gen, b.header.identity().map(|id| id.stats_gen()));
        if statistics != Some(key) {
            for p in Arc::get_mut(&mut arena).expect("all previous batch jobs returned").iter_mut() {
                p.folds = crate::framestats::folds_for(&v.m);
            }
            statistics = Some(key);
        }
        current = prepare(current, slot, &b, &v, identity);
        let Some(deposit) = current.as_mut() else { break };
        draw(&b, &v, deposit, &mut pool, &mut arena, &mut jobs);
        let finished = current.take().expect("a batch always has an open deposit");
        current = slot.put(finished);
    }
    if let Some(deposit) = current {
        deposit.reject(oos_hal::sync::lock(view).identity, counters);
    }
}

fn batch_view(b: &Batch, view: &Mutex<ViewState>) -> Option<(Arc<View>, Option<oos_core::change::Identity>)> {
    let state = oos_hal::sync::lock(view);
    let v = state.view.clone().filter(|_| accepts(b, feed::legacy_stamp(&state), state.identity));
    if v.is_none() {
        let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
        if !same_pair(b.header.identity(), state.identity) {
            Counters::add(&b.counters.losses.pipe, records);
        }
        Counters::add(&b.counters.dropped, records);
    }
    v.map(|v| (v, state.identity))
}

fn prepare(
    mut current: Option<Deposit>,
    slot: &Handoff,
    b: &Batch,
    v: &View,
    identity: Option<oos_core::change::Identity>,
) -> Option<Deposit> {
    if current.as_ref().is_some_and(|d| !d.matches(v.gen, v.w, v.h) || !same_pair(d.identity, b.header.identity())) {
        let old = current.take().expect("the preceding check found a deposit");
        old.reject(identity, b.counters);
        slot.recycle(old);
    }
    let records = b.frames.saturating_sub(b.next.load(Ordering::Relaxed)) as u64;
    if current.as_ref().is_some_and(|d| d.records + records > u16::MAX as u64 / 24) {
        let full = current.take().expect("the weight limit applies to an existing deposit");
        if let Some(full) = slot.send(full) {
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
    // Only the final waveform is measured by UI readers. Earlier coalesced taps belong in the pool folds.
    if b.newest.is_some() {
        if let Some(prior) = &deposit.newest {
            if crate::framestats::fold_frame(prior, &v.m, &mut deposit.folds) {
                Counters::add(&b.counters.measured, 1);
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
    arena: &mut Arc<Vec<Box<Prepared>>>,
    jobs: &mut Vec<Job>,
) {
    let start = b.next.load(Ordering::Relaxed);
    if start == b.frames {
        return;
    }
    for first in (start..b.frames).step_by(64) {
        let end = (first + 64).min(b.frames);
        let placed = Arc::get_mut(arena).expect("all tile jobs returned their arena references");
        while placed.len() < end - first {
            placed.push(Box::new(Prepared {
                index: 0,
                samples: None,
                plan: None,
                codes: None,
                folds: crate::framestats::folds_for(&v.m),
            }));
        }
        for (i, mut p) in placed.drain(..).enumerate() {
            p.index = if first + i < end { first + i } else { usize::MAX };
            jobs.push(Job::Place(b.clone(), v.clone(), p));
        }
        pool.map(jobs);
        for job in jobs.drain(..) {
            if let Job::Place(_, _, p) = job {
                placed.push(p);
            }
        }
        for band in d.planes.drain(..) {
            jobs.push(Job::Tile(b.clone(), arena.clone(), band));
        }
        pool.each(jobs);
        for job in jobs.drain(..) {
            if let Job::Tile(_, _, band) = job {
                d.planes.push(band);
            }
        }
        account(b, d, arena);
        for p in Arc::get_mut(arena).expect("tile jobs released the arena").iter_mut() {
            for (_, fold) in &mut p.folds {
                *fold = Default::default();
            }
        }
    }
    b.next.store(b.frames, Ordering::Relaxed);
    d.newest = b.newest.clone().or_else(|| d.newest.take());
    d.read_at = Some(b.fed);
    if crate::pacing::is_stale(b.header.cfg()) {
        d.stale += (b.frames - start) as u64;
    }
}

fn account(b: &Batch, d: &mut Deposit, arena: &[Box<Prepared>]) {
    for p in arena.iter() {
        if p.index >= b.frames {
            continue;
        }
        let Some(plan) = &p.plan else { continue };
        d.records += 1;
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
