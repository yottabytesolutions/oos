// SPDX-License-Identifier: GPL-2.0-or-later

//! Sources, references, layout and the plot bus.

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn references(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    let bus = a.trig_type == crate::model::TrigType::Bus || b.trig_type == crate::model::TrigType::Bus;
    let decoder = if bus { C::ACQ | C::STATS } else { C::NONE };
    emit(FieldDiff::new(244, "deskew_ch", a.deskew_ch != b.deskew_ch, C::STATS | C::PHOSPHOR));
    emit(FieldDiff::new(245, "deskew_ref", a.deskew_ref != b.deskew_ref, C::STATS));
    emit(FieldDiff::new(246, "cur_b_source", a.cur_b_source != b.cur_b_source, C::NONE));
    emit(FieldDiff::new(247, "trig_ref", a.trig_ref != b.trig_ref, C::ACQ | C::STATS));
    emit(FieldDiff::new(248, "trig_ref2", a.trig_ref2 != b.trig_ref2, C::ACQ | C::STATS));
    emit(FieldDiff::new(249, "density", a.density != b.density, C::VIEW));
    emit(FieldDiff::new(250, "ref_win", a.ref_win != b.ref_win, C::VIEW));
    emit(FieldDiff::new(251, "cat", a.cat != b.cat, C::STATS));
    emit(FieldDiff::new(252, "split_pane", a.split_pane != b.split_pane, C::VIEW));
    emit(FieldDiff::new(253, "strip_dock", a.strip_dock != b.strip_dock, C::VIEW));
    emit(FieldDiff::new(254, "dec_cphy_rate", a.dec_cphy_rate != b.dec_cphy_rate, decoder));
    emit(FieldDiff::new(255, "bus", a.bus != b.bus, C::NONE));
    emit(FieldDiff::new(256, "mask_inv", a.mask_inv != b.mask_inv, C::STATS | C::MASK));
}

pub(super) fn refresh_references(a: &mut Model, b: &Model) {
    a.deskew_ch.clone_from(&b.deskew_ch);
    a.deskew_ref.clone_from(&b.deskew_ref);
    a.cur_b_source.clone_from(&b.cur_b_source);
    a.trig_ref.clone_from(&b.trig_ref);
    a.trig_ref2.clone_from(&b.trig_ref2);
    a.density.clone_from(&b.density);
    a.ref_win.clone_from(&b.ref_win);
    a.cat.clone_from(&b.cat);
    a.split_pane.clone_from(&b.split_pane);
    a.strip_dock.clone_from(&b.strip_dock);
    a.dec_cphy_rate.clone_from(&b.dec_cphy_rate);
    a.bus.clone_from(&b.bus);
    a.mask_inv.clone_from(&b.mask_inv);
}
