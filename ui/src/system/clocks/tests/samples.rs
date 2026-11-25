// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

struct Counted {
    inner: linux::Sysfs,
    reads: Arc<Reads>,
}

#[derive(Default)]
struct Reads {
    total: AtomicUsize,
    gpu_available: AtomicUsize,
}

impl Port for Counted {
    fn read(&self, path: &str) -> io::Result<String> {
        self.reads.total.fetch_add(1, Ordering::Relaxed);
        if path.strip_prefix(GPU) == Some("/available_frequencies") {
            self.reads.gpu_available.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.read(path)
    }

    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        self.inner.write(path, value)
    }

    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.inner.entries(path)
    }

    fn read_link(&self, path: &str) -> io::Result<PathBuf> {
        self.inner.read_link(path)
    }
}

fn handle(tree: &Tree, clock: &ManualClock, conf: &str) -> (Handle, Arc<Reads>) {
    let reads = Arc::new(Reads::default());
    let port = Counted { inner: linux::Sysfs(tree.0.clone()), reads: reads.clone() };
    let controller =
        Controller::with_clock(Box::new(port), Config::parse(conf), Some(GPU), tree.0.join("saved"), clock.source());
    (Handle::fixture(controller), reads)
}

#[test]
fn absent_controller_uses_the_shared_scpi_clock_fields() {
    let reply = state_reply(None);
    for name in ["cpu", "gpu"] {
        assert!(reply.contains(&format!(
            "clock.{name}.requested_hz=0 clock.{name}.applied=0 clock.{name}.current_hz=unknown clock.{name}.reason=absent"
        )));
    }
}

#[test]
fn queries_use_the_latest_snapshot_without_sysfs_or_the_controller_lock() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let (handle, reads) = handle(&tree, &clock, "perf.cpu_floor = off\nperf.gpu_floor = off");
    let initial = reads.total.load(Ordering::Relaxed);
    {
        let _controller = oos_hal::sync::lock(&handle.controller);
        for _ in 0..10 {
            let state = with_status(String::new(), Some(&handle));
            assert!(state.contains("clock.gpu.current_hz=400000000 clock.gpu.reason=disabled"));
        }
    }
    tree.set(GPU, "cur_freq", "600000000");
    tree.set(CPU, "scaling_cur_freq", "bad");
    clock.advance(SAMPLE_PERIOD - Duration::from_nanos(1));
    handle.poll();
    assert_eq!(reads.total.load(Ordering::Relaxed), initial);
    assert!(with_status(String::new(), Some(&handle)).contains("clock.gpu.current_hz=400000000"));
    clock.advance(Duration::from_nanos(1));
    handle.poll();
    let sampled = reads.total.load(Ordering::Relaxed);
    assert!(sampled > initial);
    let state = with_status(String::new(), Some(&handle));
    assert!(state.contains("clock.gpu.current_hz=600000000 clock.gpu.reason=disabled"));
    assert!(state.contains("clock.cpu.current_hz=unknown clock.cpu.reason=disabled"));
    assert_eq!(reads.total.load(Ordering::Relaxed), sampled);
}

#[test]
fn unsupported_gpu_target_retries_at_the_deadline_or_on_new_run() {
    let tree = Tree::new();
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    let clock = ManualClock::new();
    let (handle, reads) = handle(&tree, &clock, "");
    handle.update(true);
    for _ in 0..2 {
        let previous = reads.gpu_available.load(Ordering::Relaxed);
        for _ in 0..100 {
            handle.update(true);
            handle.poll();
        }
        clock.advance(SAMPLE_PERIOD - Duration::from_nanos(1));
        handle.poll();
        assert_eq!(reads.gpu_available.load(Ordering::Relaxed), previous);
        clock.advance(Duration::from_nanos(1));
        handle.poll();
        assert_eq!(reads.gpu_available.load(Ordering::Relaxed), previous + 1);
        assert_eq!(tree.minima(), ("1416000".into(), "400000000".into()));
    }
    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    handle.poll();
    assert_eq!(tree.minima().1, "400000000");
    clock.advance(SAMPLE_PERIOD);
    handle.poll();
    assert_eq!(tree.minima().1, "600000000");

    handle.update(false);
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    handle.update(true);
    assert_eq!(tree.minima().1, "400000000");
    handle.update(false);
    tree.set(GPU, "available_frequencies", "200000000 400000000 600000000");
    handle.update(true);
    assert_eq!(tree.minima().1, "600000000");
}
