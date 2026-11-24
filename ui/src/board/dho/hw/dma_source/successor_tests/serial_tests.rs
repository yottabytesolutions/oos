// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::board::scripted::{self, Batch};
use crate::drawpipe::{budget::PICTURE_PERIOD, clock::StageClock, host, test_scope::Scope, Counters, Stats};
use crate::model::Model;
use crate::sysconf::Switches;
use std::{cell::Cell, time::Instant};

fn submit(pipe: &Pipe, (batch, count): (impl Batch<Bytes: 'static>, usize), mut clock: StageClock, cost: Duration) {
    let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
    clock = clock.with_admission().pool_for_test(cost);
    Counters::add(&pipe.batch_counters().acquired, count as u64);
    assert!(pipe
        .feed_readout(
            (Box::new(batch.into_bytes()), fb, 0..count),
            (Default::default(), clock),
            Arc::new(move |_, bytes, frame| decode(bytes, frame)),
            codes,
            (None, None),
        )
        .is_none());
}

fn compose(pipe: &Pipe) {
    let accepted = pipe.accept_into(
        &mut crate::wave::Phosphor::new(32, 32),
        &mut true,
        Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
        Some(&mut crate::picture::DepositWindow::default()),
    );
    assert_eq!(accepted.frames, 1);
}

fn prepare_read<T>(
    pipe: &Pipe,
    plan: &ArmPlan<'_>,
    grant: Grant,
    (now, release): (Instant, impl Fn()),
    capture: impl FnOnce() -> T,
) -> T {
    let (mut dma, mut armed) = (DmaSource::default(), None);
    plan.prearm(&mut dma, &mut armed, None, Some(grant), || false, |_, _| pipe.in_flight());
    assert_eq!(armed.take().is_some(), !grant.serial());
    assert!(plan.prepare(&mut dma, &mut armed, || true, |_| panic!("canceled before arm")).is_none());

    let armed_at = Cell::new(None);
    assert!(plan
        .prepare(
            &mut dma,
            &mut armed,
            || false,
            |_| {
                assert!(pipe.in_flight(), "the FPGA can arm while the previous batch is in flight");
                armed_at.set(Some(now));
            }
        )
        .is_some());
    assert_eq!(armed_at.get(), Some(now));
    let captured = capture();

    let interrupt = Cell::new(true);
    assert!(plan.read_allowed(
        || {
            if grant.serial() && pipe.in_flight() {
                if interrupt.replace(false) {
                    drawpipe::wake_source();
                } else {
                    release();
                }
            }
            false
        },
        || false
    ));
    captured
}

#[test]
fn measured_over_budget_reads_are_serial_and_stay_fresh() {
    for (cost_ms, expected) in [(27, Grant::Serial), (6, Grant::Overlapping(1))] {
        let _scope = Scope::new();
        let switches = Switches::parse(&|_| None, "acq.work_budget_ms = 20", "", Default::default());
        let pipe = drawpipe::pipe(&switches).unwrap();
        let id = Default::default();
        let mut model = Model::default();
        crate::store::set(&mut model, "tdiv", "0.000001").unwrap();
        pipe.publish_view(&model, 32, 32, switches.interp, id);
        let cost = Duration::from_millis(cost_ms);
        let budget = switches.source.work_budget();
        assert_eq!(pipe.grant(id, 1, budget), Grant::Overlapping(1));
        pipe.batch_counters().metrics.inject_worker_time(id, cost);
        let first = Instant::now();
        let second = first + PICTURE_PERIOD;
        let third = second + PICTURE_PERIOD;
        let read = || {
            let (batch, count, _) = scripted::draw_route_batch(&model, &switches, 1);
            (batch, count)
        };
        submit(pipe, read(), StageClock::from(first).dequeue_for_test(first), cost);
        host::barrier(pipe);
        submit(pipe, read(), StageClock::from(second).dequeue_for_test(second), cost);
        pipe.wait_owner_output();
        assert!(pipe.in_flight(), "the occupied UI slot holds the prior batch");

        let grant = pipe.grant(id, 1, budget);
        assert_eq!(grant, expected);
        let settings = Settings { frames: grant.records(), ..Default::default() };
        let plan = ArmPlan::new(&settings, id, true, Some(pipe), grant.serial());
        let release = || {
            compose(pipe);
            host::barrier(pipe);
        };
        let captured = prepare_read(pipe, &plan, grant, (third, release), || {
            assert!(pipe.in_flight(), "capture must precede release of the previous flight");
            read()
        });
        assert_eq!(pipe.in_flight(), !grant.serial(), "overlap at the next read boundary");
        let queued = if grant.serial() {
            Duration::ZERO
        } else {
            release();
            cost
        };
        compose(pipe);
        let clock = StageClock::from(third).dequeue_for_test(third + queued);
        submit(pipe, captured, clock, cost);
        host::barrier(pipe);
        compose(pipe);
        let totals = pipe.batch_counters().records_text();
        assert!(totals.contains("read=3 deposited=3 "), "{totals}");
        assert_eq!(pipe.batch_counters().read().2, 0, "{totals}");
    }
}

#[test]
fn complete_serial_capture_bypasses_admission_on_hold_and_only_settings_refuse_read() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let settings = Settings::default();
    let out: crate::board::HwShared = Default::default();
    let asked = (None, Default::default());
    let control = |plan: &ArmPlan<'_>| plan.settings_changed(&oos_hal::sync::lock(&out), asked);
    let serial = ArmPlan::new(&settings, Default::default(), false, Some(&pipe), true);
    let overlapping = ArmPlan::new(&settings, Default::default(), false, Some(&pipe), false);
    oos_hal::sync::lock(&out).hold = true;
    assert!(serial.read_allowed(|| serial.obsolete(&out, asked), || control(&serial)));
    assert!(overlapping.read_allowed(|| overlapping.obsolete(&out, asked), || control(&overlapping)));

    {
        let mut g = oos_hal::sync::lock(&out);
        g.hold = false;
        g.force = true;
        g.cal_req = Some((vec![0], false, false));
        assert!(!serial.handoff_obsolete(&g, asked), "queued commands wait for the complete record");
    }
    assert!(serial.obsolete(&out, asked), "calibration still prevents the next arm");

    oos_hal::sync::lock(&out).want = Some(Settings { mask: 2, ..settings.clone() });
    assert!(!serial.read_allowed(|| serial.obsolete(&out, asked), || control(&serial)));
    assert!(overlapping.read_allowed(|| overlapping.obsolete(&out, asked), || control(&overlapping)));
}
