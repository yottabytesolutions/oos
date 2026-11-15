// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::la::LaSet;
use crate::model::Model;

pub(super) fn fields(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    let LaSet { on, show, preset, user, height } = &a.la;
    emit(FieldDiff::new(238, "la.on", on != &b.la.on, C::ACQ | C::STATS));
    emit(FieldDiff::new(239, "la.show", show != &b.la.show, C::VIEW));
    emit(FieldDiff::new(240, "la.preset", preset != &b.la.preset, C::ACQ | C::STATS));
    emit(FieldDiff::new(241, "la.user", user != &b.la.user, C::ACQ | C::STATS));
    emit(FieldDiff::new(242, "la.height", height != &b.la.height, C::VIEW));
}
