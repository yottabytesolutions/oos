// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::budget::WORK_BUDGET;
use crate::drawpipe::{self, clock::StageClock, Counters, Pipe};
use crate::sparse::Interp;
use std::sync::Barrier;
use std::time::Instant;

#[test]
fn a_scripted_pool_has_one_fixed_point_at_every_batch_size() {
    let id = Identity::default();
    for work in [1, 29, 417, 1_001, 4_400, 16_000] {
        for budget in [0, 152, 1_019, 4_500, 12_000, 73_000] {
            for noise in [0, 13, 257] {
                for workers in [1, 2, 4, 8] {
                    let converge = |mut records: u32| {
                        let mut cost = Cost::default();
                        cost.set_identity(id);
                        let mut stable = None;
                        for batch in 0..48 {
                            let jitter = (batch * 11 % 16) * noise / 15;
                            let fixed = 71 + jitter;
                            let work = u64::from(records) * work;
                            let wall = fixed + work.div_ceil(u64::from(records.min(workers)));
                            let sample = Estimate::sample(records.into(), work, wall, 19, workers);
                            cost.record(id, sample.values());
                            let [work, fixed] = cost.estimate(id, [1, 0], 0);
                            let grant = Estimate::new(work, fixed).grant(142, budget.into(), workers);
                            records = grant.records();
                            let predicted = |n: u32| fixed + (u64::from(n) * work).div_ceil(u64::from(n.min(workers)));
                            if grant.serial() {
                                assert!(predicted(1) > budget);
                                assert_eq!(records, 1);
                                assert!(predicted(records) <= budget + work + fixed);
                            } else {
                                assert!(records >= workers);
                                assert!(predicted(records) <= budget);
                                assert!(records == 142 || predicted(records + 1) > budget);
                            }
                            if batch >= 15 {
                                assert_eq!(*stable.get_or_insert(grant), grant);
                            }
                        }
                        stable.unwrap()
                    };
                    assert_eq!(converge(1), converge(142), "{work} {budget} {noise} {workers}");
                }
            }
        }
    }
}

#[test]
fn published_worker_work_drives_grants_and_view_changes_select_a_new_seed() {
    use crate::drawpipe::metrics::Work;
    use crate::model::{mdepths, Model};
    use oos_core::change::{Shadow, UiAcq};
    let counters = Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(4, 0, counters);
    model_pool(&pipe, 4);
    let mut model = Model::default();
    crate::store::set(&mut model, "tdiv", "0.000001").unwrap();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let id = shadow.identity();
    pipe.publish_view(&model, 970, 448, Interp::Exact, id);
    let initial = pipe.grant(id, 142, WORK_BUDGET);
    assert!(initial.records() > 10);

    let millis = Duration::from_millis;
    let mut flight = pipe.admission.submit(&pipe.source, id, counters);
    let work = Work { refine: millis(1), envelope: millis(2), tiles: millis(1), pool: millis(4) };
    counters.metrics.work(Some(id), &work, 1, false);
    flight.worked(1, work.pool);
    flight.published(millis(1));
    drop(flight);
    assert_eq!(pipe.grant(id, 142, WORK_BUDGET), Grant::Overlapping(11));
    let text = counters.pipeline_text(WORK_BUDGET, 0);
    assert!(text.contains(" work_per_record_ns=4000000 fixed_cost_ns=1000000 parallelism=4"));

    let mut stale = pipe.admission.submit(&pipe.source, id, counters);
    counters.metrics.work(Some(id), &work, 1, false);
    stale.worked(1, work.pool);
    pipe.publish_view(&model, 900, 448, Interp::Exact, id);
    let resized = pipe.grant(id, 142, WORK_BUDGET);
    assert!(resized.records() > 11);
    stale.published(millis(1));
    drop(stale);
    assert_eq!(pipe.grant(id, 142, WORK_BUDGET), resized);

    let generation = lock(&pipe.view).gen;
    model.mdepth = mdepths().iter().position(|&(_, depth)| depth > 1_000_000).unwrap();
    let next = shadow.observe(&model, UiAcq::default(), &[], &config).unwrap().identity();
    pipe.publish_view(&model, 900, 448, Interp::Exact, next);
    assert_eq!(lock(&pipe.view).gen, generation, "memory depth changes without replacing the mapping");
    assert_eq!(pipe.grant(next, 142, WORK_BUDGET), Grant::Serial);
}

#[test]
fn p90_uses_sixteen_published_batches_and_resets_with_acquisition_identity() {
    let (_, next) = drawpipe::identity_tests::identities(oos_core::change::Request::Clear);
    let id = Identity::default();
    let mut cost = Cost::default();
    cost.set_identity(id);
    assert_eq!(cost.grant(id, 100, WORK_BUDGET).records(), 12);
    for sample in 1..=16 {
        cost.observe(id, 2, Duration::from_micros(100 * sample), Duration::from_micros(200));
    }
    assert_eq!(cost.grant(id, 100, WORK_BUDGET).records(), 14); // p90 = 750 us pool + 100 us publication.
    cost.observe(id, 2, Duration::from_millis(20), Duration::from_micros(200));
    assert_eq!(cost.grant(id, 100, WORK_BUDGET).records(), 13);
    cost.set_identity(next);
    cost.observe(id, 1, Duration::from_nanos(1), Duration::ZERO); // Late completion belongs to the old identity.
    assert_eq!(cost.grant(next, 100, WORK_BUDGET).records(), 12);
}

#[test]
fn a_singleton_grant_is_serial_only_when_cost_exceeds_the_configured_budget() {
    let id = Identity::default();
    let mut cost = Cost::default();
    cost.set_identity(id);
    cost.observe(id, 1, Duration::from_millis(7), Duration::from_millis(1));
    assert_eq!(cost.grant(id, 99, Duration::from_millis(4)), Grant::Serial);
    assert_eq!(cost.grant(id, 99, Duration::from_millis(8)), Grant::Overlapping(1));
    assert_eq!(cost.grant(id, 99, Duration::from_millis(20)), Grant::Overlapping(2));
    assert_eq!(cost.grant(id, 1, Duration::from_millis(20)), Grant::Overlapping(1));
}

#[test]
fn two_threads_release_every_old_flight_across_identity_changes() {
    let (_, new) = drawpipe::identity_tests::identities(oos_core::change::Request::Clear);
    let old = Identity::default();
    let admission = Arc::new(Admission::default());
    let counters = Counters::default();
    let event = Arc::new(SourceWake::default());
    let boundary = Barrier::new(2);
    std::thread::scope(|threads| {
        threads.spawn(|| {
            for _ in 0..10_000 {
                admission.set_identity(old);
                let mut flight = admission.submit(&event, old, &counters);
                flight.worked(1, Duration::from_nanos(1));
                boundary.wait();
                boundary.wait();
                flight.published(Duration::ZERO);
                drop(flight);
                boundary.wait();
                boundary.wait();
            }
        });
        for _ in 0..10_000 {
            boundary.wait();
            assert!(admission.busy());
            let seen = event.generation();
            admission.set_identity(new);
            boundary.wait();
            let _ = event.wait(seen, Duration::from_secs(100));
            boundary.wait();
            assert!(!admission.busy());
            assert_ne!(event.generation(), seen);
            assert_eq!(admission.grant(new, 100, WORK_BUDGET).records(), 12);
            boundary.wait();
        }
    });
}

struct Script {
    pipe: Pipe,
    picture: crate::wave::Phosphor,
    window: crate::picture::DepositWindow,
    model: crate::model::Model,
    now: Instant,
    records: u64,
}

impl Script {
    fn new() -> Self {
        let mut model = crate::model::Model::default();
        crate::store::set(&mut model, "tdiv", "0.000001").unwrap();
        let counters = Box::leak(Box::<Counters>::default());
        let pipe = Pipe::start(2, 0, counters);
        pipe.publish_view(&model, 32, 32, Interp::Exact, Identity::default());
        Self {
            pipe,
            picture: crate::wave::Phosphor::new(32, 32),
            window: crate::picture::DepositWindow::default(),
            model,
            now: Instant::now(),
            records: 0,
        }
    }

    fn read(&mut self, cost: Duration, waiting: Duration) -> u32 {
        use crate::board::scripted::{self, Batch as _};
        let id = Identity::default();
        let grant = self.pipe.grant(id, 256, WORK_BUDGET).records();
        let (batch, count, _) = scripted::draw_route_batch(&self.model, &Default::default(), grant as usize);
        let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
        let clock = StageClock::from(self.now)
            .with_admission()
            .dequeue_for_test(self.now + waiting)
            .pool_for_test(cost * grant);
        let counters = self.pipe.counters;
        Counters::add(&counters.acquired, count as u64);
        assert!(self
            .pipe
            .feed_readout(
                (Box::new(batch.into_bytes()), fb, 0..count),
                (id, clock),
                Arc::new(move |_, bytes, frame| decode(bytes, frame)),
                codes,
                (None, None),
            )
            .is_none());
        drawpipe::host::barrier(&self.pipe);
        assert!(!self.pipe.in_flight());
        drop(self.pipe.take_buffer().unwrap());
        let accepted = self.pipe.accept_into(
            &mut self.picture,
            &mut true,
            drawpipe::Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
            Some(&mut self.window),
        );
        assert_eq!(accepted.frames, u64::from(grant));
        self.records += accepted.frames;
        self.now += drawpipe::budget::PICTURE_PERIOD;
        grant
    }

    fn assert_conservation(&self) {
        let counters = self.pipe.counters;
        let text = counters.records_text();
        assert!(text.contains(&format!("read={} deposited={} ", self.records, self.records)));
        assert!(text.ends_with("rejected.stale_key=0 rejected.epoch=0"));
        assert_eq!(counters.read().2, 0);
        assert!(counters.pipeline_text(WORK_BUDGET, 0).contains("intake.unexpected_full=0"));
    }
}

#[test]
fn scripted_reads_scale_inversely_with_pool_cost_and_stay_fresh() {
    for (scale, count) in [(3, 200), (10, 60), (100, 6)] {
        let mut script = Script::new();
        let cost = Duration::from_micros(20 * scale);
        assert_eq!(script.read(cost, Duration::ZERO), 12);
        for _ in 0..3 {
            assert_eq!(script.read(cost, Duration::from_millis(12)), count);
        }
        script.assert_conservation();
    }
}

#[test]
fn scripted_step_change_converges_without_rejection() {
    let mut script = Script::new();
    let before = Duration::from_micros(375);
    let after = Duration::from_micros(750);
    for _ in 0..16 {
        script.read(before, Duration::from_millis(10));
    }
    assert_eq!(script.read(after, Duration::from_millis(10)), 32);
    assert_eq!(script.read(after, Duration::from_millis(10)), 32);
    assert_eq!(script.read(after, Duration::from_millis(10)), 16);
    script.assert_conservation();
}

#[test]
fn an_occupied_ui_slot_keeps_the_next_batch_in_flight_after_dma_return() {
    let mut script = Script::new();
    let id = Identity::default();
    let gen = lock(&script.pipe.view).gen;
    let occupied = crate::picture::deposit::Deposit::new(gen, (32, 32), Some(id), [true; 4]);
    assert!(script.pipe.slot.put(occupied).is_ok());

    let (batch, count, _) = crate::board::scripted::draw_route_batch(&script.model, &Default::default(), 2);
    use crate::board::scripted::Batch as _;
    let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
    let clock = StageClock::from(script.now)
        .with_admission()
        .dequeue_for_test(script.now)
        .pool_for_test(Duration::from_millis(2));
    script.pipe.feed_readout(
        (Box::new(batch.into_bytes()), fb, 0..count),
        (id, clock),
        Arc::new(move |_, bytes, frame| decode(bytes, frame)),
        codes,
        (None, None),
    );
    script.pipe.wait_owner_output();
    assert!(script.pipe.take_buffer().is_some());
    assert!(script.pipe.in_flight());

    let prior = script.pipe.slot.take().unwrap();
    script.pipe.slot.recycle(prior);
    drawpipe::host::barrier(&script.pipe);
    assert!(!script.pipe.in_flight());
    let published = script.pipe.slot.take().unwrap();
    assert_eq!(published.records, count as u64);
    script.pipe.slot.recycle(published);

    let draw = script.read(Duration::from_millis(1), Duration::from_millis(100));
    assert!(draw > 0);
    assert_eq!(script.pipe.counters.read().2, 0);
}

#[test]
fn a_second_submit_counts_the_invariant_violation_without_a_hidden_wait() {
    let counters = Counters::default();
    let admission = Arc::new(Admission::default());
    let source = Arc::new(SourceWake::default());
    let id = Identity::default();
    let first = admission.submit(&source, id, &counters);
    let second = std::panic::catch_unwind(|| admission.submit(&source, id, &counters));
    assert_eq!(second.is_err(), cfg!(debug_assertions));
    drop(second);
    assert!(admission.busy(), "the original flight still owns admission");
    drop(first);
    assert!(!admission.busy());
    assert!(counters.pipeline_text(WORK_BUDGET, 0).contains("admission.unexpected_busy=1"));
}
