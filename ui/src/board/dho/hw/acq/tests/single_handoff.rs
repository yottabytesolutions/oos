// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    model::{RunState, TrigMode},
    ui::{change, key, Ui},
    wave::Source,
};

const LIMIT: Duration = Duration::from_secs(3);
const EPSILON: Duration = Duration::from_micros(1);
const MISSED: &str = "SINGLE completed but the UI did not take over";
const STOP_UNCONFIRMED: &str = "stop not confirmed by the hardware";

struct Rig {
    _scope: Scope,
    ui: Ui,
    source: AcqLoop<FixedPacing>,
    io: Io<sim::Sim>,
    trace: std::rc::Rc<std::cell::RefCell<sim::Trace>>,
}

impl Rig {
    fn new(single: bool) -> Self {
        let scope = Scope::new();
        let out = HwShared::default();
        let mut ui = Ui::for_tests(Source::Hw { shared: out.clone(), seen: 0 });
        ui.m = picture::model(settings().tdiv);
        ui.m.trig_mode = TrigMode::Normal;
        // Keep the fixture on the decoded-record route after switching from SINGLE to RUN.
        ui.m.z_src = Some(0);
        if single {
            ui.key(key::SINGLE);
        } else {
            ui.m.run = RunState::Run;
        }
        change::settle(&mut ui);
        let want = lock(&out).want.clone().unwrap();
        assert_eq!(want.single, single);
        let (io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 1, false), None);
        let (source, _) = source(&out, &io);
        Self { _scope: scope, ui, source, io, trace }
    }

    fn out(&self) -> &HwShared {
        &self.source.out
    }

    fn request(&self) -> Request {
        Request::read(&lock(self.out()))
    }

    fn poll(&mut self, request: &Request) -> Option<(Settings, hz::Params)> {
        let mut last_read = std::mem::take(&mut self.source.replay.fresh);
        self.source.reconfigure(&mut self.io, request, &mut last_read)
    }

    fn capture(&mut self) {
        capture(&mut self.source, &mut self.io);
    }

    fn state(&mut self) -> String {
        let mut scpi = crate::scpi::Scpi::disconnected();
        let reply = scpi.exec_line(&mut self.ui, &crate::gfx::Canvas::new(1, 1), ":SYSTem:STATe?");
        String::from_utf8(reply).unwrap()
    }
}

#[test]
fn expiry_reports_and_wakes_once_across_running_ui_publications() {
    twice(|| {
        let mut rig = Rig::new(true);
        rig.capture();
        rig.io.clock.advance(LIMIT + EPSILON);
        let mut reports = 0;
        let mut wakes = 0;
        let mut holds = 0;
        for _ in 0..3 {
            change::settle(&mut rig.ui);
            let before = lock(rig.out()).record_flow.command_generations();
            let request = rig.request();
            assert!(!request.run.hold);
            assert!(rig.poll(&request).is_none());
            let mut g = lock(rig.out());
            reports += usize::from(g.status.take_latest() == MISSED);
            wakes += g.record_flow.command_generations().0 - before.0;
            holds += usize::from(g.hold);
            assert_eq!(g.queue.len(), 1);
        }
        assert_eq!((reports, wakes, holds), (1, 1, 0));
        assert!(rig.state().contains(&format!(" acq.status={MISSED:?} ")));
        rig.trace.take().events
    });
}

#[test]
fn ui_takes_single_just_before_and_just_after_the_bound() {
    for expired in [false, true] {
        twice(|| ui_takes_single(expired));
    }
}

fn ui_takes_single(expired: bool) -> Vec<Event> {
    let mut rig = Rig::new(true);
    rig.capture();
    let posted = rig.trace.borrow().events.len();
    rig.io.clock.advance(if expired { LIMIT + EPSILON } else { LIMIT - EPSILON });
    if expired {
        assert!(rig.poll(&rig.request()).is_none());
        assert!(!rig.request().run.hold, "the source cannot take the UI's hold");
    }
    rig.ui.step(0.04);
    assert_eq!(rig.ui.records, 1);
    assert_eq!(lock(rig.out()).frame.seq, 1);
    assert!(lock(rig.out()).queue.is_empty());
    rig.ui.step(0.04);
    assert!(rig.request().run.hold);
    assert_eq!(rig.state().contains(MISSED), expired);
    // Exercise the final acknowledgement independently of a requested stopped-view reread.
    assert!(rig.ui.src.hw_stop(None).1.is_none());
    rig.ui.src.hw_hold(true, 7);
    assert!(rig.poll(&rig.request()).is_none());
    rig.ui.step(0.04);
    assert_eq!(rig.ui.records, 1);
    let g = lock(rig.out());
    assert!(g.queue.is_empty());
    assert_eq!(g.final_ack, 7);
    let events = rig.trace.take().events;
    assert_eq!(events.iter().filter(|e| is_arm(e)).count(), 1);
    assert!(events[..posted].iter().any(|e| matches!(e, Event::Dma(_))));
    assert!(!events[posted..].iter().any(|e| matches!(e, Event::Dma(_))));
    events
}

#[test]
fn same_epoch_hold_release_does_not_report_a_missed_single() {
    twice(|| {
        let mut rig = Rig::new(true);
        rig.capture();
        let pair = rig.request().pair();
        rig.io.clock.stop();
        assert!(rig.poll(&rig.request()).is_none());
        rig.io.clock.release();
        rig.io.clock.advance(LIMIT);
        let request = rig.request();
        assert_eq!(request.pair(), pair);
        assert!(request.want.as_ref().unwrap().single);
        assert!(rig.poll(&request).is_none());
        assert!(!rig.state().contains(MISSED));
        let events = rig.trace.take().events;
        assert_eq!(events.iter().filter(|e| is_arm(e)).count(), 1);
        events
    });
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Settings,
    Run,
    Single,
    Clear,
}

#[test]
fn boundary_publication_preempts_expiry_of_the_old_single() {
    for boundary in [Boundary::Settings, Boundary::Run, Boundary::Single, Boundary::Clear] {
        twice(|| cross_boundary(boundary));
    }
}

fn cross_boundary(boundary: Boundary) -> Vec<Event> {
    let mut rig = Rig::new(true);
    rig.capture();
    let stale = rig.request();
    rig.io.clock.advance(LIMIT);
    match boundary {
        Boundary::Settings => rig.ui.m.trig_level = 0.01,
        Boundary::Run => {
            rig.ui.m.run = RunState::Stop;
            rig.ui.key(key::RUNSTOP);
        }
        Boundary::Single => rig.ui.key(key::SINGLE),
        Boundary::Clear => rig.ui.key(key::CLEAR),
    }
    change::settle(&mut rig.ui);
    let current = rig.request();
    assert_ne!(stale.pair(), current.pair());
    if matches!(boundary, Boundary::Settings) {
        assert_eq!(current.identity.epoch(), stale.identity.epoch());
    } else {
        assert_eq!(current.identity.epoch(), stale.identity.epoch() + 1);
    }
    assert!(lock(rig.out()).queue.is_empty());
    let before = lock(rig.out()).record_flow.command_generations();
    let start = rig.trace.borrow().events.len();
    assert!(rig.poll(&stale).is_none());
    assert!(!rig.trace.borrow().events[start..].iter().any(is_arm));
    assert_eq!(lock(rig.out()).record_flow.command_generations(), before);
    assert!(!rig.request().run.hold);
    assert!(!rig.state().contains(MISSED));
    rig.capture();
    let g = lock(rig.out());
    assert!(!g.queue.is_empty(), "{boundary:?}");
    assert!(g.queue.iter().all(|f| (f.cfg, f.stamp) == current.pair()));
    rig.trace.take().events
}

#[test]
fn accepted_run_with_equal_settings_clears_unconfirmed_stop() {
    twice(|| {
        let mut rig = Rig::new(false);
        rig.io.clock.stop_next_fill(0);
        rig.io.clock.suppress_stopped_status();
        rig.io.clock.stop_triggers();
        rig.capture();
        rig.ui.key(key::RUNSTOP);
        rig.ui.key(key::CLEAR);
        change::settle(&mut rig.ui);
        assert!(rig.state().contains(STOP_UNCONFIRMED));
        let before = rig.request();
        assert!(before.run.hold);
        rig.ui.key(key::RUNSTOP);
        change::settle(&mut rig.ui);
        let run = rig.request();
        assert_ne!(run.pair(), before.pair());
        assert_eq!(run.want, before.want);
        let start = rig.trace.borrow().events.len();
        assert!(rig.poll(&run).is_some());
        assert_eq!(trigger_plans(&rig.trace.borrow().events[start..]), 0);
        assert!(!rig.state().contains(" acq.status="));
        rig.trace.take().events
    });
}

#[test]
fn accepted_single_clears_missed_handoff_and_advances_epoch_on_each_press() {
    twice(|| {
        let mut rig = Rig::new(true);
        rig.capture();
        rig.io.clock.advance(LIMIT + EPSILON);
        assert!(rig.poll(&rig.request()).is_none());
        assert!(rig.state().contains(MISSED));
        rig.ui.m.trig_level = 0.01;
        rig.ui.key(key::CLEAR);
        change::settle(&mut rig.ui);
        assert!(rig.state().contains(MISSED));
        for _ in 0..2 {
            let before = rig.request();
            rig.ui.key(key::SINGLE);
            change::settle(&mut rig.ui);
            let next = rig.request();
            assert_eq!(next.identity.epoch(), before.identity.epoch() + 1);
            assert_eq!(next.want, before.want);
            assert!(!rig.state().contains(" acq.status="));
        }
        let epoch = rig.request().identity.epoch();
        rig.ui.key(key::SINGLE);
        rig.ui.key(key::SINGLE);
        assert_eq!(rig.request().identity.epoch(), epoch);
        change::settle(&mut rig.ui);
        assert_eq!(rig.request().identity.epoch(), epoch + 1);
        rig.trace.take().events
    });
}
