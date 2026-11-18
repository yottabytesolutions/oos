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
    assert_eq!(text.matches("device=cpu-a72 running=false error=").count(), 1, "{text}");
    assert_eq!(text.matches("device=cpu-a72 restored=408000").count(), 1, "{text}");
}

#[test]
fn thermal_restore_failures_remain_engaged_until_restored_or_rearmed() {
    for (temperature, rearm) in [("65000", false), ("bad", false), ("65000", true)] {
        let tree = Tree::new();
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
        let running: Status = c.status();
        *oos_hal::sync::lock(&failures) = 1;
        tree.set("class/thermal/thermal_zone2", "temp", temperature);
        clock.advance(Duration::from_secs(1));
        c.poll();
        let pending = c.status();
        assert!(pending.to_string().contains("a72 engaged reason restore-pending"));
        assert_eq!(tree.minima().0, "1416000");
        assert!(tree.0.join("saved").exists());
        if rearm {
            tree.set("class/thermal/thermal_zone2", "temp", "59999");
        }
        clock.advance(Duration::from_secs(1));
        c.poll();
        let expected = if rearm { "a72 engaged reason none" } else { "a72 disabled reason thermal-" };
        assert!(c.status().to_string().contains(expected));
        c.update(false);
        assert_eq!(tree.minima().0, "408000");
        assert!(!tree.0.join("saved").exists());
        assert!(running.to_string().contains("a72 engaged reason none min_khz=1416000"));
        assert!(pending.to_string().contains("a72 engaged reason restore-pending min_khz=1416000"));
    }
}

#[test]
fn sampled_guard_does_not_change_status_until_controller_update() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let mut c = tree.controller_with_clock(&clock);
    c.update(true);
    tree.set("class/thermal/thermal_zone2", "temp", "65000");
    clock.advance(Duration::from_secs(1));
    assert!(!c.thermal.allowed(c.port.as_ref(), (c.clock)()));
    let text = c.status().to_string();
    for label in ["a72", "gpu_floor"] {
        assert!(text.contains(&format!("{label} engaged reason none")), "{text}");
    }
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
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

#[test]
fn run_after_failed_restore_reapplies_and_verifies_the_floor() {
    for minimum in ["408000", "816000"] {
        let tree = Tree::new();
        let failures = Arc::new(Mutex::new(0));
        let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
        let mut c = Controller::new(Box::new(port), Config::parse("perf.gpu_floor = off"), None, tree.0.join("saved"));
        c.update(true);
        let receipt = std::fs::read(tree.0.join("saved")).unwrap();
        *oos_hal::sync::lock(&failures) = 1;
        c.update(false);
        tree.set(CPU, "scaling_min_freq", minimum);
        assert!(c.status().to_string().contains("a72 engaged reason restore-pending"));
        c.update(true);
        assert_eq!(tree.minima().0, "1416000");
        assert!(c.status().to_string().contains("a72 engaged reason none"));
        assert_eq!(std::fs::read(tree.0.join("saved")).unwrap(), receipt);
        c.update(false);
        assert_eq!(tree.minima().0, "408000");
        assert!(!tree.0.join("saved").exists());
    }
}

#[test]
fn retry_after_unverified_raise_keeps_the_original_minimum() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let port = Clamping(linux::Sysfs(tree.0.clone()), CPU, 123);
    let mut c = Controller::with_clock(
        Box::new(port),
        Config::parse("perf.gpu_floor = off"),
        None,
        tree.0.join("saved"),
        clock.source(),
    );
    c.update(true);
    assert_eq!(tree.minima().0, "123");
    assert!(c.status().to_string().contains("a72 disabled reason not-engaged"));
    let receipt = std::fs::read(tree.0.join("saved")).unwrap();
    tree.set(CPU, "scaling_max_freq", "408000");
    clock.advance(thermal::SAMPLE_PERIOD);
    c.update(true);
    assert_eq!(tree.minima().0, "408000");
    assert!(c.status().to_string().contains("a72 engaged reason none"));
    assert_eq!(std::fs::read(tree.0.join("saved")).unwrap(), receipt);
    c.update(false);
    assert_eq!(tree.minima().0, "408000");
}
