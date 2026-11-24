// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{self, Accepted, Access, Pipe, Stats};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct BatchPicture {
    pub(super) frames: u32,
    pub(super) tap_statistics: Option<deep_draw::TapStatistics>,
    window: crate::picture::DepositWindow,
    owner: crate::drawpipe::stage::owner::Owner,
    capture: capture::CaptureStatus,
}

pub(super) fn release_window(ui: &mut Ui) {
    let wake = ui.batch_picture.owner.take_wake();
    ui.batch_picture = BatchPicture::default();
    if let Some(wake) = wake {
        ui.batch_picture.owner.set_wake(wake);
    }
}

pub(super) fn reset_window(ui: &mut Ui) {
    if let Some(pipe) = crate::drawpipe::pipe(crate::drawpipe::Access::Existing) {
        pipe.discard_window(&mut ui.batch_picture.window);
    }
}

pub(super) fn zoom_view(ui: &Ui) -> drawpipe::route::ZoomView {
    drawpipe::route::ZoomView::new((ui.phz.w, ui.phz.h), ui.zoom_vfactor, ui.zoom_vcenter)
}

pub(super) fn prepare_window(ui: &mut Ui) -> bool {
    let active = owner::render_owner_active(ui);
    if !active {
        ui.batch_picture.owner.clear();
        reset_window(ui);
    }
    active
}

fn merge(ui: &mut Ui, pipe: &Pipe, clear: &mut bool, keep_window: bool) -> Accepted {
    let window = keep_window.then_some(&mut ui.batch_picture.window);
    let stats = Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off };
    let picture = crate::picture::PerMap::new(&mut ui.ph, ui.m.zoom.then_some(&mut ui.phz));
    ui.batch_picture.owner.compose_picture(pipe, picture, clear, stats, window)
}

fn accept(ui: &mut Ui, d: Accepted, composition: &mut Composition) {
    if d.triggered > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    #[cfg(feature = "private-picture-counts")]
    if d.frames > 0 {
        composition.oldest = if composition.merged == 0 {
            d.oldest
        } else {
            match (composition.oldest, d.oldest) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            }
        };
    }
    composition.merged += d.frames;
    (composition.deposit, composition.newest) = (composition.deposit + d.frames as usize, d.newest);
    crate::pacing::drawn_stale(d.stale);
    ui.mstat_fold(&d.folds);
}

#[derive(Default)]
pub(super) struct Fast {
    pub(super) frames: usize,
    pub(super) raw: Option<Record>,
    pub(super) window_frames: Option<u64>,
    pub(super) ms: f32,
}

pub(super) fn intake_count(ui: &Ui, fast: bool, taps: usize) -> usize {
    match ui.m.trig_type {
        // Peak read-outs that reach the legacy queue are still consumed in FIFO order.
        _ if fast && (ui.m.acq == AcqMode::Peak || ui.m.zoom) => taps.max(crate::board::RECORDS_PER_STEP),
        _ if fast => taps.max(1),
        _ if ui.src.is_hw() => crate::board::RECORDS_PER_STEP,
        TrigType::Edge | TrigType::Pattern => 6,
        TrigType::Video => 1,
        _ => 2,
    }
}

pub(super) struct RecordDraw {
    pub(super) records: u64,
    pub(super) elapsed: Duration,
}

impl RecordDraw {
    pub(super) fn new(queued: usize, elapsed: Duration) -> Self {
        Self { records: queued as u64, elapsed: if queued == 0 { Duration::ZERO } else { elapsed } }
    }

    pub(super) fn acquired(&mut self, tap: Option<bool>, elapsed: impl FnOnce() -> Duration) -> bool {
        let record = tap == Some(false);
        if record {
            self.records += 1;
            self.elapsed += elapsed();
        }
        record
    }

    pub(super) fn placed(&mut self, record: bool, elapsed: Duration) -> Duration {
        if record {
            self.elapsed += elapsed;
        }
        elapsed
    }
}

#[cfg(test)]
#[test]
fn record_draw_cost_includes_queued_records_and_excludes_deposit_taps() {
    let mut cost = RecordDraw::new(3, Duration::from_millis(6));
    let tap = cost.acquired(Some(true), || panic!("a deposit tap contributes no record-route cost"));
    cost.placed(tap, Duration::from_millis(10));
    assert!(!cost.acquired(None, || panic!("an empty read contributes no record-route cost")));
    let record = cost.acquired(Some(false), || Duration::from_millis(1));
    cost.placed(record, Duration::from_millis(1));
    assert_eq!((cost.records, cost.elapsed), (4, Duration::from_millis(8)));
    assert_eq!(RecordDraw::new(0, Duration::from_millis(6)).elapsed, Duration::ZERO);
}

pub(super) fn place_records(
    frames: &[std::sync::Arc<crate::board::Frame>],
    plan: impl Fn(&std::sync::Arc<crate::board::Frame>) -> crate::picture::deposit::standard::FastFrame + Sync,
) -> Vec<crate::picture::deposit::standard::FastFrame> {
    let placed = crate::fastdraw::par_map(frames, plan);
    drawpipe::counters().routes.add(drawpipe::route::Route::Record, frames.len() as u64);
    placed
}

pub(super) fn fast(
    ui: &mut Ui,
    sw: &crate::sysconf::Switches,
    clear: &mut bool,
    keep_window: bool,
    composition: &mut Composition,
    draw: &mut [Duration; 5],
) -> Fast {
    let started = Instant::now();
    let frames = if ui.m.acq == AcqMode::Peak || ui.m.zoom { Vec::new() } else { ui.src.drain_newest() };
    let before_merge = Instant::now();
    let pipe = crate::drawpipe::pipe(sw);
    let merged = pipe.map(|pipe| merge(ui, pipe, clear, keep_window));
    let after_merge = Instant::now();
    if !frames.is_empty() && std::mem::take(clear) {
        reset_window(ui);
        ui.ph.clear();
    }
    let mut result = Fast { frames: frames.len(), ..Default::default() };
    for f in &frames {
        crate::pacing::drawn(f.cfg, 1);
    }
    composition.deposit += result.frames;
    let deep_raw = crate::deepmeas::want(&ui.m).is_some();
    if let Some(f) = frames.iter().rev().find(|f| deep_raw && crate::deepmeas::deep_raw_frame(f)) {
        let t = Instant::now();
        result.raw = Some(crate::wave::hw_record(&ui.m, f.clone()));
        crate::deepmeas::cost::set(&crate::deepmeas::cost::RECORD_US, t.elapsed());
    }
    if result.frames > 0 && ui.fast_draw_all(frames) > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    draw[1] += before_merge - started + after_merge.elapsed();
    let ms = draw[1].as_secs_f32() * 1e3;
    ui.t_fast += 0.05 * (ms - ui.t_fast);

    let processing = Instant::now();
    if let Some(d) = merged {
        result.window_frames = d.window_frames;
        result.frames += d.frames as usize;
        accept(ui, d, composition);
    }
    draw[2] += after_merge - before_merge + processing.elapsed();
    let merge_ms = draw[2].as_secs_f32() * 1e3;
    result.ms = ms + merge_ms;
    crate::pacing::fast_block(ms, merge_ms);
    result
}

pub(super) fn replay(
    ui: &mut Ui,
    sw: &crate::sysconf::Switches,
    clear: &mut bool,
    composition: &mut Composition,
) -> usize {
    let Some(pipe) = crate::drawpipe::pipe(sw) else { return 0 };
    let Some(frame) = ui.last[0].as_ref().and_then(|rec| rec.raw.as_ref()).map(|raw| &raw.frame) else {
        return 0;
    };
    let Some(eligibility) = pipe.take_preview(frame, (ui.ph.w, ui.ph.h)) else { return 0 };
    let Source::Hw { shared, .. } = &ui.src else { return 0 };
    {
        let shared = crate::perfguard::deep::ui_lock(shared);
        let latest = &shared.frame;
        if (latest.cfg, latest.stamp) == (frame.cfg, frame.stamp) && latest.seq > frame.seq {
            return 0;
        }
    }
    let rec = crate::wave::hw_record(&ui.m, frame.clone());
    if std::mem::take(clear) {
        reset_window(ui);
        ui.ph.clear();
    }
    composition.draw_record(ui, &rec, sw.interp, None, eligibility)
}

pub(super) fn stopped(ui: &mut Ui) {
    // stopped: the whole stored record for the view shown (read again by the thread, `stop_replay`), then
    // redraw the stored record with the current scale/offset
    ui.stop_replay();
    ui.ph.clear();
    ui.phz.clear();
    let mainv = (ui.m.time_per_div(), ui.m.delay);
    let zv = (tscales()[ui.m.zscale], ui.m.zdelay);
    let zoom_model = ui.zoom_display_model();
    let detail = ui.captured_view(zv);
    // the view read again as pairs while the raw record of the STOP is the one measured: the pairs are drawn
    let shown = ui.stop_picture().cloned().or_else(|| ui.last[0].clone());
    if let Some(rec) = shown {
        let zw = ui.z_weights(&rec);
        for c in 0..NCH {
            if ui.m.ch[c].on && !rec.ch[c].is_empty() && ui.m.z_src != Some(c) {
                let shifted_main = ui.deskew_channel_view(&rec, c, mainv);
                let main = shifted_main.as_deref().unwrap_or(&rec);
                ui.ph.draw_ex(
                    c,
                    &ui.m,
                    mainv,
                    main.t0 - ui.m.deskew_ch[c],
                    main.dt,
                    &main.ch[c],
                    24 * 6,
                    if shifted_main.is_some() { None } else { zw.as_deref() },
                    main.pairs,
                );
                if ui.m.zoom {
                    let shifted = ui.deskew_channel_view(&rec, c, zv);
                    let z = shifted.as_deref().or(detail.as_deref()).unwrap_or(&rec);
                    ui.phz.draw_ex(
                        c,
                        &zoom_model,
                        zv,
                        z.t0 - ui.m.deskew_ch[c],
                        z.dt,
                        &z.ch[c],
                        24 * 6,
                        if shifted.is_some() || detail.is_some() { None } else { zw.as_deref() },
                        z.pairs,
                    );
                }
            }
        }
    }
}

pub(super) fn render_start(ui: &mut Ui, l: &mut Layers) -> bool {
    ui.relayout();
    if let Some(o) = l.over.as_deref_mut() {
        o.no_clip();
        o.clear(0);
    }
    if ui.start_up_active() {
        ui.draw_start_up(l.under);
        // The power dialog remains over the start-up screen.
        ui.draw_overlay(l.over());
        return true;
    }
    false
}

pub(crate) fn status(ui: &Ui) -> (drawpipe::Mode, &'static str) {
    use crate::drawpipe::{funding::Profile, Mode};
    let switches = change::acquisition::accepted_draw_switches(ui);
    let staged = ui.src.is_hw() && switches.f2_funding != Profile::Unconfigured;
    let funded = staged
        && switches.pipeline == Mode::Batch
        && drawpipe::pipe(Access::Existing).is_some_and(|pipe| pipe.has_funded_taps(change::acquisition::identity(ui)));
    let pipeline = if staged && !funded { Mode::Frames } else { switches.pipeline };
    let route = if ui.record_mode() {
        "replay"
    } else if ui.m.run == RunState::Stop {
        "stopped"
    } else if !ui.fast_ok() {
        "records"
    } else if funded {
        "batch-funded"
    } else if ui.m.draw_batch && !staged {
        "batch"
    } else {
        "standard"
    };
    (pipeline, route)
}

#[derive(Default)]
pub(super) struct Composition {
    pub(super) deposit: usize,
    pub(super) record: usize,
    pub(super) newest: Option<std::time::Instant>,
    pub(super) merged: u64,
    #[cfg(feature = "private-picture-counts")]
    pub(super) oldest: Option<std::time::Instant>,
    pub(super) redrawn: usize,
}

impl Composition {
    pub(super) fn draw_record(
        &mut self,
        ui: &mut Ui,
        rec: &Record,
        interp: crate::sparse::Interp,
        input: Option<crate::drawpipe::reuse::EdgeInputs>,
        eligibility: PictureEligibility,
    ) -> usize {
        let (fast, records) = deep_draw::record(ui, rec, interp, input, eligibility);
        if ui.src.is_hw() && eligibility == PictureEligibility::Drawable {
            drawpipe::counters().routes.add(drawpipe::route::Route::Record, (fast + records) as u64);
        }
        self.deposit += fast;
        self.record += records;
        self.redrawn += if eligibility == PictureEligibility::Redraw { fast + records } else { 0 };
        fast + records
    }
}

pub(super) fn account_picture(ui: &Ui, records: usize, composition: Composition) {
    #[cfg(not(feature = "private-picture-counts"))]
    let Composition { deposit, record, newest, merged, redrawn } = composition;
    #[cfg(feature = "private-picture-counts")]
    let Composition {
        deposit,
        record,
        newest,
        merged,
        redrawn,
        #[cfg(feature = "private-picture-counts")]
        oldest,
    } = composition;
    let counters = crate::drawpipe::counters();
    crate::drawpipe::Counters::add(&counters.used, records as u64);
    if ui.src.is_hw() && counters.diagnostics_enabled() {
        crate::drawpipe::Counters::add(&counters.shown, records as u64);
        let direct = records as u64 - merged;
        if direct > 0 {
            crate::drawpipe::Counters::add(&counters.deposited, direct);
        }
    }
    #[cfg(feature = "private-picture-counts")]
    if ui.src.is_hw()
        && ui.m.run == RunState::Run
        && !ui.record_mode()
        && ui.m.draw_batch
        && ui.fast_ok()
        && counters.diagnostics_enabled()
    {
        counters.timing.picture_counts().observed(merged, oldest.filter(|_| merged > 0), records as u64 - merged);
    }
    crate::pacing::composition(deposit, record, newest.map(|t| t.elapsed().as_secs_f32() * 1e3));
    crate::pacing::picture(records + redrawn);
}

pub(super) fn finish(ui: &mut Ui, parts: [std::time::Duration; 5], records: usize) {
    ui.ph.publish();
    ui.phz.publish();
    #[cfg(feature = "private-picture-counts")]
    crate::drawpipe::counters().timing.picture_counts().published();
    let timing = change::timing_mut(ui);
    timing.draw_step(parts);
    if crate::drawpipe::counters().diagnostics_enabled() {
        timing.picture_frames(records);
    }
}

pub(crate) mod capture;
#[cfg(all(test, feature = "private-picture-counts"))]
mod count_tests;
pub(crate) mod owner;
