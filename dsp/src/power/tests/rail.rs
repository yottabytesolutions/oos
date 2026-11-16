use super::*;

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

fn pulse(start: usize, end: usize, n: usize) -> Vec<f32> {
    (0..n).map(|k| if (start..end).contains(&k) { 1.0 } else { 0.0 }).collect()
}

#[test]
fn clocked_rail_averages_complete_cycles_and_ignores_partial_ends() {
    // Rising crossings 3.5, 11.5, 19.5 select samples 4..11 and 12..19.
    let clock: Vec<f32> = (0..24).map(|k| if k % 8 >= 4 { 1.0 } else { 0.0 }).collect();
    let mut v = vec![100.0; 24];
    v[4..12].copy_from_slice(&[4.0, 6.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]);
    v[12..20].copy_from_slice(&[3.0, 9.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0]);
    let r = rail(&v, 1.0, Some(5.0), None, Some(&clock));
    assert_eq!((r.overshoot, r.undershoot, r.dc, r.turn_on), (2.5, 1.5, 5.5, 100.0));
    v[4..12].fill(f32::NAN);
    let r = rail(&v, 1.0, Some(5.0), None, Some(&clock));
    assert_eq!((r.overshoot, r.undershoot, r.dc), (4.0, 2.0, 6.0));
    v[12..20].fill(f32::NAN);
    let r = rail(&v, 1.0, Some(5.0), None, Some(&clock));
    assert!(r.overshoot.is_nan() && r.undershoot.is_nan() && r.dc.is_nan());
}

#[test]
fn rail_uses_finite_samples_and_only_post_input_peaks() {
    let v = [20.0, f32::NAN, 4.0, 5.0, 7.0, 6.0, 5.0, 5.0];
    let vin = pulse(3, 8, 8);
    let r = rail(&v, 1.0, Some(5.0), Some(&vin), None);
    assert_eq!((r.overshoot, r.undershoot, r.turn_on), (15.0, 1.0, 7.0));
    close(r.dc, 52.0 / 7.0);
    let constant = [5.0; 8];
    let r = rail(&constant, 1.0, None, None, Some(&[0.0; 8]));
    assert_eq!((r.overshoot, r.undershoot, r.dc, r.turn_on), (0.0, 0.0, 5.0, 5.0));
    let r = rail(&constant, 1.0, Some(f64::NAN), None, Some(&[0.0; 3]));
    assert_eq!((r.overshoot, r.undershoot, r.dc), (0.0, 0.0, 5.0));
    assert!(rail(&constant, 1.0, Some(5.0), Some(&[0.0; 8]), None).turn_on.is_nan());
    assert!(rail(&constant, 1.0, Some(5.0), Some(&pulse(0, 4, 8)), None).turn_on.is_nan());
    let mut missing_tail = [5.0; 8];
    missing_tail[3..].fill(f32::NAN);
    assert!(rail(&missing_tail, 1.0, Some(5.0), Some(&vin), None).turn_on.is_nan());
    for (v, dt) in [(&[][..], 1.0), (&[5.0][..], 1.0), (&constant[..], 0.0), (&[f32::NAN; 8][..], 1.0)] {
        let r = rail(v, dt, None, None, None);
        assert!(r.overshoot.is_nan() && r.undershoot.is_nan() && r.dc.is_nan() && r.turn_on.is_nan());
    }
}

#[test]
fn turn_times_require_a_source_edge_and_a_later_destination_edge() {
    let input = pulse(4, 20, 32);
    let output = pulse(8, 24, 32);
    close(turn_on_time(&input, &output, 0.5), 2.25);
    close(turn_off_time(&input, &output, 0.5), 2.25);
    for (a, b, dt) in [
        (&[][..], &output[..], 1.0),
        (&input[..], &output[..], 0.0),
        (&[f32::NAN; 32][..], &output[..], 1.0),
        (&[0.0; 32][..], &output[..], 1.0),
        (&pulse(0, 20, 32)[..], &output[..], 1.0),
        (&input[..], &pulse(0, 32, 32)[..], 1.0),
        (&input[..], &pulse(0, 6, 32)[..], 1.0),
    ] {
        assert!(turn_on_time(a, b, dt).is_nan());
    }
    let reversed = pulse(0, 8, 32);
    assert!(turn_off_time(&input, &reversed, 1.0).is_nan());
}

#[test]
fn pwm_duty_uses_only_complete_rising_to_rising_periods() {
    let v: Vec<f32> = (0..32)
        .map(|k| if (4..6).contains(&k) || (12..16).contains(&k) || (20..26).contains(&k) { 1.0 } else { 0.0 })
        .collect();
    let trend = duty_trend(&v, 0.5);
    assert_eq!(trend, [(1.75, 0.25), (5.75, 0.5)]);
    assert!(duty_trend(&[], 1.0).is_empty());
    assert!(duty_trend(&[1.0; 32], 1.0).is_empty());
    assert!(duty_trend(&pulse(4, 32, 32), 1.0).is_empty());
    assert!(duty_trend(&pulse(4, 8, 16), 1.0).is_empty());
    let fast: Vec<f32> = (0..64).map(|k| if k % 8 < 4 { 0.0 } else { 1.0 }).collect();
    assert_eq!(switch_hz(&fast, 1e-9), 1e6);
}

#[test]
#[ignore = "bug: rail panics when clock cycles start beyond the rail sample count"]
fn longer_clock_cannot_index_past_the_rail() {
    let clock: Vec<f32> = (0..64).map(|k| if k % 8 < 4 { 0.0 } else { 1.0 }).collect();
    let r = rail(&[5.0; 8], 1.0, Some(5.0), None, Some(&clock));
    assert_eq!((r.overshoot, r.undershoot, r.dc), (0.0, 0.0, 5.0));
}
