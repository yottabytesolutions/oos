// SPDX-License-Identifier: GPL-2.0-or-later

//! Reference publication and the reusable draw-only decoder profiles.

use super::{lock, publication, raw, Pipe, Settings};
use crate::{
    board::{dho::switches::HeaderProbe, BasicStats, Frame, HwShared},
    drawpipe::{clock::StageClock, CodesInto, IndexedDecodeInto, RawBatch, TapReservation},
};
use oos_core::change::Identity;
use oos_hal::dho::hz;
use std::sync::Arc;

type Callbacks = (IndexedDecodeInto, Option<CodesInto>);

struct Profile {
    settings: Settings,
    params: hz::Params,
    probe: HeaderProbe,
    callbacks: Callbacks,
}

#[derive(Default)]
pub(in crate::board::dho::hw::acq) struct Decoders {
    full: Option<Profile>,
    peak: Option<Profile>,
}

impl Decoders {
    pub(super) fn for_profile(&mut self, s: &Settings, p: hz::Params, probe: HeaderProbe) -> Callbacks {
        let peak = raw::is_reduced(s);
        let slot = if peak { &mut self.peak } else { &mut self.full };
        // Batch admission changes the ring count without changing individual frame geometry.
        let settings = Settings { frames: 1, ..s.clone() };
        if let Some(profile) = slot
            .as_ref()
            .filter(|profile| profile.settings == settings && profile.params == p && profile.probe == probe)
        {
            return profile.callbacks.clone();
        }
        let callbacks = callbacks(&settings, p, probe, peak);
        *slot = Some(Profile { settings, params: p, probe, callbacks: callbacks.clone() });
        callbacks
    }
}

fn callbacks(s: &Settings, p: hz::Params, probe: HeaderProbe, peak: bool) -> Callbacks {
    use super::super::super::decode;
    let settings = s.clone();
    let decode: IndexedDecodeInto = Arc::new(move |_, bytes, frame| {
        decode::decode_into_matched(bytes, &settings, &p, 0, frame, probe, None);
        if peak {
            raw::peak_word_axis(frame, &settings, &p);
        }
    });
    let codes = peak.then(|| {
        let s = s.clone();
        let codes: CodesInto = Arc::new(move |_, bytes, frame| {
            let layout = decode::codes_into(bytes, &s, &p, 0, frame)?;
            raw::peak_word_axis(frame, &s, &p);
            if s.header && probe.on() {
                decode::probe_codes(bytes, &s, &p, &layout, probe, frame);
            }
            Some(layout)
        });
        codes
    });
    (decode, codes)
}

pub(super) fn decode_reference(
    bytes: &[u8],
    (s, p, probe): (&Settings, &hz::Params, HeaderProbe),
    cores: Option<&mut crate::board::dho::cores::Cores>,
    (id, sequence, tap): (Identity, u64, Option<&Arc<TapReservation>>),
) -> Option<(Arc<Frame>, bool)> {
    let (mut frame, uncorrected) = raw::decode_raw(bytes, (s, p), probe, cores, tap)?;
    frame.seq = sequence;
    *frame.basic = std::array::from_fn(|c| BasicStats::of(&frame.ch[c]));
    // Readers and the picture retain independent snapshots. Give each reference its own funded backing.
    Some((Arc::new(publication::tag(frame, id)), uncorrected))
}

pub(super) fn submit_reference(
    pipe: &Pipe,
    (buffer, fb): (Box<dyn RawBatch>, usize),
    stage: (Identity, StageClock),
    (newest, tap): (Arc<Frame>, super::super::batch::Reserved),
    decode: IndexedDecodeInto,
    out: &HwShared,
) {
    let fallback =
        pipe.feed_source((buffer, fb, 0..1, false), stage, (decode, None), (Some(newest.clone()), tap), true);
    let mut g = lock(out);
    if !publication::boundary(Some((newest.cfg, newest.stamp)), g.acquisition.identity()) {
        g.frame = newest;
        if let Some(fallback) = fallback {
            g.queue.push_back(fallback);
        }
    }
}

#[cfg(test)]
mod tests;
