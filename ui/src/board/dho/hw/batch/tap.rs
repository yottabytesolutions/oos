// SPDX-License-Identifier: GPL-2.0-or-later
//! Captured waveform storage, independent of the visible plot's sample count.

use super::*;

/// Allocate a decode target, then inspect the allocator's real capacities before reservation.
pub(super) fn storage(s: &Settings, fb: usize) -> Option<Frame> {
    let mut storage = Frame::default();
    let payload = fb.checked_sub(if s.header { 16 } else { 0 })?;
    let map = oos_hal::dho::seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    for (c, channel) in storage.ch.iter_mut().enumerate() {
        if let Some(word) = map.word[c].filter(|&word| word < payload / 2) {
            channel.try_reserve_exact((payload / 2 - word).div_ceil(map.ratio)).ok()?;
        }
    }
    Some(storage)
}

/// Reserve before matcher learning/newest decode. Earlier raw replay and later LA tagging
/// remain outside this seam until F3 owns their source reservation.
pub(super) fn reserve(
    pipe: &Pipe,
    s: &Settings,
    fb: usize,
    raw: Option<&Frame>,
    deposit: Option<Identity>,
) -> Result<(Frame, Option<std::sync::Arc<crate::drawpipe::TapReservation>>), crate::drawpipe::Lost> {
    use crate::drawpipe::Lost;
    let Some(identity) = deposit.filter(|_| s.la.is_none() && pipe.accepting()) else {
        return Ok((Frame::default(), None));
    };
    let storage = if raw.is_some() { Frame::default() } else { storage(s, fb).ok_or(Lost::Error)? };
    let reservation = pipe.reserve_waveform(raw.unwrap_or(&storage), identity).ok_or(Lost::Settings)?;
    Ok((storage, Some(reservation)))
}
