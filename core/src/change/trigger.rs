// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn trigger_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(4, "trig_src", a.trig_src != b.trig_src, C::ACQ | C::STATS));
    emit(FieldDiff::new(5, "trig_la", a.trig_la != b.trig_la, C::ACQ | C::STATS));
    emit(FieldDiff::new(6, "pat_la", a.pat_la != b.pat_la, C::ACQ | C::STATS));
    emit(FieldDiff::new(7, "trig_level", a.trig_level != b.trig_level, C::ACQ | C::STATS));
    emit(FieldDiff::new(8, "slope", a.slope != b.slope, C::ACQ | C::STATS));
    emit(FieldDiff::new(
        9,
        "trig_mode",
        a.trig_mode != b.trig_mode,
        if a.run == b.run && b.trig_mode != crate::model::TrigMode::Single { C::ACQ | C::STATS } else { C::NONE },
    ));
    emit(FieldDiff::new(10, "holdoff", a.holdoff != b.holdoff, C::ACQ | C::STATS));
    emit(FieldDiff::new(11, "trig_type", a.trig_type != b.trig_type, C::ACQ | C::STATS));
    emit(FieldDiff::new(12, "trig_level2", a.trig_level2 != b.trig_level2, C::ACQ | C::STATS));
    emit(FieldDiff::new(13, "trig_nrej", a.trig_nrej != b.trig_nrej, C::ACQ | C::STATS));
    emit(FieldDiff::new(14, "auto_nrej", a.auto_nrej != b.auto_nrej, C::NONE));
    emit(FieldDiff::new(15, "trig_coup", a.trig_coup != b.trig_coup, C::ACQ | C::STATS));
    emit(FieldDiff::new(16, "trig_src2", a.trig_src2 != b.trig_src2, C::ACQ | C::STATS));
    emit(FieldDiff::new(17, "trig_slope2", a.trig_slope2 != b.trig_slope2, C::ACQ | C::STATS));
    emit(FieldDiff::new(18, "trig_pol", a.trig_pol != b.trig_pol, C::ACQ | C::STATS));
    emit(FieldDiff::new(19, "trig_when", a.trig_when != b.trig_when, C::ACQ | C::STATS));
    emit(FieldDiff::new(20, "trig_t1", a.trig_t1 != b.trig_t1, C::ACQ | C::STATS));
    emit(FieldDiff::new(21, "trig_t2", a.trig_t2 != b.trig_t2, C::ACQ | C::STATS));
    emit(FieldDiff::new(22, "video_std", a.video_std != b.video_std, C::ACQ | C::STATS));
    emit(FieldDiff::new(23, "video_sync", a.video_sync != b.video_sync, C::ACQ | C::STATS));
    emit(FieldDiff::new(24, "video_line", a.video_line != b.video_line, C::ACQ | C::STATS));
    emit(FieldDiff::new(25, "pat", a.pat != b.pat, C::ACQ | C::STATS));
    emit(FieldDiff::new(26, "pat_levels", a.pat_levels != b.pat_levels, C::ACQ | C::STATS));
    emit(FieldDiff::new(27, "win_mode", a.win_mode != b.win_mode, C::ACQ | C::STATS));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.trig_src.clone_from(&b.trig_src);
    a.trig_la.clone_from(&b.trig_la);
    a.pat_la.clone_from(&b.pat_la);
    a.trig_level.clone_from(&b.trig_level);
    a.slope.clone_from(&b.slope);
    a.trig_mode.clone_from(&b.trig_mode);
    a.holdoff.clone_from(&b.holdoff);
    a.trig_type.clone_from(&b.trig_type);
    a.trig_level2.clone_from(&b.trig_level2);
    a.trig_nrej.clone_from(&b.trig_nrej);
    a.auto_nrej.clone_from(&b.auto_nrej);
    a.trig_coup.clone_from(&b.trig_coup);
    a.trig_src2.clone_from(&b.trig_src2);
    a.trig_slope2.clone_from(&b.trig_slope2);
    a.trig_pol.clone_from(&b.trig_pol);
    a.trig_when.clone_from(&b.trig_when);
    a.trig_t1.clone_from(&b.trig_t1);
    a.trig_t2.clone_from(&b.trig_t2);
    a.video_std.clone_from(&b.video_std);
    a.video_sync.clone_from(&b.video_sync);
    a.video_line.clone_from(&b.video_line);
    a.pat.clone_from(&b.pat);
    a.pat_levels.clone_from(&b.pat_levels);
    a.win_mode.clone_from(&b.win_mode);
}

pub(super) fn trigger_1(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(28, "nth_idle", a.nth_idle != b.nth_idle, C::ACQ | C::STATS));
    emit(FieldDiff::new(29, "nth_n", a.nth_n != b.nth_n, C::ACQ | C::STATS));
    emit(FieldDiff::new(30, "setup", a.setup != b.setup, C::ACQ | C::STATS));
    emit(FieldDiff::new(31, "hold", a.hold != b.hold, C::ACQ | C::STATS));
    emit(FieldDiff::new(105, "trig_bus", a.trig_bus != b.trig_bus, C::ACQ | C::STATS));
    emit(FieldDiff::new(106, "trig_bus_val", a.trig_bus_val != b.trig_bus_val, C::ACQ | C::STATS));
    emit(FieldDiff::new(107, "ser", a.ser != b.ser, C::ACQ | C::STATS));
    emit(FieldDiff::new(164, "auto_fast", a.auto_fast != b.auto_fast, C::ACQ | C::STATS));
}

pub(super) fn refresh_1(a: &mut Model, b: &Model) {
    a.nth_idle.clone_from(&b.nth_idle);
    a.nth_n.clone_from(&b.nth_n);
    a.setup.clone_from(&b.setup);
    a.hold.clone_from(&b.hold);
    a.trig_bus.clone_from(&b.trig_bus);
    a.trig_bus_val.clone_from(&b.trig_bus_val);
    a.ser.clone_from(&b.ser);
    a.auto_fast.clone_from(&b.auto_fast);
}
