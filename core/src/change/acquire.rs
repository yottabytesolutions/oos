// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn acquire_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(32, "run", a.run != b.run, C::NONE));
    emit(FieldDiff::new(33, "status", a.status != b.status, C::NONE));
    emit(FieldDiff::new(34, "acq", a.acq != b.acq, C::ACQ | C::STATS | C::PHOSPHOR | C::HW_AVERAGE));
    emit(FieldDiff::new(35, "avg_count", a.avg_count != b.avg_count, C::ACQ | C::STATS | C::PHOSPHOR | C::HW_AVERAGE));
    emit(FieldDiff::new(36, "avg_hw", a.avg_hw != b.avg_hw, C::ACQ | C::STATS | C::PHOSPHOR | C::HW_AVERAGE));
    emit(FieldDiff::new(37, "segments", a.segments != b.segments, C::ACQ | C::STATS | C::HISTORY));
    emit(FieldDiff::new(38, "seg_hw", a.seg_hw != b.seg_hw, C::ACQ | C::STATS | C::HISTORY));
    emit(FieldDiff::new(39, "seg_ivl", a.seg_ivl != b.seg_ivl, C::ACQ | C::STATS | C::HISTORY));
    emit(FieldDiff::new(40, "mdepth", a.mdepth != b.mdepth, C::ACQ | C::STATS));
    emit(FieldDiff::new(162, "hw_stamp", a.hw_stamp != b.hw_stamp, C::NONE));
    emit(FieldDiff::new(163, "hw_avg_clear", a.hw_avg_clear != b.hw_avg_clear, C::NONE));
    emit(FieldDiff::new(193, "la", a.la != b.la, C::NONE));
    emit(FieldDiff::new(243, "cores", a.cores != b.cores, C::ACQ | C::STATS | C::PHOSPHOR));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.run.clone_from(&b.run);
    a.status.clone_from(&b.status);
    a.acq.clone_from(&b.acq);
    a.avg_count.clone_from(&b.avg_count);
    a.avg_hw.clone_from(&b.avg_hw);
    a.segments.clone_from(&b.segments);
    a.seg_hw.clone_from(&b.seg_hw);
    a.seg_ivl.clone_from(&b.seg_ivl);
    a.mdepth.clone_from(&b.mdepth);
    a.hw_stamp.clone_from(&b.hw_stamp);
    a.hw_avg_clear.clone_from(&b.hw_avg_clear);
    a.la.clone_from(&b.la);
    a.cores.clone_from(&b.cores);
}
