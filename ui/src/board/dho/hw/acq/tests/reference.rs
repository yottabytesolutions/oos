// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::identity::Publication;
use oos_core::change::{Shadow, UiAcq};

#[test]
fn references_and_peak_batches_share_one_picture_identity() {
    let mut rig = Rig::new();
    rig.pass();
    let first = rig.latest();
    assert_eq!((first.seq, first.ch[0].len(), first.pre, first.dt), (1, 14_064, 7032, 1.0 / 1_250_000_000.0));
    rig.pass(); // geometry boundary back to peak
    rig.pass(); // steady peak batch
    assert_eq!(rig.latest().seq, first.seq, "draw-only batches leave the reader on the reference");
    let last_reduced = rig.source.seq;
    assert!(last_reduced >= 4);
    rig.advance_reference();
    rig.pass();
    let second = rig.latest();
    assert_eq!((second.seq, second.cfg, second.stamp), (last_reduced + 1, first.cfg, first.stamp));
    let trace = rig.trace.borrow();
    let batches: Vec<_> = trace
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Intake(batch) => Some((batch.range().len(), batch.newest_seq(), batch.first_words()[0] >> 8)),
            _ => None,
        })
        .collect();
    assert_eq!(
        batches,
        [(1, Some(1), 1), (1, None, 1), (last_reduced as usize - 2, None, 0), (1, Some(second.seq), 1)]
    );
    let pictures: Vec<_> = trace
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Picture(p) => Some((p.frames, p.tap.as_ref().map(|tap| (tap.frame.seq(), tap.envelope)))),
            _ => None,
        })
        .collect();
    assert_eq!(pictures, [(1, Some((1, false))), (1, None), (last_reduced - 2, None), (1, Some((second.seq, false)))]);
    let counters = RecordCounts::read();
    assert_eq!((counters.read, counters.deposited, counters.losses), (second.seq, second.seq, vec![]));
}

#[test]
fn stop_at_multiple_phases_keeps_the_host_reference_and_run_uses_its_new_epoch() {
    for peak_passes in 0..=2 {
        let mut rig = Rig::new();
        rig.pass();
        let first = rig.latest();
        for _ in 0..peak_passes {
            rig.pass();
        }
        rig.io.clock.stop();
        capture(&mut rig.source, &mut rig.io);
        {
            let g = lock(&rig.source.out);
            assert!(std::sync::Arc::ptr_eq(&first, &g.frame));
            assert!(g.queue.is_empty());
            assert_eq!(g.final_ack, 7);
            assert!(g.stop_replies.loss_for(first.seq).is_some());
        }
        let id = rig.next_epoch();
        rig.io.clock.resume(id);
        for _ in 0..3 {
            rig.pass();
        }
        let next = rig.latest();
        assert_eq!((next.cfg, next.stamp), (id.gen(), id.epoch()));
        assert!(next.seq > first.seq && !next.envelope);
    }
}

#[test]
fn edited_settings_restart_the_reference_and_single_and_average_take_samples() {
    let mut rig = Rig::new();
    rig.pass();
    let old = rig.latest();
    let id = rig.next_epoch();
    {
        let mut g = lock(&rig.source.out);
        g.acquisition = Publication::new(id, true);
        g.want.as_mut().unwrap().ch[0].offset = 0.75;
    }
    rig.io.clock.resume(id);
    for _ in 0..3 {
        rig.pass();
    }
    assert_eq!(rig.latest().stamp, id.epoch());
    assert!(rig.latest().seq > old.seq);
    let model = picture::model(1e-6);
    let sw = Switches::parse(&|_| None, "acq.capture = peak:2\nacq.capture.inferred = on", "", Default::default());
    let want = |m: &crate::model::Model| super::super::super::settings(m, false, &sw);
    assert!(want(&model).mode.is_peak());
    let single = crate::model::Model { trig_mode: crate::model::TrigMode::Single, ..model.clone() };
    assert!(want(&single).single && want(&single).mode == Mode::Normal);
    let mut average = crate::model::Model { acq: crate::model::AcqMode::Average, avg_hw: false, ..model };
    assert_eq!(want(&average).mode, Mode::Normal);
    average.avg_hw = true;
    assert!(matches!(want(&average).mode, Mode::Average { .. }));
}

struct Rig {
    source: AcqLoop<FixedPacing>,
    io: Io<sim::Sim>,
    trace: std::rc::Rc<std::cell::RefCell<sim::Trace>>,
    _scope: Scope,
}

impl Rig {
    fn new() -> Self {
        let mut rig = Self::unqualified();
        let pair = Request::read(&lock(&rig.source.out)).pair();
        let reference = &mut rig.source.batch.raw.reference;
        reference.track(pair);
        for _ in 0..2 {
            reference.observe(pair, 4, Duration::from_millis(4));
        }
        rig
    }

    fn unqualified() -> Self {
        let scope = Scope::new();
        let mut settings = Settings { tdiv: 1e-6, depth: 14_064, ..settings() };
        settings.reduce_capture(true, 2, true).unwrap();
        let out = requested(&settings, Identity::default(), true);
        let intake = scope.gated_worker();
        drawpipe::pipe().unwrap().publish_view(
            &picture::model(settings.tdiv),
            picture::SIZE.0,
            picture::SIZE.1,
            crate::sparse::Interp::Exact,
            Identity::default(),
        );
        let (io, trace) = sim::fixture(&settings, out.clone(), (Run::CompressedTap, 4, false), Some(intake));
        let options = Spawn { mask: 1, program_afe: false, cal_dir: String::new(), start_up: None };
        let switches = Switches::parse(&|_| None, "", "acq.frame0 = keep", Default::default());
        let source = AcqLoop::new(options, switches, out, Service::default(), None, io.clock.now(), FixedPacing);
        Self { source, io, trace, _scope: scope }
    }

    fn triggers_every(&mut self, period: Duration) {
        self.io.clock.stop_triggers();
        let mut next = self.io.clock.now() + period;
        self.io.clock.drive(move |clock, pause| {
            if matches!(pause, Pause::Wait(_)) && clock.now() >= next {
                let due = 1 + clock.now().duration_since(next).as_nanos() / period.as_nanos();
                next += period * (due as u32);
                if clock.capture_armed() {
                    clock.record_triggers(due as u32);
                }
            }
        });
    }

    fn pass(&mut self) {
        capture(&mut self.source, &mut self.io);
        self.io.clock.compose();
    }

    fn advance_reference(&self) {
        self.io.clock.advance(crate::compress::RAW_PERIOD);
    }

    fn latest(&self) -> std::sync::Arc<crate::board::Frame> {
        lock(&self.source.out).frame.clone()
    }

    fn latest_tap(&self) -> (u64, usize) {
        self.trace
            .borrow()
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Picture(picture) => picture.tap.as_ref().map(|tap| (tap.frame.seq(), tap.points)),
                _ => None,
            })
            .expect("the full-rate record reached the picture tap")
    }

    fn next_epoch(&self) -> Identity {
        let model = picture::model(1e-6);
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        shadow.observe(&model, UiAcq::default(), &[oos_core::change::Request::Run], &config).unwrap().identity()
    }
}

#[test]
fn stop_during_reference_dma_keeps_the_one_completed_reference() {
    let mut rig = Rig::new();
    rig.pass();
    rig.pass();
    rig.advance_reference();
    let out = rig.source.out.clone();
    rig.io.c2h.on_dma(move |_| {
        let source = crate::wave::Source::Hw { shared: out, seen: 0 };
        source.hw_hold(true, 7);
    });
    rig.io.clock.drive(|clock, pause| {
        if matches!(pause, Pause::Park) {
            clock.compose();
        }
    });
    capture(&mut rig.source, &mut rig.io);
    let final_capture = rig.latest();
    capture(&mut rig.source, &mut rig.io);
    assert_eq!(
        lock(&rig.source.out).final_ack,
        0,
        "let the last reference reach the picture before acknowledging STOP"
    );
    capture(&mut rig.source, &mut rig.io);
    assert_eq!(final_capture.seq, 3);
    assert!(std::sync::Arc::ptr_eq(&final_capture, &lock(&rig.source.out).frame));
    assert_eq!(lock(&rig.source.out).final_ack, 7);
    assert_eq!(RecordCounts::read().deposited, 3);
}

#[test]
fn stop_while_waiting_for_a_new_reference_performs_no_dma() {
    let mut rig = Rig::new();
    rig.pass();
    let previous = rig.latest();
    rig.advance_reference();
    rig.io.clock.stop_triggers();
    rig.io.clock.drive(|clock, pause| {
        if matches!(pause, Pause::Wait(_)) && clock.capture_armed() {
            clock.stop();
        }
    });
    rig.pass();
    capture(&mut rig.source, &mut rig.io);
    assert_eq!(lock(&rig.source.out).final_ack, 7);
    assert!(std::sync::Arc::ptr_eq(&previous, &rig.latest()));
    assert_eq!(RecordCounts::read().read, 1);
}

#[test]
fn transition_to_single_or_average_publishes_samples_under_the_new_epoch() {
    for (single, mode) in [(true, Mode::Normal), (false, Mode::Average { count: 2 })] {
        let mut rig = Rig::new();
        rig.pass();
        rig.pass();
        let id = rig.next_epoch();
        let mut g = lock(&rig.source.out);
        g.acquisition = Publication::new(id, false);
        let want = g.want.as_mut().unwrap();
        (want.single, want.mode) = (single, mode);
        drop(g);
        rig.pass();
        let next = rig.latest();
        assert_eq!((next.cfg, next.stamp), (id.gen(), id.epoch()));
        assert!(!next.envelope);
        assert_eq!(next.hw_avg, !single);
        assert_eq!((next.ch[0].len(), next.dt), (14_064, 1.0 / 1_250_000_000.0));
        assert_eq!(rig.source.batch.single_done.is_some(), single);
    }
}

#[test]
fn warmed_reference_dma_backing_is_reused_when_ownership_is_exchanged() {
    let mut source = DmaSource::<DmaBuf>::default();
    let mut raw = DmaBuf::default();
    let size = 28_128;
    let mut input = std::io::repeat(0);
    for dma in [source.destination(), &mut raw] {
        dma.read(&mut input, size, |_, _| {}).unwrap();
    }
    let (allocations, ()) = crate::system::test_allocator::count_own(|| {
        for _ in 0..8 {
            raw.read(&mut input, size, |_, _| {}).unwrap();
            source.exchange(&mut raw);
            assert_eq!(source.destination().data().len(), size);
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn sparse_normal_triggers_keep_the_identity_full_rate() {
    let mut rig = Rig::unqualified();
    lock(&rig.source.out).want.as_mut().unwrap().auto = false;
    rig.triggers_every(Duration::from_millis(1200));
    for _ in 0..3 {
        rig.pass();
        assert_eq!(rig.latest_tap().1, 14_064);
        assert!(!rig.source.batch.raw.reference.can_reduce((0, 0)));
    }
    assert!(rig.latest_tap().0 >= 3);
    assert!(!drawpipe::counters().acquisition.text().contains("acq_capture_transition_timeout="));
}

#[test]
fn transition_timeout_falls_back_once_without_reporting_an_acquire_error() {
    for completed_guard in [false, true] {
        let mut rig = Rig::new();
        rig.pass();
        rig.pass();
        rig.advance_reference();
        let last = rig.latest().seq;
        rig.io.clock.stop_triggers();
        let mut trigger = completed_guard;
        rig.io.clock.drive(move |clock, pause| {
            if trigger && matches!(pause, Pause::Wait(_)) && clock.capture_armed() {
                clock.record_triggers(1);
                trigger = false;
            }
        });
        let status = lock(&rig.source.out).status.latest().to_owned();
        rig.pass();
        assert_eq!(rig.latest().seq, last);
        assert_eq!(lock(&rig.source.out).status.latest(), status);
        assert!(!rig.io.clock.capture_armed());
        rig.io.clock.resume_triggers(4);
        for _ in 0..4 {
            rig.pass();
            assert_eq!(rig.source.configured.current.as_ref().unwrap().0.mode, Mode::Normal);
        }
        assert!(rig.latest_tap().0 > last);
        assert!(drawpipe::counters().acquisition.text().contains("acq_capture_transition_timeout=1"));
    }
}

#[test]
fn steady_peak_references_conserve_records_over_two_virtual_seconds() {
    let mut rig = Rig::unqualified();
    let start = rig.io.clock.now();
    rig.triggers_every(Duration::from_millis(8));
    while rig.io.clock.now() - start < Duration::from_secs(2) {
        rig.pass();
    }
    let trace = rig.trace.borrow();
    let references = trace
        .events
        .iter()
        .filter(
            |event| matches!(event, Event::Intake(batch) if batch.newest_seq().is_some() && batch.range().len() == 1),
        )
        .count();
    let peaks = trace
        .events
        .iter()
        .filter(|event| matches!(event, Event::Intake(batch) if batch.newest_seq().is_none()))
        .count();
    assert!(references >= 8 && peaks > references);
    let counters = RecordCounts::read();
    assert_eq!(counters.read, counters.deposited + counters.losses.iter().map(|(_, n)| n).sum::<u64>());
    assert!(counters.losses.is_empty());
    assert!(!drawpipe::counters().acquisition.text().contains("acq_capture_transition_timeout="));
    assert!(drawpipe::counters().pipeline_text(Duration::from_millis(12), 0).contains("captured_discarded=0"));
}

#[test]
fn automatic_capture_replans_using_the_published_plot_width() {
    let mut rig = Rig::new();
    rig.source.switches = Switches::parse(&|_| None, "acq.capture = auto", "", Default::default());
    let model = picture::model(1e-6);
    lock(&rig.source.out).want = Some(super::super::super::settings(&model, false, &rig.source.switches));
    let src = crate::wave::Source::Hw { shared: rig.source.out.clone(), seen: 0 };
    for (width, expected) in [(600, "peak:4"), (900, "peak:2"), (1800, "full")] {
        drawpipe::pipe().unwrap().publish_view(
            &model,
            width,
            picture::SIZE.1,
            crate::sparse::Interp::Exact,
            Identity::default(),
        );
        rig.pass();
        let mode = rig.source.configured.current.as_ref().unwrap().0.mode;
        let active = match mode {
            Mode::Reduced(reduction) => reduction.to_string(),
            _ => "full".into(),
        };
        assert_eq!(active, expected, "width {width}");
        assert!(crate::board::board().capture_state(&src).contains(&format!(" acq.capture={expected} ")));
    }
}
