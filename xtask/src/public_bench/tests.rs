// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_tools::scpi::TimedReply;
use serde_json::Value;
use std::{cell::Cell, collections::BTreeMap};

const NO_ERROR: &str = "0,\"No error\"";
const FORK: &str = "fps 30.0 frames 100 records 30 run Run status Triggered t 35.0 step 2.4 ms (acq 0.7 draw 1.1) render 1.2 ms present 0.2 ms edges 3 2 1 0 off 0 0 route=batch interp Exact acquired 90 used 80 dropped 7 sd.mode=hs hw frames 123 dropped 13";
const UPSTREAM: &str = "fps 29.0 frames 120 records 80 run Run status Triggered t 36.0 step 4.0 ms (acq 1.0 draw 2.0) render 1.3 ms present 0.0 ms edges 0 0 0 0 off 0 0 acquired 40 used 38 dropped 1 graphics gpu hw frames 800 dropped 13";

#[derive(Debug, Default)]
struct FakeClock(Cell<Duration>);

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.0.get()
    }

    fn wait(&self, duration: Duration) {
        self.0.set(self.now() + duration);
    }
}

#[derive(Debug)]
struct FakeScope<'a> {
    clock: &'a FakeClock,
    settings: BTreeMap<String, String>,
    measurements: Vec<String>,
    sends: Vec<String>,
    states: usize,
    probes: usize,
    mismatch: bool,
    error: Option<i32>,
    card: &'static str,
}

impl<'a> FakeScope<'a> {
    fn new(clock: &'a FakeClock) -> Self {
        Self {
            clock,
            settings: BTreeMap::new(),
            measurements: Vec::new(),
            sends: Vec::new(),
            states: 0,
            probes: 0,
            mismatch: false,
            error: None,
            card: "sd.mode=hs",
        }
    }

    fn readback(&mut self, query: &str) -> String {
        let value = match query {
            ":ACQ:MDEP?" => "1.000000E+04".into(),
            ":ACQ:TYPE?" if self.mismatch => "NORM".into(),
            ":SYST:GET? mdepth" => "0".into(),
            ":MEAS:STRIP?" => self.measurements.join(";"),
            ":TRIG:EDGE:SOUR?" if self.mismatch => "CHAN4".into(),
            _ => self.settings[query.trim_end_matches('?')].clone(),
        };
        format!("{value};{NO_ERROR}")
    }

    fn state(&mut self) -> String {
        self.states += 1;
        let seconds = self.clock.now().as_secs_f64();
        format!(
            "fps 30 frames {} records 0 run Run step 2.4 ms render 1.2 ms present 0.2 ms edges 0 acquired {} used {} dropped {} {}",
            (seconds * 20.0).round() as u64,
            (seconds * 100.0).round() as u64,
            (seconds * 60.0).round() as u64,
            (seconds * 5.0).round() as u64,
            self.card,
        )
    }
}

impl Scpi for FakeScope<'_> {
    fn send(&mut self, command: &str) -> Result<(), String> {
        self.sends.push(command.into());
        if matches!(command, "*CLS" | ":RUN") {
            return Ok(());
        }
        let (key, value) = command.split_once(' ').unwrap();
        match key {
            ":ACQ:TYPE" if self.mismatch && value == "HRES" => self.error = Some(-224),
            ":MEAS:CLE" => self.measurements.clear(),
            ":MEAS:ITEM" => self.measurements.push(format!("{value},9.9E37")),
            _ => {
                self.settings.insert(key.into(), value.into());
            }
        }
        Ok(())
    }

    fn query_timed<T>(&mut self, command: &str, mut now: impl FnMut() -> T) -> Result<TimedReply<T>, String> {
        let before_write = now();
        let (line, cost) = match command {
            "*IDN?" => ("OOS,DHO924S,0,oos-ui 0.2.1".into(), 0),
            OPC => {
                self.probes += 1;
                ("1".into(), if self.probes.is_multiple_of(2) { 60 } else { 20 })
            }
            STATE => {
                // Generate the counter values at the midpoint of the simulated query.
                self.clock.wait(Duration::from_millis(50));
                (self.state(), 50)
            }
            ":SYST:ERR?" => (self.error.take().map_or_else(|| NO_ERROR.into(), |code| format!("{code},\"error\"")), 0),
            ":ACQ:MDEP:AUTO?;:SYST:ERR?" if self.mismatch => ("-113,\"Undefined header\"".into(), 0),
            program => (self.readback(program.strip_suffix(";:SYST:ERR?").unwrap()), 0),
        };
        self.clock.wait(Duration::from_millis(cost));
        Ok(TimedReply { line, before_write, after_line: now() })
    }

    fn reconnect(&mut self) -> Result<(), String> {
        panic!("the benchmark uses one connection");
    }
}

fn options() -> BenchmarkOptions<'static> {
    BenchmarkOptions::parse(&[
        "--host",
        "example.invalid",
        "--label",
        "base",
        "--signal-channel",
        "2",
        "--out",
        "results",
    ])
    .unwrap()
}

#[test]
fn validates_cli_and_defaults_to_three_runs() {
    assert_eq!(options().runs, 3);
    assert!(BenchmarkOptions::parse(&["--label", "x", "--out", "results", "--signal-channel", "2"]).is_err());
    assert!(BenchmarkOptions::parse(&["--host", "scope", "--label", "scope"]).is_err());
    assert!(BenchmarkOptions::parse(&["--host", "example.invalid", "--label", "x", "--signal-channel", "5"]).is_err());
    assert!(BenchmarkOptions::parse(&[
        "--host",
        "example.invalid",
        "--label",
        "x",
        "--signal-channel",
        "1",
        "--runs",
        "0"
    ])
    .is_err());
}

#[test]
fn fork_state_uses_ui_and_pipeline_counters() {
    let before = State::parse(FORK).unwrap();
    let after = State::parse(
        &FORK
            .replace("frames 100", "frames 140")
            .replace("acquired 90 used 80 dropped 7", "acquired 110 used 92 dropped 9"),
    )
    .unwrap();
    let values = after.values_since(&before, Duration::from_secs(2));
    assert_eq!(values[&Metric::PicturesPerS], Some(20.0));
    assert_eq!(values[&Metric::AcquiredPerS], Some(10.0));
    assert_eq!(values[&Metric::ShownPerS], Some(6.0));
    assert_eq!(values[&Metric::DroppedPerS], Some(1.0));
    assert_eq!(values[&Metric::StepMs], Some(2.4));
    assert_eq!(values[&Metric::RenderMs], Some(1.2));
    assert_eq!(values[&Metric::PresentMs], Some(0.2));
    assert_eq!(before.card_mode.as_deref(), Some("hs"));
    assert_eq!(before.values_since(&after, Duration::from_secs(2))[&Metric::PicturesPerS], None);
}

#[test]
fn upstream_state_leaves_absent_fields_unreported() {
    let before = State::parse(UPSTREAM).unwrap();
    let after = State::parse(&UPSTREAM.replace("frames 120", "frames 130")).unwrap();
    let values = after.values_since(&before, Duration::from_secs(2));
    assert_eq!(values[&Metric::ReportedFps], Some(29.0));
    assert_eq!(values[&Metric::PicturesPerS], Some(5.0));
    assert_eq!(values[&Metric::AcquiredPerS], Some(0.0));
    assert_eq!(values[&Metric::PresentMs], Some(0.0));
    assert_eq!(before.card_mode, None);

    let partial = UPSTREAM
        .trim_start_matches("fps 29.0 ")
        .replace(" present 0.0 ms", "")
        .replace(" acquired 40 used 38 dropped 1", "");
    let partial = State::parse(&partial).unwrap();
    let values = partial.values_since(&before, Duration::from_secs(2));
    for metric in [Metric::ReportedFps, Metric::AcquiredPerS, Metric::ShownPerS, Metric::DroppedPerS, Metric::PresentMs]
    {
        assert_eq!(values[&metric], None);
    }
}

#[test]
fn verifies_setting_rejections_mismatches_and_unavailable_queries() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.mismatch = true;
    let settings = settings::apply(&mut scope, &VIEWS[13], 2).unwrap();
    let json = serde_json::to_value(settings).unwrap();
    assert_eq!(json[":TRIG:EDGE:SOUR"]["requested"], "CHAN2");
    assert_eq!(json[":TRIG:EDGE:SOUR"]["observed"], "CHAN4");
    assert_eq!(json[":TRIG:EDGE:SOUR"]["status"], "mismatch");
    assert_eq!(json[":ACQ:TYPE"]["status"], "rejected");
    assert_eq!(json[":ACQ:TYPE"]["setter_errors"][0], -224);
    assert_eq!(json[":ACQ:MDEP:AUTO"]["status"], "unavailable");
    assert_eq!(json[":ACQ:MDEP:AUTO"]["query_error"], -113);
    assert_eq!(json[":MEAS:CLE"]["status"], "verified");
}

#[test]
fn window_uses_snapshot_midpoints_and_times_complete_opc_round_trips() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    let (elapsed, values) = measure_window(&mut scope, &clock).unwrap();
    assert_eq!(elapsed, WINDOW + Duration::from_millis(50));
    assert_eq!(scope.states, 2);
    assert_eq!(scope.probes, 28);
    assert_eq!(values[&Metric::PicturesPerS], Some(20.0));
    assert_eq!(values[&Metric::AcquiredPerS], Some(100.0));
    assert_eq!(values[&Metric::ShownPerS], Some(60.0));
    assert_eq!(values[&Metric::OpcHostMedianMs], Some(40.0));
    assert_eq!(values[&Metric::OpcHostMaxMs], Some(60.0));
}

#[test]
fn collects_all_views_for_every_run_and_saves_only_the_version() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    let report = collect(&mut scope, &clock, &options(), Duration::from_secs(600)).unwrap();
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["idn_version"], "oos-ui 0.2.1");
    assert_eq!(json["started_unix_s"], 600.0);
    assert!(!json.to_string().contains("DHO924S"));
    assert!(!json.to_string().contains("example.invalid"));
    let views = json["views"].as_array().unwrap();
    assert_eq!(views.len(), 18);
    assert!(views.iter().all(|view| view["samples"].as_array().unwrap().len() == 3));
    assert_eq!(scope.sends.iter().filter(|command| *command == ":TRIG:EDGE:SOUR CHAN2").count(), 54);
    assert!(clock.now() >= (SETTLE + WINDOW) * 54);
    let first = &views[0]["samples"][0]["settings"];
    assert_eq!(first[":CHAN1:DISP"]["observed"], "OFF");
    assert_eq!(first[":CHAN2:DISP"]["observed"], "ON");
    let two = &views[7]["samples"][1]["settings"];
    assert_eq!(two[":CHAN1:DISP"]["observed"], "ON");
    assert_eq!(two[":CHAN3:DISP"]["observed"], "OFF");
    assert_eq!(views[15]["samples"][0]["settings"][":TIM:DEL:ENAB"]["observed"], "ON");
    let measurements = &views[16]["samples"][0]["settings"][":MEAS:ITEM"];
    assert_eq!(measurements["observed"], "FREQUENCY,CHAN2;RTIME,CHAN2;VPP,CHAN2");
    assert_eq!(measurements["status"], "verified");
    let last = &views[17]["samples"][0]["settings"];
    assert_eq!(last[":MEAS:CLE"]["observed"], "");
    assert_eq!(last[":TIM:DEL:ENAB"]["observed"], "OFF");
}

#[test]
fn compare_prints_spreads_ratios_and_warnings() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    let mut options = options();
    options.runs = 1;
    let baseline = collect(&mut scope, &clock, &options, Duration::ZERO).unwrap();
    scope.card = "";
    scope.mismatch = true;
    let candidate = collect(&mut scope, &clock, &options, Duration::ZERO).unwrap();
    let mut json = serde_json::to_value(candidate).unwrap();
    json["views"][0]["summary"]["pictures_per_s"]["median"] = Value::from(10.0);
    json["views"][0]["summary"]["pictures_per_s"]["min"] = Value::from(8.0);
    json["views"][0]["summary"]["pictures_per_s"]["max"] = Value::from(12.0);
    let candidate = serde_json::from_value(json).unwrap();
    let mut output = Vec::new();
    report::compare(&baseline, &candidate, &mut output).unwrap();
    let table = String::from_utf8(output).unwrap();
    assert!(table.contains("warning: boot card mode differs (A: hs, B: not reported)"));
    assert!(table.contains("warning: recorded settings differ for 1ch 5ns"));
    assert!(table.lines().any(|line| {
        line.starts_with("1ch 5ns") && line.contains("10.000 [8.000, 12.000]") && line.ends_with("0.500")
    }));
}
