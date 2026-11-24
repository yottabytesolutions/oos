// SPDX-License-Identifier: GPL-2.0-or-later

//! Sampled claims travel with deposits. The picture owner updates the shared timing rings.

pub(crate) mod histogram;
#[cfg(feature = "private-picture-counts")]
mod picture_counts;
use crate::perfguard::ring::Ring;
use oos_hal::sync::lock;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One observation every 64 claims, with no shared state on the producer path.
#[derive(Default)]
pub(crate) struct Sampler(usize);
impl Sampler {
    pub(crate) fn start(&mut self) -> Option<Instant> {
        self.0 = self.0.wrapping_add(1);
        self.0.is_multiple_of(64).then(Instant::now)
    }
}

#[derive(Default)]
pub(crate) struct Progress {
    pub(crate) stale: u64,
    pub(crate) measured: u64,
    pub(crate) sample: Option<(Duration, u64)>,
}

#[cfg(not(feature = "private-picture-counts"))]
#[derive(Default)]
pub struct Timing(Mutex<Rings>);
#[cfg(feature = "private-picture-counts")]
#[derive(Default)]
pub struct Timing(Mutex<Rings>, picture_counts::PictureCounts);

#[derive(Clone, Default)]
struct Rings {
    slice: Ring,
    record: Ring,
    batch: Ring,
}
impl Timing {
    pub(crate) fn picture(&self, sample: Option<(Duration, u64)>, age: Option<Duration>) {
        if sample.is_none() && age.is_none() {
            return;
        }
        let mut rings = lock(&self.0);
        if let Some((elapsed, records)) = sample {
            let ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
            rings.slice.add(ns);
            rings.record.add(ns / records.max(1));
        }
        if let Some(age) = age {
            rings.batch.add(age.as_nanos().min(u64::MAX as u128) as u64);
        }
        #[cfg(feature = "private-picture-counts")]
        {
            drop(rings);
            if let Some(age) = age {
                self.1.accepted(age);
            }
        }
    }
    #[cfg(feature = "private-picture-counts")]
    pub(crate) fn picture_counts(&self) -> &picture_counts::PictureCounts {
        &self.1
    }
    pub(crate) fn text(&self) -> String {
        let rings = lock(&self.0).clone();
        rings.slice.format("deposit.slice", "ns")
            + &rings.record.format("deposit.record", "ns")
            + &rings.batch.format("deposit.batch", "ns")
    }
}

pub fn summary() -> String {
    crate::drawpipe::counters().timing.text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_every_sixty_fourth_claim_reads_the_clock() {
        let mut sampler = Sampler::default();
        for claim in 1..=256 {
            assert_eq!(sampler.start().is_some(), claim % 64 == 0);
        }
    }

    #[test]
    fn claim_rings_use_supplied_duration_and_record_count_without_per_record_clocks() {
        let timing = Timing::default();
        for i in 1..=400 {
            timing.picture(Some((Duration::from_nanos(i * 4), 4)), Some(Duration::from_nanos(i)));
        }
        let text = timing.text();
        assert!(text.contains("deposit.slice n=256 total=400 p50_ns=1088 p90_ns=1500 p99_ns=1592 max_ns=1600"));
        assert!(text.contains("deposit.record n=256 total=400 p50_ns=272 p90_ns=375 p99_ns=398 max_ns=400"));
        assert!(text.contains("deposit.batch n=256 total=400 p50_ns=272 p90_ns=375 p99_ns=398 max_ns=400"));
        #[cfg(not(feature = "private-picture-counts"))]
        assert!(std::mem::size_of::<Timing>() < 7 * 1024);
        #[cfg(feature = "private-picture-counts")]
        assert!(std::mem::size_of::<picture_counts::PictureCounts>() <= 6144 + 256);
    }
}
