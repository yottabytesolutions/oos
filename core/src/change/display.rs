// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn display_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(41, "persist", a.persist != b.persist, C::PHOSPHOR));
    emit(FieldDiff::new(42, "intensity", a.intensity != b.intensity, C::NONE));
    emit(FieldDiff::new(43, "grid", a.grid != b.grid, C::NONE));
    emit(FieldDiff::new(44, "grid_bright", a.grid_bright != b.grid_bright, C::NONE));
    emit(FieldDiff::new(45, "grid_frame", a.grid_frame != b.grid_frame, C::NONE));
    emit(FieldDiff::new(46, "win_layout", a.win_layout != b.win_layout, C::VIEW));
    emit(FieldDiff::new(47, "ch_win", a.ch_win != b.ch_win, C::VIEW));
    emit(FieldDiff::new(48, "math_win", a.math_win != b.math_win, C::VIEW));
    emit(FieldDiff::new(49, "la_win", a.la_win != b.la_win, C::VIEW));
    emit(FieldDiff::new(88, "pane", a.pane != b.pane, C::VIEW));
    emit(FieldDiff::new(65, "touch_lock", a.touch_lock != b.touch_lock, C::NONE));
    emit(FieldDiff::new(121, "color_grade", a.color_grade != b.color_grade, C::PHOSPHOR));
    emit(FieldDiff::new(122, "dots", a.dots != b.dots, C::NONE));
    emit(FieldDiff::new(123, "hdmi", a.hdmi != b.hdmi, C::NONE));
    emit(FieldDiff::new(124, "views", a.views != b.views, C::NONE));
    emit(FieldDiff::new(125, "view_cur", a.view_cur != b.view_cur, C::NONE));
    emit(FieldDiff::new(126, "view_slots", a.view_slots != b.view_slots, C::NONE));
    emit(FieldDiff::new(127, "z_src", a.z_src != b.z_src, C::ACQ | C::STATS));
    emit(FieldDiff::new(128, "hist", a.hist != b.hist, C::NONE));
    emit(FieldDiff::new(129, "view", a.view != b.view, C::VIEW));
    emit(FieldDiff::new(192, "text_size", a.text_size != b.text_size, C::NONE));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.persist.clone_from(&b.persist);
    a.intensity.clone_from(&b.intensity);
    a.grid.clone_from(&b.grid);
    a.grid_bright.clone_from(&b.grid_bright);
    a.grid_frame.clone_from(&b.grid_frame);
    a.win_layout.clone_from(&b.win_layout);
    a.ch_win.clone_from(&b.ch_win);
    a.math_win.clone_from(&b.math_win);
    a.la_win.clone_from(&b.la_win);
    a.pane.clone_from(&b.pane);
    a.touch_lock.clone_from(&b.touch_lock);
    a.color_grade.clone_from(&b.color_grade);
    a.dots.clone_from(&b.dots);
    a.hdmi.clone_from(&b.hdmi);
    a.views.clone_from(&b.views);
    a.view_cur.clone_from(&b.view_cur);
    for (a, b) in a.view_slots.iter_mut().zip(&b.view_slots) {
        a.clone_from(b);
    }
    a.z_src.clone_from(&b.z_src);
    a.hist.clone_from(&b.hist);
    a.view.clone_from(&b.view);
    a.text_size.clone_from(&b.text_size);
}
