// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::{Frame, Gen, HwShared, MaskWant, Slow, Want, ZoneWant, QUEUE_MAX};
use oos_core::change::Identity;
use oos_hal::sync::lock;

type Request = (Option<Gen>, Option<Want>, Slow, u64, Option<MaskWant>, Option<ZoneWant>, bool, Identity, bool);

pub(super) fn snapshot(out: &HwShared) -> Request {
    let g = lock(out);
    (
        g.afg[0],
        g.want.clone(),
        g.slow,
        g.avg_clear,
        g.mask_want.clone(),
        g.zone_want.clone(),
        g.needs_raw,
        g.acquisition.identity(),
        g.acquisition.route(),
    )
}

/// Frames of a batch into the UI queue, the oldest dropped beyond `QUEUE_MAX`.
pub(super) fn push_frames(out: &HwShared, mut frames: Vec<Frame>) {
    if frames.is_empty() {
        return;
    }
    let mut g = lock(out);
    frames.retain(|f| current(&g, f));
    g.queue.extend(frames);
    let over = g.queue.len().saturating_sub(QUEUE_MAX);
    g.queue.drain(..over);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.overflow, over as u64);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, over as u64);
    if let Some(f) = g.queue.back() {
        g.frame = f.clone();
    }
}

pub(super) fn boundary(applied: Option<(u64, u64)>, identity: Identity) -> bool {
    applied != Some((identity.gen(), identity.epoch()))
}

pub(super) fn tag(mut frame: Frame, identity: Identity) -> Frame {
    (frame.cfg, frame.stamp) = (identity.gen(), identity.epoch());
    frame
}

/// Only effective hardware settings or mode changes restart segmented recording and roll.
pub(super) fn reconfigure(current: Option<(&Want, Slow)>, want: &Want, mode: Slow) -> bool {
    current != Some((want, mode))
}

/// Check a producer output under the publication lock, counting identity loss before any handoff.
pub(super) fn current(g: &crate::board::Shared, f: &Frame) -> bool {
    let id = g.acquisition.identity();
    if (f.cfg, f.stamp) == (id.gen(), id.epoch()) {
        return true;
    }
    let counters = crate::drawpipe::counters();
    crate::drawpipe::Counters::add(&counters.losses.worker_push, 1);
    crate::drawpipe::Counters::add(&counters.dropped, 1);
    false
}
