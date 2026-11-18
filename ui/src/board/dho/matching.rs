// SPDX-License-Identifier: GPL-2.0-or-later

//! Acquisition-owned learning and immutable, frame-specific worker corrections.

use super::cores::{Cores, Correction};
use crate::board::Frame;
use oos_hal::dho::{
    acq::Mode,
    hz,
    seq::{self, Settings},
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default)]
pub(super) struct FrameMatch {
    channels: [Option<Correction>; 4],
}

pub(super) fn eligible(s: &Settings) -> bool {
    s.mode == Mode::Normal && s.compress <= 1
}

fn payload<'a>(bytes: &'a [u8], s: &Settings) -> &'a [u8] {
    if s.header && bytes.len() >= 16 {
        &bytes[16..]
    } else {
        bytes
    }
}

impl FrameMatch {
    pub(super) fn learn(bytes: &[u8], s: &Settings, p: &hz::Params, cores: &mut Cores) -> Self {
        Self::prepare(bytes, s, p, cores, true)
    }

    fn prepare(bytes: &[u8], s: &Settings, p: &hz::Params, cores: &mut Cores, learn: bool) -> Self {
        let bytes = payload(bytes, s);
        let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
        let words = bytes.len() / 2;
        Self {
            channels: std::array::from_fn(|c| {
                let word = map.word[c].filter(|&w| w < words)?;
                let len = (words - word).div_ceil(map.ratio);
                let sample = |i| {
                    let j = 2 * (i * map.ratio + word);
                    u16::from_le_bytes([bytes[j], bytes[j + 1]])
                };
                let key = (s.mask, c, p.rate);
                if learn {
                    cores.learn(key, len, sample)
                } else {
                    cores.snapshot(key, len, sample)
                }
            }),
        }
    }

    /// Read the payload into kept vectors, correcting codes before the usual volts conversion.
    pub(super) fn decode(&self, bytes: &[u8], s: &Settings, f: &mut Frame) {
        let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
        for (c, out) in f.ch.iter_mut().enumerate() {
            out.clear();
            let Some(word) = map.word[c] else { continue };
            out.extend(bytes.as_chunks::<2>().0.iter().skip(word).step_by(map.ratio).enumerate().map(|(i, b)| {
                let raw = u16::from_le_bytes([b[0], b[1]]);
                let code = self.channels[c].map_or(raw, |correction| correction.word(i, raw));
                ((code as f64 - 32768.0) / 7500.0 * s.ch[c].scale - s.ch[c].offset) as f32
            }));
        }
    }
}

/// One value per frame, allocated together once per batch. At most 1000 frames in `hw::batch_plan`;
/// 192 kB on the 64-bit targets, owned by the decoder until the DMA lease's last worker finishes.
/// The three-buffer pool bounds in-flight snapshots to 576 kB. No worker accesses `Cores`.
pub(super) struct BatchMatch {
    frames: Vec<FrameMatch>,
}

impl BatchMatch {
    pub(super) fn learn(
        bytes: &[u8],
        fb: usize,
        frames: std::ops::Range<usize>,
        s: &Settings,
        p: &hz::Params,
        cores: &mut Cores,
    ) -> Self {
        let frames = bytes
            .chunks_exact(fb)
            .take(frames.end)
            .enumerate()
            .map(|(i, b)| FrameMatch::prepare(b, s, p, cores, frames.contains(&i)))
            .collect();
        Self { frames }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn newest(
        &self,
        bytes: &[u8],
        s: &Settings,
        p: &hz::Params,
        seq: u64,
        probe: super::switches::HeaderProbe,
    ) -> Frame {
        let mut frame = Frame::default();
        super::hw::decode_into_matched(bytes, s, p, seq, &mut frame, probe, self.frames.last());
        frame
    }

    pub(super) fn decoder(
        self,
        s: Settings,
        p: hz::Params,
        probe: super::switches::HeaderProbe,
    ) -> crate::drawpipe::IndexedDecodeInto {
        Arc::new(move |i, bytes, frame| {
            super::hw::decode_into_matched(bytes, &s, &p, 0, frame, probe, Some(&self.frames[i]));
        })
    }
}

/// Allocation census uses the same preparation and kept-vector decode as acquisition and workers.
pub(crate) fn allocation_probe(mut sample: impl FnMut(&str, &mut dyn FnMut())) {
    let s = Settings { mask: 15, header: true, compress: 1, ..Settings::default() };
    let p = hz::prep(s.mask, 1_000_000_000, 4096, 0);
    let mut bytes = vec![0; 16];
    for i in 0..p.memsel as usize {
        for c in 0..4 {
            bytes.extend((32768 + ((i + c) % 8) as u16 * 20).to_le_bytes());
        }
    }
    let mut cores = Cores::default();
    let mut frame = Frame::default();
    sample("core matching learn and decode", &mut || {
        let correction = FrameMatch::learn(&bytes, &s, &p, &mut cores);
        super::hw::decode_into_matched(
            &bytes,
            &s,
            &p,
            0,
            &mut frame,
            super::switches::HeaderProbe::Off,
            Some(&correction),
        );
        std::hint::black_box(&frame);
    });
    let batch = BatchMatch::learn(&bytes, bytes.len(), 0..1, &s, &p, &mut cores);
    let decode = batch.decoder(s, p, super::switches::HeaderProbe::Off);
    sample("core matching worker decode", &mut || {
        decode(0, &bytes, &mut frame);
        std::hint::black_box(&frame);
    });
}

/// Prepare before any worker can observe the batch, then decode its final record with its own snapshot.
#[cfg(any(target_os = "linux", test))]
pub(super) fn prepare(
    bytes: &[u8],
    geometry: (usize, usize, usize),
    s: &Settings,
    p: &hz::Params,
    newest: (u64, super::switches::HeaderProbe, Option<Frame>),
    cores: Option<&mut Cores>,
) -> (Option<BatchMatch>, Frame) {
    // `skip`: the frames left out of the batch (frame 0, a frame 1 without its edge) teach nothing
    let (fb, frames, skip) = geometry;
    let (seq, probe, raw) = newest;
    let matched =
        cores.filter(|_| eligible(s)).map(|c| BatchMatch::learn(bytes, fb, skip.min(frames)..frames, s, p, c));
    let bytes = &bytes[(frames - 1) * fb..frames * fb];
    let frame = raw.unwrap_or_else(|| match &matched {
        Some(matched) => matched.newest(bytes, s, p, seq, probe),
        None => super::hw::decode_probed(bytes, s, p, seq, probe),
    });
    (matched, Frame { seq, ..frame })
}

/// A prepared decoder must not use the uncorrected raw-code drawing shortcut.
#[cfg(any(target_os = "linux", test))]
pub(super) fn feed(
    pipe: &crate::drawpipe::Pipe,
    raw: (Box<dyn crate::drawpipe::RawBatch>, usize, std::ops::Range<usize>),
    identity: oos_core::change::Identity,
    settings: (Settings, hz::Params, super::switches::HeaderProbe),
    matched: Option<BatchMatch>,
    newest: Option<Arc<Frame>>,
) -> Option<Arc<Frame>> {
    let (raw, fb, frames) = raw;
    let (s, p, probe) = settings;
    match matched {
        Some(matched) => pipe.feed_prepared(raw, fb, frames, identity, matched.decoder(s, p, probe), newest),
        None => {
            let (cs, cp) = (s.clone(), p);
            let codes: crate::drawpipe::CodesInto =
                Arc::new(move |bytes, f| super::hw::codes_into(bytes, &cs, &cp, 0, f));
            let decode: crate::drawpipe::DecodeInto =
                Arc::new(move |bytes, f| super::hw::decode_into_probed(bytes, &s, &p, 0, f, probe));
            pipe.feed_with(raw, fb, frames, identity, decode, Some(codes), newest)
        }
    }
}
