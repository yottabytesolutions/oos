// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

struct Counted {
    inner: linux::Sysfs,
    reads: Arc<AtomicUsize>,
}
impl Port for Counted {
    fn read(&self, path: &str) -> io::Result<String> {
        if path.starts_with(GPU) {
            self.reads.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.read(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        self.inner.write(path, value)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }
}

#[test]
fn unsupported_floor_retries_only_at_sample_deadlines_and_recovers() {
    let tree = Tree::new();
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    let clock = ManualClock::new();
    let reads = Arc::new(AtomicUsize::new(0));
    let port = Counted { inner: linux::Sysfs(tree.0.clone()), reads: reads.clone() };
    let c = Handle::fixture(Controller::with_clock(
        Box::new(port),
        Config::parse(""),
        Some(GPU),
        tree.0.join("saved"),
        clock.source(),
    ));
    c.update(true);
    assert_eq!(tree.minima(), ("1416000".into(), "400000000".into()));
    for _ in 0..2 {
        let previous = reads.load(Ordering::Relaxed);
        for _ in 0..100 {
            c.update(true);
            c.poll();
        }
        clock.advance(thermal::SAMPLE_PERIOD - Duration::from_nanos(1));
        c.poll();
        assert_eq!(reads.load(Ordering::Relaxed), previous, "no sysfs retry before the deadline");
        clock.advance(Duration::from_nanos(1));
        c.poll();
        assert!(reads.load(Ordering::Relaxed) > previous, "retry at the deadline");
    }
    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    c.update(true);
    assert_eq!(tree.minima().1, "400000000");
    clock.advance(thermal::SAMPLE_PERIOD);
    c.poll();
    assert_eq!(tree.minima().1, "600000000");
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn failed_write_retries_at_sample_deadlines_without_replacing_receipt() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let failures = Arc::new(Mutex::new(3));
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
    let mut c = Controller::with_clock(
        Box::new(port),
        Config::parse("perf.gpu_floor = off"),
        None,
        tree.0.join("saved"),
        clock.source(),
    );
    c.update(true);
    let receipt = std::fs::read(tree.0.join("saved")).unwrap();
    for remaining in [2, 1, 0] {
        assert_eq!(*oos_hal::sync::lock(&failures), remaining);
        for _ in 0..100 {
            c.update(true);
        }
        assert_eq!(*oos_hal::sync::lock(&failures), remaining);
        assert_eq!(tree.minima().0, "408000");
        clock.advance(thermal::SAMPLE_PERIOD);
        c.update(true);
        assert_eq!(std::fs::read(tree.0.join("saved")).unwrap(), receipt);
    }
    assert_eq!(tree.minima().0, "1416000");
    c.update(false);
    assert_eq!(tree.minima().0, "408000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn new_run_retries_immediately_after_a_failed_raise() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    let mut c = tree.controller_with_clock(&clock);
    c.update(true);
    c.update(false);
    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    c.update(true);
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
}

#[test]
fn raise_errors_log_only_changes_and_reset_after_success() {
    let tree = Tree::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "system::clocks::tests::retries::raise_errors_child", "--nocapture"])
        .env("OOS_CLOCK_RETRY_ROOT", &tree.0)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.matches("device=gpu running=true error=").count(), 3, "{text}");
    assert_eq!(text.matches("error=minimum is not in available frequencies").count(), 2, "{text}");
    assert_eq!(text.matches("error=invalid digit found in string").count(), 1, "{text}");
}

#[test]
fn raise_errors_child() {
    let Some(root) = std::env::var_os("OOS_CLOCK_RETRY_ROOT") else { return };
    let tree = std::mem::ManuallyDrop::new(Tree(root.into()));
    let clock = ManualClock::new();
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    let mut c = tree.controller_with_clock(&clock);
    for minimum in ["400000000", "bad"] {
        tree.set(GPU, "min_freq", minimum);
        for _ in 0..3 {
            c.update(true);
            clock.advance(thermal::SAMPLE_PERIOD);
        }
    }
    tree.set(GPU, "min_freq", "400000000");
    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    c.update(true);
    assert_eq!(tree.minima().1, "600000000");
    c.update(false);
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    c.update(true);
}
