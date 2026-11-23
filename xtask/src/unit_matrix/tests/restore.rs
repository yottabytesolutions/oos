// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_tools::with_restore;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn restores_twice_offsets_last_and_verifies_every_key_after_run() {
    for failure in ["none", "work", "restore once", "restore twice", "unwind"] {
        let keys = ["ch1.offset", "ch1.probe", "tdiv", "zdelay"].map(String::from);
        let events = Events::default();
        let mut script = Script::default();
        for (key, value) in keys.iter().zip(["-20", "10", "10", "5"]) {
            script.get(key, value);
        }
        script.reply(STATE, RUNNING);
        script.get("ch1.probe", "1");
        script.write_setup("ch1.probe", "10", NO_ERROR);
        script.get("tdiv", "1");
        script.write_setup("tdiv", "10", NO_ERROR);
        script.get("zdelay", "0"); // Timebase change moved the zoom delay.
        let failed = failure.starts_with("restore");
        let queue = if failed { "-224,\"Illegal parameter value\"" } else { NO_ERROR };
        script.write_setup("zdelay", "5", queue);
        script.get("ch1.offset", "-2"); // Probe scale change takes effect before the offset.
        script.write_setup("ch1.offset", "-20", NO_ERROR);
        script.get("ch1.probe", "10");
        script.get("tdiv", "10");
        script.get("zdelay", if failed { "0" } else { "4" }); // A later read sees a derived change.
        let persistent = failure == "restore twice";
        script.write_setup("zdelay", "5", if persistent { queue } else { NO_ERROR });
        script.get("ch1.offset", "-20");
        script.run_state(":RUN");
        for (key, value) in keys.iter().zip(["-20", "10", "10", if persistent { "0" } else { "5" }]) {
            script.get(key, value);
        }
        script.reply(STATE, RUNNING);
        let mut scpi = script.scpi(&events);
        let snapshot = Snapshot::read(&mut scpi, keys.iter()).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_restore(
                &mut scpi,
                |scpi| snapshot.restore(scpi),
                |_| match failure {
                    "work" | "restore twice" => Err("group failure".into()),
                    "unwind" => panic!("work panicked"),
                    _ => Ok(()),
                },
            )
        }));
        match failure {
            "unwind" => assert!(result.is_err()),
            "work" => assert_eq!(result.unwrap().unwrap_err(), "group failure"),
            "restore twice" => {
                let error = result.unwrap().unwrap_err();
                assert!(error.starts_with("group failure; restore failed"));
                assert!(error.contains("mismatched or unverified keys: zdelay"));
                assert!(error.contains("select RUN"));
            }
            _ => assert!(result.unwrap().is_ok()),
        }
        assert!(scpi.replies.is_empty(), "{failure}");
    }
}

#[test]
fn unsafe_scpi_values_and_unrepresentable_decoder_layouts_fail_before_changes() {
    for (key, original, target) in [("ch1.label", "label;:RUN", ""), ("dec_sel", "1", "0"), ("d2.dec_on", "false", "")]
    {
        let events = Events::default();
        let mut script = Script::default();
        script.get(key, original);
        script.reply(STATE, STOPPED);
        let mut scpi = script.scpi(&events);
        let base: Setup = [(key.into(), target.into())].into();
        let snapshot = Snapshot::read(&mut scpi, base.keys()).unwrap();
        assert!(snapshot.check_replayable(&base).unwrap_err().contains(key));
        assert!(scpi.replies.is_empty());
        assert_eq!(events.borrow().len(), 2);
    }
}

#[test]
fn sends_the_codec_values_for_omitted_defaults_and_reads_back_empty() {
    for (key, current, wire) in [
        ("ch1.vfine", "1.125", "1"),
        ("ch1.bias", "0.005", "0"),
        ("ch1.unit", "A", "V"),
        ("cur_b_source", "2", "none"),
        ("meas", "Vpp:2", ""),
    ] {
        let events = Events::default();
        let mut script = Script::default();
        script.get(key, current);
        script.reply(STATE, STOPPED);
        script.get(key, current);
        script.set_reply(key, wire, "", NO_ERROR);
        let mut scpi = script.scpi(&events);
        let target: Setup = [(key.into(), "".into())].into();
        let snapshot = Snapshot::read(&mut scpi, target.keys()).unwrap();
        snapshot.apply(&mut scpi, &target).unwrap();
        assert!(scpi.replies.is_empty());
    }
}

#[test]
fn address_like_snapshot_values_are_never_applied_or_restored() {
    for value in [
        "192.0.2.1",
        "example.invalid:5555",
        "https://example.invalid/path",
        "[2001:db8::1]:5555",
        "00:00:5e:00:53:00",
        "scopehost",
        "bench-host",
    ] {
        let key = "ch1.label";
        let target: Setup = [(key.into(), "".into())].into();
        let events = Events::default();
        let mut script = Script::default();
        script.get(key, value);
        script.reply(STATE, STOPPED);
        script.run_state(":STOP");
        script.reply(STATE, STOPPED);
        let mut scpi = script.scpi(&events);
        let snapshot = Snapshot::read(&mut scpi, target.keys()).unwrap();
        assert_eq!(snapshot.skipped_keys, [key]);
        snapshot.apply(&mut scpi, &target).unwrap();
        snapshot.restore(&mut scpi).unwrap();
        assert!(scpi.replies.is_empty());
        assert_eq!(events.borrow().iter().filter(|event| event.contains(key)).count(), 1);
    }
}

#[test]
fn three_consecutive_transport_failures_stop_restore_and_report_verified_keys() {
    let keys = ["a", "b", "c", "d", "e"].map(String::from);
    let events = Events::default();
    let mut script = Script::default();
    for key in &keys {
        script.get(key, "1");
    }
    script.reply(STATE, STOPPED);
    for _ in 0..2 {
        script.transport_failure(":SYSTem:GET? a"); // The following success resets the failure streak.
        for key in &keys[1..] {
            script.get(key, "1");
        }
    }
    script.run_state(":STOP");
    script.get("a", "1");
    for key in &keys[1..4] {
        script.transport_failure(format!(":SYSTem:GET? {key}"));
    }
    let mut scpi = script.scpi(&events);
    let snapshot = Snapshot::read(&mut scpi, keys.iter()).unwrap();
    let error = snapshot.restore(&mut scpi).unwrap_err();
    assert!(error.contains("stopped after three consecutive SCPI transport failures"));
    assert!(error.contains("verified restored keys: a; mismatched or unverified keys: b, c, d, e, RUN/STOP"));
    assert!(scpi.replies.is_empty());
    assert_eq!(events.borrow().last().unwrap(), ":SYSTem:GET? d");
}

#[test]
fn run_stop_restore_requires_acknowledgment_and_matching_state() {
    for (mode, saved, opposite) in [("RUN", RUNNING, STOPPED), ("STOP", STOPPED, RUNNING)] {
        for fail_command in [true, false] {
            let events = Events::default();
            let mut script = Script::default();
            script.reply(STATE, saved);
            if fail_command {
                script.transport_failure(format!(":{mode};*OPC?"));
            } else {
                script.run_state(&format!(":{mode}"));
            }
            script.reply(STATE, if fail_command { saved } else { opposite });
            let mut scpi = script.scpi(&events);
            let snapshot = Snapshot::read(&mut scpi, [].iter()).unwrap();
            let error = snapshot.restore(&mut scpi).unwrap_err();
            assert!(error.contains(if fail_command {
                "RUN/STOP command failed"
            } else {
                "mismatched or unverified keys: RUN/STOP"
            }));
            assert!(error.contains(&format!("select {mode}")));
            assert!(scpi.replies.is_empty());
        }
    }
}
