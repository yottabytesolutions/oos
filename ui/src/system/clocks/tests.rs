// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
const CPU: &str = "devices/system/cpu/cpufreq/policy4";
const GPU: &str = "class/devfreq/ff9a0000.gpu";

struct ManualClock(Arc<Mutex<Instant>>);
impl ManualClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Instant::now())))
    }
    fn advance(&self, duration: Duration) {
        *oos_hal::sync::lock(&self.0) += duration;
    }
    fn source(&self) -> Box<dyn Fn() -> Instant + Send> {
        let now = self.0.clone();
        Box::new(move || *oos_hal::sync::lock(&now))
    }
}

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let me = Self(std::env::temp_dir().join(format!(
            "oos-floors-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        )));
        me.set(CPU, "scaling_available_frequencies", "408000 816000 1416000 1800000");
        me.set(CPU, "scaling_min_freq", "408000");
        me.set(CPU, "scaling_max_freq", "1416000");
        me.set(GPU, "available_frequencies", "200000000 400000000 600000000");
        me.set(GPU, "min_freq", "400000000");
        for (number, kind) in [(7, "cpu-thermal"), (2, "gpu-thermal")] {
            let zone = format!("class/thermal/thermal_zone{number}");
            me.set(&zone, "type", kind);
            me.set(&zone, "temp", "45000");
            me.set(&zone, "trip_point_0_type", "passive");
            me.set(&zone, "trip_point_0_temp", "70000");
        }
        me
    }
    fn set(&self, dir: &str, file: &str, value: &str) {
        let dir = self.0.join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), value).unwrap();
    }
    fn minima(&self) -> (String, String) {
        (
            std::fs::read_to_string(self.0.join(CPU).join("scaling_min_freq")).unwrap(),
            std::fs::read_to_string(self.0.join(GPU).join("min_freq")).unwrap(),
        )
    }
    fn controller(&self, conf: &str) -> Controller {
        linux::controller(&self.0, &self.0.join("saved"), Config::parse(conf))
    }
    fn controller_with_clock(&self, clock: &ManualClock) -> Controller {
        Controller::with_clock(
            Box::new(linux::Sysfs(self.0.clone())),
            Config::parse(""),
            Some(GPU),
            self.0.join("saved"),
            clock.source(),
        )
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

mod issue_195;
mod policies;
mod recovery;
mod retries;
mod review;

#[test]
fn repeated_run_stop_restores_each_runs_previous_values_and_logs_once() {
    let tree = Tree::new();
    let mut c = tree.controller("");
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    c.update(true);
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    c.update(true);
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    tree.set(CPU, "scaling_min_freq", "816000");
    c.update(true);
    c.update(false);
    assert_eq!(tree.minima(), ("816000".into(), "400000000".into()));
}

#[test]
fn independent_default_on_switches_and_higher_prior_floor() {
    for (conf, cpu, gpu) in [
        ("perf.cpu_floor = off", "408000", "600000000"),
        ("perf.gpu_floor = off", "1416000", "400000000"),
        ("perf.cpu_floor = off\nperf.gpu_floor = off", "408000", "400000000"),
    ] {
        let tree = Tree::new();
        let mut c = tree.controller(conf);
        c.update(true);
        assert_eq!(tree.minima(), (cpu.into(), gpu.into()));
        drop(c);
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
    let tree = Tree::new();
    tree.set(CPU, "scaling_min_freq", "1800000");
    tree.set(CPU, "scaling_max_freq", "1800000");
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima().0, "1800000");
    c.update(false);
    assert_eq!(tree.minima().0, "1800000");
}

#[test]
fn invalid_prior_minimum_or_no_opp_under_maximum_is_never_written() {
    for (available, previous) in [("1800000", "1800000"), ("1416000", "408000"), ("408000 1416000", "bad")] {
        let tree = Tree::new();
        tree.set(CPU, "scaling_available_frequencies", available);
        tree.set(CPU, "scaling_min_freq", previous);
        let mut c = tree.controller("");
        c.update(true);
        assert_eq!(tree.minima(), (previous.into(), "600000000".into()));
        c.update(false);
        assert_eq!(tree.minima(), (previous.into(), "400000000".into()));
    }
}

#[test]
fn missing_nodes_do_not_affect_other_devices_or_create_files() {
    let tree = Tree::new();
    std::fs::remove_file(tree.0.join(CPU).join("scaling_available_frequencies")).unwrap();
    tree.set("class/devfreq/other-device", "available_frequencies", "600000000");
    tree.set("class/devfreq/other-device", "min_freq", "200000000");
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "600000000".into()));
    assert_eq!(std::fs::read_to_string(tree.0.join("class/devfreq/other-device/min_freq")).unwrap(), "200000000");
    c.update(false);
    std::fs::remove_dir_all(tree.0.join(GPU)).unwrap();
    let mut c = tree.controller("");
    c.update(true);
    assert!(!tree.0.join(GPU).exists());
}

#[test]
fn unwinding_and_exit_guard_restore_running_floors() {
    let tree = Tree::new();
    let caught = std::panic::catch_unwind(|| {
        let guard = Handle::fixture(tree.controller(""));
        guard.update(true);
        panic!("fixture panic");
    });
    assert!(caught.is_err());
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn accepted_ui_run_stop_and_teardown_drive_the_port() {
    use crate::model::RunState;
    use crate::ui::{host::Host, Ui};
    let tree = Tree::new();
    let shared = crate::board::HwShared::default();
    let host = Host::fixture_with_clocks(Handle::fixture(tree.controller("")));
    let mut ui = Ui::new(crate::wave::Source::Hw { shared, seen: 0 }, host);
    ui.step(1.0 / 30.0);
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    ui.m.run = RunState::Stop;
    ui.step(1.0 / 30.0);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    ui.m.run = RunState::Run;
    ui.step(1.0 / 30.0);
    ui.stop_workers(std::time::Instant::now());
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

struct Failing {
    inner: linux::Sysfs,
    failures: Arc<Mutex<usize>>,
}
impl Port for Failing {
    fn read(&self, path: &str) -> io::Result<String> {
        self.inner.read(path)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        let mut failures = oos_hal::sync::lock(&self.failures);
        if *failures > 0 {
            *failures -= 1;
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.inner.write(path, value)
    }
}

#[test]
fn failed_restore_retains_original_for_exit_retry() {
    let tree = Tree::new();
    let failures = Arc::new(Mutex::new(0));
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
    let mut c = Controller::new(Box::new(port), Config::parse("perf.gpu_floor = off"), None, tree.0.join("saved"));
    c.update(true);
    *oos_hal::sync::lock(&failures) = 1;
    c.update(false);
    assert_eq!(tree.minima().0, "1416000");
    assert!(tree.0.join("saved").exists());
    assert!(c.status().to_string().contains("a72 disabled reason restore-pending"));
    c.restore();
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn failed_raise_keeps_original_and_gpu_still_runs() {
    let tree = Tree::new();
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: Arc::new(Mutex::new(1)) };
    let mut c = Controller::new(Box::new(port), Config::parse(""), Some(GPU), tree.0.join("saved"));
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "600000000".into()));
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn panic_hook_restores_before_exit_without_unwinding() {
    let tree = Tree::new();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::panic_hook_child", "--nocapture"])
        .env("OOS_CLOCK_PANIC_FIXTURE", &tree.0)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(72));
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn panic_hook_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_PANIC_FIXTURE") else { return };
    // Exit in the previous hook models abort: no stack destructors can help restore.
    std::panic::set_hook(Box::new(|_| std::process::exit(72)));
    let guard = Handle::guarded(linux::controller(
        std::path::Path::new(&root),
        &std::path::Path::new(&root).join("saved"),
        Config::parse(""),
    ));
    guard.update(true);
    panic!("verify pre-abort restoration");
}

#[test]
fn restore_refuses_an_original_no_longer_listed_as_available() {
    let tree = Tree::new();
    let mut c = tree.controller("");
    c.update(true);
    tree.set(CPU, "scaling_available_frequencies", "1416000 1800000");
    c.update(false);
    assert_eq!(tree.minima(), ("1416000".into(), "400000000".into()));
    tree.set(CPU, "scaling_available_frequencies", "408000 1416000");
    c.restore();
    assert_eq!(tree.minima().0, "408000");
}

#[test]
fn successful_applied_values_are_structured_and_logged_once() {
    let tree = Tree::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::logging_child", "--nocapture"])
        .env("OOS_CLOCK_LOG_FIXTURE", &tree.0)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.matches(" applied=").count(), 2);
    assert!(text.contains("device=cpu-a72 applied=1416000 previous=408000"));
    assert!(text.contains("device=gpu applied=600000000 previous=400000000"));
}

#[test]
fn logging_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_LOG_FIXTURE") else { return };
    let mut c =
        linux::controller(std::path::Path::new(&root), &std::path::Path::new(&root).join("saved"), Config::parse(""));
    for _ in 0..2 {
        c.update(true);
        c.update(true);
        c.update(false);
    }
}

struct Clamping(linux::Sysfs, &'static str, u64);
impl Port for Clamping {
    fn read(&self, path: &str) -> io::Result<String> {
        self.0.read(path)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.0.entries(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        let clamp = path == format!("{}/scaling_min_freq", self.1) && value > 408000;
        self.0.write(path, if clamp { self.2 } else { value })
    }
}

#[test]
fn mismatched_read_back_still_restores_the_original() {
    let tree = Tree::new();
    let port = Clamping(linux::Sysfs(tree.0.clone()), CPU, 816000);
    let mut c = Controller::new(Box::new(port), Config::parse("perf.gpu_floor = off"), None, tree.0.join("saved"));
    c.update(true);
    assert_eq!(tree.minima().0, "816000");
    c.update(false);
    assert_eq!(tree.minima().0, "408000");
}
