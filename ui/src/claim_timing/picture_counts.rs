// SPDX-License-Identifier: GPL-2.0-or-later
//! Private lifetime composition counts and bounded software-boundary observations.
//!
//! A fresh picture is one that composes at least one record from a deposit: a record read by a batch (with its
//! `read_at`), placed by the pool into the picture planes and accepted by the owner. Placed means a span or dot set was
//! written; a record whose samples are all off screen or invalid is placed with zero hits and still counts. Records
//! drawn directly on the UI thread (legacy queue, record route) have no batch read time: they count as
//! `direct_records`, never as fresh.
use super::histogram::Histogram;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct PictureCounts(Mutex<Totals>);
#[derive(Default)]
struct Totals {
    origin: Option<Instant>,
    eligible: u64,
    fresh: u64,
    empty: u64,
    records: u64,
    direct: u64,
    pending: Option<Instant>,
    missing: u64,
    accepted: Histogram,
    first: Histogram,
    published: Histogram,
}
impl PictureCounts {
    /// Same historic newest accepted-feed endpoint, before retained rebuild.
    pub(crate) fn accepted(&self, age: Duration) {
        oos_hal::sync::lock(&self.0).accepted.add(age);
    }
    /// One eligible picture: `deposit` records composed from deposits, whose oldest batch read is `oldest`, and
    /// `direct` records drawn without a batch read time.
    pub(crate) fn observed(&self, deposit: u64, oldest: Option<Instant>, direct: u64) {
        let now = Instant::now();
        self.observe_at(deposit, oldest, now);
        let mut t = oos_hal::sync::lock(&self.0);
        t.direct = t.direct.saturating_add(direct);
    }
    fn observe_at(&self, records: u64, oldest: Option<Instant>, now: Instant) {
        let mut t = oos_hal::sync::lock(&self.0);
        t.origin.get_or_insert(now);
        t.eligible = t.eligible.saturating_add(1);
        t.records = t.records.saturating_add(records);
        t.pending = None;
        if records == 0 {
            t.empty = t.empty.saturating_add(1);
        } else {
            t.fresh = t.fresh.saturating_add(1);
            if let Some(oldest) = oldest {
                t.first.add(now.saturating_duration_since(oldest));
                t.pending = Some(oldest);
            } else {
                t.missing = t.missing.saturating_add(1);
            }
        }
    }
    /// After ph/phz.publish returns; CPU maps published, not GPU completion or scanout.
    pub(crate) fn published(&self) {
        self.publish_at(Instant::now());
    }
    fn publish_at(&self, now: Instant) {
        let mut t = oos_hal::sync::lock(&self.0);
        if let Some(oldest) = t.pending.take() {
            t.published.add(now.saturating_duration_since(oldest));
        }
    }
    #[cfg(test)]
    pub(crate) fn composed(&self, records: u64) {
        self.observed(records, None, 0);
    }
    pub(crate) fn text(&self) -> String {
        let t = oos_hal::sync::lock(&self.0);
        let ns = t.origin.map_or(0, |o| o.elapsed().as_nanos());
        format!(" fresh.v1 ns={ns} eligible={} fresh={} empty={} first_records={} panel=unknown latency.v1 width_ns=1000000 overflow_bin=255 missing={} accepted={} first={} published={} gpu=unknown direct_records={}",
            t.eligible, t.fresh, t.empty, t.records, t.missing, t.accepted.text(), t.first.text(), t.published.text(), t.direct)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_repeated_getters_and_positive_mix_are_exact() {
        let c = PictureCounts::default();
        let now = Instant::now();
        for n in [0, 23, 0, 7] {
            c.observe_at(n, None, now);
            c.text();
            c.text();
            c.publish_at(now);
        }
        let t = oos_hal::sync::lock(&c.0);
        assert_eq!((t.eligible, t.fresh, t.empty, t.records, t.missing), (4, 2, 2, 30, 2));
        assert_eq!(t.first.bins().iter().sum::<u64>(), 0);
    }
    #[test]
    fn first_and_publish_are_distinct_once_and_redraw_cannot_reuse_pending() {
        let c = PictureCounts::default();
        let feed = Instant::now();
        c.observe_at(7, Some(feed), feed + Duration::from_millis(12));
        c.publish_at(feed + Duration::from_millis(18));
        c.publish_at(feed + Duration::from_secs(3));
        c.observe_at(0, Some(feed), feed + Duration::from_millis(20));
        c.publish_at(feed + Duration::from_millis(21));
        let t = oos_hal::sync::lock(&c.0);
        assert_eq!(t.first.bins()[12], 1);
        assert_eq!(t.published.bins()[18], 1);
        assert_eq!(t.published.bins().iter().sum::<u64>(), 1);
        assert_eq!(t.empty, 1);
    }
    #[test]
    fn direct_records_never_make_a_picture_fresh() {
        let c = PictureCounts::default();
        c.observed(0, None, 9);
        c.observed(4, Some(Instant::now()), 2);
        let t = oos_hal::sync::lock(&c.0);
        assert_eq!((t.eligible, t.fresh, t.empty, t.records, t.direct, t.missing), (2, 1, 1, 4, 11, 0));
        drop(t);
        assert!(c.text().ends_with(" gpu=unknown direct_records=11"));
    }
    #[test]
    fn histogram_overflow_is_explicit_and_query_does_not_reset() {
        let c = PictureCounts::default();
        c.accepted(Duration::from_secs(2));
        assert!(c.text().contains("overflow_bin=255"));
        assert_eq!(oos_hal::sync::lock(&c.0).accepted.bins()[255], 1);
    }
}
