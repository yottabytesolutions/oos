// SPDX-License-Identifier: GPL-2.0-or-later

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

/// Workers never publish their maps. Allocate and maintain history only after a GPU consumer first asks for it.
pub(super) struct Dirty {
    height: usize,
    revisions: OnceLock<Box<Revisions>>,
}

impl Dirty {
    pub(super) fn new(height: usize) -> Self {
        Self { height, revisions: OnceLock::new() }
    }

    #[inline]
    pub(super) fn mark(&mut self, channel: usize, rows: Range<usize>) {
        if let Some(revisions) = self.revisions.get_mut() {
            revisions.mark(channel, rows);
        }
    }

    pub(super) fn since(&self, channel: usize, previous: Option<&Stamp>) -> (Stamp, Range<usize>) {
        self.revisions.get_or_init(|| Box::new(Revisions::new(self.height))).since(channel, previous)
    }
}

struct Revisions {
    identity: Arc<()>,
    revision: u64,
    rows: [Vec<u64>; NCH],
}

impl Revisions {
    fn new(height: usize) -> Self {
        Self { identity: Arc::new(()), revision: 1, rows: std::array::from_fn(|_| vec![1; height]) }
    }

    fn mark(&mut self, channel: usize, rows: Range<usize>) {
        let height = self.rows[channel].len();
        let rows = rows.start.min(height)..rows.end.min(height);
        if rows.is_empty() {
            return;
        }
        self.revision = self.revision.checked_add(1).unwrap_or_else(|| {
            self.identity = Arc::new(());
            for rows in &mut self.rows {
                rows.fill(0);
            }
            1
        });
        self.rows[channel][rows].fill(self.revision);
    }

    fn since(&self, channel: usize, previous: Option<&Stamp>) -> (Stamp, Range<usize>) {
        let stamp = Stamp { identity: self.identity.clone(), revision: self.revision, channel };
        let rows = &self.rows[channel];
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
