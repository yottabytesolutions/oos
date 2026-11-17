// SPDX-License-Identifier: GPL-2.0-or-later

//! Display edits after M2.3e: what an accepted change keeps and where a rebuilt page starts.
use super::super::*;

fn demo() -> Ui {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.step(1.0 / 30.0);
    ui
}

fn count(ui: &Ui) -> u64 {
    ui.measurement_statistics_snapshot_for_test()[0].0
}

// A display layout change was a broad settings mark before M2.3e and reset the statistics as a side effect. It has
// no STATS class: the statistics and the trend keep counting across a Windows/pane change.
#[test]
fn a_pane_layout_change_keeps_the_statistics_count() {
    let mut ui = demo();
    ui.meas_add(Meas::Vpp, WaveId::Ch(0)).expect("a free row");
    ui.set_stats_shown(true);
    ui.select_option(Item::WinLayout, "Split");
    ui.select_option(Item::PaneSel, "Spectrum");
    for _ in 0..5 {
        ui.step(1.0 / 30.0);
    }
    let before = count(&ui);
    assert!(before >= 5, "{before}");
    ui.select_option(Item::PaneSel, "Results");
    for _ in 0..3 {
        ui.step(1.0 / 30.0);
    }
    assert_eq!(count(&ui), before + 3, "the pane change resets nothing");
}
