// SPDX-License-Identifier: GPL-2.0-or-later

mod funding;

use super::admission::{Funding, Plan};
use super::{Handoff, Tap, TapProjection, TapView};
use crate::board::{PictureEligibility, QUEUE_MAX, QUEUE_SAMPLES};
use oos_core::change::Identity;
use oos_hal::sync::lock;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};

pub(super) type Entry = (u64, Tap, Option<Arc<TapReservation>>);
pub(super) type Unpublished = (u64, Option<Identity>, u64);

pub(crate) enum TapRead<T> {
    Ready(T),
    Stale,
    Pending,
    Empty,
}

pub(crate) struct TapOrder {
    handoff: Weak<Handoff>,
    number: u64,
    pending: AtomicBool,
}

impl Drop for TapOrder {
    fn drop(&mut self) {
        if *self.pending.get_mut() {
            if let Some(handoff) = self.handoff.upgrade() {
                lock(&handoff.state).taps.published(self.number);
            }
        }
    }
}

struct Preview {
    number: u64,
    projection: TapProjection,
    frame: Weak<crate::board::Frame>,
}

#[derive(Clone, Copy)]
pub(crate) enum TapSettlement {
    Accepted(u64),
    Expired,
}

impl TapProjection {
    pub(crate) fn eligibility(&self, view: Option<TapView>) -> PictureEligibility {
        use PictureEligibility::*;
        match *self {
            Self::Accounted { mapping_gen, size }
                if view.is_some_and(|view| (view.mapping_gen, view.size) == (mapping_gen, size)) =>
            {
                AlreadyDeposited
            }
            Self::Accounted { .. } => Redraw,
            Self::Expired => Expired,
            Self::Projected { gen, size } if view.is_some_and(|view| (view.worker_gen, view.size) == (gen, size)) => {
                Pending
            }
            // A projected tap may reach the reader before its obsolete planes reach compose.
            Self::Projected { .. } => ShownOnly,
            Self::Recoverable => Recoverable,
            Self::Unpainted | Self::Average(_) => Drawable,
        }
    }

    fn preview(&self, view: Option<TapView>) -> Option<Self> {
        let view = view?;
        match self.eligibility(Some(view)) {
            PictureEligibility::Pending => Some(Self::Projected { gen: view.worker_gen, size: view.size }),
            PictureEligibility::AlreadyDeposited => {
                Some(Self::Accounted { mapping_gen: view.mapping_gen, size: view.size })
            }
            _ => None,
        }
    }

    fn settle(&mut self, settlement: TapSettlement) {
        if let Self::Projected { size, .. } = *self {
            *self = match settlement {
                TapSettlement::Accepted(mapping_gen) => Self::Accounted { mapping_gen, size },
                TapSettlement::Expired => Self::Expired,
            };
        }
    }
}

#[derive(Default)]
pub(super) struct Queue {
    entries: VecDeque<Entry>,
    samples: usize,
    reserved: usize,
    charged: usize,
    next: u64,
    next_order: u64,
    unpublished: Vec<Unpublished>,
    preview: Option<Preview>,
    identity: Option<Identity>,
    admission: Admission,
}

#[derive(Default)]
struct Admission {
    held: bool,
    stop_generation: u64,
    staged: bool,
    funding: Option<Arc<Funding>>,
    plan: Option<Arc<Plan>>,
    plans: Vec<Weak<Plan>>,
}

impl Queue {
    pub(super) fn accepts_deposit(&self, identity: Option<Identity>) -> bool {
        self.identity.is_none() || identity.is_none() || crate::drawpipe::same_pair(self.identity, identity)
    }

    fn published(&mut self, number: u64) {
        if let Some(index) = self.unpublished.iter().position(|&(n, _, _)| n == number) {
            self.unpublished.remove(index);
        }
    }

    fn fits(&self, samples: usize, plan: Option<&Arc<Plan>>) -> bool {
        if let Some(plan) = plan {
            return self.entries.len() + self.reserved < self.entries.capacity() && plan.fits();
        }
        self.entries.len() + self.reserved < QUEUE_MAX
            && (self.charged == 0 && self.entries.is_empty()
                || self.samples.checked_add(samples).is_some_and(|total| total <= QUEUE_SAMPLES))
    }

    pub(super) fn clear(&mut self) -> VecDeque<Entry> {
        self.preview = None;
        self.admission.plan = None;
        self.unpublished.clear();
        let entries = std::mem::take(&mut self.entries);
        for (_, tap, charge) in &entries {
            self.release_entry(tap, charge.as_deref());
        }
        entries
    }

    fn release_entry(&mut self, tap: &Tap, charge: Option<&TapReservation>) {
        if let Some(charge) = charge {
            charge.dequeued();
        } else {
            self.samples -= tap.samples();
        }
    }

    pub(super) fn restore_storage(&mut self, entries: VecDeque<Entry>) {
        if self.admission.staged && self.entries.capacity() < entries.capacity() {
            self.entries = entries;
        }
    }
}

/// Holds the original capacity until both the painter and record reader release it.
pub(crate) struct TapReservation {
    handoff: Weak<Handoff>,
    samples: usize,
    submitted: u64,
    pending: AtomicBool,
    plan: Option<Arc<Plan>>,
    queued: AtomicBool,
}

impl TapReservation {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn covers(&self, frame: &crate::board::Frame) -> bool {
        frame.ch.iter().map(Vec::capacity).sum::<usize>() <= self.samples
    }

    pub(crate) fn funded(&self) -> bool {
        self.plan.is_some()
    }

    fn dequeued(&self) {
        if let Some(plan) = &self.plan {
            if self.queued.swap(false, Ordering::Relaxed) {
                plan.dequeued();
            }
        }
    }

    pub(crate) fn submitted(&self) -> u64 {
        self.submitted
    }
}

impl Drop for TapReservation {
    fn drop(&mut self) {
        let handoff = self.handoff.upgrade();
        let mut state = handoff.as_ref().map(|handoff| lock(&handoff.state));
        if let Some(plan) = &self.plan {
            plan.release(*self.queued.get_mut());
        }
        if let (Some(handoff), Some(state)) = (&handoff, &mut state) {
            if self.plan.is_none() {
                state.taps.samples -= self.samples;
            }
            state.taps.reserved -= usize::from(*self.pending.get_mut());
            state.taps.charged -= 1;
            handoff.space.notify_all();
        }
    }
}

impl Handoff {
    pub(crate) fn track_tap(self: &Arc<Self>, submitted: u64, id: Option<Identity>, seq: u64) -> Option<TapOrder> {
        let mut state = lock(&self.state);
        if state.closed
            || submitted < state.reset_before
            || (state.taps.admission.staged && state.taps.unpublished.len() == state.taps.unpublished.capacity())
        {
            return None;
        }
        let number = state.taps.next_order;
        state.taps.next_order += 1;
        state.taps.unpublished.push((number, id, seq));
        Some(TapOrder { handoff: Arc::downgrade(self), number, pending: AtomicBool::new(true) })
    }

    pub(crate) fn stage_tap_view(&self, identity: Identity, active: bool) {
        let mut state = lock(&self.state);
        let obsolete = !crate::drawpipe::same_pair(state.taps.identity, Some(identity));
        if !active || obsolete {
            state.taps.admission.plan = None;
        }
        if obsolete {
            state.taps.preview = None;
        }
        state.taps.identity = Some(identity);
    }

    pub(crate) fn take_unusable(&self, identity: Option<Identity>, keep_current: bool) -> Option<super::Deposit> {
        lock(&self.state).pending.take_if(|d| !keep_current || !crate::drawpipe::same_pair(d.identity, identity))
    }

    #[cfg(test)]
    pub(crate) fn set_tap_identity(&self, identity: Identity) {
        self.stage_tap_view(identity, true);
        self.wake_tap_commands();
    }

    #[cfg(test)]
    pub(crate) fn send_tap(&self, tap: Tap, submitted: u64, charge: Option<&Arc<TapReservation>>) -> Option<u64> {
        self.publish_tap(tap, submitted, charge, None)
    }

    pub(crate) fn publish_tap(
        &self,
        mut tap: Tap,
        submitted: u64,
        charge: Option<&Arc<TapReservation>>,
        order: Option<&TapOrder>,
    ) -> Option<u64> {
        let samples = tap.samples();
        let mut state = if let Some(charge) = charge {
            let mut state = lock(&self.state);
            if state.closed
                || submitted < state.reset_before
                || !state.taps.accepts_deposit(tap.identity)
                || !std::ptr::eq(self, charge.handoff.as_ptr())
                || charge.plan.as_ref().is_some_and(|p| tap.identity.is_none_or(|id| !p.identity(id)))
            {
                return None;
            }
            if samples > charge.samples {
                if let TapProjection::Average(output) = &mut tap.projection {
                    output.set_loss(crate::drawpipe::Lost::Buffer);
                }
                return None;
            }
            if !charge.pending.swap(false, Ordering::Relaxed) {
                return None;
            }
            state.taps.reserved -= 1;
            state
        } else {
            // The worker can publish an already submitted waveform while the source is held.
            let mut state = self.wait_for_tap(samples, submitted, tap.identity, None, false)?;
            state.taps.samples += samples;
            state
        };
        let number = state.taps.next;
        state.taps.next += 1;
        state.taps.entries.push_back((number, tap, charge.cloned()));
        if let Some(order) = order {
            state.taps.published(order.number);
            order.pending.store(false, Ordering::Relaxed);
        }
        Some(number)
    }

    pub(crate) fn tap_count(&self) -> usize {
        lock(&self.state).taps.entries.len()
    }

    pub(crate) fn has_tap(&self, identity: Identity) -> bool {
        let state = lock(&self.state);
        let current = |id| crate::drawpipe::same_pair(Some(identity), id);
        state.taps.unpublished.iter().any(|&(_, id, _)| current(id))
            || state.taps.entries.iter().any(|(_, tap, _)| current(tap.identity))
    }

    pub(crate) fn take_tap(
        &self,
        view: Option<TapView>,
        identity: Option<Identity>,
        fallback: Option<u64>,
        can_take: bool,
    ) -> TapRead<(Tap, Option<Arc<TapReservation>>)> {
        let mut state = lock(&self.state);
        let current = |id| crate::drawpipe::same_pair(identity, id);
        let unpublished =
            state.taps.unpublished.iter().filter(|&&(_, id, _)| current(id)).map(|&(_, _, seq)| seq).min();
        let first = state.taps.entries.front().map(|(_, tap, _)| (current(tap.identity), tap.frame.seq));
        let before_fallback = |seq| fallback.is_none_or(|fallback| seq <= fallback);
        let eligible = first.is_some_and(|(valid, seq)| {
            !valid || (before_fallback(seq) && unpublished.is_none_or(|older| seq <= older))
        });
        if !can_take || !eligible {
            return if unpublished.is_some_and(before_fallback)
                || state.taps.entries.iter().any(|(_, tap, _)| current(tap.identity) && before_fallback(tap.frame.seq))
            {
                TapRead::Pending
            } else {
                TapRead::Empty
            };
        }
        let (number, tap, charge) = state.taps.entries.pop_front().expect("checked tap FIFO head");
        let valid = current(tap.identity);
        if let Some(projection) = valid.then(|| tap.projection.preview(view)).flatten() {
            state.taps.preview = Some(Preview { number, projection, frame: Arc::downgrade(&tap.frame) });
        }
        state.taps.release_entry(&tap, charge.as_deref());
        self.space.notify_all();
        drop(state);
        if valid {
            TapRead::Ready((tap, charge))
        } else {
            TapRead::Stale
        }
    }

    pub(crate) fn settle_taps(&self, numbers: &std::ops::Range<u64>, settlement: TapSettlement) {
        if numbers.is_empty() {
            return;
        }
        let mut state = lock(&self.state);
        if let Some(mut preview) = state.taps.preview.take_if(|preview| numbers.contains(&preview.number)) {
            if !matches!(settlement, TapSettlement::Expired) {
                preview.projection.settle(settlement);
                state.taps.preview = Some(preview);
            }
        }
        for (number, tap, _) in &mut state.taps.entries {
            if *number >= numbers.end {
                break;
            }
            // A coalesced deposit can span taps from batches rejected before painting.
            if numbers.contains(number) {
                tap.projection.settle(settlement);
            }
        }
    }

    pub(crate) fn take_preview(
        &self,
        view: Option<TapView>,
        frame: &Arc<crate::board::Frame>,
    ) -> Option<PictureEligibility> {
        let mut state = lock(&self.state);
        let preview = state.taps.preview.as_ref().filter(|preview| preview.frame.as_ptr() == Arc::as_ptr(frame))?;
        let eligibility = preview.projection.eligibility(view);
        if eligibility.draws() {
            state.taps.preview = None;
            return Some(eligibility);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Frame;

    #[test]
    fn settlement_preserves_the_disposition_of_a_pending_preview() {
        let view = TapView::new(1, 1, (8, 8));
        let remap = TapView::new(2, 2, (8, 8));
        for accepted in [false, true] {
            let handoff = Handoff::default();
            let frame = Arc::new(Frame::default());
            let tap = || Tap {
                frame: frame.clone(),
                identity: None,
                projection: TapProjection::Projected { gen: 1, size: (8, 8) },
                folds: None,
            };
            let n = handoff.send_tap(tap(), 1, None).unwrap();
            let last = handoff.send_tap(tap(), 1, None).unwrap();
            let TapRead::Ready((tap, _)) = handoff.take_tap(Some(view), None, None, true) else {
                panic!("missing tap")
            };
            assert_eq!(tap.projection.eligibility(Some(view)), PictureEligibility::Pending);
            let settlement = if accepted { TapSettlement::Accepted(1) } else { TapSettlement::Expired };
            handoff.settle_taps(&(n..last + 1), settlement);
            let TapRead::Ready((tap, _)) = handoff.take_tap(Some(remap), None, None, true) else {
                panic!("missing tap")
            };
            assert_eq!(
                tap.projection.eligibility(Some(remap)),
                if accepted { PictureEligibility::Redraw } else { PictureEligibility::Expired }
            );
            assert!(handoff.take_preview(Some(view), &frame).is_none());
            assert_eq!(handoff.take_preview(Some(remap), &frame), accepted.then_some(PictureEligibility::Redraw));
            assert!(handoff.take_preview(Some(remap), &frame).is_none());
        }
    }

    #[test]
    fn reset_removes_only_the_old_order_fence() {
        let handoff = Arc::new(Handoff::default());
        let old = handoff.track_tap(0, None, 1).unwrap();
        assert!(matches!(handoff.take_tap(None, None, Some(2), true), TapRead::Pending));
        handoff.reset(1);
        assert!(handoff.track_tap(0, None, 1).is_none());
        let current = handoff.track_tap(1, None, 3).unwrap();
        drop(old);
        assert!(matches!(handoff.take_tap(None, None, Some(2), true), TapRead::Empty));
        assert!(matches!(handoff.take_tap(None, None, Some(4), true), TapRead::Pending));
        drop(current);
        assert!(matches!(handoff.take_tap(None, None, Some(4), true), TapRead::Empty));
    }

    #[test]
    fn capacity_release_and_close_wake_blocked_reservations() {
        let handoff = Arc::new(Handoff::default());
        let mut held = (0..QUEUE_MAX).map(|_| handoff.reserve_tap(1, 0, None).unwrap()).collect::<Vec<_>>();
        std::thread::scope(|threads| {
            let next = threads.spawn(|| handoff.reserve_tap(1, 0, None));
            handoff.wait_for_sender();
            drop(held.pop());
            held.push(next.join().unwrap().unwrap());
            assert_eq!(lock(&handoff.state).taps.charged, QUEUE_MAX);

            let cancelled = threads.spawn(|| handoff.reserve_tap(1, 0, None));
            handoff.wait_for_sender();
            handoff.close();
            assert!(cancelled.join().unwrap().is_none());
        });
        assert_eq!(lock(&handoff.state).taps.charged, QUEUE_MAX);
        drop(held);
        assert_eq!(lock(&handoff.state).taps.samples, 0);
    }

    #[test]
    fn reset_releases_waiters_but_keeps_old_source_and_reader_charged() {
        let handoff = Arc::new(Handoff::default());
        let old = handoff.reserve_tap(QUEUE_SAMPLES, 0, None).unwrap();
        let tap = Tap {
            frame: Arc::new(Frame::default()),
            identity: None,
            projection: TapProjection::Unpainted,
            folds: None,
        };
        assert!(handoff.send_tap(tap, 0, Some(&old)).is_some());
        let TapRead::Ready((_, reader)) = handoff.take_tap(None, None, None, true) else { panic!("missing tap") };
        std::thread::scope(|threads| {
            let stale = threads.spawn(|| handoff.reserve_tap(1, 0, None));
            handoff.wait_for_sender();
            handoff.reset(1);
            assert!(stale.join().unwrap().is_none());

            let current = threads.spawn(|| handoff.reserve_tap(1, 1, None));
            handoff.wait_for_sender();
            drop(old);
            assert_eq!(lock(&handoff.state).taps.samples, QUEUE_SAMPLES);
            drop(reader);
            assert!(current.join().unwrap().is_some());
        });
        assert_eq!(lock(&handoff.state).taps.samples, 0);
    }
}
