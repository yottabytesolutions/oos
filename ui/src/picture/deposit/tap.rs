// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Handoff, Tap, TapProjection, TapView};
use crate::board::{PictureEligibility, QUEUE_MAX, QUEUE_SAMPLES};
use oos_core::change::Identity;
use oos_hal::sync::lock;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};

type Entry = (u64, Tap, Option<Arc<TapReservation>>);
type Unpublished = (u64, Option<Identity>, u64);

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

impl TapProjection {
    pub(crate) fn eligibility(self, view: Option<TapView>) -> PictureEligibility {
        use PictureEligibility::*;
        match self {
            Self::Accounted { mapping_gen, size }
                if view.is_some_and(|view| (view.mapping_gen, view.size) != (mapping_gen, size)) =>
            {
                Redraw
            }
            Self::Accounted { .. } => AlreadyDeposited,
            Self::Expired => Expired,
            Self::Projected { gen, size } if view.is_some_and(|view| (view.worker_gen, view.size) == (gen, size)) => {
                Pending
            }
            Self::Unpainted | Self::Projected { .. } => Drawable,
        }
    }

    fn settle(&mut self, accepted_mapping: Option<u64>) {
        if let Self::Projected { size, .. } = *self {
            *self = accepted_mapping.map_or(Self::Expired, |mapping_gen| Self::Accounted { mapping_gen, size });
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
}

impl Queue {
    fn published(&mut self, number: u64) {
        if let Some(index) = self.unpublished.iter().position(|&(n, _, _)| n == number) {
            self.unpublished.remove(index);
        }
    }

    fn fits(&self, samples: usize) -> bool {
        self.entries.len() + self.reserved < QUEUE_MAX
            && (self.charged == 0 && self.entries.is_empty()
                || self.samples.checked_add(samples).is_some_and(|total| total <= QUEUE_SAMPLES))
    }

    pub(super) fn clear(&mut self) -> VecDeque<Entry> {
        self.preview = None;
        self.unpublished.clear();
        let entries = std::mem::take(&mut self.entries);
        for (_, tap, charge) in &entries {
            if charge.is_none() {
                self.samples -= tap.samples();
            }
        }
        entries
    }
}

/// Holds the original capacity until both the painter and record reader release it.
pub(crate) struct TapReservation {
    handoff: Weak<Handoff>,
    samples: usize,
    submitted: u64,
    pending: AtomicBool,
}

impl TapReservation {
    pub(crate) fn submitted(&self) -> u64 {
        self.submitted
    }
}

impl Drop for TapReservation {
    fn drop(&mut self) {
        if let Some(handoff) = self.handoff.upgrade() {
            let mut state = lock(&handoff.state);
            state.taps.samples -= self.samples;
            state.taps.reserved -= usize::from(*self.pending.get_mut());
            state.taps.charged -= 1;
            handoff.space.notify_all();
        }
    }
}

impl Handoff {
    pub(crate) fn track_tap(self: &Arc<Self>, submitted: u64, id: Option<Identity>, seq: u64) -> Option<TapOrder> {
        let mut state = lock(&self.state);
        if state.closed || submitted < state.reset_before {
            return None;
        }
        let number = state.taps.next_order;
        state.taps.next_order += 1;
        state.taps.unpublished.push((number, id, seq));
        Some(TapOrder { handoff: Arc::downgrade(self), number, pending: AtomicBool::new(true) })
    }

    pub(crate) fn set_tap_identity(&self, identity: Identity) {
        let mut state = lock(&self.state);
        if !crate::drawpipe::same_pair(state.taps.identity, Some(identity)) {
            state.taps.identity = Some(identity);
            self.space.notify_all();
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_tap(
        self: &Arc<Self>,
        samples: usize,
        submitted: u64,
        identity: Option<Identity>,
    ) -> Option<Arc<TapReservation>> {
        let mut state = self.wait_for_tap(samples, submitted, identity)?;
        state.taps.samples += samples;
        state.taps.reserved += 1;
        state.taps.charged += 1;
        Some(Arc::new(TapReservation {
            handoff: Arc::downgrade(self),
            samples,
            submitted,
            pending: AtomicBool::new(true),
        }))
    }

    fn wait_for_tap(
        &self,
        samples: usize,
        submitted: u64,
        identity: Option<Identity>,
    ) -> Option<std::sync::MutexGuard<'_, super::Slot>> {
        let mut state = lock(&self.state);
        let current = |state: &super::Slot| {
            !state.closed
                && submitted >= state.reset_before
                && identity.is_none_or(|id| crate::drawpipe::same_pair(state.taps.identity, Some(id)))
        };
        while current(&state) && !state.taps.fits(samples) {
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

    #[cfg(test)]
    pub(crate) fn send_tap(&self, tap: Tap, submitted: u64, charge: Option<&Arc<TapReservation>>) -> Option<u64> {
        self.publish_tap(tap, submitted, charge, None)
    }

    pub(crate) fn publish_tap(
        &self,
        tap: Tap,
        submitted: u64,
        charge: Option<&Arc<TapReservation>>,
        order: Option<&TapOrder>,
    ) -> Option<u64> {
        let samples = tap.samples();
        let mut state = if let Some(charge) = charge {
            let mut state = lock(&self.state);
            if state.closed || submitted < state.reset_before || samples > charge.samples {
                return None;
            }
            if !charge.pending.swap(false, Ordering::Relaxed) {
                return None;
            }
            state.taps.reserved -= 1;
            state
        } else {
            let mut state = self.wait_for_tap(samples, submitted, None)?;
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
        if valid
            && view.is_some()
            && matches!(
                tap.projection.eligibility(view),
                PictureEligibility::Pending | PictureEligibility::AlreadyDeposited
            )
        {
            state.taps.preview =
                Some(Preview { number, projection: tap.projection, frame: Arc::downgrade(&tap.frame) });
        }
        if charge.is_none() {
            state.taps.samples -= tap.samples();
        }
        self.space.notify_all();
        drop(state);
        if valid {
            TapRead::Ready((tap, charge))
        } else {
            TapRead::Stale
        }
    }

    pub(crate) fn settle_taps(&self, numbers: &std::ops::Range<u64>, accepted_mapping: Option<u64>) {
        if numbers.is_empty() {
            return;
        }
        let mut state = lock(&self.state);
        if let Some(mut preview) = state.taps.preview.take_if(|preview| numbers.contains(&preview.number)) {
            if accepted_mapping.is_some() {
                preview.projection.settle(accepted_mapping);
                state.taps.preview = Some(preview);
            }
        }
        for (number, tap, _) in &mut state.taps.entries {
            if *number >= numbers.end {
                break;
            }
            // A coalesced deposit can span taps from batches rejected before painting.
            if numbers.contains(number) {
                tap.projection.settle(accepted_mapping);
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
            };
            let n = handoff.send_tap(tap(), 1, None).unwrap();
            let last = handoff.send_tap(tap(), 1, None).unwrap();
            let TapRead::Ready((tap, _)) = handoff.take_tap(Some(view), None, None, true) else {
                panic!("missing tap")
            };
            assert_eq!(tap.projection.eligibility(Some(view)), PictureEligibility::Pending);
            handoff.settle_taps(&(n..last + 1), accepted.then_some(1));
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
    fn reset_releases_waiters_but_keeps_old_source_and_reader_charged() {
        let handoff = Arc::new(Handoff::default());
        let old = handoff.reserve_tap(QUEUE_SAMPLES, 0, None).unwrap();
        let tap = Tap { frame: Arc::new(Frame::default()), identity: None, projection: TapProjection::Unpainted };
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
