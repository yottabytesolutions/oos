// SPDX-License-Identifier: GPL-2.0-or-later

//! Keep the step loop alive when bounded change state rejects an input.

use super::*;

pub(super) fn initialize(
    observer: &mut Observer,
    model: &Model,
    needs: UiAcq,
    config: &oos_core::config::Config,
) -> bool {
    if observer.shadow.is_some() {
        return true;
    }
    match Shadow::new(model, needs, config) {
        Ok(shadow) => {
            observer.shadow = Some(shadow);
            true
        }
        Err(error) => {
            reject(observer, error);
            false
        }
    }
}

pub(super) fn reject(observer: &mut Observer, error: oos_core::change::Error) {
    observer.diagnostics.error(error);
    observer.pending.clear();
    observer.stages.quarantine();
    observer.signals = Signals::default();
}
