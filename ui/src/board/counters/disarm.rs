// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::sync::lock;
use std::{fmt::Write, sync::Mutex};

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
pub(in crate::board) enum Outcome {
    Unknown,
    NoCompletedRecord,
    Recorded,
}

#[derive(Debug, Default)]
pub(in crate::board) struct Counters(Mutex<Snapshot>);

#[derive(Debug, Default)]
struct Snapshot {
    unknown: u64,
    no_completed_record: u64,
    recorded: u64,
    dropped: u64,
}

impl Counters {
    #[cfg(any(target_os = "linux", test))]
    pub(in crate::board) fn observe(&self, outcome: Outcome) {
        let mut snapshot = lock(&self.0);
        let count = match outcome {
            Outcome::Unknown => &mut snapshot.unknown,
            Outcome::NoCompletedRecord => &mut snapshot.no_completed_record,
            Outcome::Recorded => &mut snapshot.recorded,
        };
        *count = count.saturating_add(1);
    }

    #[cfg(any(target_os = "linux", test))]
    pub(in crate::board) fn drop_replay(&self) {
        let mut snapshot = lock(&self.0);
        snapshot.dropped = snapshot.dropped.saturating_add(1);
    }

    pub(super) fn write(&self, text: &mut String) {
        let Snapshot { unknown, no_completed_record, recorded, dropped } = *lock(&self.0);
        let total = unknown.saturating_add(no_completed_record).saturating_add(recorded);
        let _ = write!(
            text,
            " disarm.v1 total={total} unknown={unknown} no_completed_record={no_completed_record} recorded={recorded} dropped={dropped}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_and_drop_counts_saturate() {
        let counters = Counters(Mutex::new(Snapshot {
            unknown: u64::MAX,
            no_completed_record: 1,
            recorded: 0,
            dropped: u64::MAX,
        }));
        counters.observe(Outcome::Unknown);
        counters.observe(Outcome::Recorded);
        counters.drop_replay();
        let mut text = String::new();
        counters.write(&mut text);
        let max = u64::MAX;
        assert_eq!(
            text,
            format!(" disarm.v1 total={max} unknown={max} no_completed_record=1 recorded=1 dropped={max}")
        );
    }
}
