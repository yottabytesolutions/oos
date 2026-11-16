// SPDX-License-Identifier: GPL-2.0-or-later
//! Full-record jobs on the persistent measurement worker. Only screen vectors and raw Arcs cross main.

mod history;

use super::super::*;

type SpectrumKey = (WaveId, f64, f64, f64, Window, usize, usize, bool, bool, usize, (f32, f32));

#[derive(Clone, Default)]
pub(in crate::ui) struct Work {
    spec_t: f64,
    /// Samples copied for synchronous offline analysis. Background DSP never increments this on main.
    samples: u64,
    queued_samples: u64,
    revision: u64,
    pub(in crate::ui) pending: bool,
    finished: Option<f64>,
    accepted_key: Option<Key>,
}

impl Work {
    pub(in crate::ui) fn copied(&mut self, n: usize) {
        self.samples += n as u64;
    }

    pub(in crate::ui) fn invalidate(&mut self) {
        self.revision += 1;
        self.pending = true;
    }

    pub(in crate::ui) fn reset(&mut self) {
        *self = Self { revision: self.revision + 1, ..Default::default() };
    }

    fn results_due(&mut self, now: f64, has_spectrum: bool) -> bool {
        if has_spectrum && (now - self.spec_t).abs() < 0.5 {
            return false;
        }
        self.spec_t = now;
        true
    }

    #[cfg(test)]
    pub(in crate::ui) fn queued_samples(&self) -> u64 {
        self.queued_samples
    }

    #[cfg(test)]
    pub(in crate::ui) fn samples(&self) -> u64 {
        self.samples
    }
}

/// These are the current pre-M16 generation tokens: acquisition stamp and measurement reset counter.
/// Configuration and the local clear revision also guard results after menu changes.
#[derive(Clone, PartialEq)]
pub(in crate::ui) struct Key {
    generation: (oos_core::change::Identity, u64),
    acquisition: (AcqMode, usize, u32, usize),
    channels: [Channel; NCH],
    view: (AView, usize),
    spectrum: SpectrumKey,
    eye: (WaveId, f64),
    power: (WaveId, WaveId, PowerMode),
    bandwidth: (usize, f64, usize),
    area: (i32, i32, i32, i32),
}

fn key(ui: &Ui) -> Key {
    let m = &ui.m;
    Key {
        generation: (change::acquisition::identity(ui), ui.azr.work.revision),
        acquisition: (m.acq, m.tscale, m.delay.to_bits(), m.mdepth),
        channels: m.ch.clone(),
        view: (m.view, m.pane),
        spectrum: (
            m.spec_src,
            m.spec_center,
            m.spec_span,
            m.spec_rbw,
            m.spec_win,
            m.spec_trace,
            m.spec_harm,
            m.spec_wf,
            m.spec_dist,
            m.spec_sort,
            (m.spec_ref, m.spec_margin),
        ),
        eye: (m.eye_src, m.eye_rate),
        power: (m.pw_v, m.pw_i, m.pw_mode),
        bandwidth: (m.bw_src, m.bw_hint, m.bw_bins),
        area: (ui.area.x, ui.area.y, ui.area.w, ui.area.h),
    }
}

/// The spectrum is on screen and follows the signal's harmonics.
pub(super) fn follows(ui: &Ui) -> bool {
    let shown = ui.m.view == AView::Spectrum
        || ui.m.view == AView::Scope && matches!(ui.m.pane, tables::PANE_SPECTRUM | tables::PANE_RESULTS);
    shown && ui.m.spec_span == 0.0 && PANE_HARMS[ui.m.spec_harm.min(PANE_HARMS.len() - 1)] > 0
}

pub(in crate::ui) struct Context {
    pub(in crate::ui) m: Model,
    pub(in crate::ui) azr: super::State,
    pub(in crate::ui) now: f64,
    pub(super) points: usize,
    pub(super) follow: bool,
    /// where the eye page is drawn, whether it fills the panel, and the chart it enlarged
    pub(in crate::ui) eye_rect: Rect,
    pub(in crate::ui) eye_fill: bool,
    pub(in crate::ui) eye_big: Option<u8>,
}

pub(in crate::ui) struct Job {
    key: Key,
    record: Record,
    context: Context,
    pane: bool,
    view: AView,
}

pub(in crate::ui) struct Done {
    key: Key,
    /// Raw acquisition identity is retained independently of the current settings.
    record: Option<(u64, (u64, u64))>,
    at: f64,
    state: super::State,
}

/// Worker-owned history. Cloning the finished raster/trace happens on the worker, never on main.
pub(in crate::ui) fn run(job: Job, previous: &mut Option<(history::Key, super::State)>) -> Done {
    let Job { key, record, mut context, pane, view } = job;
    let history_key = history::Key::of(&key, view);
    if let Some((old_key, state)) = previous.take() {
        if old_key == history_key {
            context.azr = state;
        }
    }
    let identity = record.raw.as_ref().map(|r| (r.frame.seq, (r.frame.cfg, r.frame.stamp)));
    context.calculate(&record, view, pane);
    let state = context.azr.clone();
    *previous = Some((history_key, context.azr));
    Done { key, record: identity, at: context.now, state }
}

fn accept(ui: &mut Ui, done: Done) -> bool {
    if done.key != key(ui)
        || done.record.is_some_and(|(_, pair)| pair != (done.key.generation.0.gen(), done.key.generation.0.epoch()))
    {
        return false;
    }
    let mut state = done.state;
    // These controls belong to main; a worker result contains only analyser display state.
    state.work = std::mem::take(&mut ui.azr.work);
    state.keep_main(&mut ui.azr);
    state.work.finished = Some(done.at);
    state.work.accepted_key = Some(done.key);
    state.findings = std::mem::take(&mut ui.azr.findings);
    state.watch = ui.azr.watch;
    state.watch_seen = std::mem::take(&mut ui.azr.watch_seen);
    state.watch_t = ui.azr.watch_t;
    ui.azr = state;
    true
}

pub(in crate::ui) fn poll(ui: &mut Ui) {
    let done = ui.meas_bg.as_mut().and_then(|w| w.analysis_rx.try_recv().ok());
    if let Some(done) = done {
        if let Some(w) = ui.meas_bg.as_mut() {
            w.analysis_busy = false;
        }
        accept(ui, done);
    }
    if ui.azr.work.pending && ui.meas_bg.as_ref().is_some_and(|w| !w.analysis_busy) {
        update(ui);
    }
}

pub(super) fn update(ui: &mut Ui) {
    let pane = ui.m.view == AView::Scope
        && ui.pane_h() > 0
        && matches!(ui.m.pane, tables::PANE_SPECTRUM | tables::PANE_RESULTS);
    let view = if pane { AView::Spectrum } else { ui.m.view };
    if matches!(view, AView::Scope | AView::Bode) {
        ui.azr.work.pending = false;
        return;
    }
    ui.azr.work.pending = true;
    if ui.meas_bg.as_ref().is_some_and(|w| w.analysis_busy) {
        return;
    }
    if pane && ui.m.pane == tables::PANE_RESULTS && !ui.azr.work.results_due(ui.now, ui.azr.spec.is_some()) {
        return;
    }
    submit(ui, view, pane);
}

fn prepare(ui: &Ui, view: AView, pane: bool) -> Option<Job> {
    let rec = ui.last[0].as_ref()?;
    if !change::acquisition::record(ui, rec) {
        return None;
    }
    let area = if pane { ui.pane_rect() } else { ui.area };
    let context = Context {
        m: ui.m.clone(),
        azr: super::State::default(),
        now: ui.now,
        points: ui.spec_box(area, pane).plot.w.max(100) as usize,
        follow: follows(ui),
        eye_rect: ui.eye_page_rect(),
        eye_fill: ui.full_on(),
        eye_big: ui.eye_big,
    };
    // Screen vectors are bounded by record conversion. Never clone the optional deep LA buffer.
    let record = Record {
        ch: std::array::from_fn(|c| {
            let id = WaveId::Ch(c);
            let selected = match view {
                AView::Eye => id == ui.m.eye_src,
                AView::Spectrum => id == ui.m.spec_src,
                AView::Step => c == ui.m.bw_src.min(NCH - 1),
                AView::Power => id == ui.m.pw_v || id == ui.m.pw_i,
                _ => false,
            };
            if selected {
                rec.ch[c].clone()
            } else {
                Vec::new()
            }
        }),
        raw: rec.raw.clone(),
        la: None,
        t0: rec.t0,
        dt: rec.dt,
        triggered: rec.triggered,
        partial: rec.partial,
        pairs: rec.pairs,
    };
    let mut job = Job { key: key(ui), record, context, pane, view };
    job.context.azr.limit = ui.azr.limit.clone();
    Some(job)
}

fn submit(ui: &mut Ui, view: AView, pane: bool) {
    let Some(mut job) = prepare(ui, view, pane) else { return };
    ui.azr.work.pending = false;
    if let Some(w) = ui.meas_bg.as_mut() {
        ui.azr.work.queued_samples += job.record.ch.iter().map(|v| v.len() as u64).sum::<u64>();
        match w.tx.send(lifecycle::WorkerJob::Analyze(Box::new(job))) {
            Ok(()) => {
                w.analysis_busy = true;
                return;
            }
            Err(std::sync::mpsc::SendError(lifecycle::WorkerJob::Analyze(unsent))) => {
                eprintln!("oos-ui: analysis worker disconnected; falling back to synchronous analysis");
                ui.meas_bg = None;
                job = *unsent;
            }
            Err(_) => unreachable!("the unsent job is the Analyze variant sent above"),
        }
    }
    synchronous(ui, job);
}

fn synchronous(ui: &mut Ui, mut job: Job) {
    // The existing offline measurement mode makes scripts and golden captures deterministic.
    job.context.azr = std::mem::take(&mut ui.azr);
    job.context.calculate(&job.record, job.view, job.pane);
    ui.azr = job.context.azr;
    ui.azr.work.finished = Some(ui.now);
    ui.azr.work.accepted_key = Some(job.key);
}

pub(in crate::ui) fn updating(ui: &Ui) -> bool {
    ui.meas_bg.is_some()
        && (ui.azr.work.accepted_key.as_ref() != Some(&key(ui))
            || ui.azr.work.finished.is_none_or(|at| ui.now - at > 0.5))
}

/// The requested band clipped exactly as DSP clips it, using the current record's measurement interval.
pub(super) fn span(ui: &Ui, center: f64, requested: f64) -> f64 {
    let (Some(rec), Some(c)) = (&ui.last[0], ui.m.spec_src.ch()) else { return requested };
    let dt = ui.meas_slice(rec, c).1;
    if !(dt > 0.0 && dt.is_finite()) {
        return requested;
    }
    let nyquist = 0.5 / dt;
    let (start, stop) = ((center - requested / 2.0).max(0.0), (center + requested / 2.0).min(nyquist));
    if stop > start {
        stop - start
    } else {
        nyquist
    }
}

#[cfg(test)]
mod tests;
