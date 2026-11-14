// SPDX-License-Identifier: GPL-2.0-or-later

//! Roll stream control and bounded snapshot publication; scan retains complete sweeps.

use crate::board::{Frame, Shared};
use crate::model::NCH;

/// Whether roll may poll the stream; a stopped or rearmed stream starts with an empty screen.
pub(crate) fn roll_run(
    g: &mut Shared,
    rolling: &mut Option<oos_hal::dho::roll::Params>,
    ring: &mut [std::collections::VecDeque<f32>; NCH],
    ring_la: &mut std::collections::VecDeque<u16>,
) -> bool {
    if g.hold || std::mem::take(&mut g.rearm) {
        *rolling = None;
        ring.iter_mut().for_each(|r| r.clear());
        ring_la.clear();
        g.queue.clear();
    }
    !g.hold
}

/// A roll picture into the UI queue (also called on ticks without new chunks).
pub(crate) fn publish_roll(g: &mut Shared, f: Frame) {
    // STOP may have arrived during the read; keep the stopped picture and count no publication.
    if g.hold {
        return;
    }
    // Roll pictures overlap: only the newest snapshot is needed, always within QUEUE_MAX.
    g.queue.clear();
    g.queue.push_back(f.clone());
    g.frame = f;
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().acquired, 1);
}

/// Scan retains complete sweeps; roll replaces its overlapping snapshot.
pub(crate) fn publish(g: &mut Shared, f: Frame, scan: bool) {
    if scan {
        // A complete sweep must not be dropped by the next partial picture.
        if g.queue.back().is_none_or(|b| b.partial) {
            g.queue.clear();
        }
        g.queue.push_back(f.clone());
        g.frame = f;
    } else {
        publish_roll(g, f);
    }
}

/// Apply STOP/rearm before reading another roll chunk, releasing Shared before bus IO.
#[cfg(target_os = "linux")]
pub(super) fn poll(
    mode: crate::board::Slow,
    out: &crate::board::HwShared,
    rolling: &mut Option<oos_hal::dho::roll::Params>,
    ring: &mut [std::collections::VecDeque<f32>; NCH],
    ring_la: &mut std::collections::VecDeque<u16>,
    sh: &mut oos_hal::dho::scu::Shadows,
    bar: &mut impl oos_hal::dho::bus::Bus,
) -> bool {
    if mode != crate::board::Slow::Roll {
        return true;
    }
    let was_rolling = rolling.is_some();
    let run = roll_run(&mut oos_hal::sync::lock(out), rolling, ring, ring_la);
    if was_rolling && rolling.is_none() {
        bar.apply(&oos_hal::dho::roll::stop(sh));
    }
    run
}
