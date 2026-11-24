// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::hw::decode_with;
use crate::board::dho::switches::HeaderProbe;
use oos_hal::dho::pairs::{self, Layout, Order};

#[test]
fn flat_peak_envelopes_match_matched_references_within_one_adc_code() {
    for (factor, pattern) in [(2, [-13, 30, -24, 7, -13, 30, -24, 7]), (4, [-13, 29, -42, 7, 31, -10, -3, 1])] {
        let full = Settings { tdiv: 1e-6, depth: 14_064, window: None, ..Settings::default() };
        let full_params = hz::prep(full.mask, seq::tdiv_fs(full.tdiv), full.depth, 0);
        let one_code = full.ch[0].scale / 7500.0;
        let mut peak = full.clone();
        let p = peak.reduce_capture(true, factor, true).unwrap();
        for order in [Order::MinFirst, Order::MaxFirst, Order::Time] {
            let words: Vec<_> = (0..full_params.memsel as usize).map(|i| (33_000 + pattern[i % P]) as u16).collect();
            let as_bytes = |words: &[u16]| words.iter().flat_map(|word| word.to_le_bytes()).collect::<Vec<_>>();
            let mut cores = Cores::default();
            let reference = decode_with(&as_bytes(&words), &full, &full_params, 1, HeaderProbe::Off, Some(&mut cores));
            let peaks = pairs::model(&words, factor as usize, Layout { order, ..pairs::LAYOUT });
            let mut bytes = as_bytes(&peaks[..p.memsel as usize]);
            let (allocations, uncorrected) = crate::system::test_allocator::count_own(|| {
                correct(&mut bytes, seq::frame_bytes(&peak, &p), &peak, &p, &cores)
            });
            assert_eq!((allocations, uncorrected), (0, false));
            let reduced = decode_with(&bytes, &peak, &p, 2, HeaderProbe::Off, None);
            let envelope = |frame: &crate::board::Frame| {
                let (lo, hi) = frame.ch[0]
                    .iter()
                    .copied()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
                [f64::from(lo), f64::from(hi)]
            };
            for (expected, actual) in envelope(&reference).into_iter().zip(envelope(&reduced)) {
                assert!((expected - actual).abs() <= one_code);
            }
        }
    }
}

#[test]
fn short_peak_blocks_have_a_bounded_flat_input_residual_when_phase_is_unknown() {
    let pattern = [-90, 0, 90, 0, -10, 0, 10, 0];
    assert_eq!(pair_offsets(&pattern, 8), [-90, 90]);
    assert_eq!(pair_offsets(&pattern, 4), [-50, 50]);
}
