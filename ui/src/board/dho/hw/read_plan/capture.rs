// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Capture mode and the live compression requested for the configured record.

use crate::{board::Slow, model::*, sysconf::Switches};
use oos_hal::dho::{
    acq::Mode,
    hz,
    seq::{self, Settings},
};

#[cfg(test)]
use super::super::roll::slow_mode;

/// Select capture mode and any live compression before configuring the hardware.
pub(in super::super) fn configure_capture(m: &Model, sw: &Switches, slow: Slow, s: &mut Settings) {
    if let Some(count) = hw_avg_count(m, slow, s) {
        s.mode = Mode::Average { count };
    }
    capture_reduction(m, sw, slow, s);
    if sw.compress {
        apply_live_compression(m, slow, s);
    }
}

fn capture_reduction(m: &Model, sw: &Switches, slow: Slow, s: &mut Settings) {
    let Some((peak, factor, inferred)) = sw.source.capture().reduction() else { return };
    if m.acq == AcqMode::Normal
        && slow == Slow::Off
        && m.mdepth == 0
        && m.trig_type == TrigType::Edge
        && m.segments == 0
        && !m.mask_on
        && !m.zones.iter().any(|z| z.usable())
    {
        let _ = s.reduce_capture(peak, factor, inferred);
    }
}

/// The live compression for the model (compress.rs): the factor of `compress::k_for` for the record of `s` where the
/// model allows it (not in roll or scan), else none.
#[cfg(test)]
pub(in crate::board::dho::hw) fn live_compress(m: &Model, s: &mut Settings) {
    apply_live_compression(m, slow_mode(m), s);
}

fn apply_live_compression(m: &Model, slow: Slow, s: &mut Settings) {
    if slow != Slow::Off
        || !crate::compress::allowed(m)
        || s.la.is_some()
        || s.segments.is_some()
        || !matches!(s.mode, Mode::Normal)
    {
        return;
    }
    let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, 0);
    s.compress = crate::compress::k_for(p.rate, s.tdiv, p.memsel);
}

/// Hardware averaging requires a power-of-two count and a triggered record that fits the average ring.
/// Roll, scan, LA and hardware segments use software averaging.
fn hw_avg_count(m: &Model, slow: Slow, s: &Settings) -> Option<u32> {
    if m.acq != AcqMode::Average || !m.avg_hw || slow != Slow::Off || s.la.is_some() || s.segments.is_some() {
        return None;
    }
    let p = hz::avg_params(hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, 0));
    oos_hal::dho::avg::hw_count(m.avg_count).filter(|_| p.max_frames > 0)
}
