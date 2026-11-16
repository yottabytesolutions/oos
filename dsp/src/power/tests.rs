use super::*;

mod edge;
mod rail;

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

#[test]
fn quality_handles_missing_pairs_zero_power_and_single_cycle() {
    for v in [&[][..], &[1.0; 3], &[f32::NAN; 8], &[2.0; 8]] {
        assert_eq!(quality(v, v, 1.0, None), Quality::default());
    }
    for dt in [0.0, -1.0, f64::NAN] {
        assert_eq!(quality(&[1.0; 8], &[1.0; 8], dt, Some(0.25)), Quality::default());
    }
    assert_eq!(quality(&[f32::NAN; 8], &[1.0; 8], 1.0, Some(0.25)), Quality::default());
    let q = quality(&[0.0; 4], &[0.0; 4], 1.0, Some(0.25));
    assert_eq!((q.vrms, q.irms, q.p_real, q.s_apparent, q.cycles), (0.0, 0.0, 0.0, 0.0, 1));
    assert!(q.pf.is_nan() && q.crest_v.is_nan() && q.crest_i.is_nan());
    let q = quality(&[2.0, 0.0, -2.0, 0.0, 100.0], &[1.0, 0.0, -1.0, 0.0], 1.0, Some(0.25));
    close(q.vrms, 2f64.sqrt());
    close(q.irms, 1.0 / 2f64.sqrt());
    close(q.p_real, 1.0);
    close(q.pf, 1.0);
    assert_eq!((q.cycles, q.freq), (1, 0.25));
}

#[test]
fn harmonics_resolve_known_components_and_stop_below_nyquist() {
    let x: Vec<f32> = (0..64)
        .map(|k| {
            let t = 2.0 * PI * k as f64 / 16.0;
            (2.0 * t.cos() + 4.0 * (3.0 * t).sin()) as f32
        })
        .collect();
    let hs = harmonics(&x, 1.0, Some(1.0 / 16.0), 10);
    assert_eq!(hs.len(), 7);
    close(hs[0].1, 2f64.sqrt());
    close(hs[2].1, 2.0 * 2f64.sqrt());
    assert!(!hs[2].3);
    for h in [1, 3, 4, 5, 6] {
        close(hs[h].1, 0.0);
    }
    for (order, expected) in [
        (2, 1.08),
        (3, 2.30),
        (4, 0.43),
        (5, 1.14),
        (6, 0.30),
        (7, 0.77),
        (9, 0.40),
        (11, 0.33),
        (13, 0.21),
        (8, 0.23),
        (40, 0.046),
        (15, 0.15),
        (39, 0.15 * 15.0 / 39.0),
    ] {
        close(class_a_limit(order), expected);
    }
    assert!(class_a_limit(1).is_infinite() && class_a_limit(41).is_infinite());
    assert!(harmonics(&[], 1.0, None, 5).is_empty());
    assert!(harmonics(&x, 0.0, None, 5).is_empty());
    assert!(harmonics(&[1.0; 8], 1.0, None, 5).is_empty());
    assert_eq!(dft(&[f32::NAN; 8], 1.0, 0, 8, 0.25), (0.0, 0.0));
    assert_eq!(whole_cycles(3, 1.0, 0.25), (0, 3, 0));
    assert_eq!(whole_cycles(10, 1.0, 0.25), (0, 8, 2));
}

#[test]
fn crossing_and_integral_agree_with_linear_geometry() {
    let x = [0.0, 2.0, f32::NAN, 4.0, 0.0];
    assert_eq!(crossing(&x, 0, 5, 1.0, true), Some(0.5));
    assert_eq!(crossing(&x, 0, 5, 1.0, false), Some(3.75));
    assert_eq!(crossing(&x, 1, 3, 3.0, true), None);
    assert_eq!(crossing(&x, 8, 10, 1.0, true), None);
    // Integral of p(t) = 2t + 1 is t² + t, then multiply by dt.
    for (a, b) in [(0.25, 0.75), (0.25, 2.75), (1.0, 3.0)] {
        close(integrate(&[1.0, 3.0, 5.0, 7.0], a, b, 0.5), ((b * b + b) - (a * a + a)) * 0.5);
    }
    assert_eq!(integrate(&[], 0.0, 1.0, 1.0), 0.0);
    assert_eq!(integrate(&[2.0], 0.0, 1.0, 1.0), 0.0);
    assert_eq!(integrate(&[1.0, 2.0], 1.0, 0.0, 1.0), 0.0);
}

#[test]
fn least_squares_and_conduction_resistance_skip_nonfinite_values() {
    let x = [1.0, 3.0, f32::NAN, 7.0, 9.0];
    assert_eq!(ls_slope(&x, 0.5, 4.0, 0.0), Some(4.0));
    assert_eq!(ls_slope(&x, 0.5, 0.1, 0.9), None);
    assert_eq!(ls_slope(&[f32::NAN, 2.0, f32::NAN], 1.0, 0.0, 2.0), None);
    assert_eq!(ls_slope(&x, 0.0, 0.0, 4.0), None);
    assert_eq!(conduction_r(&[2.0; 4], &[4.0; 4], 0.0, 3.0, 4.0), Some(0.5));
    for (a, b) in [(0.1, 0.9), (0.0, 3.0)] {
        assert_eq!(conduction_r(&[f32::NAN; 4], &[4.0; 4], a, b, 4.0), None);
    }
    for current in [0.0, 0.1, f32::NAN] {
        assert_eq!(conduction_r(&[2.0; 4], &[current; 4], 0.0, 3.0, 4.0), None);
    }
    let a = analyze(&[2.0; 8], 1.0);
    assert!(edge_slope(&[2.0; 8], 1.0, &a, true).is_nan());
}

#[test]
fn switching_rejects_missing_or_constant_signals_without_fake_transitions() {
    for (v, i, dt) in [
        (&[][..], &[][..], 1.0),
        (&[1.0; 8], &[2.0; 8], 0.0),
        (&[f32::NAN; 8], &[2.0; 8], 1.0),
        (&[1.0; 8], &[2.0; 8], 1.0),
    ] {
        let s = switching(v, i, dt, 10.0);
        assert!(s.transitions.is_empty());
        assert!(s.e_on.is_nan() && s.e_off.is_nan() && s.e_cond.is_nan() && s.rds_on.is_nan());
    }
    let s = switching(&[1.0; 8], &[2.0; 8], 1.0, 10.0);
    assert_eq!(s.p_avg, 2.0);
}

#[test]
fn dc_ripple_inrush_and_efficiency_have_hand_computable_results() {
    assert_eq!(ripple(&[3.0, 5.0, f32::NAN]), (2.0, 1.0));
    assert_eq!(ripple(&[7.0]), (0.0, 0.0));
    for x in [&[][..], &[f32::NAN, f32::INFINITY]] {
        let (pp, rms) = ripple(x);
        assert!(pp.is_nan() && rms.is_nan());
        let (peak, time, energy) = inrush(x, 0.5);
        assert!(peak.is_nan() && time.is_nan());
        assert_eq!(energy, 0.0);
    }
    assert_eq!(inrush(&[1.0, -3.0, 3.0, f32::NAN], 0.5), (3.0, 0.5, 9.5));
    let e = efficiency(&[10.0, 10.0, 999.0], &[2.0, f32::NAN], &[4.0; 2], &[4.0; 2], 0.5);
    assert_eq!(e, Efficiency { p_in: 20.0, p_out: 16.0, eff: 0.8, e_in: 10.0, e_out: 8.0 });
    for x in [&[][..], &[f32::NAN]] {
        let e = efficiency(x, x, x, x, 1.0);
        assert_eq!((e.p_in, e.p_out, e.e_in, e.e_out), (0.0, 0.0, 0.0, 0.0));
        assert!(e.eff.is_nan());
    }
    assert!(efficiency(&[0.0], &[1.0], &[2.0], &[1.0], 1.0).eff.is_nan());
}

#[test]
fn soa_interpolates_logarithmically_or_linearly_and_checks_strict_exceedance() {
    let limits = [(1.0, 100.0), (100.0, 1.0)];
    close(soa_limit(&limits, 10.0), 10.0);
    close(soa_limit(&limits, -10.0), 10.0);
    assert_eq!(soa_limit(&limits, 0.0), 100.0);
    close(soa_limit(&limits, 100.0), 1.0);
    assert_eq!(soa_limit(&limits, 101.0), 0.0);
    assert!(soa_limit(&[], 10.0).is_infinite());
    assert_eq!(soa_limit(&[(0.0, 10.0), (10.0, 0.0)], 5.0), 5.0);
    assert_eq!(soa(&[1.0, -10.0, 101.0, f32::NAN], &[100.0, -11.0, 0.1, 100.0], &limits), [1, 2]);
    assert!(soa(&[1.0], &[], &limits).is_empty());
}

#[test]
fn capacitance_and_tone_reject_invalid_inputs() {
    for x in [&[][..], &[1.0], &[f32::NAN; 8], &[1.0; 8]] {
        assert!(input_capacitance(x, &[1.0; 8], 1.0).is_nan());
    }
    assert!(input_capacitance(&[0.0, 1.0], &[1.0; 2], 0.0).is_nan());
    close(input_capacitance(&[0.0, 2.0, 4.0], &[2.0, f32::NAN, 2.0], 0.5), 0.25);
    for (dt, f) in [(0.0, 1.0), (1.0, 0.0), (1.0, 0.5), (1.0, f64::NAN)] {
        let t = tone_ripple(&[1.0; 8], dt, f);
        assert!(t.pp.is_nan() && t.rms.is_nan());
    }
    assert!(tone_ripple(&[], 1.0, 0.25).pp.is_nan());
    assert_eq!(line_hz(&[], 1.0), 60.0);
    assert!(switch_hz(&[], 1.0).is_nan());
}
