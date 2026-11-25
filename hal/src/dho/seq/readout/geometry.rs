// SPDX-License-Identifier: GPL-2.0-or-later

use super::{ReadoutError, COMPRESS_K_MAX};
use crate::dho::{hz, Write};

// record-play.md sections 1.6 and 3.2. The final register values before
// starting the analog play determine the slot range and bytes on the wire. LA capture doubles the
// input window, but a separate play sends the LA words (la.md 4.1-4.3).
fn played_bytes(writes: &[Write], (record, la_enabled): (u32, bool)) -> Option<u64> {
    let [mut channel_select, mut mode, mut first, mut last, mut header_off, mut tx, mut window, mut start, mut compression] =
        [0; 9];
    for &(reg, value) in writes {
        match reg {
            0x4008 => channel_select = value,
            0x400c => mode = value,
            0x4014 => first = value,
            0x4018 => last = value,
            0x1010 => header_off = value >> 31,
            0x11f0 => tx = value,
            0x1100 => window = value,
            0x1108 => start = value,
            0x1170 => compression = value,
            _ => {}
        }
    }
    if channel_select & (1 << 24) != 0 {
        return None;
    }
    let ratio = hz::ratio(channel_select & 0xf);
    // Play-last inherits its processing chain from the handover. The
    // modeled play-last path assumes the documented pass-through setup.
    let slots = if mode >> 29 & 3 == 3 { u64::from(last.checked_sub(first)?) + 1 } else { 1 };
    if tx == 0
        || (compression & (1 << 31) != 0 && !window_fits((window, start), (record, ratio, la_enabled), tx, compression))
    {
        return None;
    }
    let bytes = u64::from(header_off == 0) * 16 + 2 * u64::from(ratio) * u64::from(tx);
    Some(slots * (bytes.div_ceil(8) * 8))
}

pub(in crate::dho) fn window_fits(
    (window, start): (u32, u32),
    (record, ratio, la): (u32, u32, bool),
    tx: u32,
    compression: u32,
) -> bool {
    let factor = compression & COMPRESS_K_MAX;
    // With LA enabled the shared input window uses twice the analog length (la.md 4.3).
    let points = window.saturating_sub(0x48 | (ratio << 9)) / (1 + u32::from(la));
    let full_record = start == 0 && points == record;
    factor != 0
        && tx >= 8
        && tx.is_multiple_of(8)
        && start.is_multiple_of(8)
        && (full_record || points.is_multiple_of(8))
        && points <= record.saturating_sub(start)
        && u64::from(tx) * u64::from(factor) <= u64::from(points)
}

pub(super) fn check(writes: &[Write], record: (u32, bool), requested: usize) -> Result<(), ReadoutError> {
    let played = played_bytes(writes, record);
    if played != Some(requested as u64) {
        return Err(ReadoutError::TransferLength { requested, played });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
