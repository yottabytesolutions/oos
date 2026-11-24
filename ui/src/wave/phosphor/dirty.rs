// SPDX-License-Identifier: GPL-2.0-or-later

use super::RowBand;
use crate::model::NCH;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

/// A consumer's last publication. Keeping the identity alive also prevents allocator address reuse.
#[derive(Clone)]
pub(crate) struct Stamp {
    identity: Arc<()>,
    revision: u64,
    channel: usize,
}

/// Channel edits feed picture publication and, once a GPU consumer asks, retained upload history.
pub(super) struct Dirty {
    height: usize,
    pending: RowBand,
    revisions: [OnceLock<Revisions>; NCH],
}

impl Dirty {
    pub(super) fn new(height: usize) -> Self {
        Self { height, pending: RowBand::empty(), revisions: Default::default() }
    }

    #[inline]
    pub(super) fn mark(&mut self, channel: usize, rows: Range<usize>) {
        let rows = rows.start.min(self.height)..rows.end.min(self.height);
        self.pending.include(rows.clone());
        if let Some(revisions) = self.revisions[channel].get_mut() {
            revisions.mark(rows);
        }
    }

    pub(super) fn publish(&mut self) -> Range<usize> {
        std::mem::replace(&mut self.pending, RowBand::empty()).into_range()
    }

    /// Storage changes invalidate consumers even when the logical counts are preserved.
    pub(super) fn invalidate(&mut self) {
        for revisions in self.revisions.iter_mut().filter_map(OnceLock::get_mut) {
            revisions.mark(0..self.height);
        }
    }

    pub(super) fn since(&self, channel: usize, previous: Option<&Stamp>) -> (Stamp, Range<usize>) {
        self.revisions[channel].get_or_init(|| Revisions::new(self.height)).since(channel, previous)
    }
}

struct Revisions {
    identity: Arc<()>,
    revision: u64,
    rows: Vec<u64>,
}

impl Revisions {
    fn new(height: usize) -> Self {
        Self { identity: Arc::new(()), revision: 1, rows: vec![1; height] }
    }

    fn mark(&mut self, rows: Range<usize>) {
        if rows.is_empty() {
            return;
        }
        self.revision = self.revision.checked_add(1).unwrap_or_else(|| {
            self.identity = Arc::new(());
            self.rows.fill(0);
            1
        });
        self.rows[rows].fill(self.revision);
    }

    fn since(&self, channel: usize, previous: Option<&Stamp>) -> (Stamp, Range<usize>) {
        let stamp = Stamp { identity: self.identity.clone(), revision: self.revision, channel };
        let rows = &self.rows;
        let Some(old) = previous.filter(|old| old.channel == channel && Arc::ptr_eq(&old.identity, &self.identity))
        else {
            return (stamp, 0..rows.len());
        };
        let first = rows.iter().position(|&revision| revision > old.revision);
        let band = first.map_or(0..0, |first| {
            first..rows.iter().rposition(|&revision| revision > old.revision).map_or(first, |last| last + 1)
        });
        (stamp, band)
    }
}

#[cfg(test)]
mod tests;
