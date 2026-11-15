// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    let bus = a.trig_type == crate::model::TrigType::Bus || b.trig_type == crate::model::TrigType::Bus;
    let decoder = if bus { C::ACQ | C::STATS } else { C::NONE };
    emit(FieldDiff::new(72, "math", a.math != b.math, C::MATH | C::STATS));
    emit(FieldDiff::new(73, "math_a", a.math_a != b.math_a, C::MATH | C::STATS));
    emit(FieldDiff::new(74, "math_b", a.math_b != b.math_b, C::MATH | C::STATS));
    emit(FieldDiff::new(75, "math_scale", a.math_scale != b.math_scale, C::MATH | C::STATS));
    emit(FieldDiff::new(76, "math_fc", a.math_fc != b.math_fc, C::MATH | C::STATS));
    emit(FieldDiff::new(77, "math_lin", a.math_lin != b.math_lin, C::MATH | C::STATS));
    emit(FieldDiff::new(78, "math_expr", a.math_expr != b.math_expr, C::MATH | C::STATS));
    emit(FieldDiff::new(80, "spec_margin", a.spec_margin != b.spec_margin, C::STATS | C::VIEW));
    emit(FieldDiff::new(81, "math_offset", a.math_offset != b.math_offset, C::MATH | C::STATS));
    emit(FieldDiff::new(82, "fft_win", a.fft_win != b.fft_win, C::MATH | C::STATS));
    emit(FieldDiff::new(83, "math_bw", a.math_bw != b.math_bw, C::MATH | C::STATS));
    emit(FieldDiff::new(84, "math_sel", a.math_sel != b.math_sel, C::NONE));
    emit(FieldDiff::new(85, "maths", a.maths != b.maths, C::MATH | C::STATS));
    // Stored decoder slots are keyed decode configurations. Active bus-trigger fields are diffed separately.
    emit(FieldDiff::new(86, "decs", a.decs != b.decs, C::NONE));
    emit(FieldDiff::new(87, "dec_sel", a.dec_sel != b.dec_sel, C::NONE));
    emit(FieldDiff::new(91, "dec_on", a.dec_on != b.dec_on, decoder));
    emit(FieldDiff::new(92, "dec_src", a.dec_src != b.dec_src, decoder));
    emit(FieldDiff::new(93, "dec_baud", a.dec_baud != b.dec_baud, decoder));
    emit(FieldDiff::new(94, "dec_canrate", a.dec_canrate != b.dec_canrate, decoder));
    emit(FieldDiff::new(95, "dec_canh", a.dec_canh != b.dec_canh, decoder));
    emit(FieldDiff::new(96, "dec_fdrate", a.dec_fdrate != b.dec_fdrate, decoder));
    emit(FieldDiff::new(97, "dec_arate", a.dec_arate != b.dec_arate, decoder));
    emit(FieldDiff::new(98, "dec_tick", a.dec_tick != b.dec_tick, decoder));
    emit(FieldDiff::new(99, "dec_nibbles", a.dec_nibbles != b.dec_nibbles, decoder));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.math.clone_from(&b.math);
    a.math_a.clone_from(&b.math_a);
    a.math_b.clone_from(&b.math_b);
    a.math_scale.clone_from(&b.math_scale);
    a.math_fc.clone_from(&b.math_fc);
    a.math_lin.clone_from(&b.math_lin);
    for (a, b) in a.math_expr.iter_mut().zip(&b.math_expr) {
        a.clone_from(b);
    }
    a.spec_margin.clone_from(&b.spec_margin);
    a.math_offset.clone_from(&b.math_offset);
    a.fft_win.clone_from(&b.fft_win);
    a.math_bw.clone_from(&b.math_bw);
    a.math_sel.clone_from(&b.math_sel);
    a.maths.clone_from(&b.maths);
    a.decs.clone_from(&b.decs);
    a.dec_sel.clone_from(&b.dec_sel);
    a.dec_on.clone_from(&b.dec_on);
    a.dec_src.clone_from(&b.dec_src);
    a.dec_baud.clone_from(&b.dec_baud);
    a.dec_canrate.clone_from(&b.dec_canrate);
    a.dec_canh.clone_from(&b.dec_canh);
    a.dec_fdrate.clone_from(&b.dec_fdrate);
    a.dec_arate.clone_from(&b.dec_arate);
    a.dec_tick.clone_from(&b.dec_tick);
    a.dec_nibbles.clone_from(&b.dec_nibbles);
}
