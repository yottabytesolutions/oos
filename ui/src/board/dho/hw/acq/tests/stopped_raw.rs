// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::{AcqMode, Model};
use crate::wave::Source;
use oos_core::change::{Shadow, UiAcq};

#[test]
fn clear_during_serial_stop_dma_acknowledges_the_discarded_final_record() {
    let _scope = Scope::new();
    let want = settings();
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut shadow = oos_core::change::Shadow::new(&model, Default::default(), &config).unwrap();
    let old = shadow.identity();
    let out = requested(&want, old, false);
    {
        let mut g = lock(&out);
        g.record_flow.cost.set_identity(old);
        g.record_flow.cost.observe(old, 1, Duration::from_millis(50), Duration::ZERO);
    }
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::StopDuring, 1, false), None);
    let (mut source, started) = source(&out, &io);
    let shared = out.clone();
    io.c2h.on_dma(move |_| {
        let mut g = lock(&shared);
        assert!(g.hold && g.record_flow.serial);
        assert_eq!((g.final_req, g.final_ack), (7, 0));
        let clear = shadow
            .observe(&model, Default::default(), &[oos_core::change::Request::Clear], &config)
            .unwrap()
            .identity();
        assert_ne!(clear.epoch(), old.epoch());
        g.acquisition = crate::board::identity::Publication::new(clear, false);
        g.record_flow.wake();
    });
    capture(&mut source, &mut io);
    // Observe the DMA completion itself, before another source pass can acknowledge STOP.
    let result = snapshot(&out, &io, &trace, started);
    assert_eq!(result.counters.read, 1);
    assert_eq!(result.counters.loss("lost.settings"), 1);
    assert!(result.queue.is_empty() && result.latest.is_none());
    assert_eq!(result.final_ack, 7);
}

#[test]
fn stopped_view_discards_a_replay_completed_after_run() {
    twice(|| obsolete_stopped_replay(true));
}

#[test]
fn stopped_view_rejects_a_reply_for_an_older_settings_generation_in_the_same_epoch() {
    twice(|| obsolete_stopped_replay(false));
}

fn obsolete_stopped_replay(resume: bool) -> Outcome {
    use crate::board::identity::Publication;
    use oos_core::change::Request;

    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let mut model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let old = shadow.identity();
    let out = requested(&want, old, false);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let (mut source, started) = source(&out, &io);
    capture(&mut source, &mut io);
    io.clock.stop();
    let view = Some((1e-6, 0.0));
    lock(&out).stop_view = view;
    let shared = out.clone();
    io.c2h.on_dma(move |trace| {
        let mut g = lock(&shared);
        assert!(g.hold && g.stop_view == view);
        let requests = if resume {
            Source::stage_hold(&mut g, false, 7);
            g.stop_view = None;
            trace.events.push(Event::Run);
            &[Request::Run][..]
        } else {
            model.tscale += 1;
            g.want.as_mut().unwrap().tdiv *= 2.0;
            &[]
        };
        let id = shadow.observe(&model, UiAcq::default(), requests, &config).unwrap().identity();
        assert_eq!(id.epoch() != old.epoch(), resume);
        assert_eq!(id.gen() != old.gen(), !resume);
        g.acquisition = Publication::new(id, false);
        g.record_flow.wake();
    });
    capture(&mut source, &mut io);
    assert!(lock(&out).replay.is_none());
    assert!(source.replay.replayed.is_none());
    let result = snapshot(&out, &io, &trace, started);
    assert_eq!(result.events.iter().filter(|e| matches!(e, Event::Dma(_))).count(), 2);
    assert_eq!(result.latest, Some(Frame::stamp(4, 3)));
    result
}

#[test]
fn stopped_view_returns_a_raw_window_without_replacing_the_source_record() {
    let (outcome, capture, view) = twice(stop_with_view);
    let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
    assert_eq!(outcome.events[..stop].iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
    assert_eq!(outcome.events[stop..].iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
    assert_eq!((capture.seq, view.seq, outcome.final_ack), (4, 5, 7));
    assert_eq!(capture.identity, view.identity);
    assert_eq!(outcome.latest, Some(capture.samples));
    assert!(view.points < capture.points);
    assert!(view.trigger.is_some());
    assert!(capture.trigger.is_none());
}

#[derive(Debug, PartialEq)]
struct RawFrame {
    samples: Frame,
    seq: u64,
    points: usize,
    identity: (u64, u64),
    trigger: Option<f64>,
}

#[test]
fn stopped_view_uses_captured_geometry_and_mode_after_held_edits() {
    let (baseline, capture, view) = twice(|| stopped_edited_view(None));
    for edit in [Edit::Timebase, Edit::Depth, Edit::Channels, Edit::Mode] {
        let (edited, edited_capture, edited_view) = twice(|| stopped_edited_view(Some(edit)));
        let hardware_events = |event: &&Event| !matches!(event, Event::RingClear(_));
        assert!(
            edited.events.iter().filter(hardware_events).eq(baseline.events.iter().filter(hardware_events)),
            "{edit:?}"
        );
        assert_eq!(edited_capture, capture, "{edit:?}");
        assert_eq!(edited_view, view, "{edit:?}");
    }
    let stop = baseline.events.iter().position(|event| *event == Event::Stop).unwrap();
    assert_eq!(trigger_plans(&baseline.events[stop..]), 0);
    assert_eq!(baseline.events[stop + 1], Event::Wait(HELD_POLL));
    assert_eq!(view.identity, capture.identity);
    assert!(view.points < capture.points);
}

#[test]
fn stop_on_an_identity_change_only_rereads_when_the_ring_is_retained() {
    for prearm in [false, true] {
        let (outcome, capture, view) = twice(|| stop_at_boundary(prearm));
        let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
        let held = &outcome.events[stop + 1..];
        assert_eq!(held.iter().filter(|event| matches!(event, Event::Dma(_))).count(), usize::from(!prearm));
        assert_eq!(trigger_plans(held), 0);
        assert!(!held.iter().any(is_arm));
        assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
        assert_eq!(outcome.final_ack, 7);
        if prearm {
            assert!(view.is_none());
        } else {
            assert_eq!(view.unwrap().identity, capture.identity);
        }
    }
}

fn stop_at_boundary(prearm: bool) -> (Outcome, RawFrame, Option<RawFrame>) {
    use oos_hal::dho::bus::Bus;

    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let model = Model::default();
    let shadow = Shadow::new(&model, UiAcq::default(), &crate::model::config()).unwrap();
    let out = requested(&want, shadow.identity(), false);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let options = Spawn { mask: want.mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let conf = if prearm { "acq.prearm = on" } else { "" };
    let switches = Switches::parse(&|_| None, conf, "acq.frame0 = keep", Default::default());
    let started = io.clock.now();
    let mut source = AcqLoop::new(options, switches, out.clone(), Service::default(), None, started, FixedPacing);
    capture(&mut source, &mut io);
    assert_eq!(source.batch.armed.is_some(), prearm);
    if prearm {
        // Give the successor a full poll of triggers before STOP reaches the source.
        assert_ne!(io.bar.rd(0x4080), 0);
    }
    io.clock.stop();
    {
        let mut g = lock(&out);
        Edit::Timebase.publish(&mut g, model, shadow);
        g.stop_view = Some((1e-6, 0.0));
    }
    capture(&mut source, &mut io);
    let g = lock(&out);
    let frames = (raw_frame(&g.frame), g.replay.as_ref().map(raw_frame));
    let outcome = Outcome::snapshot(&mut trace.borrow_mut(), &g, io.clock.now().duration_since(started));
    (outcome, frames.0, frames.1)
}

#[derive(Clone, Copy, Debug)]
enum Edit {
    Timebase,
    Depth,
    Channels,
    Mode,
}

impl Edit {
    fn publish(self, g: &mut crate::board::Shared, mut model: Model, mut shadow: Shadow) {
        let want = g.want.as_mut().unwrap();
        match self {
            Self::Timebase => {
                model.tscale += 1;
                want.tdiv *= 2.0;
            }
            Self::Depth => (model.mdepth, want.depth) = (1, 140_000),
            Self::Channels => (model.ch[1].on, want.mask) = (true, 3),
            Self::Mode => (model.acq, want.mode) = (AcqMode::Peak, Mode::Peak),
        }
        let id = shadow.observe(&model, UiAcq::default(), &[], &crate::model::config()).unwrap().identity();
        assert_ne!(id.gen(), g.acquisition.identity().gen());
        g.acquisition = crate::board::identity::Publication::new(id, false);
        g.record_flow.wake();
    }
}

fn stopped_edited_view(edit: Option<Edit>) -> (Outcome, RawFrame, RawFrame) {
    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let model = Model::default();
    let shadow = Shadow::new(&model, UiAcq::default(), &crate::model::config()).unwrap();
    let out = requested(&want, shadow.identity(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut edit = edit.map(|edit| (edit, model, shadow));
    let mut source = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let view = Some((1e-6, 0.0));
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => {
            clock.stop();
            let mut g = lock(&shared);
            // CH2 is absent from the applied capture, even if the pending edit asks to enable it.
            g.dvm_want = Some(crate::board::DvmWant { ch: 1, lane: None });
            if let Some((edit, model, shadow)) = edit.take() {
                edit.publish(&mut g, model, shadow);
            }
        }
        Pause::Wait(HELD_POLL) => assert!(source.hw_stop(view).is_none()),
        Pause::Wait(delay) if delay == Duration::from_millis(20) => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(_) => {}
    });
    let g = lock(&out);
    (outcome, raw_frame(&g.frame), raw_frame(g.replay.as_ref().unwrap()))
}

fn raw_frame(frame: &crate::board::Frame) -> RawFrame {
    assert!(!frame.envelope);
    RawFrame {
        samples: Frame::read(frame),
        seq: frame.seq,
        points: frame.ch[0].len(),
        identity: (frame.cfg, frame.stamp),
        trigger: frame.fixed_ti,
    }
}

fn stop_with_view() -> (Outcome, RawFrame, RawFrame) {
    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let mut source = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let shared = out.clone();
    let view = Some((1e-6, 0.0));
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => {
            clock.stop();
            assert!(source.hw_stop(view).is_none());
        }
        Pause::Wait(delay) if delay == Duration::from_millis(20) => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(_) => {}
    });
    let mut source = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let reread = source.hw_stop(view).unwrap();
    let g = lock(&out);
    assert_eq!(g.frame.ch[0].len(), g.frame.acq.1);
    (outcome, raw_frame(&g.frame), raw_frame(&reread))
}
