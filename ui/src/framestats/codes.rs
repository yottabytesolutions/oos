// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::CodeLayout;
use crate::fastdraw::Samples;

pub(crate) fn requires_frame(m: &Model, frame: &Frame, folds: &Folds) -> bool {
    !folds.is_empty()
        && (m.trig_type != TrigType::Edge
            || m.trig_on_la()
            || frame.fixed_ti.is_some()
            || folds.iter().any(|&((kind, _), _)| needs_samples(kind)))
}

/// The record trigger has a second comparator path when smoothing finds no DC edge.
/// In that case ask the caller for a decoded frame so its fallback stays unchanged.
pub(crate) fn fold_codes(f: &Frame, m: &Model, bytes: &[u8], layout: CodeLayout, folds: &mut Folds) -> Option<bool> {
    let src = layout.channel(bytes, m.trig_src);
    let len = layout
        .word
        .iter()
        .enumerate()
        .filter_map(|(channel, word)| word.map(|_| layout.channel(bytes, channel).n))
        .max()
        .unwrap_or(0);
    if len <= crate::sparse::POINTS {
        return Some(false);
    }
    let window = m.time_per_div() as f64 * HDIV as f64;
    let t_first = m.delay as f64 - window / 2.0;
    if src.n < 2 {
        return None;
    }
    let ti = crate::board::record::smoothed_edge(f, m, &src, len, (crate::sparse::POINTS, window, t_first), false)?;
    let (i0, i1) = fold_window(f.dt, window, t_first, ti, len);
    if i1.saturating_sub(i0) <= crate::sparse::POINTS {
        return Some(false);
    }
    let mut cache: [Option<(f32, f32)>; NCH] = Default::default();
    Some(push_measurements(folds, |kind, channel| {
        let codes = layout.channel(bytes, channel);
        if i1 > codes.n {
            return None;
        }
        let (lo, hi) = *cache[channel].get_or_insert_with(|| codes.extremes(i0, i1));
        Some(match kind {
            Meas::Vmax => hi as f64,
            Meas::Vmin => lo as f64,
            Meas::Vpp => hi as f64 - lo as f64,
            _ => return None,
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_extrema_folds_equal_the_decoded_trigger_window() {
        let cal =
            crate::fastdraw::CorrectedCodes { offsets: [100, -50, 0, 0, 120, -85, 0, 0], scale: 1.25, offset: 0.375 };
        let m = Model {
            tscale: tscales().iter().position(|&t| t == 1e-6).unwrap(),
            meas: [Meas::Vmin, Meas::Vmax, Meas::Vpp].map(|kind| (kind, crate::model::WaveId::Ch(0))).to_vec(),
            ..Default::default()
        };
        let mut layout =
            CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); NCH], |code, _| f32::from(code));
        layout.corrected = Some([cal; NCH]);
        let len = 14064;
        let bytes: Vec<u8> = (0..len)
            .flat_map(|i| {
                let code: u16 = if i < len / 2 { 24000 } else { 45000 };
                (code + (i % 41) as u16).to_le_bytes()
            })
            .collect();
        let codes = layout.channel(&bytes, 0);
        let mut frame = Frame { dt: 0.8e-9, pre: len / 2, ..Default::default() };
        let (mut got, mut expected) = (folds_for(&m), folds_for(&m));
        assert!(!requires_frame(&m, &frame, &got));
        assert_eq!(fold_codes(&frame, &m, &bytes, layout, &mut got), Some(true));
        frame.ch[0].extend((0..len).map(|i| codes.at(i)));
        assert!(fold_frame(&frame, &m, &mut expected));
        assert_eq!(got, expected);
    }

    #[test]
    fn integer_moments_lose_the_rounded_voltage_mean() {
        let calibration = crate::fastdraw::CorrectedCodes { offsets: [0; 8], scale: 1.25, offset: 0.375 };
        let a = [0u16, 1, 4, 4, 6];
        let b = [0u16, 2, 2, 5, 6];
        let moments = |codes: [u16; 5]| {
            let codes = codes.map(u64::from);
            (codes[0], codes[4], codes.iter().sum::<u64>(), codes.iter().map(|code| code * code).sum::<u64>())
        };
        assert_eq!(moments(a), moments(b));
        let avg = |codes: [u16; 5]| Values::of(&codes.map(|code| calibration.volts(code)), false).unwrap().avg;
        assert_ne!(avg(a).to_bits(), avg(b).to_bits());
    }
}
