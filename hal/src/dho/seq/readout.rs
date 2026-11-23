// SPDX-License-Identifier: GPL-2.0-or-later

//! Playback sequences and the layout of frames in the C2H stream.

use super::{Settings, Step};
use crate::dho::{acq, hz, la, scu};

#[cfg(test)]
mod tests;

/// The requested analog playback path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Playback {
    /// Type-4 play with the ordinary OOS read parameters.
    #[default]
    Raw,
    /// Diagnostic play-last from a stock hand-over.
    Last,
}

impl Playback {
    /// Whether reads use the type-4 play FSM.
    pub fn is_play4(self) -> bool {
        self == Self::Raw
    }
}

pub(super) fn steps_at(
    s: &Settings,
    p: &hz::Params,
    shadows: &mut scu::Shadows,
    (first, count): (u32, u32),
) -> Result<Vec<Step>, ReadoutError> {
    let nf = if s.playback.is_play4() { count.max(1) } else { 1 };
    let (n, start) = window(s, p);
    let compression = checked_compression(s, p, n, start)?;
    let mut st = Vec::new();
    let win = scu::Window {
        n: n * p.ratio,
        start: start * p.ratio,
        ratio: p.ratio,
        fsm: s.mask,
        la: s.la.is_some(),
        avg: matches!(s.mode, acq::Mode::Average { .. }),
    };
    st.push(Step::Note(format!("read-out: {} points from memory point {} (trigger at {})", n, start, p.pre)));
    let mut ro = if s.playback.is_play4() {
        play4(&win, shadows, (first, nf), compression)
    } else {
        scu::readout(&win, shadows)
    };
    if s.header {
        // frame header on: the FPGA puts a 16-byte header in front of the frame
        for w in ro.iter_mut().filter(|w| w.0 == 0x1010) {
            w.1 &= !(1 << 31);
        }
        shadows.s1010 &= !(1 << 31);
    }
    st.extend(ro.into_iter().map(Step::Wr));
    st.push(Step::Peek(&[0x4034, 0x4080]));
    let points = compression.map_or(n, |(_, len)| len);
    st.push(Step::Read { frames: nf as usize, frame: bytes(s.header, p.ratio, points) });
    Ok(st)
}

fn play4(
    win: &scu::Window,
    shadows: &mut scu::Shadows,
    (first, count): (u32, u32),
    compression: Option<(u32, u32)>,
) -> Vec<crate::dho::Write> {
    let mut writes = scu::passthrough();
    let mut read = scu::readout_play4(win, shadows, first, count);
    if let Some((k, len)) = compression {
        let word = compress_word(k);
        for write in writes.iter_mut().filter(|write| write.0 == 0x1170) {
            write.1 = word;
        }
        for write in read.iter_mut().filter(|write| write.0 == 0x11f0) {
            write.1 = len;
        }
    }
    writes.extend(read);
    writes
}

pub(super) fn la_steps(s: &Settings, p: &hz::Params, shadows: &mut scu::Shadows) -> Vec<Step> {
    let t = la::timing(p, s.mask);
    let (n, start) = la::window(&t);
    let mut st = vec![Step::Note(format!("LA read-out: {} words from LA sample {} (trigger at {})", n, start, t.pre))];
    // wait for the stop before touching the play engine (at least 1.5 ms)
    st.push(Step::Poll { reg: 0x4034, bit: scu::status::STOPPED, ms: 30, what: "FSM stopped before the LA read" });
    st.extend(la::passthrough().into_iter().map(Step::Wr));
    st.extend(la::readout(shadows, n, start, p.ratio, s.mask).into_iter().map(Step::Wr));
    st.push(Step::Read { frames: 1, frame: la::read_bytes(n) });
    st.extend(la::restore(shadows).into_iter().map(Step::Wr));
    st
}

pub(super) fn stamp_steps(s: &Settings, p: &hz::Params, shadows: &mut scu::Shadows, k: u32) -> Vec<Step> {
    let (_, start) = window(s, p);
    let win =
        scu::Window { n: STAMP_WORDS, start: start * p.ratio, ratio: p.ratio, fsm: s.mask, la: false, avg: false };
    let mut w = scu::passthrough();
    w.extend(scu::readout_play4(&win, shadows, k, 1));
    for x in w.iter_mut().filter(|x| x.0 == 0x1010) {
        x.1 &= !(1 << 31);
    }
    shadows.s1010 &= !(1 << 31);
    let mut st = vec![Step::Note(format!("stamp play of segment {}", k))];
    st.extend(w.into_iter().map(Step::Wr));
    st.push(Step::Read { frames: 1, frame: STAMP_BYTES });
    st.extend(scu::ts_latch(shadows).into_iter().map(Step::Wr));
    st
}

/// Words (all channels) of the stamp play: 128, the smallest window of the stock raw export (record-play.md 7.1: a
/// TX length of 8 points becomes 128 / ratio).
pub const STAMP_WORDS: u32 = 128;
/// Bytes of the stamp play's read: the 16-byte frame header and `STAMP_WORDS` words.
pub const STAMP_BYTES: usize = 16 + 2 * STAMP_WORDS as usize;

/// Bytes one frame of `frame` bytes takes in the C2H stream: the FPGA pads every frame of a play-out to 8 bytes (4
/// points at ratio 1). Unit 2026-11-04, 10 MHz sine, batches of 4..6 frames: 1250 points (100 ns/div) come at a
/// stride of 1252, 625 points at 628 (the phase is the same in every frame at that stride and walks by 2 / 3 samples a
/// frame at the unpadded one); 2500 points need no padding. Reading the unpadded length shifted every later frame
/// and left the padding in the stream for the next read.
pub fn frame_stride(frame: usize) -> usize {
    frame.div_ceil(8) * 8
}

/// Total bytes of a read of `frames` frames (`frame_stride` each).
pub fn read_len(frames: usize, frame: usize) -> usize {
    frames * frame_stride(frame)
}

/// Bytes of one frame in the read: the window of all interleaved channels (+ the 16-byte header in header mode).
pub fn frame_bytes(s: &Settings, p: &hz::Params) -> usize {
    let n = if is_compressed(s) { compressed_len(s, p) } else { window(s, p).0 };
    bytes(s.header, p.ratio, n)
}

fn bytes(header: bool, ratio: u32, points: u32) -> usize {
    2 * ratio as usize * points as usize + if header { 16 } else { 0 }
}

/// Largest compression factor: 0x1170 holds k in bits 13:0, a larger value wraps (netlist simulation, private RE notes
/// docs/re/netlist/blocks/datapath.md).
pub const COMPRESS_K_MAX: u32 = 16383;

/// The written compression factor, bounded by the register field and the number of eight-point groups in the window.
pub fn compress_k(s: &Settings, p: &hz::Params) -> u32 {
    window_compression(s.compress, window(s, p).0).0
}

/// The 0x1170 word for factor `k`: k > 1 enables the min/max path with k in bits 13:0; k <= 1 is the pass-through
/// word. Enable with k = 0 stalls the read (no C2H word at all, assumed), so it is never written.
pub fn compress_word(k: u32) -> u32 {
    match k.min(COMPRESS_K_MAX) {
        0 | 1 => 0x4000_0001,
        k => 0xc000_0000 | k,
    }
}

/// Points per channel after the FPGA compression (`compress_k`): the most whole multiples of 8 points (so whole min/max
/// pairs) whose samples lie inside the window, points x k <= n. The FPGA delivers exactly 0x11f0 points and waits
/// for samples until it has them: asking for more than the window holds stalls the read. The stock rounding up to 8
/// asked for 3912 points at k = 256 at 1 M (1 001 472 samples) and the c2h read timed out on the unit, while k = 128
/// (7816 points, 1 000 448 samples) still fitted the 0x248 samples the window reads beyond the record (unitlab round 2).
pub fn compressed_len(s: &Settings, p: &hz::Params) -> u32 {
    window_compression(s.compress, window(s, p).0).1
}

fn window_compression(requested: u32, n: u32) -> (u32, u32) {
    let k = requested.clamp(1, COMPRESS_K_MAX).min((n / 8).max(1));
    (k, (n / k / 8 * 8).max(8))
}

/// Settings that cannot be issued as a compressed read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadoutError {
    /// Logic analyser words double the programmed window length and cannot share FPGA compression.
    /// The effective start or size of an explicit window is not eight-point aligned.
    UnalignedWindow { start: u32, points: u32 },
    /// The effective window exceeds the stored record.
    WindowOutsideRecord { start: u32, points: u32, memsel: u32 },
    /// The window cannot produce the minimum transfer length at this factor.
    InsufficientPoints { points: u32, factor: u32, len: u32 },
}

impl std::fmt::Display for ReadoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::UnalignedWindow { start, points } => {
                write!(f, "compressed read window start {start} and length {points} must be multiples of 8")
            }
            Self::WindowOutsideRecord { start, points, memsel } => {
                write!(f, "compressed read window of {points} points at {start} exceeds record length {memsel}")
            }
            Self::InsufficientPoints { points, factor, len } => {
                write!(f, "compressed read window of {points} points cannot supply {len} points at factor {factor}")
            }
        }
    }
}

impl std::error::Error for ReadoutError {}

/// Checks compression compatibility and type-4 geometry before constructing a sequence.
pub fn validate_readout(s: &Settings, p: &hz::Params) -> Result<(), ReadoutError> {
    let (n, start) = window(s, p);
    checked_compression(s, p, n, start).map(|_| ())
}

fn is_compressed(s: &Settings) -> bool {
    s.compress > 1 && s.playback.is_play4()
}

fn checked_compression(s: &Settings, p: &hz::Params, n: u32, start: u32) -> Result<Option<(u32, u32)>, ReadoutError> {
    if !is_compressed(s) {
        return Ok(None);
    }
    let (k, len) = window_compression(s.compress, n);
    if len < 8 || len > n / k {
        return Err(ReadoutError::InsufficientPoints { points: n, factor: k, len });
    }
    if s.window.is_some() && (!start.is_multiple_of(8) || !n.is_multiple_of(8)) {
        return Err(ReadoutError::UnalignedWindow { start, points: n });
    }
    if u64::from(start) + u64::from(n) > u64::from(p.memsel) {
        return Err(ReadoutError::WindowOutsideRecord { start, points: n, memsel: p.memsel });
    }
    Ok(Some((k, len)))
}

/// Drops the stream padding behind every frame (`frame_stride`) in place: the frames end up back to back.
pub fn unpad(buf: &mut Vec<u8>, frame: usize) {
    let n = unpad_in(buf, frame);
    buf.truncate(n);
}

/// `unpad` on a slice: returns the bytes left at the front. Frames without padding (all live set-ups of oos-ui) cost
/// nothing.
pub fn unpad_in(buf: &mut [u8], frame: usize) -> usize {
    let stride = frame_stride(frame);
    if frame == 0 || stride == frame {
        return buf.len();
    }
    let n = buf.len() / stride;
    for k in 1..n {
        buf.copy_within(k * stride..k * stride + frame, k * frame);
    }
    n * frame
}

/// (points per channel, first memory point) of the read-out window.
pub fn window(s: &Settings, p: &hz::Params) -> (u32, u32) {
    match s.window {
        Some((n, at)) => {
            let n = n.min(p.memsel);
            (n, i64::from(p.pre).saturating_add(at).clamp(0, i64::from(p.memsel - n)) as u32)
        }
        None => (p.memsel, 0),
    }
}
