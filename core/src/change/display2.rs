// SPDX-License-Identifier: GPL-2.0-or-later

//! Draw path, bottom bar and the view of an HDMI monitor.

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn display2(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(342, "draw_batch", a.draw_batch != b.draw_batch, C::PHOSPHOR));
    emit(FieldDiff::new(343, "bar_compact", a.bar_compact != b.bar_compact, C::NONE));
    emit(FieldDiff::new(344, "hdmi_view", a.hdmi_view != b.hdmi_view, C::NONE));
}

pub(super) fn refresh_display2(a: &mut Model, b: &Model) {
    a.draw_batch.clone_from(&b.draw_batch);
    a.bar_compact.clone_from(&b.bar_compact);
    a.hdmi_view.clone_from(&b.hdmi_view);
}
