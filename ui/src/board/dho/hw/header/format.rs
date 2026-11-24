// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch fixture v2, with capture geometry and the inputs for decode, selection and matching.

use crate::board::dho::cores::{Cores, Pattern};
use crate::board::dho::switches::FrameZero;
use oos_hal::dho::{acq::Mode, hz, seq};
use std::fmt::Write;

pub(super) const VERSION: &str = "oos-batch.v2";
pub(super) const META_LIMIT: u64 = 32 * 1024;
pub(super) const DATA_LIMIT: u64 = 128 * 1024 * 1024;

pub(in crate::board::dho::hw) struct Input<'a> {
    settings: &'a seq::Settings,
    params: &'a hz::Params,
    frames: usize,
    avail: usize,
    frame0: FrameZero,
    cores: &'a Cores,
    matching: bool,
}

impl<'a> Input<'a> {
    pub(in crate::board::dho::hw) fn new(
        (settings, params): (&'a seq::Settings, &'a hz::Params),
        (frames, avail, frame0): (usize, usize, impl Into<FrameZero>),
        (cores, matching): (&'a Cores, bool),
    ) -> Self {
        Self { settings, params, frames, avail, frame0: frame0.into(), cores, matching }
    }

    pub(super) fn aligned_len(&self) -> Option<usize> {
        let frame = seq::frame_bytes(self.settings, self.params);
        (self.frames > 0 && frame > 0 && frame == seq::frame_stride(frame))
            .then(|| self.frames.checked_mul(frame))
            .flatten()
    }

    pub(super) fn la_timing(&self) -> (f64, f64) {
        let timing = oos_hal::dho::la::timing(self.params, self.settings.mask);
        let (_, start) = oos_hal::dho::la::window(&timing);
        let dt = 1.0 / timing.rate.max(1) as f64;
        ((start as f64 - timing.pre as f64) * dt, dt)
    }

    pub(super) fn encode(&self, build: &str) -> String {
        let (s, p) = (self.settings, self.params);
        let frame = seq::frame_bytes(s, p);
        let mut out = format!(
            "{VERSION}\nbuild={build}\nmask={}\nrate={}\nstored_points={}\nframes={}\navail={}\nframe_bytes={frame}\nframe_stride={}\n",
            s.mask, p.rate, p.memsel, self.frames, self.avail, seq::frame_stride(frame),
        );
        let _ = writeln!(
            out,
            "ratio={}\ndot_fs={}\nsegment={}\nmax_frames={}\npre={}\npost={}",
            p.ratio, p.dot_fs, p.seg, p.max_frames, p.pre, p.post
        );
        write_read_layout(&mut out, s, p);
        let _ = writeln!(
            out,
            "tdiv={}\ndelay={}\nlevel={}\nsource={}\nslope={:?}\ncoupling={:?}\ntrigger={:?}\nholdoff={}\nnoise_reject={}",
            s.tdiv,
            s.delay,
            s.level,
            s.trig_src,
            s.slope,
            s.trig_couple,
            s.trigger, s.holdoff, s.trigger_options.noise_reject()
        );
        let (mode, average) = match s.mode {
            Mode::Normal => ("normal", 0),
            Mode::Peak => ("peak", 0),
            Mode::Average { count } => ("average", count),
            Mode::Reduced(capture) => {
                let _ = writeln!(
                    out,
                    "capture={capture}\ncapture_inferred={}\ncapture_base_depth={}",
                    capture.inferred(),
                    s.depth
                );
                (if s.mode.is_peak() { "peak" } else { "normal" }, 0)
            }
        };
        let _ = writeln!(
            out,
            "mode={mode}\naverage={average}\nmatching={}\nframe0_drop={}",
            self.matching,
            self.frame0 == FrameZero::Drop
        );
        if self.frame0 == FrameZero::Steady {
            let _ = writeln!(out, "frame0_check=true");
        }
        for (c, ch) in s.ch.iter().enumerate() {
            let _ = writeln!(out, "channel{c}={} {} {} {:?} {}", ch.scale, ch.offset, ch.invert, ch.couple, ch.bw);
            if let Some(pattern) = self.cores.fixture_pattern((s.mask, c, p.rate)) {
                write_pattern(&mut out, c, pattern);
            }
        }
        out
    }
}

fn write_read_layout(out: &mut String, s: &seq::Settings, p: &hz::Params) {
    let (points, start) = seq::window(s, p);
    let _ = writeln!(
        out,
        "header={}\nplay4={}\ncompression={}\neffective_compression={}\nwindow={} {points} {start}",
        s.header,
        s.playback.is_play4(),
        s.compress,
        seq::compress_k(s, p),
        s.window.is_some()
    );
}

fn write_pattern(out: &mut String, c: usize, pattern: Pattern) {
    let (offsets, samples, misfit) = pattern.parts();
    let _ = write!(out, "core{c}={samples} {misfit}");
    for x in offsets {
        let _ = write!(out, " {x}");
    }
    out.push('\n');
}

#[cfg(test)]
mod load;
#[cfg(test)]
pub(crate) use load::{Fixture, PictureView};
