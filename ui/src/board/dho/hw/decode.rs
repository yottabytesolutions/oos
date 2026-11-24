// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Decode DMA records and their metadata, or expose the untouched codes.

use super::{
    super::{cores::Cores, matching, switches::HeaderProbe},
    header,
};
use crate::board::{Frame, TriggerAnchor};
use oos_hal::dho::{
    hz,
    seq::{self, Settings},
};
use std::sync::Arc;

/// Decodes one read-out into a frame (tests: no header probe).
#[cfg(test)]
pub fn decode(bytes: &[u8], s: &Settings, p: &hz::Params, seq: u64) -> Frame {
    decode_probed(bytes, s, p, seq, HeaderProbe::Off)
}

/// `decode_into` without the header probe (tests).
#[cfg(test)]
pub fn decode_into(bytes: &[u8], s: &Settings, p: &hz::Params, seq: u64, f: &mut Frame) {
    decode_into_probed(bytes, s, p, seq, f, HeaderProbe::Off)
}

/// Decodes one read-out into a frame; with `probe` (`BoardSwitches::header_probe`, trigger.md 8)
/// a frame read with its header has the header logged next to where the edge actually is.
pub fn decode_probed(bytes: &[u8], s: &Settings, p: &hz::Params, seq: u64, probe: HeaderProbe) -> Frame {
    decode_with(bytes, s, p, seq, probe, None)
}

/// `decode_probed`, with the offsets of the ADC cores taken off the words of every sample (not of min/max pairs: Peak,
/// FPGA compression) when `cores` is given.
pub fn decode_with(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    seq: u64,
    probe: HeaderProbe,
    cores: Option<&mut Cores>,
) -> Frame {
    let mut f = Frame::default();
    decode_into_with(bytes, s, p, seq, &mut f, probe, cores);
    f
}

/// `decode_probed` into a kept frame: its channel vectors are refilled (no allocation once they have room), every
/// other field is set as `decode_probed` sets it.
pub fn decode_into_probed(bytes: &[u8], s: &Settings, p: &hz::Params, seq: u64, f: &mut Frame, probe: HeaderProbe) {
    decode_into_with(bytes, s, p, seq, f, probe, None);
}

/// `decode_into_probed` with the ADC cores matched (`decode_with`). The cores' offsets are taken off the words before
/// they become volts, so that path de-interleaves first; without them the fused decode (`seq::decode_volts_into`) runs.
pub fn decode_into_with(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    seq: u64,
    f: &mut Frame,
    probe: HeaderProbe,
    cores: Option<&mut Cores>,
) -> bool {
    let correction =
        cores.filter(|_| matching::eligible(s)).map(|cores| matching::FrameMatch::immediate(bytes, s, p, cores));
    decode_into_matched(bytes, s, p, seq, f, probe, correction.as_ref());
    s.mask != 0 && correction.is_none_or(|c| c.uncorrected(s.mask))
}

pub(in crate::board::dho) fn decode_into_matched(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    seq: u64,
    f: &mut Frame,
    probe: HeaderProbe,
    correction: Option<&matching::FrameMatch>,
) {
    crate::drawpipe::stage::fixture_stage!(Decode, decode_matched(bytes, s, p, seq, f, probe, correction));
}

fn decode_matched(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    seq: u64,
    f: &mut Frame,
    probe: HeaderProbe,
    correction: Option<&matching::FrameMatch>,
) {
    let (hdr, bytes) = bytes.split_at(if s.header && bytes.len() >= 16 { 16 } else { 0 });
    match correction {
        Some(correction) => correction.decode(bytes, s, f),
        None => seq::decode_volts_into(bytes, s.mask, s.ch.map(|k| (k.scale, k.offset)), &mut f.ch),
    }
    let k = pair_step(s, p);
    set_meta(s, p, seq, f, k);
    if !hdr.is_empty() && probe.on() {
        let tc = (s.trig_src.clamp(1, 4) - 1) as usize;
        header::log(hdr, &f.ch[tc], s, (f.pre, k), probe == HeaderProbe::OffTrigger);
    }
}

fn capture_pairs(s: &Settings) -> bool {
    matches!(s.mode, oos_hal::dho::acq::Mode::Reduced(_)) && s.mode.is_peak()
}

pub(in crate::board::dho) fn pair_step(s: &Settings, p: &hz::Params) -> usize {
    if capture_pairs(s) {
        // X2 stores consecutive min/max words at the programmed word rate.
        2
    } else {
        seq::compress_k(s, p) as usize
    }
}

/// Every field of a decoded frame but the channel vectors (kept as they are).
fn set_meta(s: &Settings, p: &hz::Params, seq: u64, f: &mut Frame, k: usize) {
    let (n, start) = seq::window(s, p);
    let trigger_word = p.pre.saturating_sub(start) as usize;
    let pre = trigger_word / k;
    let ch = std::mem::take(&mut f.ch);
    let mut trigger_anchor = TriggerAnchor::new(p.rate as i128 * p.ratio.max(1) as i128 == hz::CLOCK);
    if k > 1 && n < p.memsel && p.memsel <= crate::compress::MAX_DEPTH && !capture_pairs(s) {
        trigger_anchor = trigger_anchor.cropped_play(trigger_word, k);
    }
    *f = Frame {
        seq,
        dt: k as f64 / p.rate as f64,
        pre,
        ch,
        envelope: k > 1,
        acq: (p.rate as f64, p.memsel as usize),
        trigger_anchor,
        fixed_ti: capture_pairs(s).then(|| trigger_word as f64 / k as f64),
        ..Frame::default()
    };
}

/// `decode_into` without the decode (drawpipe::draw_frame_codes): the frame's fields as the decode sets them, the
/// channel vectors emptied, and where the read-out holds each channel. An envelope uses two interleaved
/// word groups per pair and its time step is the sample interval multiplied by the compression factor.
pub fn codes_into(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    seq: u64,
    f: &mut Frame,
) -> Option<crate::drawpipe::CodeLayout> {
    let (start, end) = if s.header { (16, seq::frame_bytes(s, p).min(bytes.len())) } else { (0, bytes.len()) };
    if end < start + 2 {
        return None;
    }
    for v in f.ch.iter_mut() {
        v.clear();
    }
    set_meta(s, p, seq, f, pair_step(s, p));
    let m = seq::code_map(s.mask, s.ch.map(|k| (k.scale, k.offset)));
    let layout = crate::drawpipe::CodeLayout::uncorrected(m.ratio, m.word, m.affine, seq::volt_code)
        .with_reduced_capture(matches!(s.mode, oos_hal::dho::acq::Mode::Reduced(_)));
    Some(if s.header { layout.with_payload(start, end) } else { layout })
}

#[cfg(any(target_os = "linux", test))]
pub(in crate::board::dho) fn probe_codes(
    bytes: &[u8],
    s: &Settings,
    p: &hz::Params,
    layout: &crate::drawpipe::CodeLayout,
    probe: HeaderProbe,
    frame: &mut Frame,
) {
    use crate::fastdraw::Samples;
    let tc = (s.trig_src.clamp(1, 4) - 1) as usize;
    let k = pair_step(s, p);
    let codes = layout.channel(bytes, tc);
    // The opt-in header log consumes volts on the trigger lane. Reuse the worker's decode buffer.
    let volts = &mut frame.ch[tc];
    volts.extend((0..codes.n).map(|i| codes.at(i)));
    header::log(&bytes[..16], volts, s, (frame.pre, k), probe == HeaderProbe::OffTrigger);
    volts.clear();
}

/// The read-out buffer lent to the deposit workers as it is (drawpipe.rs).
impl crate::drawpipe::RawBatch for oos_hal::dho::bus::DmaBuf {
    fn bytes(&self) -> &[u8] {
        self.data()
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

/// The LA words of the acquisition `p` (read with `seq::time::acquire_la_with_clock`) as a record: word j of the window at
/// (start + j - trigger index) / LA rate from the hardware trigger point (la.md 3.3; the trigger index is [A] until
/// test T1).
pub fn la_record(words: Vec<u16>, s: &Settings, p: &hz::Params) -> crate::ui::la::LaRec {
    let t = oos_hal::dho::la::timing(p, s.mask);
    let (_, start) = oos_hal::dho::la::window(&t);
    let dt = 1.0 / t.rate.max(1) as f64;
    crate::ui::la::LaRec { words: Arc::new(words), t0: (start as f64 - t.pre as f64) * dt, dt }
}
