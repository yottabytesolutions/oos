use super::*;

#[test]
fn single_rising_crossing_does_not_establish_a_frequency() {
    assert_eq!(freq_of(&[0.0, 0.0, 1.0, 1.0], 1.0), None);
    assert_eq!(line_hz(&[0.0, 0.0, 1.0, 1.0], 1.0), 60.0);
}

#[test]
fn partial_switching_swings_do_not_invent_complete_loss_windows() {
    let v: Vec<_> = [0.0, 10.0, 4.0, 6.0, 0.0].into_iter().flat_map(|v| [v; 20]).collect();
    let i: Vec<_> = (0..100).map(|k| if k < 20 { 0.0 } else { 1.0 }).collect();
    let s = switching(&v, &i, 1.0, 10.0);
    assert_eq!(s.transitions.len(), 1);
    assert!(!s.transitions[0].on);
    close(s.transitions[0].t0, 19.1);
    close(s.transitions[0].t1, 19.9);
    assert!(s.e_on.is_nan() && s.e_cond.is_nan() && s.rds_on.is_nan());
}

#[test]
fn instant_switch_edges_have_no_resolvable_slew_fit() {
    let v: Vec<_> = (0..80).map(|k| if (20..60).contains(&k) { 1.0 } else { 0.0 }).collect();
    let a = analyze(&v, 1.0);
    assert!(edge_slope(&v, 1.0, &a, true).is_nan());
    assert!(edge_slope(&v, 1.0, &a, false).is_nan());
}

#[test]
fn zero_current_conduction_has_energy_but_no_resistance() {
    let v: Vec<_> = [10.0, 1.0, 1.0, 10.0, 10.0].into_iter().flat_map(|v| [v; 20]).collect();
    // Current pulse outside the on interval establishes the full current range.
    let i: Vec<_> = (0..100).map(|k| if k < 10 { 1.0 } else { 0.0 }).collect();
    let s = switching(&v, &i, 1.0, 10.0);
    assert_eq!(s.transitions.len(), 2);
    assert!(s.transitions[0].on && !s.transitions[1].on);
    assert_eq!((s.e_on, s.e_off, s.e_cond), (0.0, 0.0, 0.0));
    assert!(s.rds_on.is_nan());
}

#[test]
fn destination_reaching_the_rail_before_source_midpoint_has_no_positive_delay() {
    let input: Vec<_> = (0..32).map(|k| if k < 5 { 0.0 } else { 1.0 }).collect();
    let output: Vec<_> = (0..32).map(|k| if k < 4 { 0.0 } else { 1.0 }).collect();
    assert!(turn_on_time(&input, &output, 1.0).is_nan());
}

#[test]
fn rail_cannot_select_a_turn_on_peak_without_a_rising_source_crossing() {
    let falling: Vec<_> = (0..32).map(|k| if k < 16 { 1.0 } else { 0.0 }).collect();
    let r = rail(&[5.0; 32], 1.0, Some(5.0), Some(&falling), None);
    assert_eq!((r.overshoot, r.undershoot, r.dc), (0.0, 0.0, 5.0));
    assert!(r.turn_on.is_nan());
}

#[test]
fn slew_fit_requires_two_samples_between_reference_crossings() {
    // 10% and 90% both fall around the lone 0.5 V sample, so a slope
    // fit has only one point even though both crossings are measurable.
    let mut v = vec![0.0; 20];
    v.push(0.5);
    v.extend([1.0; 20]);
    let a = analyze(&v, 1.0);
    assert_eq!(a.rises.len(), 1);
    assert!(edge_slope(&v, 1.0, &a, true).is_nan());
}

#[test]
fn rail_without_finite_input_samples_cannot_measure_a_turn_on_peak() {
    let r = rail(&[5.0; 8], 1.0, Some(5.0), Some(&[f32::NAN; 8]), None);
    assert_eq!((r.overshoot, r.undershoot, r.dc), (0.0, 0.0, 5.0));
    assert!(r.turn_on.is_nan());
}

#[test]
fn resolved_linear_switch_edges_have_the_analytical_slope() {
    let mut v = vec![0.0; 20];
    v.extend((0..=10).map(|k| k as f32));
    v.extend([10.0; 20]);
    v.extend((0..=10).rev().map(|k| k as f32));
    v.extend([0.0; 20]);
    let a = analyze(&v, 0.5);
    close(edge_slope(&v, 0.5, &a, true), 2.0);
    close(edge_slope(&v, 0.5, &a, false), -2.0);
}

#[test]
fn conduction_interval_has_known_resistance_and_trapezoid_energy() {
    let v: Vec<_> = (0..100).map(|k| if (20..60).contains(&k) { 1.0 } else { 10.0 }).collect();
    let i: Vec<_> = (0..100).map(|k| if (20..60).contains(&k) { 2.0 } else { 0.0 }).collect();
    let s = switching(&v, &i, 1.0, 10.0);
    assert_eq!(s.transitions.len(), 2);
    close(s.rds_on, 0.5);
    close(s.e_cond, 78.38);
    close(s.e_on, 0.8);
    close(s.e_off, 0.8);
    close(s.p_avg, 0.8);
}

/// The bisection finds the neighbours the scan of the whole list found: the same slopes on a noisy record.
#[test]
fn edge_slope_matches_the_scan() {
    fn scan(x: &[f32], dt: f64, a: &crate::measure::Analysis, rising: bool) -> f64 {
        let amp = a.top - a.base;
        let mut all = a.rises.clone();
        all.extend_from_slice(&a.falls);
        all.sort_by(f64::total_cmp);
        let edges = if rising { &a.rises } else { &a.falls };
        let (lo, hi) = (a.base + 0.1 * amp, a.base + 0.9 * amp);
        let mut slopes = Vec::new();
        for &te in edges {
            let i = (te / dt) as usize + 1;
            let prev = all.iter().rev().find(|&&t| t < te).map(|t| (t / dt) as usize).unwrap_or(0);
            let next = all.iter().find(|&&t| t > te).map(|t| (t / dt) as usize + 1).unwrap_or(x.len());
            let (t0, t1) = if rising {
                (crossing(x, prev.max(1), i, lo, true), crossing(x, i.saturating_sub(1), next, hi, true))
            } else {
                (crossing(x, prev.max(1), i, hi, false), crossing(x, i.saturating_sub(1), next, lo, false))
            };
            let (Some(t0), Some(t1)) = (t0, t1) else { continue };
            if let Some(s) = ls_slope(x, dt, t0, t1) {
                slopes.push(s);
            }
        }
        if slopes.is_empty() {
            f64::NAN
        } else {
            slopes.iter().sum::<f64>() / slopes.len() as f64
        }
    }
    // trapezoids with uneven plateaus and some noise; a few runts that miss a level
    let mut s = 9u32;
    let mut v = Vec::new();
    for k in 0..200 {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        let top = if k % 17 == 0 { 0.6 } else { 1.0 };
        let (on, off) = (20 + (s >> 26) as usize, 20 + (s >> 20 & 31) as usize);
        v.extend((0..8).map(|i| top * i as f32 / 8.0));
        v.extend(std::iter::repeat_n(top, on));
        v.extend((0..8).map(|i| top * (1.0 - i as f32 / 8.0)));
        v.extend(std::iter::repeat_n(0.0, off));
    }
    crate::sim::add_noise(&mut v, 0.02, 4);
    let a = analyze(&v, 1e-9);
    assert!(a.rises.len() > 150, "{}", a.rises.len());
    for rising in [true, false] {
        let (got, want) = (edge_slope(&v, 1e-9, &a, rising), scan(&v, 1e-9, &a, rising));
        assert!(got.is_finite() && got == want, "{rising}: {got} {want}");
    }
}
