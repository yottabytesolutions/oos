// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis_1(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    let bus = a.trig_type == crate::model::TrigType::Bus || b.trig_type == crate::model::TrigType::Bus;
    let decoder = if bus { C::ACQ | C::STATS } else { C::NONE };
    emit(FieldDiff::new(100, "dec_uspeed", a.dec_uspeed != b.dec_uspeed, decoder));
    emit(FieldDiff::new(101, "dec_src4", a.dec_src4 != b.dec_src4, decoder));
    emit(FieldDiff::new(102, "dec_la_in", a.dec_la_in != b.dec_la_in, decoder));
    emit(FieldDiff::new(103, "dec_la", a.dec_la != b.dec_la, decoder));
    emit(FieldDiff::new(104, "dec_diff", a.dec_diff != b.dec_diff, decoder));
    emit(FieldDiff::new(108, "dec_thr", a.dec_thr != b.dec_thr, decoder));
    emit(FieldDiff::new(109, "dec_ascii", a.dec_ascii != b.dec_ascii, C::NONE));
    emit(FieldDiff::new(110, "dec_proto", a.dec_proto != b.dec_proto, decoder));
    emit(FieldDiff::new(111, "dec_src2", a.dec_src2 != b.dec_src2, decoder));
    emit(FieldDiff::new(112, "dec_cs", a.dec_cs != b.dec_cs, decoder));
    emit(FieldDiff::new(113, "dec_mode", a.dec_mode != b.dec_mode, decoder));
    emit(FieldDiff::new(114, "dec_bits", a.dec_bits != b.dec_bits, decoder));
    emit(FieldDiff::new(115, "dec_parity", a.dec_parity != b.dec_parity, decoder));
    emit(FieldDiff::new(116, "dec_opt", a.dec_opt != b.dec_opt, decoder));
    emit(FieldDiff::new(130, "spec_src", a.spec_src != b.spec_src, C::STATS | C::VIEW));
    emit(FieldDiff::new(131, "spec_center", a.spec_center != b.spec_center, C::STATS | C::VIEW));
    emit(FieldDiff::new(132, "spec_span", a.spec_span != b.spec_span, C::STATS | C::VIEW));
    emit(FieldDiff::new(133, "spec_rbw", a.spec_rbw != b.spec_rbw, C::STATS | C::VIEW));
    emit(FieldDiff::new(134, "spec_win", a.spec_win != b.spec_win, C::STATS | C::VIEW));
    emit(FieldDiff::new(135, "spec_sort", a.spec_sort != b.spec_sort, C::NONE));
    emit(FieldDiff::new(136, "spec_harm", a.spec_harm != b.spec_harm, C::NONE));
    emit(FieldDiff::new(137, "spec_trace", a.spec_trace != b.spec_trace, C::STATS | C::VIEW));
    emit(FieldDiff::new(138, "spec_ref", a.spec_ref != b.spec_ref, C::STATS | C::VIEW));
    emit(FieldDiff::new(139, "spec_wf", a.spec_wf != b.spec_wf, C::STATS | C::VIEW));
}

pub(super) fn refresh_1(a: &mut Model, b: &Model) {
    a.dec_uspeed.clone_from(&b.dec_uspeed);
    a.dec_src4.clone_from(&b.dec_src4);
    a.dec_la_in.clone_from(&b.dec_la_in);
    a.dec_la.clone_from(&b.dec_la);
    a.dec_diff.clone_from(&b.dec_diff);
    a.dec_thr.clone_from(&b.dec_thr);
    a.dec_ascii.clone_from(&b.dec_ascii);
    a.dec_proto.clone_from(&b.dec_proto);
    a.dec_src2.clone_from(&b.dec_src2);
    a.dec_cs.clone_from(&b.dec_cs);
    a.dec_mode.clone_from(&b.dec_mode);
    a.dec_bits.clone_from(&b.dec_bits);
    a.dec_parity.clone_from(&b.dec_parity);
    a.dec_opt.clone_from(&b.dec_opt);
    a.spec_src.clone_from(&b.spec_src);
    a.spec_center.clone_from(&b.spec_center);
    a.spec_span.clone_from(&b.spec_span);
    a.spec_rbw.clone_from(&b.spec_rbw);
    a.spec_win.clone_from(&b.spec_win);
    a.spec_sort.clone_from(&b.spec_sort);
    a.spec_harm.clone_from(&b.spec_harm);
    a.spec_trace.clone_from(&b.spec_trace);
    a.spec_ref.clone_from(&b.spec_ref);
    a.spec_wf.clone_from(&b.spec_wf);
}
