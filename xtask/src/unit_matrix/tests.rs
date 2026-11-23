// SPDX-License-Identifier: GPL-2.0-or-later

mod restore;
mod timing;

use super::*;
use crate::unit_tools::scpi::{
    fake::{Events, FakeScpi},
    STATE,
};
use std::{cell::Cell, collections::VecDeque, rc::Rc, time::Duration};

const NO_ERROR: &str = "0,\"No error\"";
const ERROR: &str = ":SYSTem:ERRor?";
const STOPPED: &str = "fps 30 frames 1 records 1 run Stop watchdog ok";
const RUNNING: &str = "fps 30 frames 1 records 1 run Run watchdog ok";
const BEFORE: &str = "fps 30 frames 1 records 1 run Run watchdog ok fresh=1 first_records=1 \
    records read=2 deposited=2 shown=1 rejected.stale_key=0 records.recovered=0 present.v2 flip.shown=30 flip.late=1 fallback.v1 decoded_lines=2 decoded_dots=5";
const AFTER: &str = "fps 30 frames 2 records 7 run Run watchdog ok fresh=31 first_records=101 \
    records read=132 deposited=112 shown=111 rejected.stale_key=0 records.recovered=1 present.v2 flip.shown=180 flip.late=3 fallback.v1 decoded_lines=5 decoded_dots=5";

type Replies = VecDeque<(String, Result<String, String>)>;

#[derive(Default)]
struct Script(Replies);

impl Script {
    fn reply(&mut self, command: impl Into<String>, value: &str) {
        self.0.push_back((command.into(), Ok(value.into())));
    }

    fn get(&mut self, key: &str, value: &str) {
        self.reply(format!(":SYSTem:GET? {key}"), value);
    }

    fn transport_failure(&mut self, command: impl Into<String>) {
        self.0.push_back((command.into(), Err("read timed out".into())));
    }

    fn set_reply(&mut self, key: &str, wire: &str, reply: &str, error: &str) {
        self.write_setup(key, wire, error);
        self.get(key, reply);
    }

    fn write_setup(&mut self, key: &str, wire: &str, error: &str) {
        self.reply(format!(":SYSTem:SET {key} = {wire};*OPC?"), "1");
        self.reply(ERROR, error);
    }

    fn run_state(&mut self, command: &str) {
        self.reply(format!("{command};*OPC?"), "1");
        self.reply(ERROR, NO_ERROR);
    }

    fn readback(&mut self) {
        for (command, reply) in
            [(":TIM:MAIN:SCAL?", "1.000000E-06"), (":ACQ:MDEP?", "7032"), (":ACQ:SRAT?", "1E9"), (":ACQ:TYPE?", "NORM")]
        {
            self.reply(command, reply);
        }
    }

    fn scpi(self, events: &Events) -> FakeScpi<'_> {
        FakeScpi { events, running: false, fail_stop: false, ignore_stop: false, fail_run: 0, replies: self.0 }
    }
}

#[derive(Default)]
struct FakeClock {
    now: Rc<Cell<Duration>>,
    waits: Vec<Duration>,
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.now.get()
    }

    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
        self.now.set(self.now.get() + duration);
    }
}

fn timing() -> Timing {
    Timing { warmup: Duration::from_secs(2), window: Duration::from_secs(5), windows: None, sustained: false }
}

fn small_plan() -> Plan {
    serde_json::from_str(
        r#"{
        "base_setup":{"tdiv":"1"},
        "bench_setup":{},
        "snapshot_only":[],
        "groups":[{"id":"one","windows":1,"overrides":{"tdiv":"2"}},
                  {"id":"two","windows":1,"overrides":{"tdiv":"3"}},
                  {"id":"three","windows":3,"overrides":{"tdiv":"4"}}]
    }"#,
    )
    .unwrap()
}

#[test]
fn clamped_group_errors_and_reports_hide_setup_values_and_bad_watchdogs_abort() {
    const SAVED: &str = "9.87654321";
    const CLAMPED: &str = "3.14159265";
    for unhealthy in [false, true] {
        let plan = small_plan();
        let events = Events::default();
        let mut script = Script::default();
        script.reply("*IDN?", "OOS,Model,0,oos-ui 0.2.0 build test");
        script.get("tdiv", SAVED);
        script.reply(STATE, STOPPED);
        script.reply(STATE, STOPPED);
        script.get("tdiv", SAVED);
        script.set_reply("tdiv", "2", CLAMPED, NO_ERROR);
        script.reply(STATE, STOPPED);
        script.get("tdiv", "3"); // No write when the second group's setup already matches.
        script.run_state(":RUN");
        script.readback();
        script.reply(STATE, BEFORE);
        if unhealthy {
            script.reply(STATE, &AFTER.replace("watchdog ok", "watchdog fault"));
        } else {
            script.reply(STATE, AFTER);
        }
        script.get("tdiv", "3");
        script.write_setup("tdiv", SAVED, NO_ERROR);
        script.get("tdiv", SAVED);
        script.run_state(":STOP");
        script.get("tdiv", SAVED);
        script.reply(STATE, STOPPED);
        let mut scpi = script.scpi(&events);
        let mut clock = FakeClock::default();
        let selected = if unhealthy { &[][..] } else { &["one", "two"][..] };
        let report =
            capture(&mut scpi, &mut clock, &plan, &plan.select(selected).unwrap(), timing(), "bench".into()).unwrap();
        assert_eq!(report.groups.len(), 2);
        assert_eq!(report.build, "oos-ui 0.2.0 build test");
        assert_eq!(report.groups["one"].error.as_deref(), Some("set 'tdiv': read-back mismatch"));
        assert_eq!(report.groups["two"].windows.len(), usize::from(!unhealthy));
        assert_eq!(report.error.is_some(), unhealthy);
        let encoded = serde_json::to_string(&report).unwrap();
        assert!(!encoded.contains(SAVED));
        assert!(!encoded.contains(CLAMPED));
        let roundtrip: Report = serde_json::from_str(&encoded).unwrap();
        assert_eq!(roundtrip.groups["two"].summary.is_some(), !unhealthy);
        assert!(scpi.replies.is_empty());
    }
}

#[test]
fn unhealthy_before_the_first_change_needs_no_restore_and_touches_no_controls() {
    let plan = small_plan();
    let events = Events::default();
    let mut script = Script::default();
    script.reply("*IDN?", "OOS,Model,0,oos-ui 0.2.0");
    script.get("tdiv", "1");
    script.reply(STATE, "fps 30 frames 1 records 1 run Stop watchdog fault");
    let mut scpi = script.scpi(&events);
    let error =
        capture(&mut scpi, &mut FakeClock::default(), &plan, &plan.select(&[]).unwrap(), timing(), "bench".into())
            .unwrap_err();
    assert!(error.contains("unhealthy before setup"));
    assert!(scpi.replies.is_empty());
    assert_eq!(events.borrow().len(), 3);
}

#[test]
fn sustained_keeps_completed_minutes_when_the_next_window_fails() {
    for reset in [false, true] {
        let report = sustained_failure(reset);
        let group = &report.groups["one"];
        assert_eq!(group.windows.len(), if reset { 2 } else { 1 });
        assert_eq!(group.windows[0].start_s, 10.0);
        assert_eq!(group.summary.as_ref().unwrap().host_s, [60.0, 60.0]);
        assert_eq!(group.error.as_deref(), Some(if reset { "counter reset" } else { "read timed out" }));
        assert!(report.error.is_none());
    }
}

fn sustained_failure(reset: bool) -> Report {
    let plan = small_plan();
    let events = Events::default();
    let mut script = Script::default();
    script.reply("*IDN?", "OOS,Model,0,oos-ui 0.2.0");
    script.get("tdiv", "1");
    script.reply(STATE, RUNNING);
    script.reply(STATE, RUNNING);
    script.get("tdiv", "1");
    script.set_reply("tdiv", "2", "2", NO_ERROR);
    script.run_state(":RUN");
    script.readback();
    script.reply(STATE, BEFORE);
    script.reply(STATE, AFTER);
    if reset {
        script.reply(STATE, AFTER);
        script.reply(STATE, BEFORE);
    } else {
        script.transport_failure(STATE);
    }
    script.get("tdiv", "2");
    script.write_setup("tdiv", "1", NO_ERROR);
    script.get("tdiv", "1");
    script.run_state(":RUN");
    script.get("tdiv", "1");
    script.reply(STATE, RUNNING);

    let mut scpi = script.scpi(&events);
    let mut clock = FakeClock::default();
    let timing =
        Timing { warmup: Duration::from_secs(10), window: Duration::from_secs(60), windows: Some(2), sustained: true };
    let report =
        capture(&mut scpi, &mut clock, &plan, &plan.select(&["one"]).unwrap(), timing, "bench".into()).unwrap();
    assert_eq!(&clock.waits[..2], [Duration::from_secs(10), Duration::from_secs(60)]);
    assert!(scpi.replies.is_empty());
    report
}

#[test]
fn build_rejects_non_oos_and_missing_build_without_echoing_the_identity() {
    for identity in ["Other,Model,private-token,oos-ui 0.2.0", "OOS,Model,private-token", "OOS,Model,0,other-ui 0.2.0"]
    {
        assert_eq!(build(identity).unwrap_err(), "*IDN? did not identify an OOS build");
    }
}
