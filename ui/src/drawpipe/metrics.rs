// SPDX-License-Identifier: GPL-2.0-or-later

use super::clock::{BatchClocks, StageClock};
use crate::claim_timing::histogram::Histogram;
use crate::perfguard::ring::Ring;
use oos_core::change::Identity;
use std::fmt::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) mod fallback;
mod source;
#[cfg(any(target_os = "linux", test))]
pub(crate) use source::SourceBatch;

const STAGES: [&str; 7] = [
    "read_submit",
    "submit_dequeue",
    "dequeue_deposited",
    "deposited_published",
    "published_claimed",
    "claimed_composed",
    "trigger_composed",
];

#[derive(Default)]
pub(crate) struct Metrics(Mutex<Current>);

#[derive(Default)]
struct Current {
    identity: Option<Identity>,
    accepted: [Histogram; STAGES.len()],
    rejected: [Histogram; STAGES.len()],
    truncated: u64,
    work: Window<4>,
    worker_time: Option<(u64, u64)>,
    #[cfg(test)]
    test_worker_time: Option<u64>,
    source: source::Samples,
}

#[derive(Default)]
pub(super) struct Work {
    pub(super) refine: Duration,
    pub(super) envelope: Duration,
    pub(super) tiles: Duration,
    pub(super) pool: Duration,
}

impl Metrics {
    pub(crate) fn set_identity(&self, identity: Identity) {
        let mut current = oos_hal::sync::lock(&self.0);
        if !super::same_pair(current.identity, Some(identity)) {
            *current = Current { identity: Some(identity), ..Current::default() };
        }
    }

    fn current(&self, identity: Option<Identity>) -> Option<std::sync::MutexGuard<'_, Current>> {
        let current = oos_hal::sync::lock(&self.0);
        super::same_pair(identity, current.identity).then_some(current)
    }

    pub(super) fn work(&self, identity: Option<Identity>, work: &Work, records: u64, truncated: bool) {
        if let Some(mut current) = self.current(identity) {
            current.truncated += u64::from(truncated);
            if records > 0 {
                let times = [work.refine, work.envelope, work.tiles, work.pool].map(nanos);
                let worker_time = times[0].saturating_add(times[1]).saturating_add(times[2]);
                #[cfg(test)]
                let worker_time = current.test_worker_time.unwrap_or(worker_time);
                current.worker_time = Some((records, worker_time));
                current.work.add(times.map(|ns| Some(ns / records)));
            }
        }
    }

    pub(super) fn worker_time(&self, identity: Identity, records: u64) -> Option<u64> {
        let (count, work) = self.current(Some(identity))?.worker_time.take()?;
        (count == records && work != 0).then_some(work)
    }

    #[cfg(test)]
    pub(crate) fn inject_worker_time(&self, identity: Identity, work: Duration) {
        if let Some(mut current) = self.current(Some(identity)) {
            current.test_worker_time = Some(nanos(work));
        }
    }

    pub(super) fn discarded(&self, identity: Option<Identity>, clock: StageClock) {
        let at = Instant::now();
        if let Some(mut current) = self.current(identity) {
            record(&mut current.rejected, clock.outcome(at));
        }
    }

    pub(super) fn composed(&self, identity: Option<Identity>, clocks: &[BatchClocks]) {
        self.outcome(identity, clocks, Instant::now, true);
    }

    pub(crate) fn finish(&self, identity: Option<Identity>, clocks: &BatchClocks, at: Instant, accepted: bool) {
        self.outcome(identity, std::slice::from_ref(clocks), || at, accepted);
    }

    fn outcome(
        &self,
        identity: Option<Identity>,
        clocks: &[BatchClocks],
        at: impl FnOnce() -> Instant,
        accepted: bool,
    ) {
        if clocks.iter().all(BatchClocks::is_empty) {
            return;
        }
        let at = at();
        if let Some(mut current) = self.current(identity) {
            let histograms = if accepted { &mut current.accepted } else { &mut current.rejected };
            let outcomes = || clocks.iter().flat_map(|clocks| clocks.outcome(at));
            if accepted {
                crate::pacing::note_picture_triggers(outcomes().filter_map(|intervals| Some(at - intervals[6]?)));
            }
            for intervals in outcomes() {
                record(histograms, intervals);
            }
        }
    }

    pub(super) fn text(&self, routes: &super::route::RouteCounters, budget: Duration) -> String {
        let current = oos_hal::sync::lock(&self.0);
        let mut text = String::from(" stage.v1");
        for (prefix, histograms) in [("", &current.accepted), ("rejected.", &current.rejected)] {
            for (name, histogram) in STAGES.into_iter().zip(histograms) {
                let [p50, p90, p99] = histogram.quantiles_us();
                let _ = write!(text, " {prefix}{name}={p50}/{p90}/{p99}");
            }
        }
        let _ = write!(text, " stage.truncated={}", current.truncated);
        crate::pacing::append_presentation(&mut text);
        text.push_str(" work.v1");
        for (column, name) in ["refine", "envelope", "tiles", "pool"].into_iter().enumerate() {
            let [p50, p90] = current.work.quantiles(column);
            let _ = write!(text, " {name}={p50}/{p90}");
        }
        routes.write(&mut text);
        current.source.write(&mut text, budget);
        text
    }
}

fn record(histograms: &mut [Histogram; STAGES.len()], intervals: [Option<Duration>; STAGES.len()]) {
    for (histogram, elapsed) in histograms.iter_mut().zip(intervals) {
        if let Some(elapsed) = elapsed {
            histogram.add(elapsed);
        }
    }
}

type Window<const N: usize> = Ring<64, [Option<u64>; N]>;

pub(super) fn nanos(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
