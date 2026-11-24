// SPDX-License-Identifier: GPL-2.0-or-later

//! Use the picture schedule to place and budget acquisition batches.

use super::picture_period;
use std::time::{Duration, Instant};

/// Ring-fill time limits: the hardware source's former fixed wait before the first screen step is known (and at most
/// k screen frames after), a few frames' worth at least.
pub(super) const FILL_MAX_MS: f32 = 25.0;
pub(super) const FILL_MIN_MS: f32 = 4.0;
/// A batch is to be drawn this long before the step that shows it (the merge comes a few ms into the step).
pub(super) const MARGIN_MS: f32 = 3.0;

/// The ring-fill time of the next batch (`seq::acquire_batch_idle`'s `wait_ms`). With a fixed 25 ms fill a batch came
/// every 40-55 ms at 1-10 us/div, and every 31-34 ms at short time bases, against a screen frame of 33.3 ms: pictures
/// without a new batch and pictures with two alternated (docs/fork/stutter.md). The fill is set so that the reads come
/// one screen frame apart, each the workers' drawing time (`ready_ms`) before a screen step: from the rest of a batch
/// besides its fill (arm, first trigger, read; smoothed). The drawing of one batch overlaps the next one's recording.
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    /// the rest of a batch besides its fill (smoothed) and its mean deviation
    rest: Option<f32>,
    dev: f32,
    /// the read time the last plan aimed at
    target: Option<Instant>,
}

impl Default for Fill {
    fn default() -> Fill {
        Fill { rest: None, dev: 0.0, target: None }
    }
}

impl Fill {
    /// The fill time, ms, for a batch starting at `start`, given the next screen step after a time (`next`) and the
    /// time to draw a batch after its read (`ready`).
    pub fn plan(&mut self, start: Instant, next: impl Fn(Instant) -> Option<Instant>, ready: f32) -> u64 {
        self.plan_at_period(start, next, ready, picture_period())
    }

    pub(crate) fn plan_at_period(
        &mut self,
        start: Instant,
        next: impl Fn(Instant) -> Option<Instant>,
        ready: f32,
        period: Duration,
    ) -> u64 {
        let frame_ms = period.as_secs_f32() * 1e3;
        let rest = self.rest.unwrap_or(5.0);
        let ms = |a: f32| std::time::Duration::from_secs_f32(a.max(0.0) / 1e3);
        // the drawing overlaps the next batch: only the reads need a screen frame between them, each one `ready` +
        // `MARGIN_MS` before a step. The first such read time the batch can make, the slot after the last one's. (Every
        // second slot was tried where a batch does not fit one frame: the longer fill brought more frames and a longer
        // read, and the reads fell 20-30 ms behind their slots; docs/fork/stutter.md.)
        let lead = ready + MARGIN_MS;
        // planned with the rest's usual spread on top: a read a little early still shows in the same picture, a late
        // one a frame later
        let rest = rest + 2.0 * self.dev;
        // no room for a batch in a frame: as fast as it goes (waiting for a slot would read more frames, longer)
        if rest + FILL_MIN_MS > frame_ms {
            self.target = None;
            return FILL_MIN_MS as u64;
        }
        let mut lower = start + ms(rest + FILL_MIN_MS);
        if let Some(t) = self.target {
            lower = lower.max(t + ms(0.5 * frame_ms));
        }
        let Some(step) = next(lower + ms(lead)) else { return FILL_MAX_MS as u64 };
        let read = step - ms(lead);
        self.target = Some(read);
        let fill = read.saturating_duration_since(start).as_secs_f32() * 1e3 - rest;
        // a batch that starts early (the read before was early) fills longer to keep its slot
        fill.clamp(FILL_MIN_MS, FILL_MAX_MS.max(frame_ms)).round() as u64
    }

    /// A batch spent `rest` ms besides its fill: before it (arm, wait of a batch armed ahead, first trigger) and in the
    /// read.
    pub fn done(&mut self, r: f32) {
        let r = r.max(0.0);
        let rest = self.rest.map_or(r, |x| x + 0.3 * (r - x));
        self.dev += 0.3 * ((r - rest).abs() - self.dev);
        self.rest = Some(rest);
    }
}

/// The fill a capped batch keeps room for (the ring then fills in it, and the planner keeps the reads on their slots).
const CAP_FILL_MS: f32 = 6.0;

/// The most frames a batch reads (`Settings::frames`), where its read would otherwise leave no room for a batch in a
/// screen frame. At 5 us/div with the live compression (no prearm) a batch spent 12-20 ms arming and 21-29 ms reading
/// ~25 frames at ~0.8 ms each: it did not fit 33 ms, and about one picture in ten had nothing new
/// (docs/fork/stutter.md). The cap comes from the read time per frame and the time before the fill (both smoothed);
/// a full ring ends the fill early.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameCap {
    per_frame: Option<f32>,
    pre: Option<f32>,
    /// mean deviation of the rest (time before the fill and read)
    dev: f32,
    cap: Option<u32>,
    #[cfg(any(target_os = "linux", test))]
    window_read: Option<(Duration, Duration)>,
}

impl FrameCap {
    /// `frames` (what the batch would read) capped.
    pub fn apply(&self, frames: u32) -> u32 {
        self.cap.map_or(frames, |c| frames.min(c))
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reset_window(&mut self) {
        self.window_read = None;
    }

    /// Fixed arm and readout setup, then DMA time per played record.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn observe_window(&mut self, count: u32, fixed: Duration, dma: Duration) {
        if count == 0 || dma.is_zero() {
            return;
        }
        let per_record = dma.div_f64(f64::from(count));
        let blend = |old: Duration, sample: Duration| old.mul_f64(0.8) + sample.mul_f64(0.2);
        self.window_read = Some(self.window_read.map_or((fixed, per_record), |(old_fixed, old_per_record)| {
            (blend(old_fixed, fixed), blend(old_per_record, per_record))
        }));
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn window_read(&self, stored_words: u64) -> (Duration, Duration) {
        // Cold estimate from the cropped read model in docs/fork/played-window.md, plus 1 ms for the arm.
        self.window_read
            .unwrap_or_else(|| (Duration::from_micros(1700), Duration::from_nanos((stored_words * 73).div_ceil(10))))
    }

    /// A batch read `count` frames in `read_ms` after `pre_ms` from its start to the start of its fill; `asked` is
    /// what it would read without the cap. `keep` (prearm: a new cap re-arms): only a change of more than a quarter.
    pub fn done(
        &mut self,
        count: u32,
        read_ms: f32,
        pre_ms: f32,
        asked: u32,
        keep: bool,
        period: impl FnOnce() -> Duration,
    ) {
        if count == 0 || read_ms <= 0.0 {
            return;
        }
        let ema = |x: Option<f32>, v: f32| Some(x.map_or(v, |x| x + 0.2 * (v - x)));
        if let (Some(per), Some(pre)) = (self.per_frame, self.pre) {
            let r = pre_ms.max(0.0) + read_ms;
            self.dev += 0.2 * ((r - pre - per * count as f32).abs() - self.dev);
        }
        self.per_frame = ema(self.per_frame, read_ms / count as f32);
        self.pre = ema(self.pre, pre_ms.max(0.0));
        let (per, pre) = (self.per_frame.unwrap_or(1.0), self.pre.unwrap_or(0.0));
        // the read that leaves the batch a frame: the time before the fill, its spread, a fill of `CAP_FILL_MS` and the
        // margin taken off
        let frame_ms = period().as_secs_f32() * 1e3;
        let budget = frame_ms - MARGIN_MS - CAP_FILL_MS - pre - 2.0 * self.dev;
        let want = ((budget / per.max(0.01)).floor().max(1.0) as u32).min(asked.max(1));
        let new = (want < asked).then_some(want);
        let far = match (self.cap, new) {
            (Some(a), Some(b)) => (a as f32 - b as f32).abs() > 0.25 * a as f32,
            (a, b) => a != b,
        };
        if !keep || far {
            self.cap = new;
        }
    }
}
