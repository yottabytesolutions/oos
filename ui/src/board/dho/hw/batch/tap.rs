// SPDX-License-Identifier: GPL-2.0-or-later
//! Captured waveform storage and admission at the actual board read boundary.

use super::*;
use std::time::Duration;

/// Allocate the actual decoded backing, independently of the visible plot.
pub(in crate::board::dho::hw) fn storage(s: &Settings, fb: usize) -> Option<Frame> {
    let mut storage = Frame::default();
    for (channel, points) in storage.ch.iter_mut().zip(capacities(s, fb)?) {
        channel.try_reserve_exact(points).ok()?;
    }
    Some(storage)
}

/// A production source grant precedes the read and raw replay. Direct host readouts
/// retain the existing reservation seam for ordinary acquisition fixtures.
pub(super) fn reserve(
    pipe: &Pipe,
    s: &Settings,
    fb: usize,
    raw: Option<&Frame>,
    deposit: Option<Identity>,
    prepared: Option<Arc<TapReservation>>,
    canceled: &dyn Fn() -> bool,
) -> Result<(Frame, Option<Arc<TapReservation>>), crate::drawpipe::Lost> {
    use crate::drawpipe::Lost;
    let identity = deposit.filter(|_| s.la.is_none() && pipe.accepting());
    if prepared.is_none() && identity.is_none() {
        return Ok((Frame::default(), None));
    }
    let storage = if raw.is_some() { Frame::default() } else { storage(s, fb).ok_or(Lost::Error)? };
    let reservation = prepared
        .or_else(|| identity.and_then(|id| pipe.reserve_waveform_until(raw.unwrap_or(&storage), id, canceled)))
        .ok_or(Lost::Settings)?;
    if !reservation.covers(raw.unwrap_or(&storage)) {
        return Err(Lost::Error);
    }
    Ok((storage, Some(reservation)))
}

/// Final board format includes mandatory raw replay even when not due this read.
pub(super) fn admit(pipe: &Pipe, s: &Settings, p: &hz::Params, id: Identity) -> bool {
    if s.single
        || s.mask == 0
        || s.mask > 15
        || s.mode != oos_hal::dho::acq::Mode::Normal
        || s.la.is_some()
        || s.segments.is_some()
        || s.hwmask.is_some()
        || s.hwzone.is_some()
    {
        return false;
    }
    let fb = oos_hal::dho::seq::frame_bytes(s, p);
    let raw = super::super::raw_replay_settings(s, p, true);
    let raw = raw.as_ref().unwrap_or(s);
    let replay = oos_hal::dho::seq::frame_bytes(raw, p);
    let format =
        [fb as u64, replay as u64, s.mask as u64, p.rate, p.memsel as u64, oos_hal::dho::seq::compress_k(s, p) as u64];
    if pipe.has_tap_plan(id, format) {
        return true;
    }
    let Some(maximum) = samples(s, fb).zip(samples(raw, replay)).map(|(a, b)| a.max(b)).filter(|&n| n > 0) else {
        return false;
    };
    let charged = maximum + crate::drawpipe::average::samples_for(pipe, id);
    if !pipe.admit_taps(id, format, charged, Duration::from_secs(1)) {
        return false;
    }
    // The two extra funded slots cover these temporary descriptor probes, one at a time.
    for (settings, bytes) in [(s, fb), (raw, replay)] {
        if storage(settings, bytes).is_none_or(|f| f.ch.iter().map(Vec::capacity).sum::<usize>() > maximum) {
            pipe.retire_tap_plan();
            return false;
        }
    }
    true
}

fn capacities(s: &Settings, fb: usize) -> Option<[usize; 4]> {
    let payload = fb.checked_sub(if s.header { 16 } else { 0 })?;
    let map = oos_hal::dho::seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    Some(map.word.map(|word| word.filter(|&w| w < payload / 2).map_or(0, |w| (payload / 2 - w).div_ceil(map.ratio))))
}

fn samples(s: &Settings, fb: usize) -> Option<usize> {
    capacities(s, fb)?.into_iter().try_fold(0usize, usize::checked_add)
}

/// F2's local tap-capacity wait. General DMA/command event waits remain F3.
pub(in crate::board::dho::hw) fn source_reservation(
    pipe: Option<&Pipe>,
    id: Identity,
    held: impl Fn() -> bool,
) -> Result<Option<Arc<TapReservation>>, ()> {
    let Some(pipe) = pipe.filter(|pipe| pipe.staged_taps()) else {
        return Ok(None);
    };
    if held() {
        return Err(());
    }
    let reservation = pipe.reserve_plan_until(id, &held).ok_or(())?;
    pipe.wait_for_tap_cycle_until(&reservation, &held).ok_or(())?;
    if held() {
        Err(())
    } else {
        Ok(Some(reservation))
    }
}

pub(super) fn retain(mut frame: Frame, charge: Option<&Arc<TapReservation>>) -> Frame {
    if let Some(charge) = charge.filter(|charge| charge.funded()) {
        frame.basic.retain(charge.clone());
    }
    frame
}
