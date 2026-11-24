// SPDX-License-Identifier: GPL-2.0-or-later

//! The change point owns acquisition publication; legacy reset consumers remain at their old seams.

use super::*;
use crate::perfguard::deep::ui_lock as lock;
use oos_core::change::{Identity, Requester};

/// Startup switches with the hardware draw route selected by the supplied model.
pub(crate) fn draw_switches(ui: &Ui, model: &Model) -> crate::sysconf::Switches {
    let mut sw = ui.switches().clone();
    if ui.src.is_hw() {
        sw.pipeline = if model.draw_batch { crate::drawpipe::Mode::Batch } else { crate::drawpipe::Mode::Frames };
    }
    sw
}

pub(crate) fn accepted_draw_switches(ui: &Ui) -> crate::sysconf::Switches {
    ui.host
        .runtime()
        .change
        .shadow
        .as_ref()
        .map_or_else(|| ui.switches().clone(), |shadow| draw_switches(ui, shadow.accepted()))
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

pub(in crate::ui) fn publish(ui: &mut Ui) {
    let Some(id) = ui.host.runtime().change.prepared.as_ref().map(|p| p.changes.identity()) else { return };
    let Some(shadow) = &ui.host.runtime().change.shadow else { return };
    let (model, needs) = (shadow.accepted(), shadow.needs());
    if let Some(clocks) = &ui.host.runtime().clocks {
        clocks.update(ui.src.is_hw() && model.run == RunState::Run);
    }
    let sw = &draw_switches(ui, model);
    let batch = sw.pipeline == crate::drawpipe::Mode::Batch;
    let access = if ui.src.is_hw() && model.run == RunState::Run && batch {
        crate::drawpipe::Access::from(sw)
    } else {
        crate::drawpipe::Access::Existing
    };
    if let Some(pipe) = crate::drawpipe::pipe(access) {
        if ui.src.is_hw()
            && (model.run == RunState::Run || ui.stop_final.is_some())
            && needs.route()
            && batch
            && ui.host.runtime().change.prepared.as_ref().is_some_and(|p| p.workers)
        {
            let zoom = model.zoom.then(|| step_draw::zoom_view(ui));
            pipe.publish_picture_view(model, (ui.ph.w, ui.ph.h), sw.interp, id, zoom);
        } else {
            pipe.set_view(None, 0, 0, sw.interp);
            pipe.set_identity(id);
        }
    } else if ui.src.is_hw() {
        crate::drawpipe::counters().metrics.set_identity(id);
    }
    let Source::Hw { shared, .. } = &ui.src else { return };
    let mut g = lock(shared);
    if g.acquisition.identity().gen() != id.gen() {
        crate::pacing::changed();
    }
    g.want = Some(want(model, sw, needs.mixed_bus()));
    g.slow = board().slow_mode(model);
    g.deep_raw = crate::deepmeas::want(model);
    g.mask_want = ui.mask_hw_want();
    g.zone_want = ui.zone_hw_want();
    g.needs_raw = needs.raw();
    g.avg_clear = ui.m.hw_avg_clear;
    g.cores = model.cores;
    if model.run == RunState::Run {
        // running: no STOP view any more, the thread acquires again
        (g.stop_view, g.replay) = (None, None);
    }
    segment_quota(ui, &mut g, id);
    g.record_flow.cost.set_identity(id);
    g.acquisition = crate::board::identity::Publication::new(id, needs.route());
    for i in 0..crate::model::gen_count() {
        g.afg[i] = Some(board().gen(&model.gen_ch(i)));
    }
    g.record_flow.wake();
    // This is a metrics mirror, never an identity source for a frame or batch.
    crate::pacing::WANTED.store(id.gen(), std::sync::atomic::Ordering::Relaxed);
    let before = g.queue.len();
    g.queue.retain(|f| current(f, id));
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.ui_publish, (before - g.queue.len()) as u64);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, (before - g.queue.len()) as u64);
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
    if super::consumers::apply(ui) {
        let elapsed = since(ui, started);
        ui.host.runtime_mut().change.timing.consumers(elapsed);
    }
    let started = now(ui);
    publish(ui);
    let elapsed = since(ui, started);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::{self, test_scope::Scope, Access};

    #[test]
    fn publication_and_status_use_the_accepted_draw_route_when_live_settings_differ() {
        for batch in [false, true] {
            let _scope = Scope::new();
            let mut ui = Ui::for_tests(Source::Hw { shared: Default::default(), seen: 0 });
            ui.m.draw_batch = batch;
            legacy::intake_settings(&mut ui);
            assert_eq!(ui.host.runtime().change.shadow.as_ref().unwrap().accepted().draw_batch, batch);
            ui.m.draw_batch = !batch;
            publish(&mut ui);
            assert_eq!(drawpipe::pipe(Access::Existing).is_some(), batch);
            let expected = if batch { " pipeline Batch " } else { " pipeline Frames " };
            assert!(crate::cmd::stat_text(&ui).contains(expected));
            legacy::intake_settings(&mut ui);
            publish(&mut ui);
            let expected = if batch { " pipeline Frames " } else { " pipeline Batch " };
            assert!(crate::cmd::stat_text(&ui).contains(expected));
        }
    }
}
