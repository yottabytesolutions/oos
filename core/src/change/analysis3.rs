// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis_3(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(167, "bw_bins", a.bw_bins != b.bw_bins, C::STATS | C::VIEW));
    emit(FieldDiff::new(185, "mask_on", a.mask_on != b.mask_on, C::STATS | C::MASK));
    emit(FieldDiff::new(186, "zones", a.zones != b.zones, C::ACQ | C::STATS | C::ZONE));
    emit(FieldDiff::new(187, "mask_src", a.mask_src != b.mask_src, C::ACQ | C::STATS | C::MASK));
    emit(FieldDiff::new(188, "mask_x", a.mask_x != b.mask_x, C::MASK));
    emit(FieldDiff::new(189, "mask_y", a.mask_y != b.mask_y, C::MASK));
    emit(FieldDiff::new(190, "mask_stop", a.mask_stop != b.mask_stop, C::NONE));
}

pub(super) fn refresh_3(a: &mut Model, b: &Model) {
    a.bw_bins.clone_from(&b.bw_bins);
    a.mask_on.clone_from(&b.mask_on);
    for (a, b) in a.zones.iter_mut().zip(&b.zones) {
        a.on = b.on;
        a.src = b.src;
        a.inside = b.inside;
        a.pts.clone_from(&b.pts);
    }
    a.mask_src.clone_from(&b.mask_src);
    a.mask_x.clone_from(&b.mask_x);
    a.mask_y.clone_from(&b.mask_y);
    a.mask_stop.clone_from(&b.mask_stop);
}
