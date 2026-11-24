// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{board::dho::hw::codes_into, fastdraw::Spans, picture::deposit::standard};
use oos_hal::dho::pairs;

#[test]
fn modeled_capture_peaks_match_the_full_rate_column_envelope() {
    let model = picture::model(1e-6);
    let full = Settings { tdiv: 1e-6, depth: 14_064, ..settings() };
    let (_, full_params) = seq::State::handover(1, None).configure(&full, None, None).unwrap();
    let mut reduced = full.clone();
    let peak = reduced.reduce_capture(true, 2, true).unwrap();
    for phase in 0..4 {
        let words = fixture(full_params, phase);
        let peaks = pairs::model(&words, 2, pairs::Layout { order: pairs::Order::MinFirst, ..pairs::LAYOUT });
        assert_eq!(peaks.len(), peak.memsel as usize);
        let raw = projection(&model, (&full, &full_params), &words, (1.0 / 1.25e9, 7032, None));
        let pairs = projection(&model, (&reduced, &peak), &peaks, (1.0 / 625e6, 3516, Some(3516.0)));
        within_pixel(&raw, &pairs);
        within_pixel(&pairs, &raw);
    }
}

fn fixture(p: hz::Params, phase: usize) -> Vec<u16> {
    // A square edge at the trigger and single-sample glitches at four phases of a peak block.
    let pre = p.pre as usize;
    (0..p.memsel as usize)
        .map(|i| {
            if i == pre - 1683 + phase {
                48_000
            } else if i == pre + 1687 + phase {
                18_000
            } else if i < pre {
                29_000
            } else {
                36_000
            }
        })
        .collect()
}

fn projection(
    model: &crate::model::Model,
    (s, p): (&Settings, &hz::Params),
    words: &[u16],
    (dt, pre, trigger): (f64, usize, Option<f64>),
) -> Spans {
    let bytes: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let mut frame = crate::board::Frame::default();
    let codes = codes_into(&bytes, s, p, 1, &mut frame).unwrap();
    if super::super::super::raw::is_reduced(s) {
        super::super::super::raw::peak_word_axis(&mut frame, s, p);
    }
    assert_eq!((frame.dt, frame.pre, frame.fixed_ti), (dt, pre, trigger));
    // Compare the envelopes at the same hardware edge, independently of software edge refinement.
    frame.fixed_ti = trigger.or(Some(pre as f64));
    let planned = standard::batch_plan(model, (1000, 448), &frame, Some((&bytes, codes)), None);
    let (_, trace) = planned.plan.raw().unwrap();
    let spans = trace.spans[0].as_ref().unwrap();
    Spans { c0: spans.c0, rows: spans.rows.clone() }
}

fn within_pixel(reference: &Spans, candidate: &Spans) {
    assert!(!reference.rows.is_empty());
    for (i, &(lo, hi)) in reference.rows.iter().enumerate() {
        let col = reference.c0 + i;
        for row in lo..=hi {
            assert!(
                (col.saturating_sub(1)..=col + 1).any(|neighbor| {
                    let Some((lo, hi)) = neighbor.checked_sub(candidate.c0).and_then(|i| candidate.rows.get(i)) else {
                        return false;
                    };
                    *lo <= *hi && (*lo - 1..=*hi + 1).contains(&row)
                }),
                "column {col}, row {row}, reference {lo}..={hi}, candidate spans {:?}",
                [-2, -1, 0, 1, 2].map(|dx| {
                    col.checked_add_signed(dx)
                        .and_then(|c| c.checked_sub(candidate.c0))
                        .and_then(|i| candidate.rows.get(i))
                })
            );
        }
    }
}
