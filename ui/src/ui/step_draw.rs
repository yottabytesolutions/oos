// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{self, Accepted, Pipe, Stats};
use crate::picture::FirstDraw;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct BatchPicture {
    pub(super) frames: u32,
    pub(super) replacement_pending: bool,
    pub(super) layout_changed: bool,
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
    if let Some(pipe) = crate::drawpipe::pipe() {
        pipe.discard_window(&mut ui.batch_picture.window);
    }
}

pub(super) fn retire_picture_inputs(ui: &mut Ui) {
    if let Some(pipe) = drawpipe::pipe() {
        pipe.discard_window(&mut ui.batch_picture.window);
        ui.batch_picture.owner.retire_inputs(pipe, true);
    }
}

pub(super) fn clear_maps(ui: &mut Ui) {
    reset_window(ui);
    ui.ph.clear();
    ui.phz.clear();
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

fn merge(ui: &mut Ui, pipe: &Pipe, first: &mut FirstDraw, keep_window: bool) -> Accepted {
    let window = keep_window.then_some(&mut ui.batch_picture.window);
    let stats = Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off };
    let picture = crate::picture::PerMap::new(&mut ui.ph, ui.m.zoom.then_some(&mut ui.phz));
    let waiting = first.waiting();
    let accepted = ui.batch_picture.owner.compose_picture(pipe, picture, first, stats, window);
    if waiting && !first.waiting() && !ui.m.zoom {
        ui.phz.clear();
    }
    accepted
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

fn capture_can_replace(
    m: &Model,
    ch: &[Vec<f32>; NCH],
    partial: bool,
    has_la: bool,
    eligibility: PictureEligibility,
) -> bool {
    eligibility.draws()
        && !(partial && m.trig_mode == TrigMode::Single)
        && (has_la || (0..NCH).any(|c| m.ch[c].on && !ch[c].is_empty() && m.z_src != Some(c)))
}

pub(super) fn intake_count(ui: &Ui, fast: bool, taps: usize) -> usize {
    match ui.m.trig_type {
        // Peak read-outs on the record route are still consumed in FIFO order.
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
    first: &mut FirstDraw,
    keep_window: bool,
    composition: &mut Composition,
    draw: &mut [Duration; 5],
) -> Fast {
    let started = Instant::now();
    let mut frames = if ui.m.acq == AcqMode::Peak || ui.m.zoom { Vec::new() } else { ui.src.drain_newest() };
    let before_merge = Instant::now();
    let pipe = drawpipe::start_pipe(sw);
    let merged = merge(ui, pipe, first, keep_window);
    let after_merge = Instant::now();
    if first.waiting() {
        let before = frames.len();
        frames.retain(|f| capture_can_replace(&ui.m, &f.ch, f.partial, f.la.is_some(), PictureEligibility::Drawable));
        composition.filtered += before - frames.len();
    }
    if !frames.is_empty() && first.begin() {
        clear_maps(ui);
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
    if result.frames > 0 && prefix::draw_queued_prefix(ui, frames) > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    draw[1] += before_merge - started + after_merge.elapsed();
    let ms = draw[1].as_secs_f32() * 1e3;
    ui.t_fast += 0.05 * (ms - ui.t_fast);

    let processing = Instant::now();
    result.window_frames = merged.window_frames;
    result.frames += merged.frames as usize;
    accept(ui, merged, composition);
    draw[2] += after_merge - before_merge + processing.elapsed();
    let merge_ms = draw[2].as_secs_f32() * 1e3;
    result.ms = ms + merge_ms;
    crate::pacing::fast_block(ms, merge_ms);
    result
}

pub(super) fn replay(
    ui: &mut Ui,
    sw: &crate::sysconf::Switches,
    first: &mut FirstDraw,
    composition: &mut Composition,
) -> usize {
    if first.waiting() {
        return 0;
    }
    let pipe = drawpipe::start_pipe(sw);
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
    composition.draw_record(ui, &rec, sw.interp, None, eligibility, first)
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

pub(super) fn funded_taps(ui: &Ui) -> bool {
    drawpipe::pipe().is_some_and(|pipe| pipe.has_funded_taps(change::acquisition::identity(ui)))
}

pub(crate) fn status(ui: &Ui) -> &'static str {
    use crate::drawpipe::funding::Profile;
    if ui.record_mode() {
        return "replay";
    }
    if ui.m.run == RunState::Stop {
        return "stopped";
    }
    if !ui.fast_ok() {
        return "records";
    }
    match ui.switches().f2_funding {
        Profile::Unconfigured => "batch",
        _ if funded_taps(ui) => "batch-funded",
        _ => "records",
    }
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
    pub(super) filtered: usize,
    recovered: usize,
    unplaced: usize,
}

impl Composition {
    pub(super) fn filter_record(
        &mut self,
        ui: &mut Ui,
        rec: &Record,
        eligibility: PictureEligibility,
        first: &FirstDraw,
    ) -> bool {
        if !crate::zone::pass(&ui.m.zones, &rec.ch, rec.t0, rec.dt) {
            ui.zone_drops += 1;
        } else if ui.m.trig_type == TrigType::Bus
            && !ui.mixed_bus_mode()
            && ui.m.trig_mode != TrigMode::Auto
            && !rec.triggered
            && !rec.partial
            && ui.bus_can_match(rec)
        {
            // The decoder qualifies the hardware's edge triggers in Normal and Single.
            ui.m.status = TrigStatus::Wait;
        } else if !first.waiting() || capture_can_replace(&ui.m, &rec.ch, rec.partial, rec.la.is_some(), eligibility) {
            return false;
        }
        self.filtered += usize::from(eligibility == PictureEligibility::Drawable);
        true
    }

    pub(super) fn draw_record(
        &mut self,
        ui: &mut Ui,
        rec: &Record,
        interp: crate::sparse::Interp,
        input: Option<crate::drawpipe::reuse::EdgeInputs>,
        eligibility: PictureEligibility,
        first: &mut FirstDraw,
    ) -> usize {
        if eligibility.draws() && first.begin() {
            clear_maps(ui);
        }
        let (fast, records) = deep_draw::record(ui, rec, interp, input, eligibility);
        if ui.src.is_hw()
            && matches!(
                eligibility,
                PictureEligibility::Drawable | PictureEligibility::Recoverable | PictureEligibility::ShownOnly
            )
        {
            drawpipe::counters().routes.add(drawpipe::route::Route::Record, (fast + records) as u64);
        }
        self.deposit += fast;
        self.record += records;
        match eligibility {
            PictureEligibility::Redraw => self.redrawn += fast + records,
            PictureEligibility::Recoverable | PictureEligibility::ShownOnly => {
                self.unplaced += fast + records;
                self.recovered += (fast + records) * usize::from(eligibility == PictureEligibility::Recoverable);
            }
            _ => {}
        }
        fast + records
    }
}

pub(super) fn account_picture(ui: &Ui, records: usize, composition: Composition) {
    let Composition {
        deposit,
        record,
        newest,
        merged,
        redrawn,
        filtered,
        recovered,
        unplaced,
        #[cfg(feature = "private-picture-counts")]
        oldest,
    } = composition;
    let counters = crate::drawpipe::counters();
    if ui.src.is_hw() && counters.diagnostics_enabled() {
        counters.lose(drawpipe::Lost::Settings, filtered as u64);
        let direct = records as u64 - merged - unplaced as u64;
        counters.picture_drawn(records as u64, direct, recovered as u64);
    } else {
        crate::drawpipe::Counters::add(&counters.other_used, records as u64);
    }
    #[cfg(feature = "private-picture-counts")]
    if ui.src.is_hw()
        && ui.m.run == RunState::Run
        && !ui.record_mode()
        && ui.fast_ok()
        && counters.diagnostics_enabled()
    {
        counters.timing.picture_counts().observed(merged, oldest.filter(|_| merged > 0), records as u64 - merged);
    }
    crate::pacing::composition(deposit, record, newest.map(|t| t.elapsed().as_secs_f32() * 1e3));
    crate::pacing::picture(records + redrawn);
}

mod finish;
mod stopped;
pub(super) use {finish::finish, stopped::stopped};

pub(crate) mod capture;
#[cfg(test)]
pub(crate) mod count_tests;
pub(crate) mod owner;
pub(super) mod prefix;
