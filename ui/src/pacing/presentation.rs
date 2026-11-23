// SPDX-License-Identifier: GPL-2.0-or-later

use super::{pct, push, state};
use std::{
    collections::VecDeque,
    fmt::Write,
    time::{Duration, Instant},
};

pub(crate) const FLIP_TIMEOUT: Duration = Duration::from_millis(50);

// The compose owner supplies up to five groups of four sampled batch clocks.
type Triggers = [Option<Instant>; 20];

#[derive(Clone, Copy)]
pub(crate) struct Picture {
    seal: Instant,
    composed: Instant,
    ready: Instant,
    helper: Option<(Instant, Instant)>,
    triggers: Triggers,
}

impl Picture {
    pub(super) fn new(seal: Instant) -> Self {
        Self { seal, composed: seal, ready: seal, helper: None, triggers: Default::default() }
    }

    pub(crate) fn composed(&mut self, at: Instant) {
        self.composed = at;
        self.triggers = oos_hal::sync::lock(state()).compose.presentation.triggers.take().unwrap_or_default();
    }

    pub(crate) fn composed_at(&self) -> Instant {
        self.composed
    }

    pub(crate) fn helper_done(&mut self, submitted: Instant, done: Instant) {
        self.helper = Some((submitted, done));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlipRequest {
    Busy,
    Queued,
    Modeset,
    Failed,
}

#[derive(Default)]
pub(crate) struct Panel {
    ready: Option<(usize, Picture)>,
    flipping: Option<(Instant, Picture)>,
    retry: Option<Instant>,
}

impl Panel {
    pub(crate) fn ready_for_picture(&self) -> bool {
        self.ready.is_none()
    }

    pub(crate) fn stage(&mut self, back: usize, mut picture: Picture, now: Instant) {
        debug_assert!(self.ready_for_picture());
        picture.ready = picture.helper.map_or(now, |(_, done)| done);
        self.ready = Some((back, picture));
    }

    pub(crate) fn wait_until(&self) -> Option<Instant> {
        self.retry.or_else(|| self.flipping.map(|(at, _)| at + FLIP_TIMEOUT))
    }

    pub(crate) fn service(
        &mut self,
        now: Instant,
        flipped: Option<Instant>,
        refresh: Duration,
        mut request: impl FnMut(usize) -> FlipRequest,
    ) {
        self.service_with(now, flipped, refresh, &mut request, &mut |report| {
            oos_hal::sync::lock(state()).compose.presentation.record(report);
        });
    }

    fn service_with(
        &mut self,
        now: Instant,
        flipped: Option<Instant>,
        refresh: Duration,
        request: &mut impl FnMut(usize) -> FlipRequest,
        report: &mut impl FnMut(Outcome<'_>),
    ) {
        if let Some((requested, picture)) = self.flipping {
            if let Some(at) = flipped {
                self.flipping = None;
                report(Outcome::Shown(&picture, requested, at, refresh));
            } else if now >= requested + FLIP_TIMEOUT {
                self.flipping = None;
                report(Outcome::Timeout);
            }
        }
        self.retry = None;
        if self.flipping.is_none() {
            if let Some((back, picture)) = self.ready.take() {
                match request(back) {
                    FlipRequest::Busy => {
                        self.ready = Some((back, picture));
                        self.retry = Some(now + refresh);
                    }
                    FlipRequest::Queued => self.flipping = Some((now, picture)),
                    FlipRequest::Modeset => report(Outcome::Modeset),
                    FlipRequest::Failed => report(Outcome::Failed),
                }
            }
        }
    }
}

enum Outcome<'a> {
    Shown(&'a Picture, Instant, Instant, Duration),
    Timeout,
    Modeset,
    Failed,
}

#[derive(Clone, Default)]
pub(super) struct History {
    triggers: Option<Triggers>,
    stages: VecDeque<(Instant, [Option<f32>; 6])>,
    trigger_flip: VecDeque<f32>,
    late: u64,
    timeout: u64,
    modeset: u64,
    failed: u64,
    shown: u64,
}

impl History {
    fn record(&mut self, outcome: Outcome<'_>) {
        match outcome {
            Outcome::Shown(picture, requested, flipped, refresh) => {
                self.shown += 1;
                self.late += u64::from(flipped.saturating_duration_since(picture.ready) > refresh);
                let mut stages = [None; 6];
                if let Some((submitted, done)) = picture.helper {
                    stages[..3].copy_from_slice(&[
                        micros(submitted, picture.composed),
                        micros(done, submitted),
                        micros(requested, done),
                    ]);
                }
                stages[3] = micros(flipped, requested);
                stages[4] = micros(flipped, picture.seal);
                stages[5] = micros(flipped, picture.ready);
                push(&mut self.stages, (flipped, stages));
                for trigger in picture.triggers.into_iter().flatten() {
                    if let Some(age) = micros(flipped, trigger) {
                        push(&mut self.trigger_flip, age);
                    }
                }
            }
            Outcome::Timeout => self.timeout += 1,
            Outcome::Modeset => self.modeset += 1,
            Outcome::Failed => self.failed += 1,
        }
    }

    fn write(&self, text: &mut String, helper_skipped: u64) {
        let hz = self.stages.front().zip(self.stages.back()).map_or(0.0, |(first, last)| {
            let elapsed = last.0.saturating_duration_since(first.0).as_secs_f32();
            if elapsed > 0.0 {
                (self.stages.len() - 1) as f32 / elapsed
            } else {
                0.0
            }
        });
        let _ = write!(
            text,
            " present.v1 flip.shown={} flip.late={} flip.timeout={} flip.modeset={} flip.failed={} flip.hz={hz:.1} helper.skipped={helper_skipped}",
            self.shown, self.late, self.timeout, self.modeset, self.failed
        );
        for (i, name) in
            ["composed_submitted", "submitted_done", "done_requested", "requested_flip", "seal_flip", "done_flip"]
                .into_iter()
                .enumerate()
        {
            quantiles(text, name, self.stages.iter().filter_map(|(_, row)| row[i]).collect());
        }
        quantiles(text, "trigger_flip", self.trigger_flip.iter().copied().collect());
    }
}

fn micros(end: Instant, start: Instant) -> Option<f32> {
    Some(end.checked_duration_since(start)?.as_micros() as f32)
}

fn quantiles(text: &mut String, name: &str, mut values: Vec<f32>) {
    let [p50, p90, p99] = [0.5, 0.9, 0.99].map(|q| pct(&mut values, q));
    let _ = write!(text, " {name}={p50:.0}/{p90:.0}/{p99:.0}");
}

pub(crate) fn begin_picture() {
    oos_hal::sync::lock(state()).compose.presentation.triggers = Some(Default::default());
}

pub(crate) fn note_picture_triggers(triggers: impl Iterator<Item = Instant>) {
    if let Some(times) = &mut oos_hal::sync::lock(state()).compose.presentation.triggers {
        for (slot, at) in times.iter_mut().filter(|slot| slot.is_none()).zip(triggers) {
            *slot = Some(at);
        }
    }
}

pub(crate) fn append_presentation(text: &mut String) {
    let history = oos_hal::sync::lock(state()).compose.presentation.clone();
    history.write(text, super::CLOCK.helper_skipped());
}

#[cfg(test)]
mod tests;
