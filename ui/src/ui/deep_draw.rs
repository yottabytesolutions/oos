// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::QueuedFrame;
use crate::drawpipe::reuse::EdgeInputs;
use crate::drawpipe::{self, Pipe, TapRead};
use std::sync::{Arc, Weak};

pub(super) struct ReaderTaps {
    pipe: Option<&'static Pipe>,
    ready: usize,
    attempts: usize,
    previous_tap: Option<bool>,
    ready_folds: Option<crate::framestats::TapFolds>,
    previous_folds: Option<crate::framestats::TapFolds>,
    folds: Option<crate::framestats::Folds>,
    deep: Option<TapStatistics>,
}

impl ReaderTaps {
    pub(super) fn new(ui: &Ui) -> Self {
        let pipe = ui.src.is_hw().then(drawpipe::pipe).flatten();
        let attempts = pipe.map_or(0, Pipe::tap_drain);
        let ready = pipe.map_or(0, |pipe| pipe.tap_count().min(attempts));
        Self {
            pipe,
            ready,
            attempts,
            previous_tap: None,
            ready_folds: None,
            previous_folds: None,
            folds: None,
            deep: None,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.ready
    }

    fn take(
        &mut self,
        size: (usize, usize),
        id: oos_core::change::Identity,
        fallback: Option<u64>,
    ) -> TapRead<QueuedFrame> {
        let Some(pipe) = self.pipe else { return TapRead::Empty };
        loop {
            let result = pipe.read_waveform(size, Some(id), fallback, self.attempts > 0);
            match result {
                TapRead::Stale => self.attempts -= 1,
                TapRead::Ready((frame, folds)) => {
                    self.attempts -= 1;
                    self.ready_folds = folds;
                    return TapRead::Ready(frame);
                }
                TapRead::Pending => return TapRead::Pending,
                TapRead::Empty => return TapRead::Empty,
            }
        }
    }

    pub(super) fn next_record(&mut self, ui: &Ui, tap: bool) {
        if self.previous_tap.replace(tap) == Some(true) {
            if let Some(record) = &ui.last[0] {
                if self.fold_displaced(ui, record) {
                    drawpipe::Counters::add(&drawpipe::counters().measured, 1);
                }
            }
        }
        self.previous_folds = self.ready_folds.take().filter(|_| tap);
    }

    fn fold_displaced(&mut self, ui: &Ui, record: &Record) -> bool {
        let Some(raw) = &record.raw else { return false };
        let identity = change::acquisition::identity(ui);
        let tap = self.previous_folds.as_ref().filter(|folds| folds.current(identity.stats_gen()));
        if crate::deepmeas::deep_raw_frame(&raw.frame) {
            // A newer pair record can own the picture while this tap still owns its measurements.
            let counted =
                ui.m.meas
                    .iter()
                    .map(|&(kind, source)| {
                        source.ch().is_none_or(|c| tap.and_then(|folds| folds.displaced(kind, c)).is_some())
                    })
                    .collect();
            self.deep = Some(TapStatistics::new(ui, &raw.frame, counted));
        }
        let Some(tap) = tap else { return false };
        let folds = self.folds.get_or_insert_with(|| crate::framestats::folds_for(&ui.m));
        let mut measured = false;
        for ((kind, channel), fold) in folds {
            if let Some(value) = tap.displaced(*kind, *channel) {
                fold.push(value);
                measured = true;
            }
        }
        measured
    }

    pub(super) fn fold(self, ui: &mut Ui, raw_record: Option<&Record>) {
        if let Some(folds) = self.folds {
            ui.mstat_fold(&folds);
        }
        if held_deep(ui) {
            if let Some(record) = raw_record {
                ui.batch_picture.tap_statistics = self.deep.filter(|stats| matches_tap(Some(record), &stats.frame));
            }
        } else if let Some(tap) = self.previous_tap {
            ui.batch_picture.tap_statistics =
                if tap { TapStatistics::fold_latest(ui, self.previous_folds.as_ref()) } else { None };
        }
    }
}

pub(super) struct TapStatistics {
    frame: Weak<crate::board::Frame>,
    identity: oos_core::change::Identity,
    counted: Vec<bool>,
}

impl TapStatistics {
    fn new(ui: &Ui, frame: &Arc<crate::board::Frame>, counted: Vec<bool>) -> Self {
        Self { frame: Arc::downgrade(frame), identity: change::acquisition::identity(ui), counted }
    }

    // The generic reader uses the selected physical gate, including full-record measurements.
    fn fold_latest(ui: &mut Ui, folds: Option<&crate::framestats::TapFolds>) -> Option<Self> {
        let rec = ui.last[0].as_ref()?;
        let raw = rec.raw.as_ref()?;
        if crate::compress::live(&raw.frame) || !ui.m.meas.iter().any(|(_, source)| source.ch().is_some()) {
            return None;
        }
        let mut counted = Vec::with_capacity(ui.m.meas.len());
        let mut measured = false;
        let folds = folds
            .filter(|folds| folds.current(change::acquisition::identity(ui).stats_gen()) && folds.matches_gate(&ui.m));
        for (i, &(kind, source)) in ui.m.meas.iter().enumerate() {
            let Some(c) = source.ch() else {
                counted.push(true);
                continue;
            };
            let value = folds.and_then(|folds| folds.latest(kind, c));
            if let Some(value) = value {
                if ui.mstat.len() <= i {
                    ui.mstat.resize(ui.m.meas.len(), MStat::default());
                }
                ui.mstat[i].add(value);
                measured = true;
            }
            counted.push(value.is_some());
        }
        if measured {
            drawpipe::Counters::add(&drawpipe::counters().measured, 1);
        }
        Some(Self::new(ui, &raw.frame, counted))
    }

    fn current(&self, ui: &Ui) -> bool {
        self.identity == change::acquisition::identity(ui)
            && (matches_tap(ui.last[0].as_ref(), &self.frame) || held_deep(ui))
    }

    pub(super) fn frame_for(&self, record: &Record) -> Option<Weak<crate::board::Frame>> {
        matches_tap(Some(record), &self.frame).then(|| self.frame.clone())
    }
}

fn matches_tap(record: Option<&Record>, frame: &Weak<crate::board::Frame>) -> bool {
    record.and_then(|rec| rec.raw.as_ref()).is_some_and(|raw| Arc::as_ptr(&raw.frame) == frame.as_ptr())
}

pub(super) fn measurement_rows_ready(ui: &Ui, tap: Option<&Weak<crate::board::Frame>>) -> bool {
    if let Some(stats) = &ui.batch_picture.tap_statistics {
        if stats.identity == change::acquisition::identity(ui) && held_deep(ui) {
            return tap.is_some_and(|frame| frame.ptr_eq(&stats.frame));
        }
    }
    tap.is_none_or(|frame| matches_tap(ui.last[0].as_ref(), frame))
}

fn account_tap_rows(ui: &mut Ui, rows: &[tools::MeasRow]) -> bool {
    let Some(mut statistics) = ui.batch_picture.tap_statistics.take().filter(|statistics| statistics.current(ui))
    else {
        return false;
    };
    let held = held_deep(ui);
    for (i, (counted, (&(kind, source), row))) in
        statistics.counted.iter_mut().zip(ui.m.meas.iter().zip(rows)).enumerate()
    {
        if !*counted && row.v.is_finite() && (held || !ui.meas_number_computing(source, kind)) {
            ui.mstat[i].add(row.v);
            *counted = true;
        }
    }
    ui.batch_picture.tap_statistics = Some(statistics);
    true
}

pub(super) fn account_live_statistics(ui: &mut Ui, rows: &[tools::MeasRow]) {
    ui.mstat.resize(rows.len(), MStat::default());
    ui.counter_update();
    let tap = account_tap_rows(ui, rows);
    let singles = ui.m.meas.len();
    let catalog = singles + ui.m.meas2.len();
    for (i, row) in rows.iter().enumerate() {
        if row.computing {
            if let Some(entry) = i.checked_sub(catalog).and_then(|i| ui.m.cat.get(i)).copied() {
                ui.queue_catalog_statistics(entry, ui.wfm_total);
            }
        }
        if row.v.is_finite() && !(tap && i < singles && row.src.ch().is_some()) {
            ui.mstat[i].add(row.v);
        }
    }
}

pub(super) fn finish_reader_statistics(ui: &mut Ui) {
    if ui
        .batch_picture
        .tap_statistics
        .as_ref()
        .is_some_and(|stats| stats.counted.contains(&false) && stats.current(ui) && !held_deep(ui))
    {
        let rows = ui.meas_rows();
        ui.mstat.resize(rows.len(), MStat::default());
        account_tap_rows(ui, &rows);
    }
    ui.finish_deferred_capture_statistics();
}

pub(super) fn acquire(
    ui: &mut Ui,
    elapsed: f64,
    taps: &mut ReaderTaps,
) -> (Option<(Record, PictureEligibility, bool)>, Option<EdgeInputs>) {
    // Capture before conversion and compare again at drawing. Intervening routing
    // or settings changes cannot silently reuse a different view.
    let input = if ui.fast_ok() { EdgeInputs::of(&ui.m) } else { None };
    let record = if ui.src.is_hw() && !ui.mixed_bus_mode() {
        ui.src.acquire_hw(&ui.m, |id, fallback| taps.take((ui.ph.w, ui.ph.h), id, fallback))
    } else {
        ui.acquire_bus_candidate(elapsed).map(|record| (record, PictureEligibility::Drawable, false))
    };
    (record, input)
}

pub(super) fn record(
    ui: &mut Ui,
    rec: &Record,
    interp: crate::sparse::Interp,
    input: Option<EdgeInputs>,
    eligibility: PictureEligibility,
) -> (usize, usize) {
    if !eligibility.draws() {
        return (0, 0);
    }
    let count = eligibility == PictureEligibility::Drawable;
    if let Some(raw) = &rec.raw {
        if count {
            crate::pacing::drawn(raw.frame.cfg, 1);
        }
        if ui.fast_ok() && !ui.m.zoom && ui.m.acq != AcqMode::Peak && !crate::deepmeas::deep_raw_frame(&raw.frame) {
            let stats = count.then_some(crate::drawpipe::Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off });
            let saved = input.map(|input| (rec, input));
            crate::drawpipe::draw_frame_reusing(&mut ui.ph, &ui.m, &raw.frame, interp, stats, saved);
            return (1, 0);
        }
    }
    draw_record(ui, rec);
    (0, 1)
}

fn draw_record(ui: &mut Ui, rec: &Record) {
    let mainv = (ui.m.time_per_div(), ui.m.delay);
    let zv = (tscales()[ui.m.zscale], ui.m.zdelay);
    let zoom_model = ui.zoom_display_model();
    let detail = ui.captured_record_view(rec, zv);
    let zw = ui.z_weights(rec);
    for c in 0..NCH {
        if ui.m.ch[c].on && !rec.ch[c].is_empty() && ui.m.z_src != Some(c) {
            let shifted_main = ui.deskew_channel_view(rec, c, mainv);
            let main = shifted_main.as_deref().unwrap_or(rec);
            ui.ph.draw_ex(
                c,
                &ui.m,
                mainv,
                main.t0 - ui.m.deskew_ch[c],
                main.dt,
                &main.ch[c],
                24,
                if shifted_main.is_some() { None } else { zw.as_deref() },
                main.pairs,
            );
            if ui.m.zoom {
                let shifted = ui.deskew_channel_view(rec, c, zv);
                let z = shifted.as_deref().or(detail.as_deref()).unwrap_or(rec);
                ui.phz.draw_ex(
                    c,
                    &zoom_model,
                    zv,
                    z.t0 - ui.m.deskew_ch[c],
                    z.dt,
                    &z.ch[c],
                    24,
                    if shifted.is_some() || detail.is_some() { None } else { zw.as_deref() },
                    z.pairs,
                );
            }
        }
    }
}

pub(super) fn update_readers(ui: &mut Ui, raw_rec: Option<Record>) {
    // deep record on min/max pairs while raw reads of its screen window come (deepmeas.rs): the
    // measurement stays that of the last raw record
    let shown = ui.last[0].as_ref().and_then(|r| r.raw.as_ref()).map(|r| &*r.frame);
    let hold = held_deep(ui) || crate::compress::hold(&ui.m, shown);
    let p0 = std::time::Instant::now();
    if hold {
        // the measurement shown stays that of the last raw deep record, or of this step's
        if let Some(r) = raw_rec {
            if ui.meas_bg.is_some() {
                ui.meas_want = true;
                ui.meas_pending = Some(r);
            } else {
                ui.meas_new |= ui.measure_rec(&r, usize::MAX);
            }
        }
    } else if ui.meas_bg.is_some() {
        // A previous record's result remains useful within the same acquisition/statistics identity.
        ui.meas_want = true;
        // raw deep reads: this raw record is the one to measure, even when pair records (held) follow
        // before the worker is free; measuring the newest then measured a pair record (Mean NaN live)
        // (only a raw deep record: a clone of every record cost the screen rate, 26 instead of 27 frames a
        // second with GPU and HDMI Layout, unit 2026-11-15)
        let raw_deep =
            ui.last[0].as_ref().and_then(|r| r.raw.as_ref()).is_some_and(|r| crate::deepmeas::deep_raw_frame(&r.frame));
        if raw_deep && crate::deepmeas::want(&ui.m).is_some() {
            ui.meas_pending = ui.last[0].clone();
        }
    } else {
        ui.meas_new |= ui.measure_last();
    }
    let p1 = std::time::Instant::now();
    ui.analyzer_update();
    ui.monitor_analyzer_update();
    ui.protocol_watch();
    change::proposals::bode(ui);
    let p2 = std::time::Instant::now();
    ui.prof_add(0, p1 - p0);
    ui.prof_add(1, p2 - p1);
}

fn held_deep(ui: &Ui) -> bool {
    let shown = ui.last[0].as_ref().and_then(|r| r.raw.as_ref()).map(|r| &*r.frame);
    shown.is_some_and(crate::deepmeas::deep_pairs) && crate::deepmeas::hold(&ui.m, shown, ui.src.hw_deep_ok())
}
