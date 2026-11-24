// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::{board::Frame, model::Model, picture::deposit::TapProjection};
use std::time::{Duration, Instant};
struct Fixture {
    pipe: Pipe,
    owner: Owner,
    window: DepositWindow,
    ph: Phosphor,
    origin: Instant,
    id: oos_core::change::Identity,
}
impl Fixture {
    fn new() -> Self {
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        let id = Default::default();
        pipe.publish_view(&Model::default(), 4, 4, crate::sparse::Interp::Exact, id);
        let mut owner = Owner::default();
        owner.set_wake(Arc::new(crate::system::intake::Wake::new().unwrap()));
        Self { pipe, owner, window: Default::default(), ph: Phosphor::new(4, 4), origin: Instant::now(), id }
    }
    fn publish(&self, records: u64, ms: u64, row: usize) -> u64 {
        let (gen, sizes) = {
            let state = oos_hal::sync::lock(&self.pipe.view);
            (state.gen, state.view.as_ref().unwrap().picture_sizes())
        };
        let mut d = self.pipe.slot.acquire_picture(gen, sizes, Some(self.id), [true, false, false, false]).unwrap();
        d.records = records;
        d.extent.include(self.origin + Duration::from_millis(ms));
        d.extent.add_hits(records);
        d.edges[0] = records;
        for band in &mut d.planes {
            band.test_span(row, (24 * records) as u16);
        }
        let n = self
            .pipe
            .slot
            .send_tap(
                crate::picture::deposit::Tap {
                    frame: Arc::new(Frame { seq: ms + 1, ..Default::default() }),
                    identity: Some(self.id),
                    projection: TapProjection::Projected { gen, size: (4, 4) },
                    folds: None,
                },
                gen,
                None,
            )
            .unwrap();
        d.taps = n..n + 1;
        assert!(self.pipe.slot.put(d).is_ok());
        n
    }
    fn put(&mut self, records: u64, ms: u64, row: usize) -> u64 {
        let n = self.publish(records, ms, row);
        self.owner.intake_owned(&self.pipe, true);
        n
    }
    fn compose(&mut self) -> Accepted {
        self.owner.compose_owned(
            &self.pipe,
            &mut self.ph,
            &mut true,
            Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
            Some(&mut self.window),
        )
    }
}
#[test]
fn zero_only_completion_has_no_density_or_tap_credit() {
    let mut f = Fixture::new();
    f.put(0, 0, 0);
    let a = f.compose();
    assert_eq!(a.frames, 0);
    assert_eq!(a.window_frames, None);
    assert!(f.window.is_empty());
    assert_eq!(f.pipe.take_waveform((4, 4)).unwrap().eligibility(), crate::board::PictureEligibility::Expired);
}
#[test]
fn zero_then_positive_credits_only_the_positive_deposit() {
    let mut f = Fixture::new();
    f.put(0, 0, 0);
    f.put(32, 60, 1);
    let a = f.compose();
    assert_eq!(a.frames, 32);
    assert_eq!(a.window_frames, Some(32));
    assert_eq!(f.pipe.take_waveform((4, 4)).unwrap().eligibility(), crate::board::PictureEligibility::Expired);
    assert_eq!(f.pipe.take_waveform((4, 4)).unwrap().eligibility(), crate::board::PictureEligibility::AlreadyDeposited);
    assert_eq!(f.compose().window_frames, None);
    assert_eq!(f.window.rebuild(&mut f.ph), 32);
}
#[test]
fn delayed_positive_deposits_share_the_next_picture() {
    let mut f = Fixture::new();
    f.put(20, 0, 0);
    f.put(32, 60, 1);
    let accepted = f.compose();
    assert_eq!((accepted.frames, accepted.window_frames), (52, Some(52)));
    assert_eq!(f.pipe.counters.read().2, 0);
    assert_eq!(f.pipe.take_waveform((4, 4)).unwrap().eligibility(), crate::board::PictureEligibility::AlreadyDeposited);
    assert_eq!(f.pipe.take_waveform((4, 4)).unwrap().eligibility(), crate::board::PictureEligibility::AlreadyDeposited);
}

#[test]
fn run_epoch_rejects_staged_and_pending_deposits_from_before_run() {
    use oos_core::change::{Request, Shadow, UiAcq};
    let mut f = Fixture::new();
    f.put(20, 0, 0);
    f.publish(32, 1, 1);
    let model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    f.id = shadow.observe(&model, UiAcq::default(), &[Request::Run], &config).unwrap().identity();
    f.pipe.publish_view(&model, 4, 4, crate::sparse::Interp::Exact, f.id);
    assert_eq!(f.compose().frames, 0);
    assert!(f.window.is_empty());
    f.put(3, 2, 2);
    assert_eq!(f.compose().frames, 3);
    assert_eq!(f.window.rebuild(&mut f.ph), 3);
}

#[test]
fn fresh_deposits_above_the_combined_narrow_bound_share_the_same_picture() {
    let mut f = Fixture::new();
    f.put(1500, 0, 0);
    f.put(1500, 1, 0);
    let accepted = f.compose();
    assert_eq!((accepted.frames, accepted.window_frames), (3000, Some(3000)));
    assert!((0..4).all(|column| f.ph.count(0, column) == 72_000));
    assert_eq!(f.compose().frames, 0);
}

#[test]
fn zoom_toggle_conserves_deposited_records_without_drawing_rejected_taps() {
    use crate::board::PictureEligibility::{AlreadyDeposited, Expired};
    for initial_zoom in [false, true] {
        let mut f = Fixture::new();
        let mut detail = Phosphor::new(4, 4);
        let zoom = route::ZoomView::new((4, 4), 1.0, 0.0);
        let mut model = Model { zoom: initial_zoom, ..Model::default() };
        f.pipe.publish_picture_view(&model, (4, 4), crate::sparse::Interp::Exact, f.id, initial_zoom.then_some(zoom));
        f.put(20, 0, 0);
        model.zoom = !initial_zoom;
        f.pipe.publish_picture_view(&model, (4, 4), crate::sparse::Interp::Exact, f.id, model.zoom.then_some(zoom));
        f.put(32, 1, 1);
        let accepted = f.owner.compose_picture(
            &f.pipe,
            PerMap::new(&mut f.ph, model.zoom.then_some(&mut detail)),
            &mut true,
            Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
            Some(&mut f.window),
        );
        assert_eq!(accepted.frames, 32);
        assert_eq!(f.pipe.counters.read().2, 20);
        assert!(f.pipe.counters.records_text().contains("rejected.stale_key=20 "));
        let rejected = f.pipe.take_waveform((4, 4)).unwrap();
        let shown = f.pipe.take_waveform((4, 4)).unwrap();
        assert_eq!((rejected.seq, rejected.eligibility()), (1, Expired));
        assert_eq!((shown.seq, shown.eligibility()), (2, AlreadyDeposited));
        assert!(f.pipe.take_waveform((4, 4)).is_none());
    }
}

#[test]
fn compose_without_window_recycles_even_an_excess_deposit() {
    let mut f = Fixture::new();
    for ms in 0..5 {
        f.put(1, ms, 0);
    }
    // A sixth deposit cannot acquire a backing. Inject an unleased deposit to exercise the guard.
    let gen = oos_hal::sync::lock(&f.pipe.view).gen;
    let mut excess = Deposit::new(gen, (4, 4), Some(f.id), [true, false, false, false]);
    excess.records = 2;
    f.owner.queue.push_back(excess);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.owner.compose_owned(&f.pipe, &mut f.ph, &mut true, Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) }, None)
    }));
    if cfg!(debug_assertions) {
        assert!(result.is_err());
    } else {
        assert_eq!(result.unwrap().frames, 5);
    }
    assert!(f.owner.queue.is_empty());
    assert_eq!(std::iter::from_fn(|| f.pipe.take_recycled_deposit()).count(), 6);
    assert_eq!(f.pipe.counters.read().2, 2);
    assert!(f.pipe.counters.records_text().contains("lost.queue_overflow=2 "));
}

#[test]
fn config_only_pair_change_rejects_unshown_but_keeps_same_epoch_retained() {
    use oos_core::change::{Shadow, UiAcq};
    let mut f = Fixture::new();
    f.put(32, 0, 0);
    assert_eq!(f.compose().window_frames, Some(32));
    let old = f.ph.maps().clone();
    let mut m = Model::default();
    let mut shadow = Shadow::new(&m, UiAcq::default(), &crate::model::config()).unwrap();
    m.holdoff = 0.001;
    let id = shadow.observe(&m, UiAcq::default(), &[], &crate::model::config()).unwrap().identity();
    assert_ne!(id.gen(), f.id.gen());
    assert_eq!(id.epoch(), f.id.epoch());
    f.put(32, 1, 1);
    let gen = oos_hal::sync::lock(&f.pipe.view).gen;
    f.pipe.publish_view(&m, 4, 4, crate::sparse::Interp::Exact, id);
    assert_eq!(oos_hal::sync::lock(&f.pipe.view).gen, gen);
    let a = f.compose();
    assert_eq!(a.frames, 0);
    assert_eq!(a.window_frames, None);
    assert_eq!(f.ph.maps(), &old);
    assert_eq!(f.window.rebuild(&mut f.ph), 32);
}
#[test]
fn actual_same_picture_merge_keeps_membership_density_and_warmed_capacity() {
    let mut f = Fixture::new();
    f.put(32, 0, 0);
    f.put(32, 0, 1);
    let a = f.compose();
    assert_eq!((a.frames, a.window_frames), (64, Some(64)));
    assert!(f.ph.maps()[0][..8].iter().all(|&v| v == 32 * 24));
    // Warm normal Window/observer/owner storage before the allocation receipt.
    for i in 2..6 {
        f.put(32, i, 0);
        f.put(32, i, 1);
        assert_eq!(f.compose().frames, 64);
    }
    f.put(32, 6, 0);
    f.put(32, 6, 1);
    let capacity = f.owner.queue.capacity();
    let (allocations, a) = crate::system::test_allocator::count_own(|| f.compose());
    assert_eq!((a.frames, a.window_frames), (64, Some(64)));
    assert_eq!(allocations, 0, "warmed real commit reuses queue and fixed phase storage");
    assert_eq!(f.owner.queue.capacity(), capacity);
}

#[test]
fn distinct_off_statistics_are_credited_per_input_once_at_real_composition() {
    let mut f = Fixture::new();
    f.put(32, 0, 0);
    f.owner.queue.back_mut().unwrap().off = (1.0, 2.0);
    f.put(32, 0, 1);
    f.owner.queue.back_mut().unwrap().off = (3.0, 4.0);
    let mut off = (0.0, 0.0);
    let mut edges = [0; 4];
    for expected in [64, 0] {
        let a = f.owner.compose_owned(
            &f.pipe,
            &mut f.ph,
            &mut true,
            Stats { edges: &mut edges, off: &mut off },
            Some(&mut f.window),
        );
        assert_eq!(a.frames, expected);
        assert_eq!(off, (4.0, 4.0));
        assert_eq!(edges[0], 64);
    }
}

#[test]
fn invalid_head_is_discarded_and_the_next_deposit_is_shown() {
    let mut f = Fixture::new();
    f.put(32, 0, 0);
    f.owner.queue.front_mut().unwrap().planes[0].set_format(crate::picture::deposit::count::CountFormat::Wide);
    f.put(11, 0, 0);
    let a = f.compose();
    assert_eq!(a.frames, 11);
    assert!(!f.window.is_empty());
    assert!(f.owner.queue.is_empty());
    assert_eq!(f.pipe.counters.read().2, 32);
    assert!(f.pipe.counters.records_text().contains("lost.error=32 "));
}

#[test]
fn deposit_published_after_the_seal_stays_with_the_next_picture() {
    let mut f = Fixture::new();
    f.put(23, 0, 0);
    f.publish(31, 1, 1);
    f.owner.seal(&f.pipe);
    f.publish(37, 2, 2);

    let accepted = f.owner.compose(
        PerMap::new(&mut f.ph, None),
        &mut true,
        Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
        Some(&mut f.window),
    );
    assert_eq!((accepted.frames, accepted.window_frames), (54, Some(54)));
    assert_eq!(f.ph.channel_max(0), 31 * 24);
    assert_eq!(f.ph.count(0, 8), 0);
    assert_eq!(f.compose().frames, 37);
    assert_eq!(f.ph.count(0, 8), 37 * 24);
}

#[test]
fn seal_leaves_pending_deposit_for_next_picture_when_queue_is_full() {
    let mut f = Fixture::new();
    for ms in 0..PICTURE_INPUTS as u64 {
        f.put(1, ms, 0);
    }
    // The backing limit normally prevents this state. Inject a pending deposit to exercise the seal guard.
    let gen = oos_hal::sync::lock(&f.pipe.view).gen;
    let mut pending = Deposit::new(gen, (4, 4), Some(f.id), [true, false, false, false]);
    pending.records = 2;
    pending.extent.include(f.origin);
    pending.extent.add_hits(2);
    assert!(f.pipe.slot.put(pending).is_ok());
    assert_eq!(f.compose().frames, PICTURE_INPUTS as u64);
    assert_eq!(f.compose().frames, 2);
    assert_eq!(f.pipe.counters.read().2, 0);
}
