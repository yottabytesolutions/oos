// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "persist",
        |m| {
            m.persist += 1;
        },
        C::PHOSPHOR,
    ),
    (
        "intensity",
        |m| {
            m.intensity += 1;
        },
        C::NONE,
    ),
    (
        "grid",
        |m| {
            m.grid = if m.grid == Grid::Full { Grid::Light } else { Grid::Full };
        },
        C::NONE,
    ),
    (
        "grid_bright",
        |m| {
            m.grid_bright += 1;
        },
        C::NONE,
    ),
    (
        "grid_frame",
        |m| {
            m.grid_frame = !m.grid_frame;
        },
        C::NONE,
    ),
    (
        "win_layout",
        |m| {
            m.win_layout = if m.win_layout == WinLayout::One { WinLayout::Cols } else { WinLayout::One };
        },
        C::VIEW.union(C::MAPPING),
    ),
    (
        "ch_win",
        |m| {
            m.ch_win[0] += 1;
        },
        C::VIEW.union(C::MAPPING),
    ),
    (
        "math_win",
        |m| {
            m.math_win[0] += 1;
        },
        C::VIEW.union(C::MAPPING),
    ),
    (
        "la_win",
        |m| {
            m.la_win += 1;
        },
        C::VIEW.union(C::MAPPING),
    ),
    (
        "pane",
        |m| {
            m.pane += 1;
        },
        C::VIEW,
    ),
    (
        "touch_lock",
        |m| {
            m.touch_lock = !m.touch_lock;
        },
        C::NONE,
    ),
    (
        "color_grade",
        |m| {
            m.color_grade = !m.color_grade;
        },
        C::PHOSPHOR,
    ),
    (
        "dots",
        |m| {
            m.dots = if m.dots == DotsMode::Off { DotsMode::On } else { DotsMode::Off };
        },
        C::NONE.union(C::MAPPING),
    ),
    (
        "hdmi",
        |m| {
            m.hdmi = if m.hdmi == HdmiMode::Mirror { HdmiMode::Waveform } else { HdmiMode::Mirror };
        },
        C::NONE,
    ),
    (
        "views",
        |m| {
            m.views += 1;
        },
        C::NONE,
    ),
    (
        "view_cur",
        |m| {
            m.view_cur += 1;
        },
        C::NONE,
    ),
    (
        "view_slots",
        |m| {
            m.view_slots[0].push_str("fixture");
        },
        C::NONE,
    ),
    (
        "z_src",
        |m| {
            m.z_src = if m.z_src.is_some() { None } else { Some(1) };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "hist",
        |m| {
            m.hist = if m.hist == Hist::Off { Hist::Vertical } else { Hist::Off };
        },
        C::NONE,
    ),
    (
        "view",
        |m| {
            m.view = if m.view == AView::Scope { AView::Spectrum } else { AView::Scope };
        },
        C::VIEW,
    ),
    (
        "text_size",
        |m| {
            m.text_size = if m.text_size == TextSize::Normal { TextSize::Large } else { TextSize::Normal };
        },
        C::NONE,
    ),
];
