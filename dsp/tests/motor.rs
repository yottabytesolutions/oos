// Copyright (C) 2026 OOS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

//! Independent Clarke/Park, angle, encoder and mechanical-power expectations.

use oos_dsp::motor::{self, Direction, Shaft, Theta};
use std::f64::consts::PI;

fn near(actual: f64, expected: f64, tol: f64) {
    assert!((actual - expected).abs() <= tol, "{actual} != {expected} (tolerance {tol})");
}

fn phases(n: usize, step: f64) -> [Vec<f32>; 3] {
    std::array::from_fn(|phase| {
        (0..n).map(|k| (3.0 * (k as f64 * step - phase as f64 * 2.0 * PI / 3.0).cos() + 2.0) as f32).collect()
    })
}

#[test]
fn park_rotation_preserves_vector_magnitude_and_common_mode() {
    let [a, b, c] = phases(401, PI / 20.0);
    let theta: Vec<_> = (0..401).map(|k| k as f64 * PI / 20.0 - PI / 6.0).collect();
    let d = motor::dq0(&a, &b, &c, 0.001, Theta::Elec(&theta)).expect("finite three-phase set");
    near(d.d, 3.0 * (PI / 6.0).cos(), 1e-6);
    near(d.q, 1.5, 1e-6);
    near(d.zero, 2.0, 1e-6);
    near(d.freq, 25.0, 0.01);
    assert!(!d.from_vector && !d.short);
    assert_eq!(d.wave.len(), 401);
    for ((d, q, z), (alpha, beta)) in d.wave.iter().zip(&d.ab) {
        near(d * d + q * q, 9.0, 2e-6);
        near(alpha * alpha + beta * beta, 9.0, 2e-6);
        near(*z, 2.0, 1e-6);
    }
}

#[test]
fn dq0_skips_gaps_truncates_and_decimates() {
    let mut a = vec![5.0; 3002];
    a[2] = f32::NAN;
    let mut theta = vec![PI / 2.0; 3000];
    theta[4] = f64::NAN;
    let d = motor::dq0(&a, &[-1.0; 3001], &[-1.0; 3001], 1.0, Theta::Elec(&theta)).expect("remaining samples");
    near(d.d, 0.0, 1e-12);
    near(d.q, -4.0, 1e-12);
    near(d.zero, 1.0, 1e-12);
    assert_eq!(d.freq, 0.0);
    assert_eq!(d.wave.len(), 1498); // 1500 even indices, minus the two gaps
    assert_eq!(d.ab.len(), d.wave.len());
    assert!(d.wave.iter().all(|&(d, q, z)| d.is_finite() && q == -4.0 && z == 1.0));
    for dt in [0.0, -1.0, f64::NAN] {
        assert!(motor::dq0(&a, &a, &a, dt, Theta::Vector).is_none());
    }
    assert!(motor::dq0(&[1.0; 3], &[1.0; 4], &[1.0; 4], 1.0, Theta::Vector).is_none());
    assert!(motor::dq0(&[f32::NAN; 4], &[1.0; 4], &[1.0; 4], 1.0, Theta::Vector).is_none());
    assert!(motor::dq0(&[1.0; 4], &[1.0; 4], &[1.0; 4], 1.0, Theta::Elec(&[f64::NAN; 4])).is_none());
    let [a, b, c] = phases(65, PI / 4.0);
    let fast = motor::dq0(&a, &b, &c, 1.0, Theta::Vector).expect("eight samples per cycle");
    assert!(fast.short && fast.from_vector);
    near(fast.d, 3.0, 1e-6);
    near(fast.q, 0.0, 1e-6);
}

#[test]
fn analog_electrical_conversion_applies_offset_before_poles_and_preserves_gaps() {
    let v = [0.0, 1.0, -1.0, f32::NAN, f32::INFINITY];
    let mechanical = motor::analog_elec(&v, 90.0, 30.0, false, 3);
    let electrical = motor::analog_elec(&v, 90.0, 30.0, true, 3);
    let zero_poles = motor::analog_elec(&v, 90.0, 30.0, false, 0);
    for (k, deg) in [30.0_f64, 120.0, -60.0].into_iter().enumerate() {
        near(mechanical[k], (3.0 * deg).to_radians(), 1e-12);
        near(electrical[k], deg.to_radians(), 1e-12);
        near(zero_poles[k], electrical[k], 1e-12);
    }
    assert!(mechanical[3..].iter().all(|x| x.is_nan()));
    assert!(motor::analog_elec(&[], 1.0, 0.0, false, 1).is_empty());
}

#[test]
fn phase_shaft_unwraps_both_directions_and_divides_by_pole_pairs() {
    for sign in [-1.0, 1.0] {
        let [a, b, c] = phases(121, sign * PI / 12.0);
        let s = motor::shaft_phase(&a, &b, &c, 1.0 / 24.0, -0.5, 2);
        near(s.rpm.expect("speed"), sign * 30.0, 1e-5);
        near(s.travel_deg.expect("travel"), sign * 900.0, 1e-5);
        near(s.angle_deg.expect("trigger angle"), if sign > 0.0 { 90.0 } else { 270.0 }, 1e-5);
        assert!(s.synchronous);
        assert_eq!(s.direction, if sign > 0.0 { Direction::Forward } else { Direction::Reverse });
        assert_eq!(s.rad.len(), 121);
        for (k, &rad) in s.rad.iter().enumerate() {
            near(rad, sign * k as f64 * PI / 24.0, 1e-6);
        }
        near(s.accel.expect("multiple revolutions"), 0.0, 1e-4);
    }
}

#[test]
fn phase_shaft_restarts_after_nan_and_rejects_missing_phases() {
    let [mut a, b, c] = phases(13, PI / 6.0);
    a[4] = f32::NAN;
    let s = motor::shaft_phase(&a, &b, &c, 1.0 / 12.0, -4.0 / 12.0, 0);
    assert!(s.rad[4].is_nan());
    near(s.rad[5], 5.0 * PI / 6.0, 1e-6);
    near(s.rpm.expect("finite segments"), 60.0, 1e-5);
    near(s.travel_deg.expect("exclude gap"), 300.0, 1e-5);
    near(s.angle_deg.expect("nearest finite trigger sample"), 90.0, 1e-5);
    for dt in [0.0, f64::NAN] {
        assert_eq!(motor::shaft_phase(&a, &b, &c, dt, 0.0, 1).note, "no phase set");
    }
    assert_eq!(motor::shaft_phase(&a, &b, &[], 1.0, 0.0, 1).note, "no phase set");
    let s = motor::shaft_phase(&[f32::NAN; 4], &[0.0; 4], &[0.0; 4], 1.0, 0.0, 1);
    assert!(s.rpm.is_none() && s.angle_deg.is_none() && s.travel_deg.is_none());
}

#[test]
fn wrapped_analog_shaft_has_known_speed_and_trigger_angle() {
    for sign in [-1.0, 1.0] {
        let v: Vec<_> = (0..33).map(|k| (sign * k as f64 * 45.0).rem_euclid(360.0) as f32).collect();
        let s = motor::shaft_analog(&v, 0.125, -0.375, 1.0, 0.0, true, 2);
        near(s.rpm.expect("speed"), sign * 30.0, 1e-10);
        near(s.travel_deg.expect("four electrical revolutions"), sign * 720.0, 1e-10);
        near(s.angle_deg.expect("trigger"), if sign > 0.0 { 67.5 } else { 292.5 }, 1e-10);
        near(s.accel.expect("two mechanical revolutions"), 0.0, 1e-10);
    }
}

#[test]
fn analog_gaps_clipping_and_invalid_inputs() {
    let v = [f32::NAN, 10.0, 20.0, 30.0, f32::NAN, 100.0, 110.0, 120.0, f32::NAN];
    let s = motor::shaft_analog(&v, 1.0, -4.0, 1.0, 0.0, false, 1);
    near(s.rpm.expect("two finite runs"), 10.0 / 6.0, 1e-10);
    near(s.travel_deg.expect("travel excludes gap"), 40.0, 1e-10);
    near(s.angle_deg.expect("equidistant samples prefer earlier"), 30.0, 1e-10);
    let clipped = motor::shaft_analog(&[0.0, 30.0, 60.0, 90.0, 90.0, 90.0], 1.0, -99.0, 1.0, 0.0, false, 1);
    near(clipped.rpm.expect("clipped ramp mean derivative"), 17.5 / 6.0, 1e-10);
    near(clipped.angle_deg.expect("trigger clamped to end"), 90.0, 1e-10);
    let stopped = motor::shaft_analog(&[15.0; 4], 1.0, 2.0, 1.0, 5.0, false, 1);
    assert_eq!(stopped.direction, Direction::Stopped);
    assert_eq!(stopped.rpm, Some(0.0));
    near(stopped.angle_deg.expect("start clamp"), 20.0, 1e-10);
    assert!(stopped.accel.is_none());
    for (v, dt, scale) in [
        (&[][..], 1.0, 1.0),
        (&[0.0][..], 1.0, 1.0),
        (&[0.0; 2][..], 0.0, 1.0),
        (&[0.0; 2][..], f64::NAN, 1.0),
        (&[0.0; 2][..], 1.0, f64::NAN),
    ] {
        assert_eq!(motor::shaft_analog(v, dt, 0.0, scale, 0.0, false, 1).note, "no angle");
    }
}

fn quad() -> (Vec<bool>, Vec<bool>) {
    let states = [(false, false), (true, false), (true, true), (false, true)];
    (0..13).map(|k| states[k % 4]).unzip()
}

#[test]
fn quadrature_index_zeros_once_and_revolution_trend_is_constant() {
    let (a, b) = quad();
    let index: Vec<_> = (0..13).map(|k| k == 2 || k == 6).collect();
    let s = motor::shaft_quad(&a, &b, Some(&index), 0.25, -1.75, 4.0, false);
    assert!(!s.no_index);
    near(s.rpm.expect("count rate"), 60.0, 1e-12);
    near(s.angle_deg.expect("first index remains the origin"), 90.0, 1e-10);
    near(s.travel_deg.expect("three revolutions"), 1080.0, 1e-10);
    assert_eq!(s.trend, [(1.0, 60.0), (2.0, 60.0), (3.0, 60.0)]);
    near(s.accel.expect("constant speed"), 0.0, 1e-12);
    let no_index = motor::shaft_quad(&a, &b, Some(&[]), 0.25, 0.0, 4.0, false);
    assert!(no_index.no_index);
    let reverse = motor::shaft_quad(&a, &b, None, 0.25, 0.0, 4.0, true);
    near(reverse.accel.expect("constant reverse speed"), 0.0, 1e-12);
    assert_eq!(reverse.trend, [(1.0, -60.0), (2.0, -60.0), (3.0, -60.0)]);
}

#[test]
fn quadrature_rejects_bad_parameters_and_ignores_diagonal_transitions() {
    for (n, dt, cpr) in [(0, 1.0, 4.0), (1, 1.0, 4.0), (4, 0.0, 4.0), (4, 1.0, 0.0), (4, 1.0, f64::NAN)] {
        assert_eq!(motor::shaft_quad(&vec![false; n], &[false; 4], None, dt, 0.0, cpr, false).note, "no encoder");
    }
    let diagonal = [false, true, true, false];
    let s = motor::shaft_quad(&diagonal, &diagonal, None, 1.0, 0.0, 4.0, false);
    assert_eq!(s.note, "no encoder edges");
    assert_eq!(s.rpm, Some(0.0));
    assert_eq!(s.direction, Direction::Stopped);
    assert!(s.accel.is_none() && s.trend.is_empty());
    assert!(s.rad.iter().all(|&r| r == 0.0));
}

fn hall(codes: &[u8], dt: f64) -> Shaft {
    let a: Vec<_> = codes.iter().map(|x| x & 4 != 0).collect();
    let b: Vec<_> = codes.iter().map(|x| x & 2 != 0).collect();
    let c: Vec<_> = codes.iter().map(|x| x & 1 != 0).collect();
    motor::shaft_hall(&a, &b, &c, dt, 0.0)
}

#[test]
fn hall_sequence_gives_signed_speed_travel_and_revolution_trend() {
    let sequence = [4, 6, 2, 3, 1, 5];
    for reverse in [false, true] {
        let codes: Vec<_> = (0..19).map(|k| sequence[if reverse { (6 - k % 6) % 6 } else { k % 6 }]).collect();
        let s = hall(&codes, 0.125);
        let sign = if reverse { -1.0 } else { 1.0 };
        near(s.rpm.expect("Hall transitions"), sign * 80.0, 1e-12);
        near(s.travel_deg.expect("three turns"), sign * 1080.0, 1e-10);
        assert_eq!(s.hall_faults, 0);
        assert_eq!(s.direction, if reverse { Direction::Reverse } else { Direction::Forward });
        assert_eq!(s.trend, [(0.75, sign * 80.0), (1.5, sign * 80.0), (2.25, sign * 80.0)]);
        near(s.accel.expect("constant Hall speed"), 0.0, 1e-12);
    }
}

#[test]
fn hall_fault_bursts_hold_angle_and_valid_sequence_resumes() {
    let s = hall(&[4, 6, 0, 7, 2, 2, 7, 3, 5, 4], 1.0);
    assert_eq!(s.hall_faults, 2); // consecutive invalid codes count as one burst
    near(s.rpm.expect("four valid neighbouring steps"), 40.0 / 9.0, 1e-12);
    near(s.travel_deg.expect("skip non-neighbouring Hall transition"), 240.0, 1e-10);
    near(s.rad[2], s.rad[1], 1e-12);
    near(s.rad[3], s.rad[1], 1e-12);
    assert!(s.accel.is_none());
    let stopped = hall(&[4, 4, 2, 2], 1.0);
    assert_eq!(stopped.rpm, None);
    assert_eq!(stopped.note, "no hall edges");
    assert_eq!(stopped.direction, Direction::Stopped);
    assert_eq!(hall(&[], 1.0).note, "no hall");
    assert_eq!(hall(&[4, 6], 0.0).note, "no hall");
    let invalid = hall(&[0, 7, 0], 1.0);
    assert!(invalid.rad.iter().all(|r| r.is_nan()));
    assert!(invalid.angle_deg.is_none() && invalid.travel_deg.is_none());
}

#[test]
fn mechanical_power_uses_paired_finite_samples_and_signed_unclamped_ratio() {
    let shaft = Shaft { rpm: Some(-60.0), ..Shaft::default() };
    let m = motor::mechanical(
        shaft,
        Some(&[1.0, f32::NAN, 3.0]),
        2.0,
        Some(&[10.0, f32::NAN, 20.0, 999.0]),
        Some(&[2.0, 8.0, 1.0]),
    );
    assert_eq!(m.torque, Some(4.0));
    near(m.p_mech.expect("torque times angular velocity"), -8.0 * PI, 1e-12);
    assert_eq!(m.p_elec, Some(20.0));
    near(m.eff.expect("signed ratio"), -0.4 * PI, 1e-12);
    assert!(m.generating);
    let zero = motor::mechanical(Shaft::default(), Some(&[]), 1.0, Some(&[0.0]), Some(&[1.0]));
    assert!(zero.torque.is_none() && zero.p_mech.is_none() && zero.eff.is_none());
    assert_eq!(zero.p_elec, Some(0.0));
    let absent = motor::mechanical(Shaft::default(), Some(&[f32::INFINITY]), 1.0, Some(&[f32::NAN]), Some(&[1.0]));
    assert!(absent.torque.is_none() && absent.p_elec.is_none() && !absent.generating);
    let absent = motor::mechanical(Shaft::default(), None, 1.0, None, Some(&[]));
    assert!(absent.torque.is_none() && absent.p_elec.is_none());
}

#[test]
#[ignore = "bug: Hall decoding never recovers from an invalid initial code"]
fn hall_recovers_when_record_starts_with_invalid_code() {
    let s = hall(&[0, 4, 6, 2, 3, 1, 5, 4], 1.0 / 6.0);
    assert!(s.rad[0].is_nan());
    near(s.rad[1], 0.0, 1e-12);
    near(s.travel_deg.expect("one valid revolution after initial fault"), 360.0, 1e-10);
    assert_eq!(s.direction, Direction::Forward);
}
