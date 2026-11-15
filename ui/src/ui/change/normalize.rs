// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// The only acting M2.3a switch. Never copy shadow outputs into the live model.
pub(super) fn apply(ui: &mut Ui) {
    let sample = ui.host.runtime().change.census.begin();
    let config = crate::model::config();
    let observer = &mut ui.host.runtime_mut().change;
    let result = observer.shadow.as_mut().map(|shadow| shadow.normalize_live(&mut ui.m, &config));
    let (changed, classes) = match result {
        Some(Ok(repairs)) => repairs,
        other => {
            if let Some(Err(error)) = other {
                observer.diagnostics.error(error);
            }
            (oos_core::change::normalize(&mut ui.m, &config), C::NONE)
        }
    };
    observer.signals.normalized |= classes;
    if changed {
        ui.mark_changed();
        event(ui, Row::Normalize, C::NONE);
    }
    sample.finish(&mut ui.host.runtime_mut().change.census, 4);
}

pub(in crate::ui) fn default_setup(ui: &mut Ui) {
    dirty(ui, Row::Recall);
    stats_reset(ui);
}
