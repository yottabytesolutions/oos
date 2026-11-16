// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Runs the actual rcS restart loop against an aborting UI stand-in. Set OOS_TEST_SHELL to the image's ash
//! executable in the Linux runner; the host default is POSIX sh, never bash-specific script syntax.
mod safe_start;
mod support;
use safe_start::Fake;

fn aborting_ui(f: &Fake, rc: u32) {
    f.exec(
        "bin/ui",
        &format!(
            r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
n=$(wc -l < "$OOS_UI_TMP/launches")
if [ "$n" -ge 4 ]; then touch "$OOS_UI_TMP/oos-ui.off"; exit 0; fi
exit {rc}
"#
        ),
    );
}

#[test]
fn aborting_ui_reaches_safe_on_fourth_launch() {
    let f = Fake::new("aborts");
    aborting_ui(&f, 134);
    let out = f.run();
    assert!(out.status.success(), "{:?}", out);
    let launches = std::fs::read_to_string(f.p("tmp/launches")).unwrap();
    let lines: Vec<_> = launches.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[..3].iter().all(|l| !l.contains("--safe")), "{} {:?}", launches, out);
    assert!(lines[3].contains("--safe"), "{} {:?}", launches, out);
}

#[test]
fn slow_aborts_count_within_an_hour() {
    let f = Fake::new("slow");
    for (i, time) in [10000, 11200, 12400].iter().enumerate() {
        f.write("now", &time.to_string());
        f.count(&format!("run{}", i), "134");
    }
    assert_eq!(f.mode(), "safe");
}

#[test]
fn old_exits_expire() {
    let f = Fake::new("expiry");
    for i in 0..3 {
        f.count(&format!("run{}", i), "1");
    }
    assert_eq!(f.mode(), "safe");
    f.write("now", "13601");
    assert_eq!(f.mode(), "normal");
    // Read-only mode filters by time; the next write durably prunes expired entries.
    f.count("run2", "0");
    // Retry identity survives hourly pruning and a backwards RTC step.
    f.write("now", "9000");
    f.count("run2", "75");
    assert_eq!(f.mode(), "normal");
}

#[test]
fn memory_exit_selects_safe_immediately() {
    let f = Fake::new("memory");
    f.exec(
        "bin/ui",
        r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
if [ -e "$OOS_UI_TMP/memory" ]; then touch "$OOS_UI_TMP/oos-ui.off"; exit 0; fi
touch "$OOS_UI_TMP/memory"
exit 75
"#,
    );
    assert!(f.run().status.success());
    let launches = std::fs::read_to_string(f.p("tmp/launches")).unwrap();
    assert!(launches.lines().nth(1).unwrap().contains("--safe"), "{}", launches);
}

#[test]
fn trial_keeps_rollback_authority() {
    let f = Fake::new("trial");
    std::fs::create_dir(f.p("oos/update")).unwrap();
    f.write("oos/update/state", "OOS-UPDATE-STATE 1\ntrial 1\n");
    aborting_ui(&f, 75);
    assert!(f.run().status.success());
    assert!(!std::fs::read_to_string(f.p("tmp/launches")).unwrap().contains("--safe"));
}

#[test]
fn clean_exits_do_not_count() {
    let f = Fake::new("clean");
    aborting_ui(&f, 0);
    assert!(f.run().status.success());
    assert_eq!(f.mode(), "normal");
}

#[test]
fn trial_bookkeeping_precedes_crash_save() {
    let rc = include_str!("../../rootfs/initramfs/etc/rcS");
    assert!(rc.find("/bin/oos-update boot").unwrap() < rc.find("/bin/oos-ui-loop").unwrap());
    let loop_script = std::fs::read_to_string(safe_start::image_script("oos-ui-loop")).unwrap();
    assert!(loop_script.find("\"$crash_save\"").unwrap() < loop_script.find("while [ ! -e").unwrap());
}

#[test]
fn counter_write_failures_preserve_retry_identity() {
    for (name, cmd, call) in [("file-sync", "sync", 1), ("rename", "mv", 1), ("dir-sync", "sync", 2)] {
        let f = Fake::new(name);
        f.count("old", "1");
        let real = std::process::Command::new("sh").args(["-c", &format!("command -v {}", cmd)]).output().unwrap();
        let real = String::from_utf8(real.stdout).unwrap();
        f.exec(
            &format!("bin/{}", cmd),
            &format!(
                r#"#!/bin/sh
n=$(cat "$OOS_UI_TMP/calls" 2>/dev/null || echo 0)
n=$((n + 1)); echo "$n" > "$OOS_UI_TMP/calls"
[ "$n" -ne {call} ] || exit 1
exec '{real}' "$@"
"#,
                real = real.trim()
            ),
        );
        let path = format!("{}:{}", f.p("bin").display(), std::env::var("PATH").unwrap());
        let out = f.command("oos-ui-exits").args(["count", "new", "1", "new"]).env("PATH", path).output().unwrap();
        assert!(!out.status.success(), "{}", name);
        f.count("new", "1");
        // Retried at every durable boundary: exactly two exits, so still normal.
        assert_eq!(f.mode(), "normal", "{}", name);
        f.count("third", "1");
        assert_eq!(f.mode(), "safe", "{}", name);
    }
}

#[test]
fn stale_marker_retry_does_not_count_twice() {
    let f = Fake::new("stale");
    f.write("oos/oos-ui.running", "00000000-0000-4000-8000-000000000001:old\n");
    f.exec("bin/ui", "#!/bin/sh\ntouch \"$OOS_UI_TMP/oos-ui.off\"\nexit 0\n");
    assert!(f.run().status.success());
    std::fs::remove_file(f.p("tmp/oos-ui.off")).unwrap();
    assert!(f.run().status.success());
    let ledger = std::fs::read_to_string(f.p("oos/oos-ui.exits")).unwrap();
    assert_eq!(ledger.lines().filter(|l| l.starts_with("10000 ")).count(), 1);
}

#[test]
fn delayed_panic_matches_an_older_counted_run() {
    let f = Fake::new("delayed-panic");
    f.count("old", "1");
    f.count("new", "1");
    let out = f.command("oos-ui-exits").args(["count", "panic-delayed", "1", "old", "panic"]).output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(f.mode(), "normal", "two runs, even if the first panic count was delayed");
}

#[test]
fn every_exit_restores_clock_receipt_even_when_no_restart_is_requested() {
    for (tag, available, restored, rc) in [
        ("restore", "408000 1416000", true, 137),
        ("retain", "1416000", false, 137),
        ("clean-restore", "408000 1416000", true, 0),
        ("clean-retain", "1416000", false, 0),
    ] {
        let f = Fake::new(&format!("clock-{tag}"));
        let dir = "sys/devices/system/cpu/cpufreq/policy4";
        std::fs::create_dir_all(f.p(dir)).unwrap();
        f.write(&format!("{dir}/scaling_available_frequencies"), available);
        f.write(&format!("{dir}/scaling_min_freq"), "1416000");
        let gpu = "sys/class/devfreq/ff9a0000.gpu";
        std::fs::create_dir_all(f.p(gpu)).unwrap();
        f.write(&format!("{gpu}/available_frequencies"), "400000000 600000000");
        f.write(&format!("{gpu}/min_freq"), "600000000");
        f.write("tmp/oos-clock-floors", "cpu 408000\ngpu 400000000");
        f.exec("bin/ui", &format!("#!/bin/sh\ntouch \"$OOS_UI_TMP/oos-ui.off\"\nexit {rc}\n"));
        let out = f.command("oos-ui-loop").env("OOS_CLOCK_SYSFS", f.p("sys")).output().unwrap();
        assert!(out.status.success(), "{:?}", out);
        assert_eq!(f.p("tmp/oos-clock-floors").exists(), !restored);
        let cpu = std::fs::read_to_string(f.p(&format!("{dir}/scaling_min_freq"))).unwrap();
        let gpu = std::fs::read_to_string(f.p(&format!("{gpu}/min_freq"))).unwrap();
        assert_eq!(cpu.trim(), if restored { "408000" } else { "1416000" });
        assert_eq!(gpu.trim(), if restored { "400000000" } else { "600000000" });
        let log = std::fs::read_to_string(f.p("tmp/oos-ui.log")).unwrap();
        assert!(log.contains(if restored { "device=cpu restored=408000" } else { "recovery failed" }));
    }
}
