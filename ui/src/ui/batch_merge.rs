// SPDX-License-Identifier: GPL-2.0-or-later

//! A capacity-one picture merge. Live intake polls; deterministic headless runs consume the completion event.

use super::lifecycle::worker::Worker;
use crate::drawpipe::{Merged, Pipe};
use crate::model::TrigStatus;
use crate::wave::Phosphor;
use oos_core::change::Identity;

type Key = (u64, Option<Identity>);

pub(super) struct Input {
    pub(super) elapsed: f64,
    pub(super) persist: f32,
    pub(super) reset: bool,
    pub(super) wait: bool,
    pub(super) little: u64,
}

struct Job {
    pipe: &'static Pipe,
    ph: Phosphor,
    clear: bool,
    key: Key,
}

struct Finished {
    ph: Phosphor,
    cleared: bool,
    key: Key,
    merged: Merged,
    edges: [u64; 4],
    off: (f64, f64),
    duration: std::time::Duration,
}

#[derive(Default)]
pub(super) struct Merge {
    worker: Option<Worker<Job, Finished>>,
    elapsed: f64,
    invalid: bool,
    spare: Option<Phosphor>,
    duration: std::time::Duration,
    observed: bool,
}

impl Merge {
    pub(super) fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(Worker::busy)
    }

    pub(super) fn invalidate(&mut self) {
        self.invalid = true;
    }

    pub(super) fn merge(
        &mut self,
        pipe: &'static Pipe,
        ph: &mut Phosphor,
        clear: &mut bool,
        edges: &mut [u64; 4],
        off: &mut (f64, f64),
        input: Input,
    ) -> Merged {
        if self.busy() {
            self.elapsed += input.elapsed;
            self.invalid |= input.reset;
        } else if input.wait || pipe.has_maps() {
            self.start(pipe, ph, *clear, input.little);
            if !input.wait {
                return Merged::default();
            }
        } else {
            pipe.release();
            return Merged::default();
        }
        self.finish(pipe, ph, clear, edges, off, &input)
    }

    fn start(&mut self, pipe: &'static Pipe, ph: &Phosphor, clear: bool, little: u64) {
        if self.worker.is_none() {
            let mut reduction = crate::drawpipe::reduction::Reduction::start(little);
            self.worker = match Worker::spawn("picture-merge", move |job: Job| {
                crate::placement::to_little_cores(little);
                run(job, &mut reduction)
            }) {
                Ok(worker) => Some(worker),
                Err(error) => {
                    eprintln!("oos-ui: picture merge worker: {error}");
                    return;
                }
            };
        }
        let mut snapshot = self.spare.take().unwrap_or_else(|| Phosphor { w: 0, h: 0, map: Default::default() });
        (snapshot.w, snapshot.h) = (ph.w, ph.h);
        if !clear {
            for (dst, src) in snapshot.map.iter_mut().zip(&ph.map) {
                dst.clone_from(src);
            }
        }
        let job = Job { pipe, ph: snapshot, clear, key: pipe.merge_key() };
        let worker = self.worker.as_mut().expect("the merge worker was started");
        if let Err(error) = worker.submit(job) {
            eprintln!("oos-ui: picture merge submission: {error}");
            return;
        }
        self.elapsed = 0.0;
        self.invalid = false;
    }

    fn finish(
        &mut self,
        pipe: &Pipe,
        ph: &mut Phosphor,
        clear: &mut bool,
        edges: &mut [u64; 4],
        off: &mut (f64, f64),
        input: &Input,
    ) -> Merged {
        let Some(worker) = self.worker.as_mut() else { return Merged::default() };
        let result = if input.wait { worker.drain().next().and_then(Result::ok) } else { worker.poll().ok() };
        let Some(mut done) = result else { return Merged::default() };
        let valid = !self.invalid && (done.ph.w, done.ph.h) == (ph.w, ph.h);
        self.duration = done.duration;
        self.observed = true;
        if !pipe.accept_merge(done.key, valid, &mut done.merged) {
            self.spare = Some(done.ph);
            return Merged::default();
        }
        if done.merged.frames == 0 {
            self.spare = Some(done.ph);
            pipe.release();
            return Merged::default();
        }
        if input.persist > 0.0 && self.elapsed > 0.0 {
            done.ph.decay((-self.elapsed / input.persist as f64).exp() as f32);
        }
        std::mem::swap(ph, &mut done.ph);
        self.spare = Some(done.ph);
        if done.cleared {
            *clear = false;
        }
        for (a, b) in edges.iter_mut().zip(done.edges) {
            *a += b;
        }
        *off = (off.0 + done.off.0, off.1.max(done.off.1));
        pipe.release();
        done.merged
    }
}

fn run(mut job: Job, reduction: &mut crate::drawpipe::reduction::Reduction) -> Finished {
    let started = std::time::Instant::now();
    let mut edges = [0; 4];
    let mut off = (0.0, 0.0);
    let clear = job.clear;
    if clear {
        for map in &mut job.ph.map {
            map.resize(job.ph.w * job.ph.h, 0);
        }
    }
    let merged = job.pipe.merge_maps(&mut job.ph, &mut job.clear, &mut edges, &mut off, reduction);
    crate::framecost::add(crate::framecost::Part::Merge, started);
    Finished { ph: job.ph, cleared: clear && !job.clear, key: job.key, merged, edges, off, duration: started.elapsed() }
}

pub(super) struct Fast {
    pub(super) got: usize,
    pub(super) newest: Option<std::time::Instant>,
    pub(super) raw: Option<crate::wave::Record>,
    pub(super) drawing: f32,
    pub(super) times: [std::time::Duration; 2],
}

pub(super) fn fast(
    ui: &mut super::Ui,
    clear_pending: &mut bool,
    k: &mut usize,
    dt_wall: f64,
    picture_cleared: bool,
) -> Fast {
    let persist = crate::model::PERSIST[ui.m.persist].1;
    let deep_raw = crate::deepmeas::want(&ui.m).is_some();
    let sw = super::change::acquisition::draw_switches(ui, &ui.m);
    let merging = ui.m.draw_batch && ui.host.runtime().batch_merge.busy();
    let (mut got, mut n_dep, mut newest) = (0, 0, None);
    let mut td = 0.0;
    let mut draw = [std::time::Duration::ZERO; 2];
    let t0 = std::time::Instant::now();
    let (n, raw) = queued(ui, clear_pending, merging, deep_raw);
    let raw_rec = raw;
    got += n;
    n_dep += n;
    td += t0.elapsed().as_secs_f32() * 1000.0;
    let t_merge = std::time::Instant::now();
    draw[0] += t_merge - t0;
    // batch mode: the other frames of the batches, drawn by the deposit workers (drawpipe.rs)
    if let Some(p) = crate::drawpipe::pipe(&sw) {
        let t1 = std::time::Instant::now();
        let d = merge_picture(ui, p, clear_pending, dt_wall, persist, picture_cleared);
        if ui.host.runtime().batch_merge.busy() {
            *k = 0;
        }
        if d.triggered > 0 {
            ui.m.status = TrigStatus::Triggered;
        }
        got += d.frames as usize;
        (n_dep, newest) = (n_dep + d.frames as usize, d.newest);
        crate::pacing::drawn_stale(d.stale);
        ui.mstat_fold(&d.folds);
        td += t1.elapsed().as_secs_f32() * 1000.0;
    }
    draw[1] += t_merge.elapsed();
    crate::pacing::fast_block((t_merge - t0).as_secs_f32() * 1e3, t_merge.elapsed().as_secs_f32() * 1e3);

    debug_assert_eq!(got, n_dep);
    Fast { got, newest, raw: raw_rec, drawing: td, times: draw }
}

fn merge_picture(
    ui: &mut super::Ui,
    pipe: &'static Pipe,
    clear: &mut bool,
    elapsed: f64,
    persist: f32,
    reset: bool,
) -> Merged {
    let input = Input { elapsed, persist, reset, wait: !ui.host.time_budgets(), little: ui.switches().cores.little };
    let mut merge = std::mem::take(&mut ui.host.runtime_mut().batch_merge);
    let result = merge.merge(pipe, &mut ui.ph, clear, &mut ui.fast_edges, &mut ui.fast_off, input);
    if std::mem::take(&mut merge.observed) {
        super::change::timing_mut(ui).worker_merge(merge.duration);
    }
    ui.host.runtime_mut().batch_merge = merge;
    result
}

pub(super) fn invalidate(ui: &mut super::Ui) {
    if !ui.fast_ok() || !ui.m.draw_batch || ui.m.run != crate::model::RunState::Run {
        ui.host.runtime_mut().batch_merge.invalidate();
    }
}

fn queued(
    ui: &mut super::Ui,
    clear_pending: &mut bool,
    merging: bool,
    deep_raw: bool,
) -> (usize, Option<crate::wave::Record>) {
    let mut raw_rec = None;
    let t0 = std::time::Instant::now();
    let frames = if merging { Vec::new() } else { ui.src.drain_newest(ui.fast_max()) };
    if !frames.is_empty() && *clear_pending {
        ui.ph.clear();
        *clear_pending = false;
    }
    let n = frames.len();
    for f in &frames {
        crate::pacing::drawn(f.cfg, 1);
    }
    if let Some(f) = frames.iter().rev().find(|f| deep_raw && crate::deepmeas::deep_raw_frame(f)) {
        let t = std::time::Instant::now();
        raw_rec = Some(crate::wave::hw_record(&ui.m, f.clone()));
        crate::deepmeas::cost::set(&crate::deepmeas::cost::RECORD_US, t.elapsed());
    }
    if n > 0 && ui.fast_draw_all(frames) > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    ui.t_fast += 0.05 * (ms - ui.t_fast);
    if n > 0 && ms > 0.0 {
        // as many as would have filled the budget at this pace: with a fixed part in each screen's time
        // (starting the helpers) this settles where the time meets the budget. Down only when the budget
        // was exceeded, up only when the take was the limit: the hardware delivers in bursts, and a screen
        // with a few frames says nothing about how many fit
        ui.fast_take = super::fast_take_next(ui.fast_take, n, ms, ui.fast_budget());
    }
    (n, raw_rec)
}

pub(super) fn schedule(ui: &mut super::Ui) {
    if !ui.host.time_budgets() || !ui.fast_ok() || !ui.m.draw_batch || ui.m.run != crate::model::RunState::Run {
        return;
    }
    let sw = super::change::acquisition::draw_switches(ui, &ui.m);
    let Some(pipe) = crate::drawpipe::pipe(&sw) else { return };
    let clear = crate::model::PERSIST[ui.m.persist].1 == 0.0;
    let little = sw.cores.little;
    let mut merge = std::mem::take(&mut ui.host.runtime_mut().batch_merge);
    if !merge.busy() && pipe.has_maps() {
        merge.start(pipe, &ui.ph, clear, little);
    }
    ui.host.runtime_mut().batch_merge = merge;
}

#[cfg(test)]
mod tests;
