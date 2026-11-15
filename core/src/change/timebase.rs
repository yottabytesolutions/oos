// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn timebase_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(2, "tscale", a.tscale != b.tscale, C::ACQ | C::STATS));
    emit(FieldDiff::new(3, "delay", a.delay != b.delay, C::ACQ | C::STATS));
    emit(FieldDiff::new(68, "fine_t", a.fine_t != b.fine_t, C::NONE));
    emit(FieldDiff::new(69, "tfine", a.tfine != b.tfine, C::ACQ | C::STATS));
    emit(FieldDiff::new(71, "h_expand", a.h_expand != b.h_expand, C::NONE));
    emit(FieldDiff::new(117, "zoom", a.zoom != b.zoom, C::NONE));
    emit(FieldDiff::new(119, "zscale", a.zscale != b.zscale, C::NONE));
    emit(FieldDiff::new(120, "zdelay", a.zdelay != b.zdelay, C::NONE));
    emit(FieldDiff::new(118, "auto_roll", a.auto_roll != b.auto_roll, C::ACQ | C::STATS));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.tscale.clone_from(&b.tscale);
    a.delay.clone_from(&b.delay);
    a.fine_t.clone_from(&b.fine_t);
    a.tfine.clone_from(&b.tfine);
    a.h_expand.clone_from(&b.h_expand);
    a.zoom.clone_from(&b.zoom);
    a.zscale.clone_from(&b.zscale);
    a.zdelay.clone_from(&b.zdelay);
    a.auto_roll.clone_from(&b.auto_roll);
}
