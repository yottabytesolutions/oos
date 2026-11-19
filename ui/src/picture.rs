// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod deposit;

use crate::drawpipe::{Accepted, Counters, Stats};
use crate::wave::Phosphor;
use deposit::{Deposit, Handoff};
use oos_core::change::Identity;
use std::collections::VecDeque;
use std::time::Instant;

const MAX_DEPOSITS: usize = 3;

#[derive(Default)]
pub(crate) struct DepositWindow {
    deposits: VecDeque<Deposit>,
    target: f64,
}

impl DepositWindow {
    pub(crate) fn is_empty(&self) -> bool {
        self.deposits.is_empty()
    }

    pub(crate) fn idle(&mut self) {
        self.target *= 0.9;
    }

    pub(crate) fn reset(&mut self, slot: &Handoff) {
        for deposit in self.deposits.drain(..) {
            slot.recycle(deposit);
        }
        self.target = 0.0;
    }

    pub(crate) fn expire(&mut self, now: Instant, slot: &Handoff, valid: impl Fn(&Deposit) -> bool) -> bool {
        let mut changed = false;
        while let Some(old) = self.deposits.pop_front_if(|d| !valid(d) || d.extent.expired(now)) {
            slot.recycle(old);
            changed = true;
        }
        changed
    }

    pub(crate) fn push(&mut self, mut deposit: Deposit, slot: &Handoff) {
        if self.target == 0.0 {
            self.target = deposit.records as f64;
        } else {
            self.target += 0.1 * (deposit.records as f64 - self.target);
        }
        let target = self.target.round() as u64;
        let (mut records, mut hits) = (deposit.records, deposit.extent.hits());
        let mut keep = 0;
        for older in self.deposits.iter().rev().take(MAX_DEPOSITS - 1) {
            if records >= target
                || older.format() != deposit.format()
                || hits.saturating_add(older.extent.hits()) > deposit.format().hit_limit()
            {
                break;
            }
            records += older.records;
            hits += older.extent.hits();
            keep += 1;
        }
        let evict = self.deposits.len() - keep;
        for old in self.deposits.drain(..evict) {
            slot.recycle(old);
        }
        deposit.planes.sort_unstable_by_key(|band| (band.channel, band.first));
        self.deposits.push_back(deposit);
    }

    pub(crate) fn rebuild(&mut self, ph: &mut Phosphor) -> u64 {
        ph.clear_counts();
        let Some((newest, prior)) = self.deposits.make_contiguous().split_last_mut() else { return 0 };
        ph.set_format(newest.format());
        for (i, band) in newest.planes.iter_mut().enumerate() {
            band.fold_picture(std::array::from_fn(|j| prior.get(j).map(|deposit| &deposit.planes[i])), ph);
        }
        newest.records + prior.iter().map(|deposit| deposit.records).sum::<u64>()
    }
}

/// Account for newly accepted records once, even if their planes contribute to later pictures.
pub(crate) fn accept_deposit(
    deposit: &mut Deposit,
    stats: Stats<'_>,
    identity: Option<Identity>,
    counters: &Counters,
) -> Accepted {
    if deposit.records == 0 {
        return Accepted::default();
    }
    for (a, b) in stats.edges.iter_mut().zip(deposit.edges) {
        *a += b;
    }
    *stats.off = (stats.off.0 + deposit.off.0, stats.off.1.max(deposit.off.1));
    let folds = if deposit.identity == identity {
        std::mem::take(&mut deposit.folds)
    } else {
        deposit.discard_folds(counters);
        Vec::new()
    };
    Accepted {
        frames: deposit.records,
        triggered: deposit.triggered,
        folds,
        newest: deposit.extent.newest(),
        stale: deposit.progress.stale,
        ..Accepted::default()
    }
}
