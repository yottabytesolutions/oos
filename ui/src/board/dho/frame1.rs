// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Frames 0 and 1 of a batch (issue #5, docs/protocols/acquisition-start.md): which frames of a batch are left out
//! before they reach the record or the deposit workers.

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

/// The first frame of a batch of `avail` that the deposit workers and the matcher may use: 0, or 1 with frame 0 left
/// out (`frame0_drop`), or 2 when frame 1 has no edge at the trigger point (checked for records up to `CHECK_MAX`;
/// with two frames the newest is frame 1, which `newest_kept` judges).
#[cfg(any(target_os = "linux", test))]
pub(super) fn first_kept(
    bytes: &[u8],
    fb: usize,
    avail: usize,
    frame0_drop: bool,
    sp: (&Settings, &hz::Params),
) -> usize {
    let (s, p) = sp;
    if !frame0_drop || avail < 2 {
        return 0;
    }
    let off = avail > 2 && p.memsel <= CHECK_MAX && bytes_off_trigger(&bytes[fb..2 * fb], s, p);
    if off {
        left_out();
    }
    1 + usize::from(off)
}

/// Whether the newest frame of a batch of `avail` stays: with two frames it is frame 1, left out without its edge.
#[cfg(any(target_os = "linux", test))]
pub(super) fn newest_kept(
    (raw, bytes): (Option<&Frame>, &[u8]),
    (avail, frame0_drop): (usize, bool),
    (s, p): (&Settings, &hz::Params),
) -> bool {
    let off = avail == 2 && frame0_drop && raw.map_or_else(|| bytes_off_trigger(bytes, s, p), |f| off_trigger(f, s));
    if off {
        left_out();
    }
    !off
}

/// The record path: `all` holds the frames from `skip` on; when only frame 0 was skipped, frame 1 without its edge
/// goes too. How many more were left out.
pub(super) fn drop_frame1(all: &mut Vec<Frame>, skip: usize, frame0_drop: bool, s: &Settings) -> usize {
    if !(frame0_drop && skip == 1 && all.first().is_some_and(|f| off_trigger(f, s))) {
        return 0;
    }
    all.remove(0);
    left_out();
    1
}

/// The raw newest frame (live compression) stands for the newest record of the batch, under its sequence number.
pub(super) fn raw_newest(all: &mut [Frame], raw: Option<Frame>, (stamp, cfg): (u64, u64)) {
    if let (Some(r), Some(last)) = (raw, all.last_mut()) {
        *last = Frame { stamp, cfg, seq: last.seq, ..r };
    }
}

#[cfg(test)]
mod tests;
