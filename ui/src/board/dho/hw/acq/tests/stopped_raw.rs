// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::{AcqMode, Model};
use oos_core::change::{Shadow, UiAcq};

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
        assert_eq!(edited.events, baseline.events, "{edit:?}");
        assert_eq!(edited_capture, capture, "{edit:?}");
        assert_eq!(edited_view, view, "{edit:?}");
    }
    let stop = baseline.events.iter().position(|event| *event == Event::Stop).unwrap();
    assert_eq!(trigger_plans(&baseline.events[stop..]), 0);
    assert_eq!(baseline.events[stop + 1], Event::Wait(HELD_POLL));
    assert_eq!(view.identity, capture.identity);
    assert!(view.points < capture.points);
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
