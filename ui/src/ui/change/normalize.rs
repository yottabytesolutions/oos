// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Normalize live settings before classification, with no reset notification bridge.
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
        observer.signals.row = Row::Normalize;
    }
    sample.finish(&mut ui.host.runtime_mut().change.census, 4);
}

pub(in crate::ui) fn default_setup(ui: &mut Ui) {
    mapping_observed(ui, Row::Recall);
    stats_reset(ui);
}
