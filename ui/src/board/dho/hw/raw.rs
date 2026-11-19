// SPDX-License-Identifier: GPL-2.0-or-later

//! Raw reads decoded off the shared lock (upstream 3930159f).

use super::{batch, decode_into_with};
use crate::board::{Frame, HwShared};
use oos_hal::dho::{hz, seq::Settings};

/// A raw read (the live-compression newest frame, the deep window: up to ~24 MB) decoded with the ADC cores matched
/// when the UI asks for it. The flag is read first: a guard taken in the argument would hold `Shared` through the
/// whole decode, and the UI waits on it (upstream 3930159f).
pub(super) fn decode_raw(
    bytes: &[u8],
    (s, p): (&Settings, &hz::Params),
    probe: crate::board::dho::switches::HeaderProbe,
    (out, cores): (&HwShared, &mut crate::board::dho::cores::Cores),
    charge: Option<&std::sync::Arc<crate::drawpipe::TapReservation>>,
) -> Option<Frame> {
    let matched = crate::perfguard::deep::acq_lock(out).cores;
    let mut frame = if let Some(charge) = charge {
        let storage = batch::storage(s, bytes.len())?;
        if !charge.covers(&storage) {
            return None;
        }
        storage
    } else {
        Frame::default()
    };
    decode_into_with(bytes, s, p, 0, &mut frame, probe, matched.then_some(cores));
    if let Some(charge) = charge {
        frame.basic.retain(charge.clone());
    }
    Some(frame)
}

pub(super) fn replay_due(count: u32, live: bool, last: &mut Option<std::time::Instant>) -> bool {
    if live && count > 0 && last.is_none_or(|t| t.elapsed() >= crate::compress::RAW_PERIOD) {
        *last = Some(std::time::Instant::now());
        return true;
    }
    false
}
