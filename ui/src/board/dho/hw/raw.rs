// SPDX-License-Identifier: GPL-2.0-or-later

//! Raw reads decoded off the shared lock (upstream 3930159f).

use super::decode_with;
use crate::board::{Frame, HwShared};
use oos_hal::dho::{hz, seq::Settings};

/// A raw read (the live-compression newest frame, the deep window: up to ~24 MB) decoded with the ADC cores matched
/// when the UI asks for it. The flag is read first: a guard taken in the argument would hold `Shared` through the
/// whole decode, and the UI waits on it (upstream 3930159f).
pub(super) fn decode_raw(
    bytes: &[u8],
    (s, p): (&Settings, &hz::Params),
    probe: crate::board::dho::switches::HeaderProbe,
    out: &HwShared,
    cores: &mut crate::board::dho::cores::Cores,
) -> Frame {
    let matched = crate::perfguard::deep::acq_lock(out).cores;
    decode_with(bytes, s, p, 0, probe, matched.then_some(cores))
}
