// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Frames 0 and 1 of a batch (issue #5, docs/protocols/acquisition-start.md): which frames of a batch are left out
//! before they reach the record or the deposit workers.

use super::switches::FrameZero;
use crate::board::Frame;
#[cfg(any(target_os = "linux", test))]
use oos_hal::dho::hz;
use oos_hal::dho::seq::Settings;

/// Samples around the trigger point in which frame 1's edge must lie (`off_trigger`); the hardware puts it within a
/// few samples (unit 2026-11-16: +6.5 at 6.25 MSa/s).
const SPAN: usize = 32;

/// Largest record whose frame 1 the batch path decodes for `off_trigger`: one more decode per batch, cheap for the
/// short records batches are made of; a longer record is searched for its edge by the fast path anyway.
#[cfg(any(target_os = "linux", test))]
const CHECK_MAX: u32 = 100_000;

/// Issue #5, the rest: frame 1 of a batch (the first trigger after the arm) is now and then joined with older memory.
/// On the unit (2026-11-16, generator 1 kHz square on CH1, 1 kpts, 10 us/div, edge trigger) 27 of about 12,700
/// batches had no edge of the trigger slope near the trigger point in frame 1, and no other frame ever. True when `f`
/// is such a frame: an edge trigger on a channel with DC or HF reject coupling and no rising (falling, either)
/// crossing of the level within `SPAN` samples of `pre`. Other trigger types cannot be checked this way and keep
/// their frame 1.
pub(super) fn off_trigger(f: &Frame, s: &Settings) -> bool {
    let Some(tc) = trigger_channel(s).filter(|_| !f.envelope) else { return false };
    let v = &f.ch[tc];
    checked(v.len(), f.pre) && missing_edge(v, f.pre, s, tc)
}

fn trigger_channel(s: &Settings) -> Option<usize> {
    use oos_hal::dho::tpu::Coupling;
    (matches!(s.trigger, oos_hal::dho::seq::Trig::Edge)
        && (1..=4).contains(&s.trig_src)
        && matches!(s.trig_couple, Coupling::Dc | Coupling::HfReject))
    .then(|| (s.trig_src - 1) as usize)
}

fn checked(len: usize, pre: usize) -> bool {
    len >= 2 * SPAN && pre < len
}

fn missing_edge(v: &[f32], pre: usize, s: &Settings, tc: usize) -> bool {
    use oos_hal::dho::tpu::Slope;
    let hyst = (0.2 * s.ch[tc].scale) as f32;
    let (rising, either) = (s.slope != Slope::Falling, s.slope == Slope::Either);
    crate::fastdraw::edge_near(v, pre, s.level as f32, hyst, rising, either, SPAN).is_none()
}

#[cfg(any(target_os = "linux", test))]
fn bytes_off_trigger(bytes: &[u8], s: &Settings, p: &hz::Params) -> bool {
    use oos_hal::dho::seq;
    let Some(tc) = trigger_channel(s).filter(|_| super::hw::pair_step(s, p) == 1) else { return false };
    let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    let Some(word) = map.word[tc] else { return false };
    let bytes = super::matching::payload(bytes, s).as_chunks::<2>().0;
    let len = bytes.len().saturating_sub(word).div_ceil(map.ratio);
    let pre = p.pre.saturating_sub(seq::window(s, p).1) as usize;
    if !checked(len, pre) {
        return false;
    }
    // The edge search walks back at most 4096 samples to establish hysteresis.
    const LOOK: usize = 4096;
    let first = pre.saturating_sub(SPAN + LOOK);
    let last = (pre + SPAN + 1).min(len);
    let mut volts = [0.0; LOOK + 2 * SPAN + 1];
    let window = &mut volts[..last - first];
    for (i, v) in window.iter_mut().enumerate() {
        let code = u16::from_le_bytes(bytes[(first + i) * map.ratio + word]);
        *v = seq::volt_code(code, map.affine[tc]);
    }
    missing_edge(window, pre - first, s, tc)
}

/// Counts a frame 1 left out and logs the count at 1, 2, 4, 8, ...
pub(super) fn left_out() {
    let counters = crate::drawpipe::counters();
    let n = counters.lose_frame1();
    if n.is_power_of_two() {
        eprintln!(
            "oos-ui hw: frame 1 of a batch without its edge at the trigger point, left out (issue #5): {n} so far"
        );
    }
}

/// Worker indices close a possible hole at slot one while preserving physical DMA order.
pub(super) struct DmaSelection {
    pub(super) range: std::ops::Range<usize>,
    pub(super) skip_second: bool,
    pub(super) drop_first: bool,
}

pub(super) fn dma_selection(
    (bytes, fb, avail): (&[u8], usize, usize),
    raw: Option<&Frame>,
    policy: FrameZero,
    (s, p): (&Settings, &hz::Params),
) -> DmaSelection {
    let check = avail == 2 || p.memsel <= CHECK_MAX;
    let drop_second =
        check && second_off_trigger(avail, policy, raw, s, || bytes_off_trigger(&bytes[fb..2 * fb], s, p));
    if drop_second {
        left_out();
    }
    let drop_first = avail > 1
        && ((avail == 2 && drop_second)
            || match policy {
                FrameZero::Keep => false,
                FrameZero::Drop => true,
                FrameZero::Steady => check && bytes_off_trigger(&bytes[..fb], s, p),
            });
    let skip_second = !drop_first && drop_second;
    let start = if drop_first { 1 + usize::from(drop_second) } else { 0 };
    let end = avail - usize::from(skip_second);
    DmaSelection { range: start..end, skip_second, drop_first }
}

/// For a two-slot batch, judge slot one by the waveform that would be published.
fn second_off_trigger(
    count: usize,
    policy: FrameZero,
    raw: Option<&Frame>,
    settings: &Settings,
    dma_off_trigger: impl FnOnce() -> bool,
) -> bool {
    count > 1
        && policy != FrameZero::Keep
        && raw.filter(|_| count == 2).map_or_else(dma_off_trigger, |frame| off_trigger(frame, settings))
}

pub(super) fn off_trigger_if_checked(
    (slot, count): (usize, usize),
    policy: FrameZero,
    (frame, raw): (&Frame, Option<&Frame>),
    settings: &Settings,
) -> bool {
    let off = if slot == 0 {
        policy == FrameZero::Steady && off_trigger(frame, settings)
    } else {
        second_off_trigger(count, policy, raw, settings, || off_trigger(frame, settings))
    };
    if !off {
        return false;
    }
    if slot == 0 {
        crate::drawpipe::counters().lose(crate::drawpipe::Lost::Frame0, 1);
    } else {
        left_out();
    }
    true
}

/// The raw newest frame (live compression) stands for the newest record of the batch, under its sequence number.
pub(super) fn raw_newest(all: &mut [Frame], raw: Option<Frame>, (stamp, cfg): (u64, u64)) {
    if let (Some(r), Some(last)) = (raw, all.last_mut()) {
        *last = Frame { stamp, cfg, seq: last.seq, ..r };
    }
}

#[cfg(test)]
mod tests;
