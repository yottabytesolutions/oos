// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::CodeLayout;
use crate::fastdraw::CorrectedCodes;
use oos_hal::dho::seq;

const CALIBRATION: CorrectedCodes = CorrectedCodes { offsets: [0; 8], scale: 1.0, offset: 0.0 };

fn short_projection(model: &Model, dt: f64, window_samples: usize) -> Projection {
    let window_samples = window_samples as f64;
    let mut projection = projection(model, 1200, 64, 1200.0 / window_samples);
    projection.time_per_div = window_samples * dt / f64::from(HDIV);
    projection
}

#[test]
fn rounded_voltage_can_fire_where_the_inverted_code_threshold_cannot() {
    // Three samples is the shortest search window accepted by one-sample smoothing.
    let words = [0, 32769, 32769];
    let level = CALIBRATION.volts(words[1]);
    let model = Model { trig_level: level, ..Model::default() };
    let frame = Frame {
        pre: 2,
        dt: 0.8e-9,
        ch: [words.map(|word| CALIBRATION.volts(word)).to_vec(), Vec::new(), Vec::new(), Vec::new()],
        ..Frame::default()
    };
    let projection = short_projection(&model, frame.dt, 16);
    let (position, outcome) = trigger(&model, &frame, &projection, None, None);
    assert_eq!(position.to_bits(), 1.0f64.to_bits());
    assert_eq!(outcome, kernel::TrigOutcome::Moved(1.0));

    let threshold = 32768.0 + f64::from(level) * 7500.0;
    assert!(threshold > 32769.0 && threshold < 32770.0);
    assert!(words.into_iter().all(|word| f64::from(word) < threshold));
}

#[test]
fn code_fraction_differs_even_with_an_exactly_representable_threshold() {
    let words = [24001, 35021, 35021];
    let model = Model { trig_level: 0.0, ..Model::default() };
    let frame = Frame {
        pre: 1,
        dt: 0.8e-9,
        ch: [words.map(|word| CALIBRATION.volts(word)).to_vec(), Vec::new(), Vec::new(), Vec::new()],
        ..Frame::default()
    };
    let projection = short_projection(&model, frame.dt, 16);
    let (position, outcome) = trigger(&model, &frame, &projection, None, None);
    let [before, after, _] = words.map(|word| CALIBRATION.volts(word));
    let voltage_fraction = f64::from(-before) / f64::from(after - before);
    let code_fraction = f64::from(32768 - words[0]) / f64::from(words[1] - words[0]);
    assert!(matches!(outcome, kernel::TrigOutcome::Moved(_)));
    assert_eq!(position.to_bits(), voltage_fraction.to_bits());
    assert_ne!(position.to_bits(), code_fraction.to_bits());
}

const POINT_COUNT: usize = 512;

fn samples_at(i: usize) -> [u16; 6] {
    let low = 24001;
    let high = 41001;
    let noise = [32100, 33400, 32000, 33500];
    [
        if i < 256 { low } else { high },
        if i < 256 { high } else { low },
        match i {
            0..252 => low,
            252..260 => noise[i % noise.len()],
            _ => high,
        },
        32768,
        if i == 0 { 0 } else { u16::MAX },
        if i + 1 == POINT_COUNT { u16::MAX } else { 0 },
    ]
}

#[test]
fn screen_refinement_has_identical_bits_when_the_shared_search_reads_codes() {
    let cases = [
        ("rising", 256, Slope::Rise),
        ("falling", 256, Slope::Fall),
        ("noisy", 256, Slope::Rise),
        ("no edge", 256, Slope::Rise),
        ("first pair, clipped", 0, Slope::Rise),
        ("last pair, clipped", POINT_COUNT - 1, Slope::Rise),
    ];
    for (case, (name, pre, slope)) in cases.into_iter().enumerate() {
        for mask in [1u32, 2, 4, 8, 10, 13, 15] {
            let map = seq::code_map(mask, [(1.0, 0.0); NCH]);
            let layout = CodeLayout::uncorrected(map.ratio, map.word, map.affine, seq::volt_code);
            for (source, &word) in map.word.iter().enumerate() {
                let Some(word) = word else { continue };
                check_source(case, (name, pre, slope), (source, word), (mask, layout));
            }
        }
    }
}

fn check_source(
    case: usize,
    (name, pre, slope): (&str, usize, Slope),
    (source, word): (usize, usize),
    (mask, layout): (u32, CodeLayout),
) {
    let bytes: Vec<_> = (0..POINT_COUNT)
        .flat_map(|i| {
            (0..layout.stride)
                .flat_map(move |lane| (if lane == word { samples_at(i)[case] } else { 32768 }).to_le_bytes())
        })
        .collect();
    let frame = Frame { pre, dt: 0.8e-9, ch: seq::decode_volts(&bytes, mask, [(1.0, 0.0); NCH]), ..Frame::default() };
    let codes = layout.channel(&bytes, source);
    for coupling in [TrigCoup::Dc, TrigCoup::Ac, TrigCoup::LfReject, TrigCoup::HfReject] {
        let model = Model { trig_src: source, trig_coup: coupling, trig_level: 0.0, slope, ..Model::default() };
        // A screen window wider than this capture accepts crossings at either end.
        let projection = short_projection(&model, frame.dt, POINT_COUNT * 2);
        let window = projection.time_per_div * f64::from(HDIV);
        let geometry = (POINTS, window, projection.delay - window / 2.0);
        let from_codes = record::smoothed_edge(&frame, &model, &codes, POINT_COUNT, geometry, false);
        let (legacy, outcome) = trigger(&model, &frame, &projection, None, None);
        let expected = (outcome != kernel::TrigOutcome::NotFound).then_some(legacy);
        assert_eq!(
            from_codes.map(f64::to_bits),
            expected.map(f64::to_bits),
            "{name}, mask {mask:#x}, channel {source}, {coupling:?}"
        );
        assert_eq!(
            from_codes.is_some(),
            case != 3 || coupling != TrigCoup::Dc,
            "{name}, mask {mask:#x}, channel {source}, {coupling:?}"
        );
    }
}
