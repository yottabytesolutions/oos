// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Observe the existing analyzer-result/temporal reset. Generic measurement statistics stay legacy.
pub(in crate::ui) fn analyzer_reset(ui: &mut Ui) {
    // Keep the intake source snap before job submission until M2.3e removes the broad legacy bridge.
    if let Some(on) = (0..NCH).find(|&c| ui.m.ch[c].on) {
        for src in [&mut ui.m.spec_src, &mut ui.m.eye_src] {
            if !ui.m.ch[*src].on {
                *src = on;
            }
        }
    }
    ui.azr.spec_avg = None;
    if let Some(w) = &mut ui.azr.wf {
        w.clear();
    }
    event(ui, Row::Analyzer, C::MATH | C::VIEW);
}
