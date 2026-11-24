// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(in crate::board::dho) fn replay(
    (s, p): (&Settings, &hz::Params),
    bytes: &[u8],
    fb: usize,
    records: std::ops::Range<usize>,
    cores: &mut Cores,
) -> (usize, usize) {
    if !eligible(s) {
        return (records.len(), records.len());
    }
    let input = MatchInput::new(s, p);
    let prior = std::array::from_fn::<_, 4, _>(|c| cores.pattern(input.key(c)));
    let prepared = BatchMatch::prepare(bytes, fb, records.clone(), s, p, cores);
    let template: [Option<[f64; P]>; 4] = std::array::from_fn(|c| {
        let (pattern, _) = prior[c].or_else(|| cores.pattern(input.key(c)))?;
        let period = input.period;
        let offsets: [f64; P] = std::array::from_fn(|i| {
            (i % period..P).step_by(period).map(|j| pattern[j]).sum::<f64>() / (P / period) as f64
        });
        // Below two codes the captured patterns move fewer than one in ten thousand rows.
        (offsets.iter().any(|x| x.abs() >= 2.0)).then_some(offsets)
    });
    let mut total = 0;
    let mut same = 0;
    for index in records {
        let raw = &bytes[index * fb..(index + 1) * fb];
        let lanes = seq::deinterleave(payload(raw, s), s.mask);
        let expected: [Option<Correction>; 4] = std::array::from_fn(|c| oracle(&lanes[c], template[c]?, input.period));
        let observed = prepared.correction(index).copied().unwrap_or_default();
        if expected.iter().any(Option::is_some) {
            total += 1;
            same += usize::from(lanes.iter().zip(expected).zip(observed.channels).all(|((lane, expected), got)| {
                expected.is_none_or(|want| {
                    got.is_some_and(|got| {
                        (0..P.min(lane.len())).all(|i| want.word(i, lane[i]) == got.word(i, lane[i]))
                            && want.offsets() == got.offsets()
                    })
                })
            }));
        }
    }
    (same, total)
}

/// Maximum likelihood over the full Hann-weighted record against the batch's template.
fn oracle(lane: &[u16], template: [f64; P], period: usize) -> Option<Correction> {
    let (means, _) = super::super::super::cores::estimate(lane)?;
    let means: [f64; P] =
        std::array::from_fn(|i| (i % period..P).step_by(period).map(|j| means[j]).sum::<f64>() / (P / period) as f64);
    let mut scores: Vec<_> =
        (0..period).map(|turn| (turn, (0..P).map(|i| means[i] * template[(i + turn) % P]).sum::<f64>())).collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let [(turn, best), (_, next), ..] = scores[..] else { return None };
    let ea = means.iter().map(|x| x * x).sum::<f64>();
    let eb = template.iter().map(|x| x * x).sum::<f64>();
    let norm = (ea * eb).sqrt();
    if best <= 0.8 * norm || best - next <= 0.025 * norm {
        return None;
    }
    // Check the rotation with a different signal-rejection filter. At a core frequency the
    // waveform itself can dominate either estimate, especially in a short record.
    let mut difference = [0.0; P];
    for (i, triple) in lane.windows(3).enumerate() {
        let [a, b, c] = [triple[0], triple[1], triple[2]].map(f64::from);
        difference[(i + 1) % period] += 2.0 * b - a - c;
    }
    let filtered =
        |i: usize| 2.0 * template[i % period] - template[(i + period - 1) % period] - template[(i + 1) % period];
    let confirm = |k| (0..period).map(|i| difference[i] * filtered(i + k)).sum::<f64>();
    (0..period).filter(|&k| k != turn).all(|k| confirm(turn) > confirm(k)).then(|| Correction::new(&template, turn))
}
