// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::sync::Arc;
pub(crate) fn render_owner_active(ui: &Ui) -> bool {
    PERSIST[ui.m.persist].1 == 0.0 && ui.m.run == RunState::Run && !ui.record_mode() && ui.m.draw_batch && ui.fast_ok()
}
pub(crate) fn set_render_owner_wake(ui: &mut Ui, wake: Arc<crate::system::intake::Wake>) {
    ui.batch_picture.owner.set_wake(wake);
}
pub(crate) fn intake_render_owner(ui: &mut Ui) {
    let active = render_owner_active(ui);
    if let Some(pipe) = drawpipe::pipe(Access::Existing) {
        ui.batch_picture.owner.intake_owned(pipe, active);
    }
}
#[cfg(test)]
impl Ui {
    #[cfg(test)]
    pub(crate) fn render_owner_debug(&self) -> String {
        format!(
            "now={} active={} run={:?} persist={} fast_ok={} hw={} {}",
            self.now,
            render_owner_active(self),
            self.m.run,
            self.m.persist,
            self.fast_ok(),
            self.src.is_hw(),
            self.batch_picture.owner.debug_state()
        )
    }
    #[cfg(test)]
    pub(crate) fn render_owner_test_wake(&self) -> Arc<crate::system::intake::Wake> {
        self.batch_picture.owner.test_wake().expect("fixture installed owned Wake")
    }
}
