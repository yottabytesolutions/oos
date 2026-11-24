// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{self, clock::StageClock, route::Route, tests, Counters, Pipe};
use crate::sparse::Interp;
use crate::wave::Phosphor;
use oos_core::change::Request;
use std::sync::Arc;

#[test]
fn scripted_batch_clocks_telescope_and_routes_conserve_placed_records() {
    let subject = &tests::model(1e-6, false, false);
    let (width, height, records) = (97, 48, 3);
    let (bytes, frame_bytes, settings, params) = tests::batch(records, 12_512, false);
    let counters = Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(2, 0, counters);
    let identity = Identity::default();
    pipe.publish_view(subject, width, height, Interp::Exact, identity);

    let decode = tests::dec(&settings, &params);
    let read = Instant::now();
    let first_trigger = read - Duration::from_millis(5);
    pipe.feed_readout(
        (Box::new(tests::Bytes(bytes)), frame_bytes, 0..records),
        (identity, StageClock::read(Some(first_trigger), read)),
        Arc::new(move |_, bytes, f| decode(bytes, f)),
        None,
        (None, None),
    );
    drawpipe::host::finish_batch(&pipe, 0).unwrap();
    let mut deposit = pipe.slot.claim().unwrap().deposit.take().unwrap();
    let composed = Instant::now();
    let intervals = deposit.extent.clocks().outcome(composed).next().unwrap();
    let components: Duration = intervals[..6].iter().map(|interval| interval.unwrap()).sum();
    assert_eq!(components, composed - read);
    assert_eq!(intervals[6], Some(composed - first_trigger));

    assert!(pipe.slot.put(deposit).is_ok());
    let mut picture = Phosphor::new(width, height);
    assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, records as u64);
    assert_eq!(counters.routes.count(Route::Volts), records as u64);
    assert_eq!(counters.routes.total(), records as u64);
    assert!(oos_hal::sync::lock(&counters.metrics.0).accepted[6].quantiles_us()[0] >= 5000);
}

#[test]
fn acquisition_pair_resets_accepted_rejected_source_and_work() {
    let metrics = Metrics::default();
    let id = Identity::default();
    let (_, stats_only) = drawpipe::identity_tests::identities(Request::StatsReset);
    let (_, cleared) = drawpipe::identity_tests::identities(Request::Clear);
    metrics.set_identity(id);
    let start = Instant::now();
    let elapsed = Duration::from_millis(10);
    let mut clocks = BatchClocks::default();
    clocks.push(StageClock::read(Some(start), start));
    for accepted in [true, false] {
        metrics.finish(Some(id), &clocks, start + elapsed, accepted);
    }

    metrics.work(Some(id), &Work { pool: elapsed, ..Work::default() }, 2, true);
    metrics.source_wait(id, elapsed);
    for i in 1..=2 {
        metrics.source_batch(
            id,
            SourceBatch {
                read_at: start + elapsed * i,
                records: 2,
                arm: Duration::from_millis(1),
                fill: Duration::from_millis(5),
                read: Duration::from_millis(2),
                readout: Duration::from_millis(1),
                serial: i == 2,
                first_trigger: Duration::from_millis(3),
                grant: 3 - i,
                bytes: 8192,
            },
        );
    }
    metrics.source_capture_discarded(1);
    let routes = Default::default();
    let budget = Duration::from_millis(17);
    let text = metrics.text(&routes, budget);
    assert!(text.contains(" trigger_composed=10000/10000/10000 "));
    assert!(text.contains(" rejected.trigger_composed=10000/10000/10000 stage.truncated=1 "));
    assert!(text.contains(" pool=5000000/5000000 "));
    assert!(text.contains(" wait_admission=1,10.000/10.000 arm=1.000/1.000 fill=5.000/5.000 read=2.000/2.000 readout=1.000/1.000 records_per_s=200.000/200.000 duty=0.500/0.500"));
    assert!(text.ends_with(
        " budget_ms=17 source.serial=1 first_trigger=3.000/3.000 grant=1/2 bytes=8192/8192 captured_discarded=1 uncorrected=0 work_per_record_ns=0 fixed_cost_ns=0 parallelism=0"
    ));
    metrics.set_identity(stats_only);
    assert_eq!(metrics.text(&routes, budget), text);

    metrics.set_identity(cleared);
    metrics.finish(Some(id), &clocks, start + elapsed, false);
    metrics.work(Some(id), &Work { pool: elapsed, ..Work::default() }, 1, true);
    metrics.source_wait(id, elapsed);
    let text = metrics.text(&routes, budget);
    assert!(text.contains(" trigger_composed=0/0/0 "));
    assert!(text.contains(" rejected.trigger_composed=0/0/0 stage.truncated=0 "));
    assert!(text.contains(" work.v1 refine=0/0 envelope=0/0 tiles=0/0 pool=0/0 "));
    assert!(text.contains(" source.v1 wait_admission=0,0.000/0.000 arm=0.000/0.000 "));
    assert!(text
        .ends_with(" budget_ms=17 source.serial=0 first_trigger=0.000/0.000 grant=0/0 bytes=0/0 captured_discarded=0 uncorrected=0 work_per_record_ns=0 fixed_cost_ns=0 parallelism=0"));
}

#[test]
fn batch_windows_retain_the_latest_sixty_four_samples() {
    let mut window = Window::default();
    window.add([Some(100), Some(200)]);
    window.add([Some(0), None]);
    assert_eq!(window.quantiles(0), [0, 100]);
    assert_eq!(window.quantiles(1), [200, 200]);
    for i in 0..64 {
        window.add([Some(i), None]);
    }
    assert_eq!(window.quantiles(0), [31, 57]);
    assert_eq!(window.quantiles(1), [0, 0]);
}

#[test]
fn state_query_appends_the_versioned_sections_after_legacy_fields() {
    let _scope = drawpipe::test_scope::Scope::new();
    let conf = "acq.work_budget_ms = 17\npacing.clock = timer\npacing.flip_divisor = 4";
    let switches = crate::sysconf::Switches::parse(&|_| None, conf, "", Default::default());
    let host = crate::ui::host::Host::fixture().with_switches(switches);
    let mut ui = crate::ui::Ui::new(crate::wave::Source::Demo { t: 0.0 }, host);
    let mut scpi = crate::scpi::Scpi::open(0, false);
    let reply = scpi.exec_line(&mut ui, &crate::gfx::Canvas::new(1, 1), ":SYSTem:STATe?");
    let text = String::from_utf8(reply).unwrap();
    let (legacy, pipeline) = text.split_once(" stage.v1 ").unwrap();
    assert!(legacy.contains(" graphics ") && legacy.contains(" not_used=") && legacy.contains(" deposit.slice "));
    assert!(legacy.contains(" acq.write_gap=0 acq.sched=fifo acq.work_budget_ms=17 "));
    assert!(legacy.contains(" pacing.clock=timer pacing.flip_divisor=4 pacing.period_us=33333 "));
    let (_, work) = pipeline.split_once(" work.v1 ").unwrap();
    let (_, route) = work.split_once(" route.v1 ").unwrap();
    let (_, source) = route.split_once(" source.v1 ").unwrap();
    assert!(source.trim_end().ends_with("duty=0.000/0.000 budget_ms=17 source.serial=0 first_trigger=0.000/0.000 grant=0/0 bytes=0/0 captured_discarded=0 uncorrected=0 work_per_record_ns=0 fixed_cost_ns=0 parallelism=0 step.missed=0"));
}
