// SPDX-License-Identifier: GPL-2.0-or-later

//! Channel-parallel reduction with owned destination maps and shared, immutable producer shares.

use super::{add, count_folds, Acc, Counters, Phosphor, NCH};
use crate::ui::lifecycle::worker::Worker;
use std::sync::Arc;

type Job = (Vec<u16>, Arc<Vec<Acc>>, usize, bool);

#[derive(Default)]
pub(crate) struct Reduction {
    workers: Vec<Worker<Job, Vec<u16>>>,
}

impl Reduction {
    pub(crate) fn start(little: u64) -> Self {
        let mut workers = Vec::new();
        for channel in 0..NCH {
            match Worker::spawn(&format!("picture-channel{channel}"), move |job| {
                crate::placement::to_little_cores(little);
                reduce(job)
            }) {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    eprintln!("oos-ui: channel reduction worker: {error}");
                    return Self::default();
                }
            }
        }
        Self { workers }
    }

    pub(super) fn add(&mut self, ph: &mut Phosphor, shares: Vec<Acc>, clear: bool) -> Vec<Acc> {
        let clear = clear && !shares.is_empty();
        let shares = Arc::new(shares);
        for channel in 0..NCH {
            if !clear && !shares.iter().any(|a| a.used[channel]) {
                continue;
            }
            let job = (std::mem::take(&mut ph.map[channel]), shares.clone(), channel, clear);
            if let Some(worker) = self.workers.get_mut(channel) {
                if let Err(
                    std::sync::mpsc::TrySendError::Full(job) | std::sync::mpsc::TrySendError::Disconnected(job),
                ) = worker.submit(job)
                {
                    ph.map[channel] = reduce(job);
                }
            } else {
                ph.map[channel] = reduce(job);
            }
        }
        for (channel, worker) in self.workers.iter_mut().enumerate() {
            if let Some(result) = worker.drain().next() {
                ph.map[channel] = result.expect("channel reduction only performs infallible saturating additions");
            }
        }
        Arc::try_unwrap(shares).unwrap_or_else(|_| unreachable!("all channel jobs completed"))
    }
}

impl Acc {
    pub(super) fn add(&mut self, o: &Acc, counters: &Counters) {
        add(&mut self.ph, &o.ph, o.used);
        for (a, b) in self.used.iter_mut().zip(o.used) {
            *a |= b;
        }
        if self.identity == o.identity {
            crate::framestats::add_folds(&mut self.folds, &o.folds);
        } else if o.identity.map(|id| id.stats_gen()) > self.identity.map(|id| id.stats_gen()) {
            count_folds(counters, &self.folds);
            self.folds.clone_from(&o.folds);
            self.identity = o.identity;
        } else {
            count_folds(counters, &o.folds);
        }
        self.frames += o.frames;
        self.triggered += o.triggered;
        for (a, b) in self.edges.iter_mut().zip(o.edges) {
            *a += b;
        }
        self.off = (self.off.0 + o.off.0, self.off.1.max(o.off.1));
        for &b in &o.batches {
            self.note(b, o.newest);
        }
        self.stale += o.stale;
    }
    /// Frames of batch `id`, fed at `fed`, drawn into these maps.
    pub(super) fn note(&mut self, id: u64, fed: Option<std::time::Instant>) {
        if !self.batches.contains(&id) {
            self.batches.push(id);
        }
        self.newest = self.newest.max(fed);
    }
}

fn reduce((mut dst, shares, channel, clear): Job) -> Vec<u16> {
    if clear {
        dst.fill(0);
    }
    for share in shares.iter().filter(|a| a.used[channel]) {
        add_map(&mut dst, &share.ph.map[channel]);
    }
    dst
}

pub(super) fn add_map(dst: &mut [u16], src: &[u16]) {
    for (x, y) in dst.iter_mut().zip(src) {
        *x = x.saturating_add(*y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_reduction_preserves_saturated_pixels_and_returns_the_same_shares() {
        let mut parallel = Reduction::start(0);
        assert_eq!(parallel.workers.len(), NCH);
        for mask in 1..1 << NCH {
            let used = std::array::from_fn(|c| mask & (1 << c) != 0);
            let shares: Vec<_> = (0..5)
                .map(|i| {
                    let mut share = Acc::new(0, 16, 16, used);
                    for map in &mut share.ph.map {
                        map.fill(13_000 + i);
                    }
                    share
                })
                .collect();
            let pointers: Vec<_> = shares.iter().map(|a| a.ph.map.each_ref().map(|m| m.as_ptr())).collect();
            let mut expected = Phosphor::new(16, 16);
            expected.map.iter_mut().for_each(|m| m.fill(12));
            let mut actual = Phosphor::new(16, 16);
            actual.map.clone_from(&expected.map);
            for share in &shares {
                super::super::add(&mut expected, &share.ph, share.used);
            }
            let returned = parallel.add(&mut actual, shares, false);
            assert_eq!(actual.map, expected.map);
            assert_eq!(pointers, returned.iter().map(|a| a.ph.map.each_ref().map(|m| m.as_ptr())).collect::<Vec<_>>());
            assert!(returned.iter().all(|a| a.ph.map.iter().zip(used).all(|(m, on)| on || m.capacity() == 0)));
        }
    }
}
