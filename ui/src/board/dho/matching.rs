// SPDX-License-Identifier: GPL-2.0-or-later

//! Acquisition-owned learning and immutable, frame-specific worker corrections.

use super::cores::{Cores, Correction, Matcher, P};
use crate::board::Frame;
use oos_hal::dho::{
    acq::Mode,
    hz,
    samples::word_volts_f64,
    seq::{self, CodeMap, Settings},
};
#[cfg(any(target_os = "linux", test))]
use std::sync::Arc;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) use tests::fixtures::replay as replay_corrections;

#[derive(Clone, Copy, Default)]
pub(super) struct FrameMatch {
    channels: [Option<Correction>; 4],
}

pub(super) fn eligible(s: &Settings) -> bool {
    s.mode == Mode::Normal && s.compress <= 1
}

pub(super) fn payload<'a>(bytes: &'a [u8], s: &Settings) -> &'a [u8] {
    if s.header && bytes.len() >= 16 {
        &bytes[16..]
    } else {
        bytes
    }
}

impl FrameMatch {
    pub(super) fn immediate(bytes: &[u8], s: &Settings, p: &hz::Params, cores: &mut Cores) -> Self {
        MatchInput::new(s, p).learn(bytes, cores)
    }

    pub(super) fn uncorrected(&self, mask: u32) -> bool {
        self.channels.iter().enumerate().any(|(c, correction)| mask & (1 << c) != 0 && correction.is_none())
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
                word_volts_f64(code, s.ch[c].scale, s.ch[c].offset)
            }));
        }
    }
}

struct MatchInput<'a> {
    settings: &'a Settings,
    rate: u64,
    map: CodeMap,
    period: usize,
}

impl<'a> MatchInput<'a> {
    fn new(settings: &'a Settings, params: &hz::Params) -> Self {
        let map = seq::code_map(settings.mask, settings.ch.map(|c| (c.scale, c.offset)));
        let rate = i128::from(params.rate.max(1));
        let stride = ((oos_hal::dho::clock::adc_hz() + rate / 2) / rate) as usize;
        let period = P >> stride.max(map.ratio).trailing_zeros().min(P.trailing_zeros());
        Self { settings, rate: params.rate, map, period }
    }

    fn key(&self, channel: usize) -> super::cores::Key {
        (self.settings.mask, channel, self.rate)
    }

    fn learn(&self, bytes: &[u8], cores: &mut Cores) -> FrameMatch {
        if self.period == 1 {
            return FrameMatch::default();
        }
        let words = payload(bytes, self.settings).as_chunks::<2>().0;
        let ratio = self.map.ratio;
        FrameMatch {
            channels: std::array::from_fn(|channel| {
                let lane = self.map.word[channel]?;
                let len = words.len().saturating_sub(lane).div_ceil(ratio);
                cores.learn(self.key(channel), len, self.period, |i| u16::from_le_bytes(words[i * ratio + lane]))
            }),
        }
    }
}

/// One immutable correction per kept record, stored together with the submitted DMA read.
pub(super) struct BatchMatch {
    first: usize,
    frames: Vec<FrameMatch>,
}

impl BatchMatch {
    pub(super) fn prepare(
        bytes: &[u8],
        fb: usize,
        geometry: impl Into<MatchSlots>,
        s: &Settings,
        p: &hz::Params,
        cores: &mut Cores,
    ) -> Self {
        let MatchSlots { records, skip_second } = geometry.into();
        let input = MatchInput::new(s, p);
        let mut frames = Vec::new();
        let mut raw = records.clone().map(|i| {
            let slot = crate::drawpipe::dma_slot(i, skip_second);
            &bytes[slot * fb..(slot + 1) * fb]
        });
        if let Some(first) = raw.next().filter(|_| input.period > 1) {
            frames.reserve_exact(records.len());
            let previous = std::array::from_fn::<_, 4, _>(|c| cores.learnt(input.key(c)));
            let taught = input.learn(first, cores);
            let matchers = std::array::from_fn::<_, 4, _>(|c| {
                Matcher::new(previous[c].or_else(|| cores.learnt(input.key(c)))?, input.period)
            });
            let estimate = |bytes, first| {
                let words = payload(bytes, s).as_chunks::<2>().0;
                FrameMatch {
                    channels: std::array::from_fn(|c| {
                        if first && previous[c].is_none() {
                            taught.channels[c]
                        } else {
                            matchers[c]?.correction(words, input.map.word[c]?, input.map.ratio)
                        }
                    }),
                }
            };
            frames.push(estimate(first, true));
            frames.extend(raw.map(|bytes| estimate(bytes, false)));
        }
        Self { first: records.start, frames }
    }

    fn correction(&self, index: usize) -> Option<&FrameMatch> {
        self.frames.get(index.checked_sub(self.first)?)
    }

    #[cfg(any(target_os = "linux", test))]
    fn dispatch(
        matched: Option<Self>,
        s: Settings,
        p: hz::Params,
        (probe, probed_newest): (super::switches::HeaderProbe, Option<usize>),
    ) -> (crate::drawpipe::IndexedDecodeInto, Option<crate::drawpipe::CodesInto>) {
        use super::switches::HeaderProbe;
        // The deep raw window retains the legacy screen raster.
        let keep_codes = !(matched.is_some() && p.memsel > crate::compress::MAX_DEPTH && seq::compress_k(&s, &p) == 1);
        let decode_probe = if keep_codes { HeaderProbe::Off } else { probe };
        let matched = matched.map(Arc::new);
        let decoded = matched.clone();
        let ds = s.clone();
        let decode: crate::drawpipe::IndexedDecodeInto = Arc::new(move |i, bytes, frame| {
            let correction = decoded.as_ref().and_then(|m| m.correction(i));
            super::hw::decode_into_matched(bytes, &ds, &p, 0, frame, decode_probe, correction);
        });
        if !keep_codes {
            return (decode, None);
        }
        let codes: crate::drawpipe::CodesInto = Arc::new(move |i, bytes, frame| {
            let mut layout = super::hw::codes_into(bytes, &s, &p, 0, frame)?;
            layout.corrected = matched.as_ref().and_then(|m| m.correction(i)).map(|snapshot| {
                std::array::from_fn(|c| crate::fastdraw::CorrectedCodes {
                    offsets: snapshot.channels[c].map_or([0; 8], Correction::offsets),
                    scale: s.ch[c].scale,
                    offset: s.ch[c].offset,
                })
            });
            if s.header && probe.on() && Some(i) != probed_newest {
                super::hw::probe_codes(bytes, &s, &p, &layout, probe, frame);
            }
            Some(layout)
        });
        (decode, Some(codes))
    }
}

pub(super) struct MatchSlots {
    records: std::ops::Range<usize>,
    skip_second: bool,
}

impl From<std::ops::Range<usize>> for MatchSlots {
    fn from(records: std::ops::Range<usize>) -> Self {
        Self { records, skip_second: false }
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
    let decode = |frame: &mut Frame, correction: FrameMatch| {
        super::hw::decode_into_matched(&bytes, &s, &p, 0, frame, super::switches::HeaderProbe::Off, Some(&correction));
        std::hint::black_box(frame);
    };
    sample("core matching learn and decode", &mut || {
        decode(&mut frame, FrameMatch::immediate(&bytes, &s, &p, &mut cores));
    });
    let batch = BatchMatch::prepare(&bytes, bytes.len(), 0..1, &s, &p, &mut cores);
    sample("core matching worker decode", &mut || {
        decode(&mut frame, *batch.correction(0).expect("the allocation fixture has one record"));
    });
}

#[cfg(test)]
fn feed(
    pipe: &crate::drawpipe::Pipe,
    raw: (Box<dyn crate::drawpipe::RawBatch>, usize, std::ops::Range<usize>),
    batch: (oos_core::change::Identity, impl Into<crate::drawpipe::clock::StageClock>),
    settings: (Settings, hz::Params, super::switches::HeaderProbe),
    cores: Option<&mut Cores>,
    (newest, reservation, decode): (Option<Frame>, Option<Arc<crate::drawpipe::TapReservation>>, bool),
) -> Option<Arc<Frame>> {
    submit(pipe, (raw.0, raw.1, raw.2, false), batch, settings, cores, (newest, reservation, decode, |frame| frame))
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn submit(
    pipe: &crate::drawpipe::Pipe,
    raw: (Box<dyn crate::drawpipe::RawBatch>, usize, std::ops::Range<usize>, bool),
    batch: (oos_core::change::Identity, impl Into<crate::drawpipe::clock::StageClock>),
    (s, p, probe): (Settings, hz::Params, super::switches::HeaderProbe),
    cores: Option<&mut Cores>,
    (newest, reservation, from_dma, tag): (
        Option<Frame>,
        Option<Arc<crate::drawpipe::TapReservation>>,
        bool,
        impl FnOnce(Frame) -> Frame,
    ),
) -> Option<Arc<Frame>> {
    let (buffer, fb, records, skip_second) = raw;
    let matched = cores.filter(|_| eligible(&s)).map(|cores| {
        BatchMatch::prepare(buffer.bytes(), fb, MatchSlots { records: records.clone(), skip_second }, &s, &p, cores)
    });
    let needs_correction =
        |i| s.mask != 0 && matched.as_ref().and_then(|m| m.correction(i)).is_none_or(|frame| frame.uncorrected(s.mask));
    let missed = records.clone().filter(|&i| needs_correction(i)).count();
    let newest_missed = records.end.checked_sub(1).is_some_and(needs_correction);
    let newest = newest.map(|mut frame| {
        if from_dma {
            let i = records.end - 1;
            let correction = matched.as_ref().and_then(|m| m.correction(i));
            let slot = crate::drawpipe::dma_slot(i, skip_second);
            let bytes = &buffer.bytes()[slot * fb..(slot + 1) * fb];
            super::hw::decode_into_matched(bytes, &s, &p, frame.seq, &mut frame, probe, correction);
        }
        Arc::new(tag(frame))
    });
    let probed_newest = (from_dma && newest.is_some()).then(|| records.end - 1);
    let (decode, codes) = BatchMatch::dispatch(matched, s, p, (probe, probed_newest));
    let identity = batch.0;
    let fallback =
        pipe.feed_source((buffer, fb, records, skip_second), batch, (decode, codes), (newest, reservation), from_dma);
    let uncorrected = if fallback.is_some() { usize::from(newest_missed) } else { missed };
    pipe.batch_counters().metrics.source_uncorrected(identity, uncorrected);
    fallback
}
