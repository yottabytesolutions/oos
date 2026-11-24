// SPDX-License-Identifier: GPL-2.0-or-later

use super::{frame_clock::Flip, pct, push, state};
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
    target: Option<u32>,
    composed: Instant,
    helper: Option<(Instant, Instant)>,
    triggers: Triggers,
}

impl Picture {
    pub(super) fn new(seal: Instant, target: Option<u32>) -> Self {
        Self { seal, target, composed: seal, helper: None, triggers: Default::default() }
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

    pub(crate) fn stage(&mut self, back: usize, picture: Picture) {
        debug_assert!(self.ready_for_picture());
        self.ready = Some((back, picture));
    }

    pub(crate) fn wait_until(&self) -> Option<Instant> {
        self.retry.or_else(|| self.flipping.map(|(at, _)| at + FLIP_TIMEOUT))
    }

    pub(crate) fn service(
        &mut self,
        now: Instant,
        event: Option<Flip>,
        refresh: Duration,
        mut request: impl FnMut(usize) -> FlipRequest,
    ) {
        self.service_with(now, event, refresh, &mut request, &mut |report| {
            oos_hal::sync::lock(state()).compose.presentation.record(report);
        });
    }

    fn service_with(
        &mut self,
        now: Instant,
        event: Option<Flip>,
        refresh: Duration,
        request: &mut impl FnMut(usize) -> FlipRequest,
        report: &mut impl FnMut(Outcome<'_>),
    ) {
        if let Some((requested, picture)) = self.flipping {
            if let Some(flip) = event {
                self.flipping = None;
                report(Outcome::Shown(&picture, requested, flip));
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
    Shown(&'a Picture, Instant, Flip),
    Timeout,
    Modeset,
    Failed,
}

#[derive(Clone, Default)]
pub(super) struct History {
    triggers: Option<Triggers>,
    stages: VecDeque<[Option<f32>; 5]>,
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
            Outcome::Shown(picture, requested, flip) => {
                self.shown += 1;
                self.late +=
                    u64::from(picture.target.is_some_and(|target| (flip.sequence.wrapping_sub(target) as i32) > 0));
                let mut stages = [None; 5];
                if let Some((submitted, done)) = picture.helper {
                    stages[..3].copy_from_slice(&[
                        micros(submitted, picture.composed),
                        micros(done, submitted),
                        micros(requested, done),
                    ]);
                }
                stages[3] = micros(flip.at, requested);
                stages[4] = micros(flip.at, picture.seal);
                push(&mut self.stages, stages);
                for trigger in picture.triggers.into_iter().flatten() {
                    if let Some(age) = micros(flip.at, trigger) {
                        push(&mut self.trigger_flip, age);
                    }
                }
            }
            Outcome::Timeout => self.timeout += 1,
            Outcome::Modeset => self.modeset += 1,
            Outcome::Failed => self.failed += 1,
        }
    }

    fn write(&self, text: &mut String) {
        let _ = write!(
            text,
            " present.v1 flip.shown={} flip.late={} flip.timeout={} flip.modeset={} flip.failed={}",
            self.shown, self.late, self.timeout, self.modeset, self.failed
        );
        for (i, name) in ["composed_submitted", "submitted_done", "done_requested", "requested_flip", "seal_flip"]
            .into_iter()
            .enumerate()
        {
            quantiles(text, name, self.stages.iter().filter_map(|row| row[i]).collect());
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
    history.write(text);
}

#[cfg(test)]
mod tests;
