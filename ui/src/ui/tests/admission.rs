// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use oos_core::change::{Layout, Shadow, UiAcq};

#[test]
fn admission_geometry_and_format_identity_envelope() {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    let config = crate::model::config();
    for screen in [Screen::PANEL, Screen::of_size(1920, 1080).unwrap()] {
        for dock in StripDock::ALL {
            for layout in [WinLayout::Cols, WinLayout::Rows, WinLayout::Quad] {
                ui.m.dots = DotsMode::Off;
                let needs = |format| UiAcq::of(false, true, false, false, Layout::default()).with_read_format(format);
                let mut shadow = Shadow::new(&ui.m, needs(1), &config).unwrap();
                let id = shadow.identity();
                ui.scr = screen;
                ui.m.strip_dock = dock;
                ui.m.win_layout = layout;
                ui.m.ch_win = [0, 1, 2, 3];
                ui.relayout();
                assert!(ui.ph.w >= batch_min_columns());
                ui.m.dots = DotsMode::On;
                let geometry =
                    Layout::new((ui.ph.w, ui.ph.h), (0, 0), (screen.w as usize, screen.h as usize), None, 0, 0);
                let mapped = UiAcq::of(false, true, false, false, geometry).with_read_format(1);
                let next = shadow.observe(&ui.m, mapped, &[], &config).unwrap().identity();
                assert_eq!((id.gen(), id.epoch()), (next.gen(), next.epoch()));
                // The effective format is part of acquisition identity, even for a dots-only edit.
                let changed = shadow.observe(&ui.m, mapped.with_read_format(16), &[], &config).unwrap().identity();
                assert_ne!(changed.gen(), next.gen());
                ui.m.dots = DotsMode::Off;
            }
        }
    }
}

#[test]
fn admission_floor_covers_both_side_docks_and_eye_without_split() {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.m.win_layout = WinLayout::Quad;
    ui.m.strip_dock = StripDock::Left;
    ui.m.cursor = CursorMode::Time;
    ui.stats_on = false;
    ui.relayout();
    assert_eq!(ui.ph.w, 399);
    ui.m.bar_compact = true;
    ui.m.meas = Ui::default_meas();
    ui.stats_on = true;
    ui.workspace.measure_docked = true;
    ui.workspace.measure_float = None;
    ui.m.text_size = TextSize::Larger;
    ui.relayout();
    assert_eq!(ui.ph.w, batch_min_columns());
    ui.m.view = AView::Eye;
    assert_eq!(ui.n_wins(), 1);
}

mod wide;
