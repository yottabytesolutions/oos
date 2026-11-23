// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const DENSITIES: [f64; 5] = [0.3, 0.5, 0.76, 1.0, 1.5];
const WIDTH: usize = 96;
const COUNT: usize = 21;
const PULSE: usize = 10;
const BASE: u16 = 9000;
const PEAK: u16 = 42_000;

#[test]
fn sparse_ramp_and_square_wave_cover_every_column_between_pair_centres() {
    compare_signal::<1>(|i| [BASE + i as u16 * 900, BASE + i as u16 * 900 + 300], false);
    compare_signal::<2>(|i| [if (i / 4).is_multiple_of(2) { BASE } else { PEAK }; 2], false);
}

#[test]
fn sparse_one_pair_pulse_keeps_its_height_and_stays_in_its_own_column() {
    compare_signal::<4>(|i| [BASE, if i == PULSE { PEAK } else { BASE }], true);
}

#[test]
fn sparse_pair_bridges_reuse_warm_buffers() {
    let record = Record::new(0b11, vec![[BASE, PEAK]; 512], Some([[1, -1]; PHASES]));
    let projection = record.projection(WIDTH, 480, 1.52, 48.0);
    let inputs = std::array::from_fn(|c| record.codes(c).map_or(SamplesRef::Volts(&[]), SamplesRef::Codes));
    for layout in [Some(Layout::Two(record.lanes)), None] {
        let mut scratch = Scratch::default();
        let mut spans = Default::default();
        scratch.pairs((&inputs, layout), &projection, 256.0, &mut spans);
        let (allocations, ()) = crate::system::test_allocator::count_own(|| {
            for ti in [250.0, 123.45, 511.5, 3.0] {
                scratch.pairs((&inputs, layout), &projection, ti, &mut spans);
                std::hint::black_box(&spans);
            }
        });
        assert_eq!(allocations, 0);
    }
}

fn compare_signal<const N: usize>(signal: impl Fn(usize) -> [u16; 2], pulse: bool) {
    let words: Vec<_> = (0..COUNT).flat_map(signal).map(|code| [code; N]).collect();
    let correction = Correction::new([[0; N]; PHASES]);
    let lanes = LaneMap::new(std::array::from_fn(|c| (c < N).then_some(c)), (1 << N) - 1).unwrap();
    for density in DENSITIES {
        let map = Map { ti: 0.25, x_per_sample: 0.5 / density, x0: 2.125, y_per_volt: 1.0, y0: 0.0 };
        let centre = |pair: usize| (((2 * pair + 1) as f64 - map.ti) * map.x_per_sample + map.x0).floor() as usize;
        let mut edges = ColumnEdges::default();
        edges.update_for(WIDTH, words.len(), &map, true);
        let ranges = edges.ranges();
        let mut legacy = Envelopes::with_capacity(WIDTH);
        legacy.columns.extend(ranges.first.array_windows().map(|&[a, b]| scalar::reduce(&words, &correction, a..b)));
        let mut scalar = Envelopes::with_capacity(WIDTH);
        scalar.columns.extend_from_slice(&legacy.columns);
        adapt::pairs::join_codes(&words, &correction, &edges, (false, 0.5), &mut legacy);
        adapt::pairs::join_codes(&words, &correction, &edges, (false, map.x_per_sample), &mut scalar);
        let mut current = Envelopes::with_capacity(WIDTH);
        reduce_columns(&words, &correction, (ranges.c0, &ranges.first, false), &lanes, &mut current);
        adapt::pairs::join_codes(&words, &correction, &edges, (false, map.x_per_sample), &mut current);
        assert_eq!(current.columns, scalar.columns, "{density} pairs per column");
        if density >= 1.0 {
            assert_eq!(current.columns, legacy.columns, "{density} pairs per column");
        } else {
            assert!(legacy.columns.contains(&Extrema::EMPTY));
        }
        assert_eq!(ranges.c0, centre(0));
        assert_eq!(ranges.c0 + current.columns.len() - 1, centre(COUNT - 1));
        for (i, (span, old)) in current.columns.into_iter().zip(legacy.columns).enumerate() {
            let column = ranges.c0 + i;
            assert_ne!(span, Extrema::EMPTY, "column {column} at {density} pairs per column");
            if old != Extrema::EMPTY {
                assert_eq!(span, old, "occupied column {column} at {density} pairs per column");
            }
            if pulse {
                assert_eq!(span.low, [BASE; N]);
                assert_eq!(span.high, [if column == centre(PULSE) { PEAK } else { BASE }; N]);
            }
        }
    }
}
