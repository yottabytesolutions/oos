// Copyright (C) 2026 OOS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

//! Piecewise linear pulses with analytical crossing times and triangular recovery charge.

use oos_dsp::dpt::{self, Fourth, Pulse};

const DT: f64 = 1e-6;

fn near(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("measurement exists");
    let tol = 1e-6 * expected.abs().max(DT);
    assert!((actual - expected).abs() <= tol, "{actual} != {expected} (tolerance {tol})");
}

fn pwl(knots: &[(usize, f32)]) -> Vec<f32> {
    (0..161)
        .map(|k| {
            for pair in knots.windows(2) {
                let [(a, va), (b, vb)] = [pair[0], pair[1]];
                if k < b {
                    return va + (vb - va) * (k - a) as f32 / (b - a) as f32;
                }
            }
            knots.last().expect("nonempty knots").1
        })
        .collect()
}

fn record() -> [Vec<f32>; 3] {
    let gate = pwl(&[
        (0, 0.0),
        (10, 0.0),
        (12, 10.0),
        (50, 10.0),
        (52, 0.0),
        (100, 0.0),
        (102, 10.0),
        (140, 10.0),
        (142, 0.0),
    ]);
    let id = pwl(&[
        (0, 0.0),
        (14, 0.0),
        (18, 10.0),
        (56, 10.0),
        (60, 0.0),
        (104, 0.0),
        (108, 10.0),
        (112, 14.0),
        (116, 10.0),
    ]);
    let vds = pwl(&[
        (0, 100.0),
        (14, 100.0),
        (18, 0.0),
        (56, 0.0),
        (60, 100.0),
        (64, 120.0),
        (68, 100.0),
        (104, 100.0),
        (108, 0.0),
    ]);
    [vds, id, gate]
}

fn measure(v: &[f32], i: &[f32], g: &[f32], fourth: Fourth<'_>) -> Pulse {
    dpt::measure(v, i, Some(g), fourth, DT, 10.0)
}

#[test]
fn linear_edges_have_exact_delays_slopes_peaks_and_qoss() {
    let [v, i, g] = record();
    let p = measure(&v, &i, &g, Fourth::Off);
    assert_eq!(p.pulses, 2);
    assert!(!p.no_second && !p.sample_limit && !p.inverted && p.note.is_empty());
    near(p.vbus, 100.0);
    near(p.il, 10.0);
    near(p.vpeak, 120.0);
    near(p.overshoot, 20.0);
    near(p.ipeak, 14.0);
    near(p.i_over, 4.0);
    near(p.td_on, 4.2 * DT); // gate10=100.2, current10=104.4
    near(p.tr, 3.2 * DT);
    near(p.ton, 7.4 * DT);
    near(p.td_off, 6.2 * DT); // gate90=50.2, current90=56.4
    near(p.tf, 3.2 * DT);
    near(p.toff, 9.4 * DT);
    near(p.dvdt, 25.0 / DT);
    near(p.didt, 2.5 / DT);
    // During the 10-90% voltage rise, IL-Id ramps from 1 to 9 A over 3.2 samples.
    near(p.qoss, 16.0 * DT);
    assert!(p.qoss_approx);
    near(p.marks.gate10, 100.2 * DT);
    near(p.marks.i10, 104.4 * DT);
    near(p.marks.i90, 107.6 * DT);
    near(p.marks.gate90_off, 50.2 * DT);
    near(p.marks.i90_off, 56.4 * DT);
    near(p.marks.i10_off, 59.6 * DT);
    near(p.marks.vpeak, 64.0 * DT);
    near(p.marks.ipeak, 112.0 * DT);
    assert_eq!(p.zoom_both, (0.0, 160.0 * DT));
    near(Some(p.zoom_on.0), 81.0 * DT);
    near(Some(p.zoom_off.0), 31.0 * DT);
    near(Some(p.zoom_off.1), 121.0 * DT);
}

fn recovery(p: &Pulse) {
    // Triangle height 4 A, base 8 samples; stop at 10% removes a 0.4*0.4/2 tail.
    near(p.irrm, 4.0);
    near(p.trr, 7.6 * DT);
    near(p.qrr, 15.92 * DT);
    near(p.diode_didt, 1.0 / DT);
    near(p.marks.trr_end, 115.6 * DT);
}

#[test]
fn recovery_from_drain_current_and_each_diode_probe_has_known_charge_and_energy() {
    let [v, i, g] = record();
    let p = measure(&v, &i, &g, Fourth::Off);
    recovery(&p);
    near(p.err, 1592.0 * DT); // 100 V throughout reverse current
    assert!(p.diode_from_id && p.err_from_vds);
    let mut diode_i = pwl(&[(0, 2.0), (106, 2.0), (108, 0.0), (112, -4.0), (116, 0.0)]);
    diode_i[0] = f32::NAN; // outside recovery must not contaminate the integral
    let p = measure(&v, &i, &g, Fourth::DiodeI(&diode_i));
    recovery(&p);
    near(p.err, 1592.0 * DT);
    assert!(!p.diode_from_id && p.err_from_vds);
    let mut diode_v = vec![25.0; 161];
    diode_v[0] = f32::NAN;
    let p = measure(&v, &i, &g, Fourth::DiodeV(&diode_v));
    recovery(&p);
    near(p.err, 398.0 * DT); // direct probe measures 25 V, so Err=25*Qrr
    assert!(p.diode_from_id && !p.err_from_vds);
}

#[test]
fn dead_times_use_gate_thresholds_and_choose_the_smaller_interval() {
    let [v, i, g] = record();
    let h = pwl(&[(0, 0.0), (60, 0.0), (62, 20.0), (80, 20.0), (82, 0.0)]);
    let p = measure(&v, &i, &g, Fourth::Gate(&h));
    // Low gate falls through 10% at 51.8; high rises at 60.2, falls at 81.8.
    near(p.tdt_a, 8.4 * DT);
    near(p.tdt_b, 18.4 * DT);
    near(p.tdt, 8.4 * DT);
    let p = measure(&v, &i, &g, Fourth::Gate(&[0.0; 161]));
    assert!(p.tdt.is_none() && p.tdt_a.is_none() && p.tdt_b.is_none());
    let h = pwl(&[(0, 0.0), (60, 0.0), (62, 20.0)]);
    let p = measure(&v, &i, &g, Fourth::Gate(&h));
    near(p.tdt_a, 8.4 * DT);
    assert!(p.tdt.is_none() && p.tdt_b.is_none());
}

#[test]
fn time_scaling_and_current_polarity_preserve_physical_measurements() {
    let [v, i, g] = record();
    let inv: Vec<_> = i.iter().map(|x| -x).collect();
    let p = measure(&v, &inv, &g, Fourth::Off);
    assert!(p.inverted);
    recovery(&p);
    near(p.il, 10.0);
    near(p.ipeak, 14.0);
    near(p.td_on, 4.2 * DT);
    let p = dpt::measure(&v, &i, Some(&g), Fourth::Off, 2.0 * DT, 10.0);
    near(p.tr, 6.4 * DT);
    near(p.dvdt, 12.5 / DT);
    near(p.didt, 1.25 / DT);
    near(p.qrr, 31.84 * DT);
    near(p.err, 3184.0 * DT);
}

#[test]
fn missing_and_flat_gates_use_ungated_peaks_without_inventing_delays() {
    let [v, i, _] = record();
    for g in [None, Some(&[0.0; 161][..]), Some(&[f32::NAN; 161][..])] {
        let p = dpt::measure(&v, &i, g, Fourth::Off, DT, 10.0);
        assert_eq!(p.note, "no gate");
        near(p.vpeak, 120.0);
        near(p.ipeak, 10.0);
        assert!(p.td_on.is_none() && p.td_off.is_none() && p.tr.is_none() && p.tf.is_none());
        assert!(p.vbus.is_none() && p.il.is_none() && p.qrr.is_none());
        assert_eq!(p.pulses, 0);
    }
    // A swinging gate with no closed on interval also takes the ungated path.
    let g = pwl(&[(0, 0.0), (10, 0.0), (12, 10.0)]);
    let p = measure(&v, &i, &g, Fourth::Off);
    assert_eq!(p.note, "no gate");
    near(p.vpeak, 120.0);
    for v in [&[0.0; 161][..], &[f32::NAN; 161][..]] {
        let p = dpt::measure(v, &i, None, Fourth::Off, DT, 10.0);
        assert!(p.vpeak.is_none() && p.ipeak.is_none());
    }
}

#[test]
fn short_records_and_invalid_timebases_return_empty_measurements() {
    let [v, i, g] = record();
    for n in 0..4 {
        let p = measure(&v[..n], &i, &g, Fourth::Off);
        assert_eq!(p.note, "record too short");
        assert!(p.vpeak.is_none() && p.il.is_none());
    }
    for dt in [0.0, -DT, f64::NAN] {
        assert_eq!(dpt::measure(&v, &i, Some(&g), Fourth::Off, dt, 10.0).note, "record too short");
    }
    for fourth in [Fourth::Gate(&[0.0; 3]), Fourth::DiodeI(&[]), Fourth::DiodeV(&[0.0; 2])] {
        assert_eq!(measure(&v, &i, &g, fourth).note, "record too short");
    }
    assert_eq!(measure(&v, &i[..3], &g, Fourth::Off).note, "record too short");
    assert_eq!(measure(&v, &i, &g[..3], Fourth::Off).note, "record too short");
}

#[test]
fn a_single_pulse_reports_only_turn_off_and_ignores_nan_in_level_estimates() {
    let [mut v, mut i, mut g] = record();
    v[2] = f32::NAN;
    i[2] = f32::NAN;
    i[30] = f32::NAN;
    g[2] = f32::NAN;
    let p = measure(&v[..90], &i[..90], &g[..90], Fourth::Off);
    assert_eq!(p.pulses, 1);
    assert!(p.no_second);
    assert_eq!(p.note, "no second pulse");
    near(p.il, 10.0);
    near(p.vbus, 100.0);
    near(p.vpeak, 120.0);
    near(p.toff, 9.4 * DT);
    assert!(p.ipeak.is_none() && p.ton.is_none() && p.trr.is_none());
    assert_eq!(p.zoom_on, (0.0, 0.0));
    near(Some(p.zoom_off.1), 89.0 * DT);
}

#[test]
fn zero_current_step_and_absent_recovery_do_not_report_charge() {
    let [v, _, g] = record();
    let p = measure(&v, &[0.0; 161], &g, Fourth::Off);
    assert_eq!(p.note, "no current step");
    near(p.il, 0.0);
    assert!(p.qoss.is_none() && p.qrr.is_none() && p.irrm.is_none());
    let p = measure(&v, &[f32::NAN; 161], &g, Fourth::Off);
    assert!(p.il.is_none() && p.ipeak.is_none() && p.didt.is_none() && p.qoss.is_none());
    assert_eq!(p.note, "no current step");
    let [v, i, g] = record();
    let p = measure(&v, &i, &g, Fourth::DiodeI(&[2.0; 161]));
    assert!(!p.diode_from_id);
    assert!(p.irrm.is_none() && p.qrr.is_none() && p.err.is_none());
    let clipped_i: Vec<_> = i.iter().map(|x| x.min(10.0)).collect();
    let p = measure(&v, &clipped_i, &g, Fourth::Off);
    near(p.ipeak, 10.0);
    near(p.i_over, 0.0);
    assert!(p.irrm.is_none() && p.qrr.is_none());
}

#[test]
fn one_sample_edges_are_marked_and_gate_threshold_is_clamped() {
    let g: Vec<_> =
        (0..161).map(|k| if (10..50).contains(&k) || (100..140).contains(&k) { 10.0 } else { 0.0 }).collect();
    let i: Vec<_> =
        (0..161).map(|k| if (12..52).contains(&k) || (102..142).contains(&k) { 10.0 } else { 0.0 }).collect();
    let v: Vec<_> = i.iter().map(|x| 100.0 - 10.0 * x).collect();
    let p = measure(&v, &i, &g, Fourth::Off);
    assert!(p.sample_limit);
    near(p.tr, 0.8 * DT);
    near(p.tf, 0.8 * DT);
    near(p.dvdt, 100.0 / DT);
    near(p.didt, 10.0 / DT);
    for (pct, delay) in [(-10.0, 2.09), (0.0, 2.09), (100.0, 1.61)] {
        let p = dpt::measure(&v, &i, Some(&g), Fourth::Off, DT, pct);
        near(p.td_on, delay * DT);
        near(p.tr, 0.8 * DT); // gate percentage does not change current 10-90%
    }
}

#[test]
fn additional_pulses_do_not_change_first_turn_off_or_second_turn_on() {
    let [v, i, mut g] = record();
    g.extend([0.0; 10]);
    g.extend([10.0; 10]);
    g.extend([0.0; 10]);
    let mut v = v;
    let mut i = i;
    v.resize(g.len(), 100.0);
    i.resize(g.len(), 0.0);
    let p = measure(&v, &i, &g, Fourth::Off);
    assert_eq!(p.pulses, 3);
    assert_eq!(p.note, "further pulses ignored");
    near(p.td_on, 4.2 * DT);
    near(p.toff, 9.4 * DT);
    recovery(&p);
}
