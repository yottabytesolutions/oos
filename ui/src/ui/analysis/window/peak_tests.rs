// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use oos_dsp::spectrum::{Marker, Spectrum};

fn fixture() -> Ui {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.step(0.04);
    ui.m.view = AView::Spectrum;
    ui.now = 0.0;
    ui
}

fn result(ui: &Ui, raw: &[Marker]) -> Done {
    let job = prepare(ui, AView::Spectrum, false).unwrap();
    let mut done = run(job, &mut None);
    done.state.spec = Some(Spectrum { df: 1.0, rbw: 4.0, ..Default::default() });
    done.state.peaks = raw.to_vec();
    done
}

#[test]
fn accepted_worker_peaks_use_ui_clock_and_retain_main_owned_tracking() {
    let mut ui = fixture();
    let raw = [Marker { f: 100.0, dbv: -10.0 }];
    let first = result(&ui, &raw);
    assert!(accept(&mut ui, first));
    assert_eq!(ui.azr.peaks, raw, "raw lines are immediately available");
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
    let next = result(&ui, &raw); // job submitted at t=0; accepted at t=1
    ui.now = 1.0;
    assert!(accept(&mut ui, next));
    assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), [(1, raw[0], false)]);
    ui.now = 1.1;
    let empty = result(&ui, &[]);
    assert!(accept(&mut ui, empty));
    assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), [(1, raw[0], true)]);
    let stale = result(&ui, &[]);
    ui.m.spec_src = WaveId::Ch(1);
    ui.now = 2.1;
    assert!(!accept(&mut ui, stale));
    assert_eq!(ui.azr.work.peak_table.rows().count(), 1, "stale jobs cannot advance holds");
    let changed = result(&ui, &raw);
    assert!(accept(&mut ui, changed));
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0, "new sources must qualify again");
}

#[test]
fn synchronous_and_main_spectrum_paths_qualify_and_reset_on_clear() {
    let mut ui = fixture();
    let job = prepare(&ui, AView::Spectrum, false).unwrap();
    synchronous(&mut ui, job);
    assert!(!ui.azr.peaks.is_empty());
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
    ui.now = 1.0;
    let job = prepare(&ui, AView::Spectrum, false).unwrap();
    synchronous(&mut ui, job);
    assert!(ui.azr.work.peak_table.rows().count() > 0);
    ui.azr.work.reset();
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
    let rec = ui.last[0].as_ref().unwrap().clone();
    ui.spectrum_update(&rec, false);
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
    ui.now += 1.0;
    ui.spectrum_update(&rec, false);
    assert!(ui.azr.work.peak_table.rows().count() > 0);
}

#[test]
fn stopped_spectrum_appearance_and_release_timers_advance_without_new_jobs() {
    let mut ui = fixture();
    let raw = [Marker { f: 100.0, dbv: -10.0 }];
    let done = result(&ui, &raw);
    assert!(accept(&mut ui, done));
    ui.now = 0.999;
    poll(&mut ui);
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
    ui.now = 1.0;
    poll(&mut ui);
    assert_eq!(ui.azr.work.peak_table.rows().count(), 1);
    let done = result(&ui, &[]);
    assert!(accept(&mut ui, done));
    ui.now = 1.999;
    poll(&mut ui);
    assert_eq!(ui.azr.work.peak_table.rows().count(), 1);
    ui.now = 2.0;
    poll(&mut ui);
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
}

#[test]
fn setup_recall_resorts_existing_peaks_without_requalifying_or_renumbering() {
    let mut ui = fixture();
    let dir = std::env::temp_dir().join(format!("oos-peak-recall-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    ui.store_dir = dir.clone();
    ui.m.spec_sort = 1;
    let raw = [Marker { f: 200.0, dbv: -10.0 }, Marker { f: 100.0, dbv: -10.5 }];
    let done = result(&ui, &raw);
    assert!(accept(&mut ui, done));
    ui.now = 1.0;
    poll(&mut ui);
    assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), [(1, raw[0], false), (2, raw[1], false)]);
    for (sort, expected) in
        [(0, [(2, raw[1], false), (1, raw[0], false)]), (1, [(1, raw[0], false), (2, raw[1], false)])]
    {
        let mut saved = ui.m.clone();
        saved.spec_sort = sort;
        std::fs::write(dir.join("setup1.txt"), crate::store::setup_text(&saved)).unwrap();
        ui.recall_setup();
        assert_eq!(ui.m.spec_sort, sort);
        poll(&mut ui);
        assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), expected);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn centre_on_current_peak_keeps_tracks_until_the_frequency_grid_changes() {
    let mut ui = fixture();
    let raw = [Marker { f: 100.0, dbv: -10.0 }];
    ui.m.spec_center = raw[0].f;
    ui.m.spec_span = 100.0;
    let done = result(&ui, &raw);
    assert!(accept(&mut ui, done));
    ui.now = 1.0;
    poll(&mut ui);
    let expected = [(1, raw[0], false)];
    assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), expected);
    assert!(ui.ana_activate(Item::SpecPeak));
    assert_eq!(ui.azr.work.peak_table.rows().collect::<Vec<_>>(), expected);
    ui.azr.spec.as_mut().unwrap().f0 = 20.0;
    ui.track_peaks();
    assert_eq!(ui.azr.work.peak_table.rows().count(), 0);
}
