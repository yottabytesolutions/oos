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
    /// frames looked at by the header log in its `off` mode (`board::dho::switches::HeaderProbe`), not a loss
    pub probed: AtomicU64,
    source: [AtomicU64; Lost::ALL.len()],
    rejected: [AtomicU64; Rejection::ALL.len()],
}

/// Existing discard sites; buffer and unclaimed losses disappear with F3 back-pressure.
#[derive(Clone, Copy)]
pub(crate) enum Lost {
    Settings,
    Frame0,
    Average,
    Error,
    Buffer,
    Unclaimed,
    Budget,
    Superseded,
    Frame1,
}
impl Lost {
    const ALL: [Self; 9] = [
        Self::Settings,
        Self::Frame0,
        Self::Average,
        Self::Error,
        Self::Buffer,
        Self::Unclaimed,
        Self::Budget,
        Self::Superseded,
        Self::Frame1,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Frame0 => "frame0",
            Self::Average => "average",
            Self::Error => "error",
            Self::Buffer => "buffer",
            Self::Unclaimed => "unclaimed",
            Self::Budget => "budget",
            Self::Superseded => "superseded",
            Self::Frame1 => "frame1",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Rejection {
    StaleKey,
    Epoch,
}
impl Rejection {
    const ALL: [Self; 2] = [Self::StaleKey, Self::Epoch];

    fn label(self) -> &'static str {
        match self {
            Self::StaleKey => "stale_key",
            Self::Epoch => "epoch",
        }
    }

    pub(super) fn between(a: Option<oos_core::change::Identity>, b: Option<oos_core::change::Identity>) -> Self {
        if a.map(|id| id.epoch()) != b.map(|id| id.epoch()) {
            Self::Epoch
        } else {
            Self::StaleKey
        }
    }
}

impl super::Counters {
    pub(crate) fn diagnostics_enabled(&self) -> bool {
        #[cfg(test)]
        return !self.diagnostics_disabled.load(Ordering::Relaxed);
        #[cfg(not(test))]
        true
    }
    pub fn add(c: &AtomicU64, n: u64) {
        c.fetch_add(n, Ordering::Relaxed);
    }
    /// (acquired, used, dropped)
    pub fn read(&self) -> (u64, u64, u64) {
        (self.acquired.load(Ordering::Relaxed), self.used.load(Ordering::Relaxed), self.dropped.load(Ordering::Relaxed))
    }
    pub(super) fn publish(&self, progress: &crate::claim_timing::Progress, frames: u64) {
        if progress.measured > 0 {
            Self::add(&self.measured, progress.measured);
        }
        if self.diagnostics_enabled() {
            Self::add(&self.deposited, frames);
        }
    }

    pub(crate) fn lose(&self, reason: Lost, records: u64) -> u64 {
        if records == 0 || !self.diagnostics_enabled() {
            return 0;
        }
        self.losses.source[reason as usize].fetch_add(records, Ordering::Relaxed).wrapping_add(records)
    }
    pub(super) fn discard(&self, reason: Lost, records: u64) {
        Self::add(&self.dropped, records);
        self.lose(reason, records);
    }
    pub(super) fn reject(&self, reason: Rejection, records: u64) {
        if self.diagnostics_enabled() {
            Self::add(&self.losses.rejected[reason as usize], records);
        }
    }
    pub(crate) fn reject_deposit(
        &self,
        previous: Option<oos_core::change::Identity>,
        current: Option<oos_core::change::Identity>,
        records: u64,
    ) {
        if !super::same_pair(previous, current) {
            Self::add(&self.losses.pipe, records);
        }
        self.reject(Rejection::between(previous, current), records);
        Self::add(&self.dropped, records);
    }
    pub(crate) fn records_text(&self) -> String {
        let n = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let mut text =
            format!(" records read={} deposited={} shown={}", n(&self.acquired), n(&self.deposited), n(&self.shown));
        let (acquired, used, _) = self.read();
        text.push_str(&format!(" not_used={}", acquired.saturating_sub(used)));
        let mut source = self.losses.source.each_ref().map(n);
        source[Lost::Settings as usize] +=
            n(&self.losses.worker_push) + n(&self.losses.ui_publish) + n(&self.losses.queue_pop);
        for reason in Lost::ALL {
            let count = source[reason as usize];
            text.push_str(&format!(" lost.{}={count}", reason.label()));
        }
        text.push_str(&format!(" lost.queue_overflow={}", n(&self.losses.overflow)));
        for reason in Rejection::ALL {
            text.push_str(&format!(" rejected.{}={}", reason.label(), n(&self.losses.rejected[reason as usize])));
        }
        text
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_loss_and_rejection_keeps_its_own_label_and_count() {
        let counters = super::super::Counters::default();
        for (i, reason) in Lost::ALL.into_iter().enumerate() {
            let count = i as u64 + 1;
            assert_eq!(counters.lose(reason, count), count);
            assert_eq!(counters.lose(reason, 1), count + 1);
        }
        counters.reject(Rejection::StaleKey, 11);
        counters.reject(Rejection::Epoch, 13);
        let text = counters.records_text();
        for (i, label) in
            ["settings", "frame0", "average", "error", "buffer", "unclaimed", "budget", "superseded", "frame1"]
                .into_iter()
                .enumerate()
        {
            assert!(text.contains(&format!(" lost.{label}={} ", i + 2)), "{text}");
        }
        assert!(text.ends_with("rejected.stale_key=11 rejected.epoch=13"), "{text}");
        assert_eq!(counters.read().2, 0, "selection buckets preserve the legacy dropped aggregate");
    }
}
