// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::atomic::{AtomicU64, Ordering};

/// Independent loss causes. Identity counts are frame counts; folds count measurement samples.
#[derive(Default)]
pub struct Losses {
    pub worker_push: AtomicU64,
    pub ui_publish: AtomicU64,
    pub queue_pop: AtomicU64,
    pub pipe: AtomicU64,
    pub overflow: AtomicU64,
    pub folds: AtomicU64,
    /// frame 1 of a batch left out without its edge at the trigger point (`board::dho::frame1`)
    pub frame1: AtomicU64,
    /// frames looked at by the header log in its `off` mode (`board::dho::switches::HeaderProbe`), not a loss
    pub probed: AtomicU64,
}

impl Losses {
    pub fn text(&self) -> String {
        let n = |a: &AtomicU64| a.load(Ordering::Relaxed);
        format!(
            " stale_worker_push={} stale_ui_publish={} stale_queue_pop={} stale_pipe={} queue_overflow={} fold_drops={}",
            n(&self.worker_push), n(&self.ui_publish), n(&self.queue_pop), n(&self.pipe), n(&self.overflow), n(&self.folds)
        )
    }
}
