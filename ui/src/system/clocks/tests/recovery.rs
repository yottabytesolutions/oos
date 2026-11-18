// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn death_after_raise_restores_true_previous_before_next_run() {
    let tree = Tree::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::recovery::death_child", "--nocapture"])
        .env("OOS_CLOCK_DEATH_FIXTURE", &tree.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(73));
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    assert!(tree.0.join("saved").exists());
    let mut controller = tree.controller("");
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    assert!(!tree.0.join("saved").exists());
    controller.update(true);
    controller.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn death_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_DEATH_FIXTURE") else { return };
    let root = std::path::Path::new(&root);
    let mut controller = linux::controller(root, &root.join("saved"), Config::parse(""));
    controller.update(true);
    // No destructors, as with SIGKILL or an allocator abort.
    std::process::exit(73);
}

#[test]
fn recovery_ignores_optouts_and_retains_unverifiable_receipts() {
    let tree = Tree::new();
    std::fs::write(tree.0.join("saved"), "gpu 400000000\ncpu 408000\n").unwrap();
    tree.set(CPU, "scaling_min_freq", "1416000");
    tree.set(GPU, "min_freq", "600000000");
    let mut c = tree.controller("perf.cpu_floor = off\nperf.gpu_floor = off");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    drop(c);
    std::fs::write(tree.0.join("saved"), "cpu 408000\n").unwrap();
    tree.set(CPU, "scaling_available_frequencies", "1416000");
    let mut c = tree.controller("");
    c.update(true);
    drop(c);
    assert!(tree.0.join("saved").exists());
    assert_eq!(tree.minima().1, "400000000");
}

#[test]
fn failed_journal_write_never_raises_and_invalid_receipt_is_kept() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("saved.new")).unwrap();
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    drop(c);
    for text in [
        "",
        "cpu bad\n",
        "cpu 408000\ncpu 816000\n",
        "cpu 408000\ncpu-a72 408000\n",
        "other 1\n",
        "CPU 408000\n",
        "cpu\n",
        "gpu 400000000 extra\n",
    ] {
        std::fs::write(tree.0.join("saved"), text).unwrap();
        let mut c = tree.controller("");
        c.update(true);
        drop(c);
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), text);
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn thermal_drop_rearm_and_sensor_failure_work_during_continuous_run() {
    let tree = Tree::new();
    // Unrelated zones must not participate, regardless of their number.
    tree.set("class/thermal/thermal_zone0", "type", "other");
    tree.set("class/thermal/thermal_zone0", "temp", "99000");
    let clock = ManualClock::new();
    let c = Handle::fixture(tree.controller_with_clock(&clock));
    c.update(true);
    assert_eq!(tree.minima().0, "1416000");
    for (temp, raised) in [("65000", false), ("62000", false), ("59999", true), ("bad", false)] {
        tree.set("class/thermal/thermal_zone2", "temp", temp);
        let previous = tree.minima();
        clock.advance(Duration::from_secs(1) - Duration::from_nanos(1));
        c.poll();
        assert_eq!(tree.minima(), previous, "thermal sampling must wait for the guard cadence");
        clock.advance(Duration::from_nanos(1));
        c.poll();
        let expected = if raised { ("1416000", "600000000") } else { ("408000", "400000000") };
        assert_eq!(tree.minima(), (expected.0.into(), expected.1.into()));
        assert_eq!(tree.0.join("saved").exists(), raised);
    }
}

#[test]
fn missing_required_zone_disables_floors() {
    for file in ["thermal_zone2/type", "thermal_zone7/type"] {
        let tree = Tree::new();
        std::fs::remove_file(tree.0.join("class/thermal").join(file)).unwrap();
        let mut c = tree.controller("");
        c.update(true);
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn panic_while_controller_is_locked_does_not_deadlock() {
    let tree = Tree::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::recovery::locked_panic_child", "--nocapture"])
        .env("OOS_CLOCK_LOCKED_FIXTURE", &tree.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(74));
    assert!(tree.0.join("saved").exists());
    drop(tree.controller(""));
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn locked_panic_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_LOCKED_FIXTURE") else { return };
    let root = std::path::Path::new(&root);
    std::panic::set_hook(Box::new(|_| std::process::exit(74)));
    let handle = Handle::guarded(linux::controller(root, &root.join("saved"), Config::parse("")));
    handle.update(true);
    let _held = oos_hal::sync::lock(&handle.controller);
    panic!("locked clock panic");
}

#[test]
fn absent_floors_are_reported_once() {
    let tree = Tree::new();
    std::fs::remove_file(tree.0.join(CPU).join("scaling_available_frequencies")).unwrap();
    std::fs::remove_dir_all(tree.0.join(GPU)).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::logging_child", "--nocapture"])
        .env("OOS_CLOCK_LOG_FIXTURE", &tree.0)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.matches("device=cpu-a72 absent; disabled").count(), 1);
    assert_eq!(text.matches("device=gpu absent; disabled").count(), 1);
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn stale_staging_symlink_cannot_overwrite_another_file() {
    let tree = Tree::new();
    let other = tree.0.join("unrelated");
    std::fs::write(&other, "keep").unwrap();
    std::os::unix::fs::symlink(&other, tree.0.join("saved.new")).unwrap();
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima().0, "1416000");
    assert_eq!(std::fs::read_to_string(other).unwrap(), "keep");
}

struct PolicyFailure {
    inner: linux::Sysfs,
    failure: Arc<Mutex<Option<&'static str>>>,
}
impl Port for PolicyFailure {
    fn read(&self, path: &str) -> io::Result<String> {
        let failure = *oos_hal::sync::lock(&self.failure);
        if path == format!("{}/scaling_min_freq", policies::A53) {
            match failure {
                Some("readback") => return Err(io::ErrorKind::PermissionDenied.into()),
                Some("mismatch") => return Ok("1200000".into()),
                _ => {}
            }
        }
        self.inner.read(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        if path == format!("{}/scaling_min_freq", policies::A53) && *oos_hal::sync::lock(&self.failure) == Some("write")
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.inner.write(path, value)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }
}

#[test]
fn persistent_policy_recovery_failure_does_not_block_later_devices() {
    for config in ["", "perf.cpu_floor = off\nperf.gpu_floor = off"] {
        for fault in ["unavailable", "write", "readback", "mismatch"] {
            let tree = Tree::new();
            policies::policies(&tree);
            tree.set(policies::A53, "scaling_min_freq", "1200000");
            tree.set(CPU, "scaling_min_freq", "1416000");
            tree.set(GPU, "min_freq", "600000000");
            if fault == "unavailable" {
                std::fs::remove_file(tree.0.join(policies::A53).join("scaling_available_frequencies")).unwrap();
            }
            let receipt = "cpu-a53 408000\ncpu-a72 816000\ngpu 400000000\n";
            std::fs::write(tree.0.join("saved"), receipt).unwrap();
            let clock = ManualClock::new();
            let failure = Arc::new(Mutex::new(Some(fault)));
            let port = PolicyFailure { inner: linux::Sysfs(tree.0.clone()), failure: failure.clone() };
            let mut c = Controller::with_clock(
                Box::new(port),
                Config::parse(config),
                Some(GPU),
                tree.0.join("saved"),
                clock.source(),
            );
            for running in [true, false, true] {
                assert!(c.status().to_string().contains("a53 disabled reason restore-pending"));
                assert_eq!(tree.minima(), ("816000".into(), "400000000".into()));
                assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
                assert!(c.recover().is_err(), "{fault}");
                clock.advance(thermal::SAMPLE_PERIOD);
                c.update(running);
            }
            *oos_hal::sync::lock(&failure) = None;
            policies::policies(&tree);
            tree.set(policies::A53, "scaling_min_freq", "1200000");
            clock.advance(thermal::SAMPLE_PERIOD);
            c.update(true);
            assert_eq!(std::fs::read_to_string(tree.0.join(policies::A53).join("scaling_min_freq")).unwrap(), "408000");
            assert!(!tree.0.join("saved").exists());
            c.update(true);
            assert_eq!(tree.minima(), ("816000".into(), "400000000".into()));
            assert!(c.status().to_string().contains("a53 disabled reason recovery-failed"));
        }
    }
}

#[test]
fn startup_restore_failure_owns_all_saved_minima_and_retries_without_raising() {
    for config in ["", "perf.cpu_floor = off\nperf.gpu_floor = off"] {
        let tree = Tree::new();
        tree.set(CPU, "scaling_min_freq", "1416000");
        tree.set(GPU, "min_freq", "600000000");
        let receipt = "cpu-a72 408000\ngpu 400000000\n";
        std::fs::write(tree.0.join("saved"), receipt).unwrap();
        let clock = ManualClock::new();
        let failures = Arc::new(Mutex::new(4));
        let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
        let mut c = Controller::with_clock(
            Box::new(port),
            Config::parse(config),
            Some(GPU),
            tree.0.join("saved"),
            clock.source(),
        );
        for label in ["a72", "gpu_floor"] {
            assert!(c.status().to_string().contains(&format!("{label} disabled reason restore-pending")));
        }
        for running in [true, false] {
            c.update(running);
            assert_eq!(*oos_hal::sync::lock(&failures), 2);
        }
        clock.advance(thermal::SAMPLE_PERIOD);
        c.update(true);
        assert_eq!(*oos_hal::sync::lock(&failures), 0);
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
        assert!(c.status().to_string().contains("a72 disabled reason restore-pending"));
        clock.advance(thermal::SAMPLE_PERIOD);
        c.update(false);
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
        assert!(!tree.0.join("saved").exists());
        c.update(true);
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
        assert!(c.status().to_string().contains("a72 disabled reason recovery-failed"));
    }
}
