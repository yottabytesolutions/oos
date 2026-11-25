// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

struct AlteredWrite {
    inner: linux::Sysfs,
    first: Option<(u64, io::Result<()>)>,
}
impl Port for AlteredWrite {
    fn read(&self, path: &str) -> io::Result<String> {
        self.inner.read(path)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }
    fn read_link(&self, path: &str) -> io::Result<PathBuf> {
        self.inner.read_link(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        if let Some((minimum, result)) = self.first.take() {
            self.inner.write(path, minimum)?;
            return result;
        }
        self.inner.write(path, value)
    }
}

#[test]
fn mismatched_read_back_is_unverified_and_retries_at_guard_cadence() {
    for minimum in [816_000, 1_800_000] {
        let tree = Tree::new();
        let port = AlteredWrite { inner: linux::Sysfs(tree.0.clone()), first: Some((minimum, Ok(()))) };
        let clock = ManualClock::new();
        let mut c = Controller::with_clock(
            Box::new(port),
            Config::parse("perf.gpu_floor = off"),
            None,
            tree.0.join("saved"),
            clock.source(),
        );
        c.update(true);
        assert!(status_text(&c)
            .contains("clock.cpu.applied=0 clock.cpu.current_hz=816000000 clock.cpu.reason=apply_failed"));
        clock.advance(Duration::from_secs(1) - Duration::from_nanos(1));
        c.update(true);
        assert_eq!(tree.minima().0, minimum.to_string());
        clock.advance(Duration::from_nanos(1));
        c.update(true);
        assert_eq!(tree.minima().0, "1416000");
        assert!(status_text(&c).contains("clock.cpu.applied=1 clock.cpu.current_hz=816000000 clock.cpu.reason=none"));
        c.update(false);
        assert_eq!(tree.minima().0, "408000");
    }
}

#[test]
fn matching_read_back_succeeds_even_with_write_error() {
    let tree = Tree::new();
    let port = AlteredWrite {
        inner: linux::Sysfs(tree.0.clone()),
        first: Some((1_416_000, Err(io::ErrorKind::PermissionDenied.into()))),
    };
    let mut c = Controller::new(Box::new(port), Config::parse("perf.gpu_floor = off"), None, tree.0.join("saved"));
    c.update(true);
    assert!(status_text(&c).contains("clock.cpu.applied=1 clock.cpu.current_hz=816000000 clock.cpu.reason=none"));
    c.update(false);
    assert_eq!(tree.minima().0, "408000");
}

#[test]
fn failed_writes_retry_at_deadlines_with_the_original_receipt() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let failures = Arc::new(Mutex::new(3));
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
    let mut controller = cpu_controller(&tree, &clock, port);
    controller.update(true);
    for remaining in [2, 1, 0] {
        for _ in 0..100 {
            controller.update(true);
        }
        assert_eq!(*oos_hal::sync::lock(&failures), remaining);
        assert_eq!(tree.minima().0, "408000");
        clock.advance(SAMPLE_PERIOD);
        controller.update(true);
        assert_cpu_receipt(&tree);
    }
    assert_eq!(tree.minima().0, "1416000");
    controller.update(false);
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn stop_releases_the_run_gpu_floor_when_the_display_target_is_unavailable() {
    let tree = Tree::new();
    tree.set(GPU, "min_freq", "200000000");
    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);
    controller.update(true);
    assert_eq!(tree.minima().1, "600000000");
    tree.set(GPU, "available_frequencies", "200000000 600000000");
    controller.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "200000000".into()));
    assert!(!tree.0.join("saved").exists());
    assert!(status_text(&controller).contains("clock.gpu.reason=apply_failed"));

    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    controller.update(false);
    assert_eq!(tree.minima().1, "200000000");
    clock.advance(SAMPLE_PERIOD);
    controller.update(false);
    assert_eq!(tree.minima().1, "400000000");
    assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), "gpu 200000000\n");
}

#[derive(Default)]
struct ReadBack {
    fail_after_write: bool,
    fail_next: bool,
    cpu_writes: usize,
}

struct Uncertain {
    inner: linux::Sysfs,
    read_back: Arc<Mutex<ReadBack>>,
}

impl Port for Uncertain {
    fn read(&self, path: &str) -> io::Result<String> {
        if cpu_minimum(path) && std::mem::take(&mut oos_hal::sync::lock(&self.read_back).fail_next) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.inner.read(path)
    }

    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }

    fn read_link(&self, path: &str) -> io::Result<PathBuf> {
        self.inner.read_link(path)
    }

    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        if cpu_minimum(path) {
            let receipt = std::fs::read_to_string(self.inner.0.join("saved"))?;
            assert!(receipt.lines().any(|line| line == "cpu 408000"));
            let mut read_back = oos_hal::sync::lock(&self.read_back);
            read_back.cpu_writes += 1;
            read_back.fail_next = read_back.fail_after_write;
        }
        self.inner.write(path, value)
    }
}

fn cpu_minimum(path: &str) -> bool {
    path.strip_prefix(CPU) == Some("/scaling_min_freq")
}

fn cpu_controller(tree: &Tree, clock: &ManualClock, port: impl Port + 'static) -> Controller {
    Controller::with_clock(
        Box::new(port),
        Config::parse("perf.gpu_floor = off"),
        None,
        tree.0.join("saved"),
        clock.source(),
    )
}

fn assert_cpu_receipt(tree: &Tree) {
    assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), "cpu 408000\n");
}

fn assert_cpu_state(controller: &Controller, applied: u8, reason: &str) {
    assert!(status_text(controller)
        .contains(&format!("clock.cpu.applied={applied} clock.cpu.current_hz=816000000 clock.cpu.reason={reason}")));
}

#[test]
fn uncertain_raise_and_stop_retain_the_receipt_until_verified() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let read_back = Arc::new(Mutex::new(ReadBack { fail_after_write: true, ..ReadBack::default() }));
    let port = Uncertain { inner: linux::Sysfs(tree.0.clone()), read_back: read_back.clone() };
    let mut controller = cpu_controller(&tree, &clock, port);
    controller.update(true);
    assert_eq!(tree.minima().0, "1416000");
    assert_cpu_state(&controller, 0, "apply_failed");
    assert_cpu_receipt(&tree);

    controller.update(false);
    assert_eq!(tree.minima().0, "408000");
    assert_cpu_state(&controller, 0, "restore_failed");
    assert_cpu_receipt(&tree);
    oos_hal::sync::lock(&read_back).fail_after_write = false;
    controller.update(false);
    assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 2);
    clock.advance(SAMPLE_PERIOD);
    controller.update(false);
    assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 3);
    assert_cpu_state(&controller, 0, "stopped");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn uncertain_reraise_clears_old_verification_but_keeps_rollback_pending() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let read_back = Arc::new(Mutex::new(ReadBack::default()));
    let port = Uncertain { inner: linux::Sysfs(tree.0.clone()), read_back: read_back.clone() };
    let mut controller = cpu_controller(&tree, &clock, port);
    controller.update(true);
    assert_cpu_state(&controller, 1, "none");
    oos_hal::sync::lock(&read_back).fail_after_write = true;
    controller.update(false);
    assert_eq!(tree.minima().0, "408000");
    assert_cpu_state(&controller, 0, "restore_failed");

    controller.update(true);
    assert_eq!(tree.minima().0, "1416000");
    assert_cpu_state(&controller, 0, "restore_failed");
    assert_cpu_receipt(&tree);
    oos_hal::sync::lock(&read_back).fail_after_write = false;
    controller.update(true);
    assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 3);
    clock.advance(SAMPLE_PERIOD);
    controller.update(true);
    assert_cpu_state(&controller, 1, "none");
    assert_cpu_receipt(&tree);
    controller.update(false);
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn startup_attempts_both_restores_and_can_force_an_unverified_retry() {
    for config in ["", "perf.cpu_floor = off\nperf.gpu_floor = off"] {
        let tree = Tree::new();
        let clock = ManualClock::new();
        let receipt = "cpu 408000\ngpu 400000000\n";
        std::fs::write(tree.0.join("saved"), receipt).unwrap();
        tree.set(CPU, "scaling_min_freq", "1416000");
        tree.set(GPU, "min_freq", "600000000");
        let read_back = Arc::new(Mutex::new(ReadBack { fail_after_write: true, ..ReadBack::default() }));
        let port = Uncertain { inner: linux::Sysfs(tree.0.clone()), read_back: read_back.clone() };
        let mut controller = Controller::with_clock(
            Box::new(port),
            Config::parse(config),
            Some(GPU),
            tree.0.join("saved"),
            clock.source(),
        );
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
        assert_cpu_state(&controller, 0, "restore_failed");
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
        for running in [true, false, true] {
            controller.update(running);
            assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 1);
        }
        clock.advance(SAMPLE_PERIOD);
        controller.update(true);
        assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 2);
        assert_cpu_state(&controller, 0, "restore_failed");
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
        oos_hal::sync::lock(&read_back).fail_after_write = false;
        controller.restore();
        assert_eq!(oos_hal::sync::lock(&read_back).cpu_writes, 3);
        assert!(!tree.0.join("saved").exists());
        controller.update(true);
        assert_cpu_state(&controller, 0, "recovery_failed");
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn scpi_distinguishes_verified_and_unverified_pending_rollbacks() {
    for verified in [true, false] {
        let tree = Tree::new();
        let clock = ManualClock::new();
        let failures = Arc::new(Mutex::new(usize::from(!verified)));
        let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
        let mut controller = cpu_controller(&tree, &clock, port);
        controller.update(true);
        if !verified {
            // A later read can observe the target after an uncertain write.
            tree.set(CPU, "scaling_min_freq", "1416000");
        }
        *oos_hal::sync::lock(&failures) = 3;
        controller.update(false);
        let reply = state_reply(Some(Handle::fixture(controller)));
        assert!(reply.contains(&format!(
            "clock.cpu.requested_hz=0 clock.cpu.applied={} clock.cpu.current_hz=816000000 clock.cpu.reason=restore_failed",
            u8::from(verified)
        )));
        assert_cpu_receipt(&tree);
    }
}
