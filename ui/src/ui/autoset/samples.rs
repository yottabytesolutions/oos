// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(in crate::ui) fn search(ui: &Ui, c: usize, dt: f64) -> (std::borrow::Cow<'_, [f32]>, f64) {
    if let Some(raw) = ui.last[0].as_ref().and_then(|r| r.raw.as_ref()) {
        if raw.frame.ch[c].len() > 1 && !raw.frame.envelope {
            return (std::borrow::Cow::Borrowed(&raw.frame.ch[c]), raw.frame.dt);
        }
    }
    (std::borrow::Cow::Owned(ui.record_volts(c)), dt)
}
