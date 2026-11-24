// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::atomic::{AtomicU64, Ordering};

mod disarm;
#[cfg(any(target_os = "linux", test))]
pub(super) use disarm::Outcome;

/// Process-lifetime worker operation attempts, including failures. Never reset by acquisition changes.
#[derive(Debug, Default)]
pub struct AcquisitionCounters {
    events: [AtomicU64; 8],
    diagnostics: crate::perfguard::deep::Diagnostics,
    disarm: disarm::Counters,
}

#[derive(Clone, Copy)]
pub(super) enum Event {
    SegStart,
    SegAbort,
    RollStop,
    RollStart,
    BatchPrearm,
    BatchDisarm,
    Configure,
    SingleArm,
}

impl AcquisitionCounters {
    #[cfg(any(target_os = "linux", test))]
    pub(super) fn disarms(&self) -> &disarm::Counters {
        &self.disarm
    }

    pub(crate) fn write_disarms(&self, text: &mut String) {
        self.disarm.write(text);
    }

    pub(crate) fn diagnostics(&self) -> &crate::perfguard::deep::Diagnostics {
        &self.diagnostics
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn add(&self, event: Event) {
        // Saturation preserves monotonicity even at the lifetime limit.
        self.events[event as usize].update(Ordering::Relaxed, Ordering::Relaxed, |n| n.saturating_add(1));
    }

    pub fn text(&self) -> String {
        let n = |event: Event| self.events[event as usize].load(Ordering::Relaxed);
        format!(
            " acq_seg_start={} acq_seg_abort={} acq_roll_stop={} acq_roll_start={} acq_batch_prearm={} acq_batch_disarm={} acq_configure={} acq_single_arm={}",
            n(Event::SegStart), n(Event::SegAbort), n(Event::RollStop), n(Event::RollStart),
            n(Event::BatchPrearm), n(Event::BatchDisarm), n(Event::Configure), n(Event::SingleArm)
        )
    }
}

impl super::Shared {
    /// Queue clears at source boundaries, recovery and superseded sweeps are explicit record losses.
    pub(crate) fn clear_queue(&mut self, reason: crate::drawpipe::Lost) {
        crate::drawpipe::counters().lose(reason, self.queue.len() as u64);
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetime_limit_saturates_instead_of_wrapping() {
        let counters = AcquisitionCounters::default();
        counters.events[Event::SegStart as usize].store(u64::MAX - 1, Ordering::Relaxed);
        counters.add(Event::SegStart);
        counters.add(Event::SegStart);
        assert_eq!(counters.events[Event::SegStart as usize].load(Ordering::Relaxed), u64::MAX);
        assert!(counters.text().contains("acq_seg_abort=0"));
    }
}
