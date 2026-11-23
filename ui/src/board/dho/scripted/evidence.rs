// SPDX-License-Identifier: GPL-2.0-or-later

//! Host square-wave read-outs using the production settings, decimation and batch read plan.

use super::*;

pub(crate) fn draw_route_batch(
    m: &Model,
    sw: &crate::sysconf::Switches,
    frames: usize,
) -> (ScriptBatch, usize, String) {
    let s = hw::settings(m, false, sw);
    let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, seq::tdiv_fs(s.delay));
    let mut b = hw::batch_plan(&s, &p, sw.board.zone_batch);
    // Synthetic words have no FPGA header. All sample geometry comes from the production plan.
    b.header = false;
    let frames = if frames == 0 { b.frames as usize } else { frames };
    let fb = seq::frame_bytes(&b, &p);
    let map = seq::code_map(b.mask, b.ch.map(|c| (c.scale, c.offset)));
    let k = seq::compress_k(&b, &p) as usize;
    let points = fb / (2 * map.ratio);
    let mut bytes = Vec::with_capacity(fb * frames);
    for _ in 0..frames {
        for point in 0..points {
            let sample = if k > 1 { point / 2 * 2 * k } else { point };
            let t = (sample as f64 - p.pre as f64) / p.rate as f64;
            let high = (t * 100_000.0).rem_euclid(1.0) < 0.5;
            let code: u16 = if high { 40_000 } else { 25_000 };
            for _ in 0..map.ratio {
                bytes.extend_from_slice(&code.to_le_bytes());
            }
        }
    }
    let plan = format!("rate={} depth={} k={} frame_bytes={} plan_frames={}", p.rate, p.memsel, k, fb, b.frames);
    (
        ScriptBatch {
            bytes: ScriptBytes(bytes),
            fb,
            frames,
            settings: b,
            params: p,
            first: 1,
            codes: true,
            kind: BatchKind::Codes,
            tdiv: m.time_per_div() as f64,
        },
        frames,
        plan,
    )
}

/// One actual production-planned cohort and its original selected indices. Headerless synthetic words only.
pub(crate) fn planned_picture_cohort(
    m: &Model,
    sw: &crate::sysconf::Switches,
) -> (ScriptBatch, std::ops::Range<usize>, bool, String) {
    let (input, count, plan) = draw_route_batch(m, sw, 0);
    let (s, p) = (&input.settings, &input.params);
    use crate::board::dho::switches::FrameZero;
    // This synthetic cohort starts without a preceding arm.
    let policy = FrameZero::from(sw.board.frame0 != FrameZero::Keep);
    let selection =
        super::super::frame1::dma_selection((&input.bytes.0, input.fb, count), Some(&input.newest(0)), policy, (s, p));
    let newest = !selection.range.is_empty();
    (input, selection.range, newest, plan)
}
