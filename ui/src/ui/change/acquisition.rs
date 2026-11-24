// SPDX-License-Identifier: GPL-2.0-or-later

//! The change point owns acquisition publication; legacy reset consumers remain at their old seams.

use super::*;
use crate::{
    board::{self, identity::Publication},
    drawpipe::{Pipe, PreparedView},
    perfguard::deep::ui_lock as lock,
};
use oos_core::change::{Identity, Requester};

struct HardwareRequest {
    publication: Publication,
    want: board::Want,
    slow: board::Slow,
    deep_raw: Option<crate::deepmeas::Want>,
    mask: Option<board::MaskWant>,
    zone: Option<board::ZoneWant>,
    needs_raw: bool,
    avg_clear: u64,
    started: bool,
    cores: bool,
    afg: [Option<board::Gen>; NGEN],
}

impl HardwareRequest {
    fn of(ui: &Ui, model: &Model, needs: UiAcq, prepared: &Prepared) -> Self {
        let sw = ui.switches();
        let gen_count = crate::model::gen_count();
        Self {
            publication: Publication::new(prepared.changes.identity(), needs.route()),
            want: want(model, sw, needs.mixed_bus()),
            slow: board().slow_mode(model),
            deep_raw: crate::deepmeas::want(model),
            mask: ui.mask_hw_want(),
            zone: ui.zone_hw_want(),
            needs_raw: needs.raw(),
            avg_clear: ui.m.hw_avg_clear,
            started: prepared.run_edge || prepared.single,
            cores: model.cores,
            afg: std::array::from_fn(|i| (i < gen_count).then(|| board().gen(&model.gen_ch(i)))),
        }
    }
}

pub(crate) fn identity(ui: &Ui) -> Identity {
    let observer = &ui.host.runtime().change;
    observer
        .prepared
        .as_ref()
        .map(|p| p.changes.identity())
        .or_else(|| observer.last.as_ref().map(ChangeSet::identity))
        .unwrap_or_default()
}

pub(super) fn publish(ui: &mut Ui) -> bool {
    let shared = match &ui.src {
        Source::Hw { shared, .. } => Some(shared),
        _ => None,
    };
    let Some(prepared) = ui.host.runtime().change.prepared.as_ref() else {
        return shared.is_some_and(|shared| final_done(&lock(shared), ui.final_req));
    };
    let running = !prepared.stopped;
    if let Some(clocks) = &ui.host.runtime().clocks {
        clocks.update(shared.is_some() && running);
    }
    let pipe = if shared.is_some() && running {
        Some(crate::drawpipe::start_pipe(ui.switches()))
    } else {
        crate::drawpipe::pipe()
    };
    let Some(shared) = shared else {
        if let Some(pipe) = pipe {
            pipe.set_view(None, 0, 0, ui.switches().interp);
            pipe.set_identity(prepared.changes.identity());
        }
        return false;
    };
    let (done, final_req) = publish_hardware(ui, shared, pipe, prepared);
    if let Some(req) = final_req {
        ui.final_req = req;
        ui.stop_final = Some(ui.now + STOP_FINAL_WAIT);
    }
    done
}

fn publish_hardware(
    ui: &Ui,
    shared: &board::HwShared,
    pipe: Option<&Pipe>,
    prepared: &Prepared,
) -> (bool, Option<u64>) {
    let running = !prepared.stopped;
    let hold = !running && (ui.last[0].is_some() || ui.cleared);
    let can_drain = hold && ui.was_running && transitions::wants_final_record(ui, true);
    let needs_drain = can_drain && transitions::wants_final_record(ui, false);
    let id = prepared.changes.identity();
    let shadow = ui.host.runtime().change.shadow.as_ref().expect("the accepted change has a normalized model");
    let (model, needs) = (shadow.accepted(), shadow.needs());
    let hardware = HardwareRequest::of(ui, model, needs, prepared);
    let draw = needs.route() && prepared.workers;
    let mut view = pipe
        .filter(|_| draw && (running || ui.stop_final.is_some() || can_drain))
        .map(|pipe| {
            let zoom = model.zoom.then(|| step_draw::zoom_view(ui));
            pipe.prepare_picture_view(model, (ui.ph.w, ui.ph.h), ui.switches().interp, id, zoom)
        })
        .unwrap_or_default();
    let mut g = lock(shared);
    let drain = can_drain && (g.record_flow.serial || needs_drain);
    let req = ui.final_req + u64::from(drain);
    let commit = || {
        let active = draw && (running || ui.stop_final.is_some() || drain);
        stage(ui, &mut g, pipe, (&mut view, active), hardware, running);
        Source::stage_hold(&mut g, hold, req);
        #[cfg(test)]
        {
            g.record_flow.acquisition_commit = Some((g.acquisition.identity(), g.record_flow.command_generations()));
        }
        g.record_flow.wake();
        final_done(&g, req)
    };
    #[cfg(not(test))]
    let done = commit();
    #[cfg(test)]
    let done = {
        let (allocations, done) = crate::system::test_allocator::count_own(commit);
        g.record_flow.acquisition_commit_allocations = allocations;
        done
    };
    (done, drain.then_some(req))
}

fn final_done(g: &crate::board::Shared, req: u64) -> bool {
    g.final_ack >= req && g.queue.is_empty()
}

fn stage(
    ui: &Ui,
    g: &mut board::Shared,
    pipe: Option<&Pipe>,
    (view, active): (&mut PreparedView, bool),
    hardware: HardwareRequest,
    running: bool,
) {
    let id = hardware.publication.identity();
    if let Some(pipe) = pipe {
        pipe.stage_picture_view(view, active, id);
    } else {
        crate::drawpipe::counters().metrics.set_identity(id);
    }
    if g.acquisition.identity().gen() != id.gen() {
        crate::pacing::changed();
    }
    g.want = Some(hardware.want);
    g.slow = hardware.slow;
    g.deep_raw = hardware.deep_raw;
    g.mask_want = hardware.mask;
    g.zone_want = hardware.zone;
    g.needs_raw = hardware.needs_raw;
    g.avg_clear = hardware.avg_clear;
    g.cores = hardware.cores;
    segment_quota(ui, g, id);
    let hardware_segments = ui.seg_hw_on();
    if hardware_segments || g.seg.running || ui.seg_view.is_some() || ui.seg_pos.is_some() {
        Source::stage_segments(g, ui.seg_run, running && hardware_segments);
    }
    g.record_flow.cost.set_identity(id);
    g.acquisition.replace(hardware.publication, hardware.started);
    if running {
        (g.stop_view, g.replay) = (None, None);
    }
    for (slot, gen) in g.afg.iter_mut().zip(hardware.afg) {
        if gen.is_some() {
            *slot = gen;
        }
    }
    // This is a metrics mirror, never an identity source for a frame or batch.
    crate::pacing::WANTED.store(id.gen(), std::sync::atomic::Ordering::Relaxed);
    let before = g.queue.len();
    g.queue.retain(|f| current(f, id));
    let removed = (before - g.queue.len()) as u64;
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.ui_publish, removed);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, removed);
}

fn segment_quota(ui: &Ui, g: &mut crate::board::Shared, id: Identity) {
    let old = g.acquisition.identity();
    let enabled = ui.m.segments > 0 && !ui.seg_hw_on() && g.slow == crate::board::Slow::Off;
    if (old.gen(), old.epoch()) != (id.gen(), id.epoch()) || enabled != g.record_flow.segment_bound().is_some() {
        g.record_flow.start_segments(enabled.then(|| ui.m.segments.min(SEG_MAX).saturating_sub(ui.seg_n) as u32));
    }
}

pub(in crate::ui) fn record_progress(ui: &Ui, records: u64, draw: std::time::Duration) {
    ui.src.note_record_draw(records, draw);
    if ui.m.segments == 0 || ui.seg_hw_on() {
        return;
    }
    if let Source::Hw { shared, .. } = &ui.src {
        let mut g = lock(shared);
        if g.queue.is_empty() {
            g.record_flow.segments_drawn(ui.m.segments.min(SEG_MAX).saturating_sub(ui.seg_n) as u32);
        }
    }
}

/// Project the accepted Bus setup into the hardware candidate request without changing visible settings.
pub(super) fn want(model: &Model, sw: &crate::sysconf::Switches, mixed: bool) -> crate::board::Want {
    if !mixed {
        return board().want(model, sw);
    }
    let mut candidate = model.clone();
    candidate.trig_type = TrigType::Edge;
    candidate.trig_mode = TrigMode::Auto;
    candidate.trig_src = candidate.dec_src;
    candidate.trig_ref = None;
    candidate.trig_level = candidate.dec_thr;
    candidate.slope = Slope::Rise;
    board().want(&candidate, sw)
}

pub(crate) fn current(f: &crate::board::Frame, id: Identity) -> bool {
    (f.cfg, f.stamp) == (id.gen(), id.epoch())
}

/// The record belongs to the current acquisition generation. A replay record is made synchronously on the step that
/// shows it, so it carries no generation tag.
pub(crate) fn record(ui: &Ui, rec: &Record) -> bool {
    matches!(ui.src, Source::Replay { .. }) || rec.raw.as_ref().is_none_or(|r| current(&r.frame, identity(ui)))
}

pub(in crate::ui) fn stage_record(ui: &Ui, requester: Requester, rec: &Record) -> bool {
    let Some(id) = proposals::resolved(ui, requester) else { return false };
    rec.raw.as_ref().is_none_or(|r| current(&r.frame, id))
}

/// Script producers run the same intake point before feeding a batch, then Ui::step runs its consumers.
#[cfg(test)]
pub(in crate::ui) fn script_prepare(ui: &mut Ui) -> Identity {
    script_prepare_route(ui, false)
}

pub(in crate::ui) fn script_prepare_route(ui: &mut Ui, records: bool) -> Identity {
    if ui.host.runtime().change.prepared.is_none() {
        // the same split intake accounting as the live step: polls, external body and shared waits
        ui.host.runtime_mut().change.timing.intake_begin();
        let ready = lifecycle::polls(ui);
        ui.host.runtime_mut().change.timing.intake_end();
        if !ready {
            return identity(ui);
        }
        let started = now(ui);
        legacy::intake_settings(ui);
        let elapsed = since(ui, started);
        ui.host.runtime_mut().change.timing.queue_change(elapsed);
    }
    if let Some(p) = &mut ui.host.runtime_mut().change.prepared {
        p.workers = !records;
    }
    let started = now(ui);
    let (_, consumer_time) = settle::commit(ui);
    let elapsed = since(ui, started).saturating_sub(consumer_time);
    ui.host.runtime_mut().change.timing.queue_change(elapsed);
    identity(ui)
}

/// A measurement still out, and a record waiting for the worker (`meas_pending`), belong to the record cleared away
/// (upstream 99dd12e9: the waiting record held the next measurement cycle back).
pub(in crate::ui) fn cancel_measurement(ui: &mut Ui) {
    if let Some(worker) = &mut ui.meas_bg {
        worker.accept = false;
    }
    ui.meas_pending = None;
}

pub(in crate::ui) fn autoset_fresh(ui: &Ui) -> Option<bool> {
    ui.last[0].as_ref().filter(|r| r.raw.is_some()).map(|r| stage_record(ui, Requester::Autoset, r))
}
