// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis_2(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(140, "spec_dist", a.spec_dist != b.spec_dist, C::MATH | C::VIEW));
    emit(FieldDiff::new(141, "eye_src", a.eye_src != b.eye_src, C::VIEW));
    emit(FieldDiff::new(142, "eye_rate", a.eye_rate != b.eye_rate, C::VIEW));
    emit(FieldDiff::new(143, "pw_v", a.pw_v != b.pw_v, C::VIEW));
    emit(FieldDiff::new(144, "pw_i", a.pw_i != b.pw_i, C::VIEW));
    emit(FieldDiff::new(145, "pw_mode", a.pw_mode != b.pw_mode, C::VIEW));
    emit(FieldDiff::new(146, "bode_start", a.bode_start != b.bode_start, C::STATS | C::VIEW));
    emit(FieldDiff::new(147, "bode_stop", a.bode_stop != b.bode_stop, C::STATS | C::VIEW));
    emit(FieldDiff::new(148, "bode_points", a.bode_points != b.bode_points, C::STATS | C::VIEW));
    emit(FieldDiff::new(149, "bode_amp", a.bode_amp != b.bode_amp, C::STATS | C::VIEW));
    emit(FieldDiff::new(150, "bode_in", a.bode_in != b.bode_in, C::STATS | C::VIEW));
    emit(FieldDiff::new(151, "bode_out", a.bode_out != b.bode_out, C::STATS | C::VIEW));
    emit(FieldDiff::new(152, "bode_gen", a.bode_gen != b.bode_gen, C::STATS | C::VIEW));
    emit(FieldDiff::new(153, "bode_addr", a.bode_addr != b.bode_addr, C::STATS | C::VIEW));
    emit(FieldDiff::new(154, "bode_usb", a.bode_usb != b.bode_usb, C::STATS | C::VIEW));
    emit(FieldDiff::new(155, "bode_sw_time", a.bode_sw_time != b.bode_sw_time, C::STATS | C::VIEW));
    emit(FieldDiff::new(156, "bode_sw_log", a.bode_sw_log != b.bode_sw_log, C::STATS | C::VIEW));
    emit(FieldDiff::new(157, "bode_sync", a.bode_sync != b.bode_sync, C::STATS | C::VIEW));
    emit(FieldDiff::new(158, "bode_sync_slope", a.bode_sync_slope != b.bode_sync_slope, C::STATS | C::VIEW));
    emit(FieldDiff::new(159, "bode_sync_level", a.bode_sync_level != b.bode_sync_level, C::STATS | C::VIEW));
    emit(FieldDiff::new(160, "bode_sw_in", a.bode_sw_in != b.bode_sw_in, C::STATS | C::VIEW));
    emit(FieldDiff::new(161, "bode_ref", a.bode_ref != b.bode_ref, C::STATS | C::VIEW));
    emit(FieldDiff::new(165, "bw_src", a.bw_src != b.bw_src, C::STATS | C::VIEW));
    emit(FieldDiff::new(166, "bw_hint", a.bw_hint != b.bw_hint, C::STATS | C::VIEW));
}

pub(super) fn refresh_2(a: &mut Model, b: &Model) {
    a.spec_dist.clone_from(&b.spec_dist);
    a.eye_src.clone_from(&b.eye_src);
    a.eye_rate.clone_from(&b.eye_rate);
    a.pw_v.clone_from(&b.pw_v);
    a.pw_i.clone_from(&b.pw_i);
    a.pw_mode.clone_from(&b.pw_mode);
    a.bode_start.clone_from(&b.bode_start);
    a.bode_stop.clone_from(&b.bode_stop);
    a.bode_points.clone_from(&b.bode_points);
    a.bode_amp.clone_from(&b.bode_amp);
    a.bode_in.clone_from(&b.bode_in);
    a.bode_out.clone_from(&b.bode_out);
    a.bode_gen.clone_from(&b.bode_gen);
    a.bode_addr.clone_from(&b.bode_addr);
    a.bode_usb.clone_from(&b.bode_usb);
    a.bode_sw_time.clone_from(&b.bode_sw_time);
    a.bode_sw_log.clone_from(&b.bode_sw_log);
    a.bode_sync.clone_from(&b.bode_sync);
    a.bode_sync_slope.clone_from(&b.bode_sync_slope);
    a.bode_sync_level.clone_from(&b.bode_sync_level);
    a.bode_sw_in.clone_from(&b.bode_sw_in);
    a.bode_ref.clone_from(&b.bode_ref);
    a.bw_src.clone_from(&b.bw_src);
    a.bw_hint.clone_from(&b.bw_hint);
}
