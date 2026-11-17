// SPDX-License-Identifier: GPL-2.0-or-later

//! Held analyser state is invalidated even when its pane is hidden.

use super::*;

pub(super) fn apply(ui: &mut Ui, classes: C) {
    if classes.without(panes::TREND_RESETS_ON) != classes {
        ui.trend.clear();
    }
    let explicit = ui.host.runtime().change.prepared.as_ref().is_some_and(|p| p.reset_views);
    let mut eye_classes = if cursor_only(ui) { classes.without(C::MAPPING) } else { classes };
    let eye_resets = if ui.azr.eye_auto.enabled || ui.auto_eye_large() {
        eyepage::automatic::RESETS_ON
    } else if matches!(ui.m.eye_src, WaveId::Ref(_)) {
        if ui.host.runtime().change.prepared.as_ref().is_some_and(|p| !p.reference_view) {
            eye_classes = eye_classes.without(C::VIEW);
        }
        eyepage::REFERENCE_RESETS_ON
    } else {
        eyepage::EYE_RESETS_ON
    };
    let eye_reset = explicit || eye_classes.without(eye_resets) != eye_classes;
    let analysis_reset = classes.without(analysis::RESETS_ON) != classes;
    if eye_reset || analysis_reset {
        if explicit {
            ui.azr.work.reset();
            ui.azr.eye_auto.invalidate();
        } else {
            if eye_reset {
                ui.azr.work.invalidate_eye(ui.m.view == AView::Eye);
            }
            if analysis_reset {
                ui.azr.work.invalidate_analysis(ui.m.view != AView::Eye);
            }
        }
    }
    if eye_reset {
        ui.azr.eye = None;
        ui.azr.eye_ref = None;
        ui.azr.eye_sim_revision = None;
        ui.azr.eye_res = None;
        eyepage::reset::clear(&mut ui.azr.page);
    }
    if analysis_reset {
        reset_analysis(ui);
    }
}

fn reset_analysis(ui: &mut Ui) {
    ui.azr.spec = None;
    ui.azr.dist = None;
    ui.azr.peaks.clear();
    ui.azr.spec_avg = None;
    ui.azr.spec_stop = 0.0;
    if let Some(w) = &mut ui.azr.wf {
        w.clear();
    }
    ui.azr.power.clear();
    ui.azr.mag_note.clear();
    ui.azr.inv_note.clear();
    ui.azr.dpt_note.clear();
    ui.azr.dpt_v.clear();
    ui.azr.dpt_i.clear();
    ui.azr.dpt_g.clear();
    ui.azr.bw_info.clear();
    ui.azr.mag = None;
    ui.azr.dq = None;
    ui.azr.mech = None;
    ui.azr.dpt = None;
    ui.azr.dpt_loss = None;
    ui.azr.bw = None;
    ui.azr.bw_t = 0.0;
}

// Cursor controls change the scope layout but the accepted cursor contract retains temporal views.
fn cursor_only(ui: &Ui) -> bool {
    let Some(p) = &ui.host.runtime().change.prepared else { return false };
    let mut cursor = false;
    for f in p.fields.iter().flatten() {
        match f.name() {
            "cursor" | "cur_a" | "cur_b" | "cur_c" | "cur_d" => cursor = true,
            "status" | "hw_stamp" | "hw_avg_clear" => {}
            _ => return false,
        }
    }
    cursor
}

// Position fields classified as VIEW carry only driver presentation changes.
// Every other VIEW field or request keeps the reference Eye reset contract.
pub(super) fn reference_view(fields: &[Option<oos_core::change::FieldDiff>], requests: &[Request]) -> bool {
    requests.iter().any(|r| r.classes().contains(C::VIEW) && !matches!(r, Request::PlaybackPosition { .. }))
        || fields.iter().flatten().any(|f| f.classes().contains(C::VIEW) && !matches!(f.name(), "delay" | "zdelay"))
}
