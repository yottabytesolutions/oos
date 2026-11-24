// SPDX-License-Identifier: GPL-2.0-or-later

//! Raw reads decoded off the shared lock (upstream 3930159f).

use super::{batch, decode_into_with};
use crate::board::Frame;
use oos_hal::dho::{hz, seq::Settings};

/// Decode a raw replay using the current batch’s ADC matching decision.
pub(super) fn decode_raw(
    bytes: &[u8],
    (s, p): (&Settings, &hz::Params),
    probe: crate::board::dho::switches::HeaderProbe,
    cores: Option<&mut crate::board::dho::cores::Cores>,
    charge: Option<&std::sync::Arc<crate::drawpipe::TapReservation>>,
) -> Option<batch::DecodedRecord> {
    let mut frame = if let Some(charge) = charge {
        let storage = batch::storage(s, bytes.len())?;
        if !charge.covers(&storage) {
            return None;
        }
        storage
    } else {
        Frame::default()
    };
    let uncorrected = decode_into_with(bytes, s, p, 0, &mut frame, probe, cores);
    if let Some(charge) = charge {
        frame.basic.retain(charge.clone());
    }
    Some((frame, uncorrected))
}

pub(super) fn replay_due(count: u32, live: bool, last: &mut Option<std::time::Instant>) -> bool {
    if live && count > 0 && last.is_none_or(|t| t.elapsed() >= crate::compress::RAW_PERIOD) {
        *last = Some(std::time::Instant::now());
        return true;
    }
    false
}
