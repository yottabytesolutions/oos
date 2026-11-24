// SPDX-License-Identifier: GPL-2.0-or-later

#[cfg(test)]
use super::test_scope;
use super::{metrics, route};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Frames acquired, used and dropped since startup, with placement and publication diagnostics.
#[derive(Default)]
pub struct Counters {
    pub acquired: AtomicU64,
    // Hardware first use is already recorded in shown.
    pub(crate) other_used: AtomicU64,
    /// frames whose cheap measurements went into the statistics (framestats.rs)
    pub measured: AtomicU64,
    /// Worker-completed placements and direct UI draws, before deducting deposit losses.
    pub placed: AtomicU64,
    pub shown: AtomicU64,
    recovered: AtomicU64,
    pub timing: crate::claim_timing::Timing,
    pub losses: Losses,
    pub acquisition: crate::board::AcquisitionCounters,
    pub(crate) metrics: metrics::Metrics,
    pub(crate) routes: route::RouteCounters,
    #[cfg(test)]
    pub(crate) diagnostics_disabled: std::sync::atomic::AtomicBool,
}

/// The process's counters (the acquisition thread, the workers and the UI add to them).
pub fn counters() -> &'static Counters {
    #[cfg(test)]
    if let Some(counters) = test_scope::counters() {
        return counters;
    }
    static C: OnceLock<Counters> = OnceLock::new();
    C.get_or_init(Counters::default)
}

/// Independent loss causes. Identity counts are frame counts; folds count measurement samples.
#[derive(Default)]
pub struct Losses {
    pub worker_push: AtomicU64,
    pub ui_publish: AtomicU64,
    pub queue_pop: AtomicU64,
    stale_batch: AtomicU64,
    pub overflow: AtomicU64,
    pub(super) unexpected_full: AtomicU64,
    pub(super) unexpected_busy: AtomicU64,
    pub(crate) record_handoff: RecordHandoff,
    pub folds: AtomicU64,
    /// frames looked at by the header log in its `off` mode (`board::dho::switches::HeaderProbe`), not a loss
    pub probed: AtomicU64,
    source: SourceLosses,
    rejected: [AtomicU64; Rejection::ALL.len()],
}

#[derive(Default)]
struct SourceLosses {
    other: [AtomicU64; Lost::ALL.len()],
    // Some sites for the same reason (e.g. a short read) also contribute to legacy dropped.
    dropped: [AtomicU64; Lost::ALL.len()],
    compose_error: AtomicU64,
}

impl SourceLosses {
    fn counts(&self, dropped: [u64; Lost::ALL.len()], compose_error: u64) -> [u64; Lost::ALL.len()] {
        let mut counts = std::array::from_fn(|i| load(&self.other[i]).wrapping_add(dropped[i]));
        counts[Lost::Error as usize] = counts[Lost::Error as usize].wrapping_add(compose_error);
        counts
    }
}

#[derive(Default)]
pub(crate) struct RecordHandoff {
    pub(crate) unexpected_queue: AtomicU64,
    pub(crate) sample_mismatch: AtomicU64,
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
enum Rejection {
    Mapping,
    Generation,
    Epoch,
}
impl Rejection {
    const ALL: [Self; 3] = [Self::Mapping, Self::Generation, Self::Epoch];

    fn between(a: Option<oos_core::change::Identity>, b: Option<oos_core::change::Identity>) -> Self {
        if a.map(|id| id.epoch()) != b.map(|id| id.epoch()) {
            Self::Epoch
        } else if !super::same_pair(a, b) {
            Self::Generation
        } else {
            Self::Mapping
        }
    }
}

fn load(value: &AtomicU64) -> u64 {
    value.load(Ordering::Relaxed)
}

fn sum(values: impl IntoIterator<Item = u64>) -> u64 {
    values.into_iter().fold(0, u64::wrapping_add)
}

impl Counters {
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
        let used = load(&self.other_used).wrapping_add(load(&self.shown));
        (load(&self.acquired), used, self.losses.snapshot().dropped())
    }

    pub(crate) fn picture_drawn(&self, shown: u64, direct: u64, recovered: u64) {
        Self::add(&self.shown, shown);
        if direct > 0 {
            Self::add(&self.placed, direct);
        }
        if recovered > 0 {
            Self::add(&self.recovered, recovered);
        }
    }
    pub(super) fn publish(&self, progress: &crate::claim_timing::Progress, frames: u64) {
        if progress.measured > 0 {
            Self::add(&self.measured, progress.measured);
        }
        if self.diagnostics_enabled() {
            Self::add(&self.placed, frames);
        }
    }

    pub(crate) fn lose(&self, reason: Lost, records: u64) {
        self.lose_selected(reason, records);
    }
    /// Counts one frame 1 left out and returns the `lost.frame1` total. Frame 1 has no discard site.
    pub(crate) fn lose_frame1(&self) -> u64 {
        self.lose_selected(Lost::Frame1, 1)
    }
    /// Returns the new selection half of `reason`, without the discards the reply adds to it.
    fn lose_selected(&self, reason: Lost, records: u64) -> u64 {
        if records == 0 || !self.diagnostics_enabled() {
            return 0;
        }
        self.losses.source.other[reason as usize].fetch_add(records, Ordering::Relaxed).wrapping_add(records)
    }
    pub(crate) fn discard(&self, reason: Lost, records: u64) {
        Self::add(&self.losses.source.dropped[reason as usize], records);
    }
    pub(crate) fn discard_compose_error(&self, records: u64) {
        self.late_loss(&self.losses.source.compose_error, records);
    }
    pub(super) fn discard_compose_overflow(&self, records: u64) {
        self.late_loss(&self.losses.overflow, records);
    }
    pub(super) fn reject_batch(&self, stale_identity: bool, records: u64) {
        if stale_identity {
            Self::add(&self.losses.stale_batch, records);
        } else {
            self.discard(Lost::Settings, records);
        }
    }
    pub(crate) fn reject_deposit(
        &self,
        previous: Option<oos_core::change::Identity>,
        current: Option<oos_core::change::Identity>,
        records: u64,
    ) {
        self.late_loss(&self.losses.rejected[Rejection::between(previous, current) as usize], records);
    }
    fn late_loss(&self, counter: &AtomicU64, records: u64) {
        if self.diagnostics_enabled() {
            counter.fetch_add(records, Ordering::Release);
        }
    }
    pub(crate) fn pipeline_text(&self, budget: std::time::Duration, step_missed: u64) -> String {
        let full = self.losses.unexpected_full.load(Ordering::Relaxed);
        let busy = self.losses.unexpected_busy.load(Ordering::Relaxed);
        let mut text = format!(
            " intake.unexpected_full={full} admission.unexpected_busy={busy}{} step.missed={step_missed}",
            self.metrics.text(&self.routes, budget),
        );
        self.routes.fallbacks.write(&mut text);
        text
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn capture_counts(&self) -> (u64, u64) {
        let source = &self.losses.source;
        let lost = Lost::ALL
            .into_iter()
            .filter(|reason| !matches!(reason, Lost::Settings | Lost::Superseded | Lost::Frame0))
            .flat_map(|reason| [&source.other[reason as usize], &source.dropped[reason as usize]]);
        (load(&self.shown), sum(lost.chain([&source.compose_error, &self.losses.overflow]).map(load)))
    }

    pub(crate) fn snapshot(&self) -> RecordSnapshot {
        let handoff = &self.losses.record_handoff;
        // Acquire late losses before loading placed so each observed loss brings its placement into view.
        let losses = self.losses.snapshot();
        let placed = load(&self.placed);
        let late_losses = sum(losses.rejected).wrapping_add(sum([losses.overflow, losses.compose_error]));
        debug_assert!(placed >= late_losses, "{placed} placements for {late_losses} late losses");
        let shown = load(&self.shown);
        RecordSnapshot {
            acquired: load(&self.acquired),
            used: load(&self.other_used).wrapping_add(shown),
            measured: load(&self.measured),
            deposited: placed.saturating_sub(late_losses),
            shown,
            recovered: load(&self.recovered),
            handoff: [load(&handoff.unexpected_queue), load(&handoff.sample_mismatch)],
            losses,
            #[cfg(feature = "private-picture-counts")]
            picture_counts: self.timing.picture_counts().text(),
        }
    }

    #[cfg(test)]
    pub(crate) fn records_text(&self) -> String {
        self.snapshot().records_text()
    }
}

struct LossSnapshot {
    source: [u64; Lost::ALL.len()],
    discarded: u64,
    compose_error: u64,
    worker_push: u64,
    ui_publish: u64,
    queue_pop: u64,
    stale_batch: u64,
    overflow: u64,
    folds: u64,
    rejected: [u64; Rejection::ALL.len()],
}

impl Losses {
    fn snapshot(&self) -> LossSnapshot {
        let dropped = self.source.dropped.each_ref().map(load);
        let late = |counter: &AtomicU64| counter.load(Ordering::Acquire);
        let compose_error = late(&self.source.compose_error);
        LossSnapshot {
            source: self.source.counts(dropped, compose_error),
            discarded: sum(dropped).wrapping_add(compose_error),
            compose_error,
            worker_push: load(&self.worker_push),
            ui_publish: load(&self.ui_publish),
            queue_pop: load(&self.queue_pop),
            stale_batch: load(&self.stale_batch),
            overflow: late(&self.overflow),
            folds: load(&self.folds),
            rejected: self.rejected.each_ref().map(late),
        }
    }
}

impl LossSnapshot {
    fn stale_before_deposit(&self) -> u64 {
        sum([self.worker_push, self.ui_publish, self.queue_pop, self.stale_batch])
    }

    fn dropped(&self) -> u64 {
        sum(self.rejected).wrapping_add(sum([self.discarded, self.stale_before_deposit(), self.overflow]))
    }

    fn stale_pipe(&self) -> u64 {
        sum([self.stale_batch, self.rejected[Rejection::Generation as usize], self.rejected[Rejection::Epoch as usize]])
    }
}

/// One read of each raw record counter for both state response formats.
pub(crate) struct RecordSnapshot {
    acquired: u64,
    used: u64,
    measured: u64,
    deposited: u64,
    shown: u64,
    recovered: u64,
    handoff: [u64; 2],
    losses: LossSnapshot,
    #[cfg(feature = "private-picture-counts")]
    picture_counts: String,
}

impl RecordSnapshot {
    pub(crate) fn legacy_totals(&self) -> (u64, u64, u64, u64) {
        (self.acquired, self.used, self.losses.dropped(), self.measured)
    }

    pub(crate) fn records_text(&self) -> String {
        use std::fmt::Write;
        let [unexpected_queue, sample_mismatch] = self.handoff;
        let mut text = format!(
            " records read={} deposited={} shown={} records.unexpected_queue={unexpected_queue} records.sample_mismatch={sample_mismatch}",
            self.acquired, self.deposited, self.shown
        );
        #[cfg(feature = "private-picture-counts")]
        text.push_str(&self.picture_counts);
        let _ = write!(text, " not_used={}", self.acquired.saturating_sub(self.used));
        let mut source = self.losses.source;
        source[Lost::Settings as usize] =
            source[Lost::Settings as usize].wrapping_add(self.losses.stale_before_deposit());
        for (reason, count) in Lost::ALL.into_iter().zip(source) {
            let _ = write!(text, " lost.{}={count}", reason.label());
        }
        let [mapping, generation, epoch] = self.losses.rejected;
        let _ = write!(
            text,
            " lost.queue_overflow={} rejected.stale_key={} rejected.epoch={epoch} records.recovered={}",
            self.losses.overflow,
            mapping.wrapping_add(generation),
            self.recovered
        );
        text
    }

    pub(crate) fn legacy_losses_text(&self) -> String {
        let LossSnapshot { worker_push, ui_publish, queue_pop, overflow, folds, .. } = &self.losses;
        format!(
            " stale_worker_push={worker_push} stale_ui_publish={ui_publish} stale_queue_pop={queue_pop} stale_pipe={} queue_overflow={overflow} fold_drops={folds}",
            self.losses.stale_pipe()
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
            counters.lose(reason, i as u64 + 1);
            counters.lose(reason, 1);
        }
        assert_eq!(counters.read().2, 0, "selection buckets preserve the legacy dropped aggregate");
        Counters::add(&counters.placed, 24);
        counters.reject_deposit(None, None, 11);
        counters.reject_deposit(None, Some(Default::default()), 13);
        let text = counters.records_text();
        for (i, label) in
            ["settings", "frame0", "average", "error", "buffer", "unclaimed", "budget", "superseded", "frame1"]
                .into_iter()
                .enumerate()
        {
            assert!(text.contains(&format!(" lost.{label}={} ", i + 2)), "{text}");
        }
        assert!(text.ends_with("rejected.stale_key=11 rejected.epoch=13 records.recovered=0"), "{text}");
        assert_eq!(counters.read().2, 24);
    }
    #[test]
    fn frame1_loss_returns_the_total_the_reply_reports() {
        let counters = super::super::Counters::default();
        assert_eq!(counters.lose_frame1(), 1);
        assert_eq!(counters.lose_frame1(), 2);
        assert!(counters.records_text().contains(" lost.frame1=2 "), "{}", counters.records_text());
        counters.diagnostics_disabled.store(true, Ordering::Relaxed);
        assert_eq!(counters.lose_frame1(), 0, "a disabled count logs nothing");
    }
}
