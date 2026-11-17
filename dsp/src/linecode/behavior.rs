// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn convolution_and_fir_handle_missing_samples_and_invalid_inputs() {
    assert_eq!(conv(&[1.0, f32::NAN, 3.0], &[1.0; 3]), [1.0, 4.0, 3.0]);
    assert_eq!(conv(&[2.0], &[]), [2.0]);
    assert!(conv(&[], &[1.0]).is_empty());
    for (dt, fc) in [(0.0, 1.0), (1.0, 0.0), (f64::NAN, 1.0), (1.0, f64::NAN)] {
        assert_eq!(fir_lowpass(&[2.0, 3.0], dt, fc), [2.0, 3.0]);
    }
    assert!(fir_lowpass(&[], 1.0, 1.0).is_empty());
}

#[test]
#[ignore = "bug: conv correlates (kernel not flipped)"]
fn convolution_centers_and_flips_the_kernel() {
    assert_eq!(conv(&[1.0, 2.0, 3.0], &[1.0, 2.0, 4.0]), [4.0, 11.0, 14.0]);
}

#[test]
fn slicers_and_every_gate_follow_the_truth_table() {
    let a = [-2.0, -2.0, 2.0, 2.0];
    let b = [-5.0, 5.0, -5.0, 5.0];
    for (g, expected) in [
        (Gate::And, [0.0, 0.0, 0.0, 1.0]),
        (Gate::Or, [0.0, 1.0, 1.0, 1.0]),
        (Gate::Xor, [0.0, 1.0, 1.0, 0.0]),
        (Gate::Nand, [1.0, 1.0, 1.0, 0.0]),
        (Gate::Nor, [1.0, 0.0, 0.0, 0.0]),
    ] {
        assert_eq!(gate(&a, &b, g), expected);
    }
    assert_eq!(gate(&a, &b[..2], Gate::Or), [0.0, 1.0]);
    assert_eq!(a2d(&[-2.0, 0.0, 2.0, f32::NAN]), [0.0, 1.0, 1.0, 0.0]);
    assert_eq!(a2d(&[f32::NAN, f32::INFINITY]), [0.0, 0.0]);
    assert_eq!(d2a(&[-2.0, 0.0, 2.0, f32::NAN]), [-2.0, 2.0, 2.0, -2.0]);
    assert_eq!(unnrzi(&[0.0, 1.0, 1.0, 0.0, 0.0]), [1.0, 0.0, 1.0, 0.0, 1.0]);
    assert!(a2d(&[]).is_empty());
    assert!(d2a(&[]).is_empty());
    assert!(unnrzi(&[]).is_empty());
}

#[test]
#[ignore = "bug: d2a invents 0 V and 1 V extrema for traces inside or outside that range"]
fn d2a_uses_the_actual_trace_extrema() {
    assert_eq!(d2a(&[2.0, 3.0, 4.0]), [2.0, 4.0, 4.0]);
}

#[test]
fn folding_has_zero_error_for_repetition_and_known_error_for_offsets() {
    let v = [0.0, 1.0, 2.0, 3.0, 2.0, 3.0, 4.0, 5.0];
    assert_eq!(fold_rms(&v, 1.0, 4.0), 1.0);
    assert_eq!(fold_rms(&[0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0], 1.0, 4.0), 0.0);
    let mut sparse = v;
    sparse[0] = f32::NAN;
    sparse[4] = f32::INFINITY;
    assert_eq!(fold_rms(&sparse, 1.0, 4.0), 1.0);
    assert!(fold_rms(&[f32::NAN; 8], 1.0, 4.0).is_nan());
    assert!(fold_rms(&v[..7], 1.0, 4.0).is_nan());
    assert!(fold_rms(&v, 1.0, 1.0).is_nan());
    assert_eq!(unit_interval_runs(&[1.0, 2.0, 3.0, 20.0, f64::NAN, -1.0, 0.0]), Some(1.0));
    assert_eq!(unit_interval_runs(&[1.0, 1.1, 2.1, 3.1]), Some(7.3 / 7.0));
    assert_eq!(unit_interval_runs(&[f64::NAN, 1.0]), None);
}

#[test]
fn byte_packing_is_lsb_first_and_discards_an_incomplete_byte() {
    let bits: Vec<_> = [true, false, true, false, false, true, false, true, true]
        .into_iter()
        .enumerate()
        .map(|(k, b)| (k as f64, b))
        .collect();
    assert_eq!(bits_to_bytes(&bits), [(0.0, 7.0, 0xa5)]);
    assert!(bits_to_bytes(&bits[..7]).is_empty());
}

#[test]
fn ethernet_fields_have_network_byte_order_and_exact_sample_spans() {
    let mut data = vec![0x55, 0xd5, 0x00, 0x00, 0x5e, 0x00, 0x53, 0x01, 0x00, 0x00, 0x5e, 0x00, 0x53, 0x02, 0x08, 0x00];
    data.extend([0; 12]);
    data.extend([192, 0, 2, 1, 192, 0, 2, 2, 0]);
    let bytes: Vec<_> = data.iter().enumerate().map(|(k, &b)| (k as f64 * 8.0, k as f64 * 8.0 + 7.0, b)).collect();
    let mut out = Vec::new();
    ethernet_fields(&bytes, &mut out);
    let expected = [
        ("MAC", 0x00005e00, Some(0x5301), 16.0, 63.0),
        ("SRC", 0x00005e00, Some(0x5302), 64.0, 111.0),
        ("TYPE", 0x0800, None, 112.0, 127.0),
        ("IP", 0xc0000201, None, 224.0, 255.0),
        ("DIP", 0xc0000202, None, 256.0, 287.0),
    ];
    assert_eq!(out.len(), expected.len());
    for (p, (name, value, value2, t0, t1)) in out.iter().zip(expected) {
        assert_eq!((p.kind, p.value, p.value2, p.t0, p.t1, p.ok), (Kind::Field(name), value, value2, t0, t1, true));
    }
    for short in [&bytes[..1], &bytes[..10]] {
        let mut out = Vec::new();
        ethernet_fields(short, &mut out);
        assert!(out.is_empty());
    }
}

fn manchester_signal(bytes: &[u8]) -> Vec<f32> {
    bytes
        .iter()
        .flat_map(|&byte| {
            (0..8).flat_map(move |bit| {
                let one = byte & (1 << bit) != 0;
                (0..16).map(move |k| if (k >= 8) == one { 1.0 } else { 0.0 })
            })
        })
        .collect()
}

#[test]
fn manchester_recovers_bytes_at_known_and_inferred_rates() {
    let v = manchester_signal(&[0x55, 0xd5, 0xa5, 0x33]);
    for rate in [0.0, 1.0 / 16.0] {
        let out = manchester(&v, 1.0, rate, 0.5, 0.1);
        assert_eq!(out.iter().map(|p| p.value).collect::<Vec<_>>(), [0x55, 0xd5, 0xa5, 0x33]);
        assert!(out.iter().all(|p| p.kind == Kind::Data && p.ok && p.t1 > p.t0));
    }
    for v in [&[][..], &[0.0; 20], &[0.0, 1.0, 0.0]] {
        assert!(manchester(v, 1.0, 0.0, 0.5, 0.1).is_empty());
        assert!(mlt3(v, 1.0, 0.0, 0.5, 0.1).is_empty());
    }
    assert!(manchester(&v, 1.0, 1.0, 0.5, 0.1).is_empty());
    assert!(mlt3(&v, 1.0, 1.0, 0.5, 0.1).is_empty());
}

#[test]
fn mlt3_transition_and_hold_cells_have_known_bits() {
    // Valid MLT-3 sequence 0,+1,0,-1,0,+1, with eight zero-level
    // cells before -1. In the first decoded byte, only cell 1 changes.
    let mut v = vec![0.0; 16];
    v.extend([1.0; 16]);
    v.extend([0.0; 128]);
    v.extend([-1.0; 16]);
    v.extend([0.0; 16]);
    v.extend([1.0; 16]);
    for rate in [0.0, 1.0 / 16.0] {
        let out = mlt3(&v, 1.0, rate, 0.5, 0.1);
        assert_eq!(out.len(), 1);
        // The initial comparator crossing has no preceding bit history.
        assert_eq!((out[0].kind, out[0].value & 0xfe, out[0].ok), (Kind::Data, 0x02, true));
        assert_eq!((out[0].t0, out[0].t1), (15.5, 127.5));
    }
}

#[test]
fn manchester_reacquires_mid_cells_after_a_missing_transition() {
    let mut v = manchester_signal(&[0xff, 0xff, 0xff, 0xff]);
    // Clip one mid-cell transition and its following boundary. The decoder
    // should advance past the missing cell and retain the complete bytes.
    v[128..144].fill(0.0);
    let out = manchester(&v, 1.0, 1.0 / 16.0, 0.5, 0.1);
    assert_eq!(out.iter().map(|p| p.value).collect::<Vec<_>>(), [0xff, 0xff, 0xff]);
    assert!(out.windows(2).all(|p| p[0].t1 < p[1].t0));
}

#[test]
fn mlt3_decodes_a_three_level_all_ones_signal() {
    // Independent MLT-3 encoding of 40 ones: 0,+1,0,-1,0,+1,... .
    // The first byte may lack the first transition, but later bytes are 0xff.
    let v: Vec<_> = (0..41).flat_map(|k| [[0.0, 1.0, 0.0, -1.0][k % 4]; 16]).collect();
    let out = mlt3(&v, 1.0, 1.0 / 16.0, 0.5, 0.1);
    assert!(out.len() >= 2);
    assert_eq!(out[1].value, 0xff);
}
