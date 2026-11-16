// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

struct Intake<'a>(&'a mut Ui);
impl Drop for Intake<'_> {
    fn drop(&mut self) {
        self.0.host.runtime_mut().change.timing.intake_end();
    }
}

pub(crate) fn intake<T>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> T) -> T {
    ui.host.runtime_mut().change.timing.intake_begin();
    super::proposals::apply(ui);
    let guard = Intake(ui);
    f(guard.0)
}
