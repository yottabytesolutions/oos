// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn marker(f: f64, dbv: f64) -> Marker {
    Marker { f, dbv }
}

fn feed(t: &mut Table, raw: &[Marker], now: f64, by_level: bool) {
    t.refresh(
        raw,
        now,
        &spectrum::Spectrum { df: 1.0, rbw: 4.0, ..Default::default() },
        (WaveId::Ch(0), Window::Hann, 0),
        by_level,
    );
}

fn rows(t: &Table) -> Vec<(usize, Marker, bool)> {
    t.rows().collect()
}

#[test]
fn short_peaks_never_appear_and_absence_restarts_qualification() {
    let mut t = Table::default();
    let raw = [marker(100.0, -20.0)];
    feed(&mut t, &raw, 0.0, false);
    feed(&mut t, &raw, 0.999, false);
    assert!(rows(&t).is_empty());
    feed(&mut t, &[], 1.0, false);
    feed(&mut t, &raw, 1.1, false);
    feed(&mut t, &raw, 2.0, false);
    assert!(rows(&t).is_empty());
    feed(&mut t, &raw, 2.1, false);
    assert_eq!(rows(&t), vec![(1, raw[0], false)]);
}

#[test]
fn release_holds_last_value_and_number_until_one_second_absent() {
    let mut t = Table::default();
    let raw = [marker(100.0, -20.0), marker(200.0, -30.0)];
    feed(&mut t, &raw, 0.0, false);
    feed(&mut t, &raw, 1.0, false);
    feed(&mut t, &raw[1..], 1.1, false);
    feed(&mut t, &raw[1..], 2.099, false);
    assert_eq!(rows(&t), vec![(1, raw[0], true), (2, raw[1], false)]);
    feed(&mut t, &raw, 2.0995, false);
    assert_eq!(rows(&t), vec![(1, raw[0], false), (2, raw[1], false)]);
    feed(&mut t, &raw[1..], 3.0, false);
    feed(&mut t, &raw[1..], 4.0, false);
    assert_eq!(rows(&t), vec![(1, raw[1], false)]);
}

#[test]
fn three_four_peak_fluctuation_keeps_slots_and_jitter_does_not_renumber() {
    let mut t = Table::default();
    let mut raw = [marker(100.0, -10.0), marker(200.0, -20.0), marker(300.0, -30.0), marker(400.0, -40.0)];
    feed(&mut t, &raw[..3], 0.0, false);
    feed(&mut t, &raw[..3], 1.0, false);
    for i in 0..40 {
        raw[1].f = 200.0 + if i % 2 == 0 { 0.4 } else { -0.4 };
        raw.swap(0, 2);
        feed(&mut t, &raw[..if i % 2 == 0 { 4 } else { 3 }], 1.1 + i as f64 * 0.1, false);
        let r = rows(&t);
        assert_eq!(r.len(), 3);
        assert_eq!(r.iter().map(|r| r.0).collect::<Vec<_>>(), [1, 2, 3]);
        assert!((r[1].1.f - 200.0).abs() < 1.0);
    }
    feed(&mut t, &raw, 6.0, false);
    feed(&mut t, &raw, 7.0, false);
    assert_eq!(rows(&t).len(), 4);
}

#[test]
fn level_order_has_one_db_hysteresis_with_stable_numbers_on_trace() {
    let mut t = Table::default();
    let mut raw = [marker(100.0, -10.0), marker(200.0, -10.5)];
    feed(&mut t, &raw, 0.0, true);
    feed(&mut t, &raw, 1.0, true);
    raw[1].dbv = -9.5;
    feed(&mut t, &raw, 1.1, true);
    assert_eq!(rows(&t)[0].0, 1);
    raw[1].dbv = -8.9;
    feed(&mut t, &raw, 1.2, true);
    assert_eq!(rows(&t).iter().map(|r| r.0).collect::<Vec<_>>(), [2, 1]);
    raw[0].dbv = -8.4;
    feed(&mut t, &raw, 1.3, true);
    assert_eq!(rows(&t)[0].0, 2);
    t.tick(1.3, false);
    assert_eq!(rows(&t)[0], (1, raw[0], false));
}

#[test]
fn nearest_matching_is_one_to_one_and_uses_rbw_or_two_bins() {
    let mut t = Table::default();
    let raw = [marker(100.0, -10.0), marker(103.0, -20.0)];
    feed(&mut t, &raw, 0.0, false);
    feed(&mut t, &raw, 1.0, false);
    feed(&mut t, &[marker(101.5, -9.0)], 1.1, false);
    assert_eq!(rows(&t).iter().filter(|r| !r.2).count(), 1);
    assert_eq!(rows(&t)[1], (2, raw[1], true));
    let sp = spectrum::Spectrum { df: 1.0, rbw: 20.0, ..Default::default() };
    t.refresh(&raw[..1], 2.0, &sp, (WaveId::Ch(0), Window::Hann, 0), false);
    t.refresh(&[marker(109.0, -9.0)], 3.0, &sp, (WaveId::Ch(0), Window::Hann, 0), false);
    assert_eq!(rows(&t)[0].1.f, 109.0);
}

#[test]
fn full_table_keeps_held_slots_while_replacements_qualify_in_bounded_storage() {
    let mut t = Table::default();
    let raw: [_; MAX] = std::array::from_fn(|i| marker(100.0 * (i + 1) as f64, -(i as f64)));
    feed(&mut t, &raw, 0.0, false);
    feed(&mut t, &raw, 1.0, false);
    let next = raw.map(|m| marker(m.f + 20.0, m.dbv));
    feed(&mut t, &next, 1.1, false);
    assert_eq!(rows(&t).len(), MAX);
    assert!(rows(&t).iter().all(|r| r.2));
    assert!(std::mem::size_of::<Table>() < 2048);
    feed(&mut t, &next, 2.1, false);
    assert!(rows(&t).iter().all(|r| !r.2));
    assert_eq!(rows(&t).iter().map(|r| r.1.f).collect::<Vec<_>>(), next.map(|m| m.f));
}

#[test]
fn setup_changes_clock_rewind_and_invalid_input_do_not_reuse_identities() {
    let mut t = Table::default();
    let raw = [marker(100.0, -10.0)];
    feed(&mut t, &raw, 0.0, false);
    feed(&mut t, &raw, 1.0, false);
    feed(&mut t, &raw, 0.0, false);
    assert!(rows(&t).is_empty());
    feed(&mut t, &raw, 1.0, false);
    t.refresh(
        &raw,
        2.0,
        &spectrum::Spectrum { df: 1.0, rbw: 4.0, ..Default::default() },
        (WaveId::Ch(1), Window::Hann, 0),
        false,
    );
    assert!(rows(&t).is_empty());
    feed(&mut t, &[marker(f64::NAN, 0.0), marker(0.0, 0.0), marker(1.0, f64::NAN)], 3.0, false);
    feed(&mut t, &[], 4.0, false);
    assert!(rows(&t).is_empty());
}

#[test]
fn explicit_sort_changes_row_order_without_changing_live_marker_numbers() {
    let mut t = Table::default();
    let raw = [marker(200.0, -10.0), marker(100.0, -20.0)];
    feed(&mut t, &raw, 0.0, true);
    feed(&mut t, &raw, 1.0, true);
    feed(&mut t, &raw, 1.0, false);
    assert_eq!(rows(&t), [(2, raw[1], false), (1, raw[0], false)]);
    feed(&mut t, &[marker(200.1, -10.0), marker(100.1, -20.0)], 1.1, false);
    assert_eq!(rows(&t).iter().map(|r| r.0).collect::<Vec<_>>(), [2, 1]);
}
