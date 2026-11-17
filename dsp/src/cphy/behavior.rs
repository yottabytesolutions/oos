// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

// Independent voltage ordering table: +x,+y,+z,-x,-y,-z.
const WIRES: [[f32; 3]; 6] =
    [[0.4, 0.0, 0.2], [0.2, 0.4, 0.0], [0.0, 0.2, 0.4], [0.0, 0.4, 0.2], [0.2, 0.0, 0.4], [0.4, 0.2, 0.0]];

fn signal(runs: &[([f32; 3], usize)]) -> [Vec<f32>; 3] {
    let mut wires: [Vec<f32>; 3] = Default::default();
    for &(values, count) in runs {
        for (wire, value) in wires.iter_mut().zip(values) {
            wire.extend(std::iter::repeat_n(value, count));
        }
    }
    wires
}

fn packets(wires: &[Vec<f32>; 3], rate: Option<f64>, gap: f32) -> Vec<Packet> {
    decode(&wires[0], &wires[1], &wires[2], 1.0, rate, gap)
}

#[test]
fn mapper_words_have_known_boundaries_and_reject_invalid_symbols() {
    assert_eq!(word(&[0; 7]), Some(0));
    assert_eq!(word(&[3; 7]), Some(0x3fff));
    assert_eq!(word(&[0, 0, 0, 0, 0, 0, 4]), Some(0x4000));
    assert_eq!(word(&[4, 0, 0, 0, 0, 0, 0]), Some(0xa000));
    assert_eq!(word(&[5, 0, 0, 0, 0, 0, 0]), None);
    assert_eq!(word(&[4, 4, 4, 0, 0, 0, 0]), None);
}

#[test]
fn mapper_regions_partition_the_16_bit_word_space_without_gaps_or_overlap() {
    let mut next = 0;
    let mut masks = std::collections::HashSet::new();
    for (mask, base) in regions() {
        // A seven-symbol word has zero, one or two flips; each other
        // symbol carries two payload bits. Every region must fit exactly.
        assert!(mask < 128 && mask.count_ones() <= 2);
        assert!(masks.insert(mask));
        assert_eq!(base, next);
        next += 1 << (2 * (7 - mask.count_ones()));
    }
    assert_eq!(next, 1 << 16);
}

#[test]
fn electrical_states_reject_ambiguity_and_nonfinite_samples() {
    for (id, [a, b, c]) in WIRES.into_iter().enumerate() {
        assert_eq!(wire_values(id as u8), [a, b, c]);
        assert_eq!(state(a, b, c, 0.02), id as u8);
        assert_eq!(symbol(id as u8, id as u8), None);
    }
    for values in [
        [f32::NAN, 0.0, 0.2],
        [0.4, f32::INFINITY, 0.0],
        [0.4, 0.0, f32::NEG_INFINITY],
        [0.2, 0.2, 0.4],
        [0.0, 0.2, 0.21],
    ] {
        assert_eq!(state(values[0], values[1], values[2], 0.02), 255);
    }
    // Truth table for low-power wire bits A,B,C.
    for bits in 0..8 {
        let [a, b, c] = [4, 2, 1].map(|mask| if bits & mask != 0 { 1.2 } else { 0.0 });
        assert_eq!(state(a, b, c, 0.02), 8 + bits);
    }
}

#[test]
fn source_and_sampling_errors_have_precise_bounds() {
    let valid = signal(&[(WIRES[0], 8), (WIRES[1], 8), (WIRES[2], 8)]);
    for dt in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let out = decode(&valid[0], &valid[1], &valid[2], dt, None, 0.02);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].kind, out[0].t0, out[0].t1, out[0].ok), (Kind::Field("!SOURCE"), 0.0, 24.0, false));
    }
    for count in [0, 7] {
        let out = decode(&valid[0], &valid[1], &valid[2][..count], 1.0, None, 0.02);
        assert_eq!((out[0].kind, out[0].t1, out[0].ok), (Kind::Field("!SOURCE"), count as f64, false));
    }
    assert_eq!(packets(&signal(&[(WIRES[0], 24)]), None, 0.02)[0].kind, Kind::Field("!SOURCE"));
    let out = packets(&valid, Some(0.5), 0.02);
    assert_eq!((out[0].kind, out[0].t1, out[0].ok), (Kind::Field("!SAMPLE"), 24.0, false));
}

#[test]
fn unaligned_symbols_follow_an_independent_transition_table() {
    // +x -> +y -> -z -> +z -> +x: symbols 2,3,4,2.
    let v = signal(&[(WIRES[0], 8), (WIRES[1], 8), (WIRES[5], 8), (WIRES[2], 8), (WIRES[0], 8)]);
    for (rate, gap) in [(Some(0.125), 0.02), (None, f32::NAN), (Some(f64::NAN), -0.02), (Some(-1.0), 0.02)] {
        let out = packets(&v, rate, gap);
        assert_eq!(out.len(), 4);
        for (k, (p, value)) in out.iter().zip([2, 3, 4, 2]).enumerate() {
            assert_eq!((p.kind, p.value, p.ok), (Kind::Field("SYM"), value, true));
            assert_eq!((p.t0, p.t1), ((k + 1) as f64 * 8.0, (k + 2) as f64 * 8.0));
        }
    }
}

#[test]
fn low_power_patterns_reset_history_and_report_the_wire_bits() {
    for bits in 0..8 {
        let lp = [4, 2, 1].map(|mask| if bits & mask != 0 { 1.2 } else { 0.0 });
        let v = signal(&[(WIRES[0], 8), (WIRES[1], 8), (lp, 8), (WIRES[2], 8), (WIRES[0], 8)]);
        let out = packets(&v, None, 0.02);
        let name = ["!LP000", "!LP001", "!LP010", "!LP011", "!LP100", "!LP101", "!LP110", "!LP111"][bits];
        assert_eq!(out.len(), 3);
        assert_eq!((out[1].kind, out[1].t0, out[1].t1, out[1].ok), (Kind::Field(name), 16.0, 24.0, true));
        assert_eq!((out[2].kind, out[2].value, out[2].t0), (Kind::Field("SYM"), 2, 32.0));
    }
}

#[test]
fn short_glitches_preserve_state_but_long_invalid_gaps_break_history() {
    let short = signal(&[(WIRES[0], 8), (WIRES[1], 1), (WIRES[0], 8), (WIRES[1], 8), (WIRES[2], 8)]);
    let out = packets(&short, Some(0.125), 0.02);
    assert_eq!(
        out.iter().map(|p| (p.kind, p.value, p.t0)).collect::<Vec<_>>(),
        [(Kind::Field("SYM"), 2, 17.0), (Kind::Field("SYM"), 2, 25.0)]
    );
    let long = signal(&[(WIRES[0], 8), ([f32::NAN; 3], 8), (WIRES[1], 8), (WIRES[2], 8)]);
    let out = packets(&long, Some(0.125), 0.02);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].kind, out[0].value, out[0].t0), (Kind::Field("SYM"), 2, 24.0));
}

#[test]
fn invalid_aligned_word_is_reported_without_inventing_data() {
    let mut symbols = vec![3; 12];
    symbols.extend([3, 4, 4, 4, 4, 4, 3]);
    symbols.extend([0, 4, 4, 4, 0, 0, 0]);
    let runs: Vec<_> = states_for_symbols(0, &symbols).into_iter().map(|s| (WIRES[s as usize], 8)).collect();
    let out = packets(&signal(&runs), Some(0.125), 0.02);
    assert!(out.iter().any(|p| p.kind == Kind::Field("SYNC")));
    let code = out.iter().find(|p| p.kind == Kind::Field("!CODE")).expect("three flips cannot encode a word");
    assert_eq!((code.value, code.ok, code.t0, code.t1), (0, false, 160.0, 216.0));
    assert!(!out.iter().any(|p| p.kind == Kind::Word { cmd: false }));
}

#[test]
fn run_limit_reports_where_processing_stops() {
    let wires: [Vec<f32>; 3] = std::array::from_fn(|wire| (0..1_000_001).map(|k| WIRES[k % 2][wire]).collect());
    let out = packets(&wires, None, 0.02);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].kind, out[0].t0, out[0].t1, out[0].ok), (Kind::Field("!LIMIT"), 0.0, 1_000_000.0, false));
}

#[test]
fn packet_limit_preserves_a_bounded_prefix_and_marks_the_remainder() {
    let wires: [Vec<f32>; 3] = std::array::from_fn(|wire| (0..750_003).map(|k| WIRES[(k / 3) % 2][wire]).collect());
    let out = packets(&wires, Some(1.0 / 3.0), 0.02);
    assert_eq!(out.len(), 250_000);
    assert!(out[..249_999].iter().enumerate().all(|(k, p)| {
        p.kind == Kind::Field("SYM")
            && p.value == if k % 2 == 0 { 2 } else { 0 }
            && p.ok
            && p.t0 == (k + 1) as f64 * 3.0
            && p.t1 == (k + 2) as f64 * 3.0
    }));
    let last = out.last().expect("limit marker follows the bounded symbol prefix");
    assert_eq!((last.kind, last.t0, last.t1, last.ok), (Kind::Field("!LIMIT"), 750_000.0, 750_003.0, false));
}
