// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::Slot;
use super::*;

use crate::picture::deposit::admission::Share;
use std::time::Duration;
#[cfg(any(target_os = "linux", test))]
use std::time::Instant;

impl TapReservation {
    pub(crate) fn fallback(&self) {
        if self.funded() && self.pending.swap(false, Ordering::Relaxed) {
            if let Some(handoff) = self.handoff.upgrade() {
                let mut state = lock(&handoff.state);
                self.dequeued();
                state.taps.reserved -= 1;
                handoff.space.notify_all();
            }
        }
    }
}

impl Handoff {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_tap(
        self: &Arc<Self>,
        samples: usize,
        submitted: u64,
        identity: Option<Identity>,
    ) -> Option<Arc<TapReservation>> {
        self.reserve(samples, submitted, identity, None)
    }

    #[cfg(any(target_os = "linux", test))]
    fn reserve(
        self: &Arc<Self>,
        samples: usize,
        submitted: u64,
        identity: Option<Identity>,
        plan: Option<Arc<Plan>>,
    ) -> Option<Arc<TapReservation>> {
        let mut state = self.wait_for_tap(samples, submitted, identity, plan.as_ref(), true)?;
        if let Some(plan) = &plan {
            plan.reserve();
        } else {
            state.taps.samples += samples;
        }
        state.taps.reserved += 1;
        state.taps.charged += 1;
        Some(Arc::new(TapReservation {
            handoff: Arc::downgrade(self),
            samples,
            submitted,
            pending: AtomicBool::new(true),
            queued: AtomicBool::new(plan.is_some()),
            plan,
        }))
    }

    fn current(state: &Slot, submitted: u64, identity: Option<Identity>, plan: Option<&Arc<Plan>>) -> bool {
        !state.closed
            && submitted >= state.reset_before
            && identity.is_none_or(|id| crate::drawpipe::same_pair(state.taps.identity, Some(id)))
            && plan.map_or(!state.taps.admission.staged, |plan| {
                state.taps.admission.plan.as_ref().is_some_and(|p| Arc::ptr_eq(p, plan))
            })
    }

    pub(super) fn wait_for_tap(
        &self,
        samples: usize,
        submitted: u64,
        identity: Option<Identity>,
        plan: Option<&Arc<Plan>>,
        cancel_on_hold: bool,
    ) -> Option<std::sync::MutexGuard<'_, Slot>> {
        let mut state = lock(&self.state);
        let stop_generation = state.taps.admission.stop_generation;
        let current = |state: &Slot| {
            let admission = &state.taps.admission;
            (!cancel_on_hold || (!admission.held && admission.stop_generation == stop_generation))
                && Self::current(state, submitted, identity, plan)
        };
        while current(&state) && !state.taps.fits(samples, plan) {
            #[cfg(test)]
            {
                state.waiting = true;
                self.space.notify_all();
            }
            state = self.space.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        #[cfg(test)]
        {
            state.waiting = false;
        }
        current(&state).then_some(state)
    }

    pub(crate) fn require_tap_funding(&self) {
        lock(&self.state).taps.admission.staged = true;
    }

    pub(crate) fn fund_taps(&self, share: Share, interval: Duration) -> Option<()> {
        let funding = Funding::new(share, interval)?;
        let mut state = lock(&self.state);
        let q = &mut state.taps;
        if q.admission.funding.is_some() || q.charged != 0 || !q.entries.is_empty() || !q.unpublished.is_empty() {
            return None;
        }
        let entries = funding.entries();
        let mut fifo = VecDeque::new();
        let mut plans = Vec::new();
        let mut unpublished = Vec::new();
        fifo.try_reserve_exact(entries).ok()?;
        plans.try_reserve_exact(entries).ok()?;
        unpublished.try_reserve_exact(entries).ok()?;
        if [fifo.capacity(), plans.capacity(), unpublished.capacity()] != [entries; 3] {
            return None;
        }
        (q.entries, q.admission.plans, q.unpublished) = (fifo, plans, unpublished);
        (q.admission.staged, q.admission.funding) = (true, Some(funding));
        Some(())
    }

    pub(crate) fn staged_taps(&self) -> bool {
        lock(&self.state).taps.admission.staged
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn has_tap_plan(&self, id: Identity, format: [u64; 6]) -> bool {
        lock(&self.state).taps.admission.plan.as_ref().is_some_and(|p| p.format(id, format))
    }

    pub(crate) fn has_funded_taps(&self, id: Identity) -> bool {
        lock(&self.state).taps.admission.plan.as_ref().is_some_and(|p| p.identity(id))
    }

    pub(crate) fn retire_tap_plan(&self) {
        let mut state = lock(&self.state);
        if state.taps.admission.plan.take().is_some() {
            self.space.notify_all();
        }
    }

    pub(crate) fn hold_taps(&self, hold: bool) {
        let mut state = lock(&self.state);
        let admission = &mut state.taps.admission;
        admission.held = hold;
        if hold {
            admission.stop_generation += 1;
            admission.plan = None;
            self.space.notify_all();
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn admit_taps(&self, id: Identity, format: [u64; 6], samples: usize, limit: Duration) -> bool {
        let mut state = lock(&self.state);
        if state.closed || state.taps.admission.held || !crate::drawpipe::same_pair(state.taps.identity, Some(id)) {
            return false;
        }
        let q = &mut state.taps;
        q.admission.plan = None;
        q.admission.plans.retain(|p| p.strong_count() > 0);
        q.admission.plan = q.admission.funding.as_ref().and_then(|f| f.admit(id, format, samples, limit));
        if let Some(plan) = &q.admission.plan {
            q.admission.plans.push(Arc::downgrade(plan));
        }
        self.space.notify_all();
        q.admission.plan.is_some()
    }

    pub(crate) fn tap_drain(&self) -> usize {
        let state = lock(&self.state);
        if !state.taps.admission.staged {
            return QUEUE_MAX / 2;
        }
        state.taps.admission.plans.iter().filter_map(Weak::upgrade).map(|p| p.drain()).max().unwrap_or(0)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_plan(self: &Arc<Self>, submitted: u64, id: Identity) -> Option<Arc<TapReservation>> {
        let plan = lock(&self.state).taps.admission.plan.as_ref().filter(|p| p.identity(id))?.clone();
        self.reserve(plan.samples(), submitted, Some(id), Some(plan))
    }

    #[cfg(test)]
    pub(crate) fn tap_start(&self, id: Identity, now: Instant) -> Option<Duration> {
        let state = lock(&self.state);
        let plan = state.taps.admission.plan.as_ref().filter(|p| p.identity(id))?;
        Some(plan.start(now))
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn wait_for_cycle(&self, reservation: &TapReservation, now: impl Fn() -> Instant) -> Option<()> {
        let plan = reservation.plan.as_ref()?;
        let mut state = lock(&self.state);
        while Self::current(&state, reservation.submitted, None, Some(plan)) {
            let wait = plan.start(now());
            if wait.is_zero() {
                return Some(());
            }
            #[cfg(test)]
            {
                state.waiting = true;
                self.space.notify_all();
            }
            let (next, _) = self.space.wait_timeout(state, wait).unwrap_or_else(std::sync::PoisonError::into_inner);
            state = next;
            #[cfg(test)]
            {
                state.waiting = false;
            }
        }
        None
    }
}
