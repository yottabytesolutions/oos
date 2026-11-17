// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn child(tree: &Tree, case: &str) -> String {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::review::review_child", "--nocapture"])
        .env("OOS_CLOCK_REVIEW_ROOT", &tree.0)
        .env("OOS_CLOCK_REVIEW_CASE", case)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn floor_switches_accept_only_on_off_and_default_on() {
    assert_eq!(Config::parse(""), Config { cpu: true, gpu: true });
    for (value, enabled) in
        [("on", true), ("off", false), ("", false), ("0", false), ("ON", false), ("true", false), ("typo", false)]
    {
        for key in ["perf.cpu_floor", "perf.gpu_floor"] {
            let config = Config::parse(&format!("{key} = {value}"));
            assert_eq!(if key == "perf.cpu_floor" { config.cpu } else { config.gpu }, enabled, "{key}={value}");
            assert!(if key == "perf.cpu_floor" { config.gpu } else { config.cpu });
        }
    }
    let tree = Tree::new();
    let mut c = tree.controller("perf.cpu_floor = typo\nperf.gpu_floor = true");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn invalid_floor_switches_log_the_key_and_value() {
    let text = child(&Tree::new(), "config");
    for key in ["perf.cpu_floor", "perf.gpu_floor"] {
        assert!(text.contains(&format!("{key}=typo")), "{text}");
    }
}

#[test]
fn failed_restore_retries_once_per_second_and_logs_only_failure_then_success() {
    let text = child(&Tree::new(), "restore");
    assert_eq!(text.matches("device=cpu running=false error=").count(), 1, "{text}");
    assert_eq!(text.matches("device=cpu restored=408000").count(), 1, "{text}");
}

fn restore_retries(tree: &Tree) {
    let failures = Arc::new(Mutex::new(0));
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
    let clock = ManualClock::new();
    let c = Handle::fixture(Controller::with_clock(
        Box::new(port),
        Config::parse("perf.gpu_floor = off"),
        None,
        tree.0.join("saved"),
        clock.source(),
    ));
    c.update(true);
    *oos_hal::sync::lock(&failures) = 3;
    c.update(false);
    assert_eq!(*oos_hal::sync::lock(&failures), 2);
    for _ in 0..100 {
        c.poll();
        c.update(false);
    }
    assert_eq!(*oos_hal::sync::lock(&failures), 2, "per-frame polls must not retry writes");
    assert!(tree.0.join("saved").exists());
    clock.advance(Duration::from_secs(1) - Duration::from_nanos(1));
    c.poll();
    assert_eq!(*oos_hal::sync::lock(&failures), 2, "no retry before the deadline");
    clock.advance(Duration::from_nanos(1));
    c.poll();
    assert_eq!(*oos_hal::sync::lock(&failures), 1, "retry at guard cadence");
    *oos_hal::sync::lock(&failures) = 0;
    for _ in 0..100 {
        c.poll();
    }
    assert_eq!(tree.minima().0, "1416000");
    clock.advance(Duration::from_secs(1) - Duration::from_nanos(1));
    c.poll();
    assert_eq!(tree.minima().0, "1416000", "restoration must wait for the next deadline");
    assert!(tree.0.join("saved").exists());
    clock.advance(Duration::from_nanos(1));
    c.poll();
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn thermal_transitions_log_zone_and_reading_or_read_error() {
    let text = child(&Tree::new(), "thermal");
    assert!(text.contains("floors=dropped zone=gpu-thermal temp_mc=65000"), "{text}");
    for (zone, temp) in [("cpu-thermal", 45000), ("gpu-thermal", 59999)] {
        assert!(text.contains(&format!("floors=rearmed zone={zone} temp_mc={temp}")), "{text}");
    }
    assert!(text.contains("floors=dropped zone=gpu-thermal error="), "{text}");
    assert!(text.contains("floors=dropped zone=cpu-thermal error="), "{text}");
    assert_eq!(text.matches("floors=dropped").count(), 3, "{text}");
    assert_eq!(text.matches("floors=rearmed").count(), 4, "{text}");
}

fn thermal_transitions(tree: &Tree) {
    let clock = ManualClock::new();
    let c = Handle::fixture(tree.controller_with_clock(&clock));
    c.update(true);
    for (temp, raised) in [("65000", false), ("62000", false), ("59999", true), ("bad", false), ("45000", true)] {
        tree.set("class/thermal/thermal_zone2", "temp", temp);
        clock.advance(Duration::from_secs(1));
        c.poll();
        c.poll();
        assert_eq!(tree.minima().0, if raised { "1416000" } else { "408000" });
    }
    std::fs::remove_file(tree.0.join("class/thermal/thermal_zone7/temp")).unwrap();
    clock.advance(Duration::from_secs(1));
    c.poll();
    assert_eq!(tree.minima().0, "408000");
}

#[test]
fn review_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_REVIEW_ROOT") else { return };
    // The parent owns cleanup; this child exits before fixture destructors run.
    let tree = std::mem::ManuallyDrop::new(Tree(root.into()));
    match std::env::var("OOS_CLOCK_REVIEW_CASE").unwrap().as_str() {
        "config" => {
            Config::parse("perf.cpu_floor = typo\nperf.gpu_floor = typo");
        }
        "restore" => restore_retries(&tree),
        "thermal" => thermal_transitions(&tree),
        case => panic!("unknown review case {case}"),
    }
}
