// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_tools::scpi::fake::{Events, FakeScpi};
use std::{
    fs::{self, File},
    io::Read,
    process::ExitCode,
};

struct FakeRemote<'a> {
    events: &'a Events,
    fail_on: Option<&'static str>,
    scripts: Vec<String>,
}

impl Remote for FakeRemote<'_> {
    fn run(&mut self, operation: &str, script: &str, _seconds: u64, input: Option<File>) -> Result<String, String> {
        self.events.borrow_mut().push(operation.into());
        self.scripts.push(script.into());
        if operation.starts_with("upload ") {
            let mut bytes = [0; 1];
            input.unwrap().read_exact(&mut bytes).unwrap();
        } else {
            assert!(input.is_none());
        }
        if self.fail_on == Some(operation) {
            return Err(format!("{operation}: timed out"));
        }
        match operation {
            "read CPU identification" => Ok("0x00000000410fd030\n".into()),
            "list benchmark test" => Ok(format!("{BENCHMARK}: test\n")),
            "check benchmark cache" | "check fixture cache" | "upload benchmark binary" | "upload fixture group" => {
                Ok(String::new())
            }
            name if name.starts_with("group '") => Ok("round samples".into()),
            _ => panic!("unexpected SSH operation"),
        }
    }
}

const UPLOADS: &[&str] = &[
    "read CPU identification",
    "check benchmark cache",
    "upload benchmark binary",
    "list benchmark test",
    "check fixture cache",
    "upload fixture group",
    "check fixture cache",
    "upload fixture group",
];
const ROUNDS: &[&str] =
    &["group 'dense', round 1", "group 'dense', round 2", "group 'short', round 1", "group 'short', round 2"];

fn exercise(
    acquiring: bool,
    allow_stop: bool,
    fail_on: Option<&'static str>,
) -> (Result<Capture, String>, Vec<String>) {
    let (result, events) = exercise_session(acquiring, allow_stop, fail_on, false);
    (result.map(|sides| sides.result), events)
}

fn exercise_session(
    acquiring: bool,
    allow_stop: bool,
    fail_on: Option<&'static str>,
    ab: bool,
) -> (Result<Sides<Capture>, String>, Vec<String>) {
    let scratch = Scratch::for_test();
    let build = |side: &str| {
        let binary = scratch.path().join(side);
        fs::write(&binary, side).unwrap();
        Build { binary, commit: side.into() }
    };
    let builds = Sides { reference: ab.then(|| build("reference")), result: build("result") };
    let options = Run {
        unit: "example.invalid",
        fixtures: scratch.path().join("fixtures"),
        out: scratch.path().join("report.json"),
        groups: vec![],
        rounds: if ab { 5 } else { 2 },
        cpu: 2,
        stop: allow_stop,
        average: false,
    };
    for name in ["short", "dense"] {
        let group = options.fixtures.join(name);
        fs::create_dir_all(&group).unwrap();
        fs::write(group.join("sample.bin"), b"fixture").unwrap();
    }
    let groups = cache::groups(&options.fixtures, &options.groups).unwrap();
    let events = Events::default();
    let mut remote = FakeRemote { events: &events, fail_on, scripts: Vec::new() };
    let mut scpi = FakeScpi {
        events: &events,
        running: acquiring,
        fail_stop: false,
        ignore_stop: false,
        fail_run: 0,
        replies: Default::default(),
    };
    let result = capture(&mut remote, &mut scpi, &scratch, &options, builds, &groups).map(|(_, captures)| captures);
    assert_eq!(scpi.running, acquiring);
    assert_round_scripts(&remote, &events.borrow(), result.as_ref().ok());
    (result, events.into_inner())
}

fn assert_round_scripts(remote: &FakeRemote<'_>, events: &[String], captures: Option<&Sides<Capture>>) {
    let ssh_events = events.iter().filter(|event| ![":STOP", ":RUN", "query state"].contains(&event.as_str())).count();
    assert_eq!(remote.scripts.len(), ssh_events);
    let round_scripts: Vec<_> = remote.scripts.iter().filter(|script| script.starts_with("exec timeout")).collect();
    let operations: Vec<_> = events.iter().filter(|event| event.starts_with("group '")).collect();
    assert_eq!(round_scripts.len(), operations.len());
    for (operation, script) in operations.into_iter().zip(round_scripts) {
        assert!(
            script.starts_with("exec timeout -s KILL 120 env -u OOS_BENCH_STAGE OOS_FIXTURES=/tmp/oos-bench/group-")
        );
        assert!(script.contains(" OOS_BENCH_CPU=2 OOS_BENCH_REPEATS=1 "));
        assert!(script.ends_with(&format!("{BENCHMARK} --ignored --nocapture --exact --test-threads=1")));
        assert!(!script.contains(":STOP") && !script.contains(":RUN"));
        if let Some(captures) = captures {
            let side =
                if operation.contains(", reference") { captures.reference.as_ref().unwrap() } else { &captures.result };
            assert!(script.contains(&side.runner), "{operation} used the wrong binary");
        }
    }
}

#[test]
fn refuses_acquiring_scope_before_upload_without_stop() {
    let (result, events) = exercise(true, false, None);
    assert_eq!(result.unwrap_err(), "scope is acquiring; stop it or use --stop");
    assert_eq!(events, ["query state"]);
}

#[test]
fn stops_before_uploading_and_restores_after_all_rounds() {
    let (result, events) = exercise(true, true, None);
    let capture = result.unwrap();
    assert_eq!(capture.groups["dense"].rounds, ["round samples", "round samples"]);
    assert_eq!(capture.groups["short"].rounds, ["round samples", "round samples"]);
    assert!(hex(&capture.groups["dense"].sha256, 64));
    assert_eq!(capture.groups["dense"].sha256, capture.groups["short"].sha256);
    assert_eq!(
        events,
        [&["query state", ":STOP", "query state"][..], UPLOADS, ROUNDS, &[":RUN", "query state"]].concat()
    );
}

#[test]
fn ab_uploads_both_builds_and_interleaves_each_group_under_one_stop() {
    let (result, events) = exercise_session(true, true, None, true);
    let captures = result.unwrap();
    let reference = captures.reference.unwrap();
    assert_ne!(reference.binary_sha256, captures.result.binary_sha256);
    assert_eq!(&events[..3], ["query state", ":STOP", "query state"]);
    assert_eq!(&events[events.len() - 2..], [":RUN", "query state"]);
    assert_eq!(&events[3..14], [&UPLOADS[..4], &UPLOADS[1..4], &UPLOADS[4..]].concat());
    let mut rounds = events[14..events.len() - 2].iter();
    for group in ["dense", "short"] {
        for (side, round) in [
            ("reference", 1),
            ("result", 1),
            ("result", 2),
            ("reference", 2),
            ("reference", 3),
            ("result", 3),
            ("result", 4),
            ("reference", 4),
            ("reference", 5),
            ("result", 5),
        ] {
            assert_eq!(rounds.next().unwrap(), &format!("group '{group}', {side} round {round}"));
        }
        assert_eq!(reference.groups[group].rounds.len(), 5);
        assert_eq!(captures.result.groups[group].rounds.len(), 5);
        assert_eq!(reference.groups[group].sha256, captures.result.groups[group].sha256);
    }
    assert!(rounds.next().is_none());
}

#[test]
fn ab_restores_after_an_interleaved_round_fails() {
    let operation = "group 'dense', result round 2";
    let (result, events) = exercise_session(true, true, Some(operation), true);
    assert_eq!(result.unwrap_err(), format!("{operation}: timed out"));
    assert_eq!(&events[events.len() - 3..], [operation, ":RUN", "query state"]);
}

#[test]
fn failed_stop_and_unconfirmed_stop_skip_work_and_attempt_restore() {
    for (fail_stop, ignore_stop, error) in
        [(true, false, "stop write failed"), (false, true, "scope did not stop acquiring")]
    {
        let events = Events::default();
        let mut scpi = FakeScpi {
            events: &events,
            running: true,
            fail_stop,
            ignore_stop,
            fail_run: 0,
            replies: Default::default(),
        };
        let result = with_scope_stopped(&mut scpi, true, || -> Result<(), String> { panic!("scope was not stopped") });
        assert_eq!(result.unwrap_err(), error);
        assert!(scpi.running);
        let mut expected = vec!["query state", ":STOP"];
        if ignore_stop {
            expected.push("query state");
        }
        expected.extend([":RUN", "query state"]);
        assert_eq!(events.into_inner(), expected);
    }
}

#[test]
fn failed_restore_reconnects_once_and_confirms_acquisition() {
    let events = Events::default();
    let mut scpi = FakeScpi {
        events: &events,
        running: true,
        fail_stop: false,
        ignore_stop: false,
        fail_run: 1,
        replies: Default::default(),
    };
    assert_eq!(with_scope_stopped(&mut scpi, true, || Ok(42)).unwrap(), 42);
    assert!(scpi.running);
    assert_eq!(
        events.into_inner(),
        ["query state", ":STOP", "query state", ":RUN", "reconnect", ":RUN", "query state"]
    );
}

#[test]
fn restore_failure_names_recovery_with_or_without_a_work_error() {
    for work in [Ok(()), Err("benchmark failed".into())] {
        let work_error = work.is_err();
        let events = Events::default();
        let mut scpi = FakeScpi {
            events: &events,
            running: true,
            fail_stop: false,
            ignore_stop: false,
            fail_run: 2,
            replies: Default::default(),
        };
        let error = with_scope_stopped(&mut scpi, true, || work).unwrap_err();
        assert_eq!(error.starts_with("benchmark failed; "), work_error);
        assert!(error.contains("could not confirm :RUN restoration: run write failed; retry: run write failed"));
        assert!(error.ends_with(RESTART));
        assert!(!scpi.running);
        assert_eq!(events.into_inner(), ["query state", ":STOP", "query state", ":RUN", "reconnect", ":RUN"]);
    }
}

#[test]
fn combines_stop_and_restore_errors() {
    let events = Events::default();
    let mut scpi = FakeScpi {
        events: &events,
        running: true,
        fail_stop: true,
        ignore_stop: false,
        fail_run: 2,
        replies: Default::default(),
    };
    let error =
        with_scope_stopped(&mut scpi, true, || -> Result<(), String> { panic!("scope was not stopped") }).unwrap_err();
    assert!(error.starts_with("stop write failed; could not confirm :RUN restoration:"));
    assert!(error.ends_with(RESTART));
    assert_eq!(events.into_inner(), ["query state", ":STOP", ":RUN", "reconnect", ":RUN"]);
}

#[test]
fn leaves_initially_stopped_scope_stopped() {
    let (result, events) = exercise(false, true, None);
    assert!(result.is_ok());
    assert_eq!(events, [&["query state"][..], UPLOADS, ROUNDS].concat());
}

#[test]
fn restores_after_fixture_upload_failure() {
    let (result, events) = exercise(true, true, Some("upload fixture group"));
    assert_eq!(result.unwrap_err(), "upload fixture group: timed out");
    assert_eq!(
        events,
        [&["query state", ":STOP", "query state"][..], &UPLOADS[..6], &[":RUN", "query state"]].concat()
    );
}

#[test]
fn restores_after_group_timeout_and_reports_only_operation() {
    let (result, events) = exercise(true, true, Some("group 'short', round 1"));
    assert_eq!(result.unwrap_err(), "group 'short', round 1: timed out");
    assert_eq!(
        events,
        [&["query state", ":STOP", "query state"][..], UPLOADS, &ROUNDS[..3], &[":RUN", "query state"]].concat()
    );
}

#[test]
fn validates_output_before_build_and_preserves_an_existing_report_on_error() {
    let scratch = Scratch::for_test();
    let report = scratch.path().join("existing.json");
    fs::write(&report, "previous report").unwrap();
    let missing_checkout = scratch.path().join("missing");
    let mut options = Run {
        unit: "example.invalid",
        fixtures: scratch.path().join("fixtures"),
        out: scratch.path().to_owned(),
        groups: vec![],
        rounds: 1,
        cpu: 2,
        stop: false,
        average: false,
    };
    assert!(run(&missing_checkout, &options).unwrap_err().starts_with("open --out:"));
    options.out = scratch.path().join("missing.json/");
    assert!(run(&missing_checkout, &options).unwrap_err().starts_with("open --out:"));
    options.out.clone_from(&report);
    assert_eq!(run(&missing_checkout, &options).unwrap_err(), "read rust-toolchain.toml");
    assert_eq!(fs::read_to_string(report).unwrap(), "previous report");
    assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 1);
}

#[test]
fn compare_command_uses_distinct_exit_codes_for_regression_and_insufficient_data() {
    use crate::unit_bench::report::Timing;

    let scratch = Scratch::for_test();
    let reference = scratch.path().join("reference.json");
    let candidate = scratch.path().join("candidate.json");
    let write = |path, trigger| {
        let stages = BTreeMap::from([
            ("place".into(), Some(Timing::new(100.0, 100.0, 100.0, 5, 10))),
            ("trigger".into(), trigger),
        ]);
        let groups = BTreeMap::from([("group".into(), Group::new("a".repeat(64), stages))]);
        let report = Report::new("offline".into(), "offline".into(), 2, "offline".into(), groups);
        PendingReport::open(path).unwrap().write(&report).unwrap();
    };
    let exit = || {
        crate::unit_bench::run(
            scratch.path(),
            &["compare", "--reference", reference.to_str().unwrap(), "--result", candidate.to_str().unwrap()],
        )
    };
    write(&reference, Some(Timing::new(10.0, 10.0, 10.0, 5, 10)));
    for (trigger, code) in [
        (Some(Timing::new(11.0, 11.0, 11.0, 5, 10)), 0),
        (Some(Timing::new(15.0, 15.0, 15.0, 5, 10)), 1),
        (Some(Timing::new(15.0, 15.0, 15.0, 3, 30)), 2),
        (Some(Timing::new(15.0, 15.0, 15.0, 5, 5)), 2),
        (None, 2),
    ] {
        write(&candidate, trigger);
        assert_eq!(exit(), ExitCode::from(code));
    }
}
