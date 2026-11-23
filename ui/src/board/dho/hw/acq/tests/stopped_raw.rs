// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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
