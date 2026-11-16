// SPDX-License-Identifier: GPL-2.0-or-later

//! The change point owns acquisition publication; legacy reset consumers remain at their old seams.

use super::*;
use oos_core::change::{Identity, Requester};
use oos_hal::sync::lock;

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
    let Source::Hw { shared, .. } = &ui.src else { return };
    let Some(shadow) = &ui.host.runtime().change.shadow else { return };
    let (model, needs) = (shadow.accepted(), shadow.needs());
    let sw = ui.switches();
    let access =
        if model.run == RunState::Run { crate::drawpipe::Access::from(sw) } else { crate::drawpipe::Access::Existing };
    if let Some(pipe) = crate::drawpipe::pipe(access) {
        if model.run == RunState::Run && needs.route() {
            pipe.publish_view(model, ui.ph.w, ui.ph.h, sw.interp, id);
        } else {
            pipe.set_identity(id);
        }
    }
    let mut g = lock(shared);
    if g.acquisition.identity().gen() != id.gen() {
        crate::pacing::changed();
    }
    g.want = Some(board().want(model, sw));
    g.slow = board().slow_mode(model);
    g.deep_raw = crate::deepmeas::want(model, sw);
    g.mask_want = ui.mask_hw_want();
    g.zone_want = ui.zone_hw_want();
    g.needs_raw = needs.raw();
    g.avg_clear = ui.m.hw_avg_clear;
    g.cores = model.cores;
    if model.run == RunState::Run {
        // running: no STOP view any more, the thread acquires again
        (g.stop_view, g.replay) = (None, None);
    }
    g.acquisition = crate::board::identity::Publication::new(id, needs.route());
    for i in 0..crate::model::gen_count() {
        g.afg[i] = Some(board().gen(&model.gen_ch(i)));
    }
    // This is a metrics mirror, never an identity source for a frame or batch.
    crate::pacing::WANTED.store(id.gen(), std::sync::atomic::Ordering::Relaxed);
    let before = g.queue.len();
    g.queue.retain(|f| current(f, id));
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.ui_publish, (before - g.queue.len()) as u64);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, (before - g.queue.len()) as u64);
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
pub(in crate::ui) fn script_prepare(ui: &mut Ui) -> Identity {
    if ui.host.runtime().change.prepared.is_none() {
        if !lifecycle::polls(ui) {
            return identity(ui);
        }
        legacy::intake_settings(ui);
        publish(ui);
    }
    identity(ui)
}

pub(in crate::ui) fn cancel_measurement(ui: &mut Ui) {
    if let Some(worker) = &mut ui.meas_bg {
        worker.accept = false;
    }
}

pub(in crate::ui) fn autoset_fresh(ui: &Ui) -> Option<bool> {
    ui.last[0].as_ref().filter(|r| r.raw.is_some()).map(|r| stage_record(ui, Requester::Autoset, r))
}
