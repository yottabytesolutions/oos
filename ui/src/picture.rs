// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod deposit;

use crate::drawpipe::{Accepted, Counters, Stats};
use crate::wave::Phosphor;
use deposit::{count::CountFormat, Deposit, DiffBand, Handoff};
use oos_core::change::Identity;
use std::collections::VecDeque;

const MAX_DEPOSITS: usize = 3;

/// Clearing is paid by the first write, never by an empty composition.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FirstDraw {
    Keep,
    Minimum,
    Replace,
}

impl FirstDraw {
    pub(crate) fn waiting(self) -> bool {
        self == Self::Replace
    }

    pub(crate) fn begin(&mut self) -> bool {
        std::mem::replace(self, Self::Keep) != Self::Keep
    }
}

/// Values for the overview and for the optional zoom detail of the same picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PerMap<T> {
    main: T,
    zoom: Option<T>,
}

impl<T> PerMap<T> {
    pub(crate) fn new(main: T, zoom: Option<T>) -> Self {
        Self { main, zoom }
    }

    pub(crate) fn main(self) -> T {
        self.main
    }

    pub(crate) fn zoom(self) -> Option<T> {
        self.zoom
    }

    pub(crate) fn into_parts(self) -> (T, Option<T>) {
        (self.main, self.zoom)
    }
}

impl PerMap<&mut Phosphor> {
    pub(crate) fn as_mut(&mut self) -> PerMap<&mut Phosphor> {
        PerMap::new(self.main, self.zoom.as_deref_mut())
    }

    pub(crate) fn sizes(&self) -> PerMap<(usize, usize)> {
        PerMap::new((self.main.w, self.main.h), self.zoom.as_ref().map(|ph| (ph.w, ph.h)))
    }
}

#[derive(Default)]
pub(crate) struct DepositWindow {
    deposits: VecDeque<Deposit>,
    target: f64,
}

impl DepositWindow {
    #[cfg(test)]
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

    pub(crate) fn remove_invalid(&mut self, slot: &Handoff, valid: impl Fn(&Deposit) -> bool) -> bool {
        let mut changed = false;
        while let Some(old) = self.deposits.pop_front_if(|d| !valid(d)) {
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
                || older.sizes() != deposit.sizes()
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
        deposit.sort_bands();
        self.deposits.push_back(deposit);
    }

    #[cfg(test)]
    pub(crate) fn rebuild(&mut self, ph: &mut Phosphor) -> u64 {
        self.rebuild_picture(PerMap::new(ph, None))
    }

    pub(crate) fn rebuild_picture(&mut self, picture: PerMap<&mut Phosphor>) -> u64 {
        let (main, mut zoom) = picture.into_parts();
        main.clear_counts();
        if let Some(ph) = zoom.as_deref_mut() {
            ph.clear_counts();
        }
        let Some((newest, prior)) = self.deposits.make_contiguous().split_last_mut() else { return 0 };
        let format = newest.format();
        let weight = newest.weights();
        fold_map(
            main,
            &mut newest.planes,
            std::array::from_fn(|j| prior.get(j).map(|d| d.planes.as_slice())),
            format,
            weight.main() + prior.iter().map(|d| d.weights().main()).sum::<f64>(),
        );
        if let Some((ph, planes)) = zoom.zip(newest.zoom_planes_mut()) {
            fold_map(
                ph,
                planes,
                std::array::from_fn(|j| prior.get(j).and_then(Deposit::zoom_planes)),
                format,
                weight.zoom().unwrap_or_default() + prior.iter().filter_map(|d| d.weights().zoom()).sum::<f64>(),
            );
        }
        newest.records + prior.iter().map(|deposit| deposit.records).sum::<u64>()
    }
}

fn fold_map(
    ph: &mut Phosphor,
    newest: &mut [DiffBand],
    earlier: [Option<&[DiffBand]>; 2],
    format: CountFormat,
    weight: f64,
) {
    ph.set_format(format);
    for (i, band) in newest.iter_mut().enumerate() {
        band.fold_picture(earlier.map(|bands| bands.map(|bands| &bands[i])), ph);
    }
    ph.add_weight(weight);
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
        #[cfg(feature = "private-picture-counts")]
        oldest: deposit.extent.eligible_oldest(),
        stale: deposit.progress.stale,
        ..Accepted::default()
    }
}

#[cfg(test)]
mod owner_mechanics_tests;
