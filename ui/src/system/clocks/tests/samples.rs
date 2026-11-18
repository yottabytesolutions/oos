// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

struct CountReads {
    inner: linux::Sysfs,
    reads: Arc<AtomicUsize>,
}
impl Port for CountReads {
    fn read(&self, path: &str) -> io::Result<String> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read(path)
    }
    fn entries(&self, path: &str) -> io::Result<Vec<String>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.entries(path)
    }
    fn write(&mut self, path: &str, value: u64) -> io::Result<()> {
        self.inner.write(path, value)
    }
}

#[test]
fn repeated_state_queries_do_no_io_and_later_samples_refresh_readbacks() {
    let tree = Tree::new();
    policies::policies(&tree);
    tree.set(GPU, "cur_freq", "200000000");
    tree.set(GPU, "max_freq", "600000000");
    let reads = Arc::new(AtomicUsize::new(0));
    let clock = ManualClock::new();
    let c = Controller::with_clock(
        Box::new(CountReads { inner: linux::Sysfs(tree.0.clone()), reads: reads.clone() }),
        Config::parse(""),
        Some(GPU),
        tree.0.join("saved"),
        clock.source(),
    );
    let host = crate::ui::host::Host::fixture_with_clocks(Handle::fixture(c));
    let mut ui = crate::ui::Ui::new(crate::wave::Source::Hw { shared: Default::default(), seen: 0 }, host);
    let mut scpi = crate::scpi::Scpi::open(0, false);
    let canvas = crate::gfx::Canvas::new(1, 1);
    let mut query = |ui: &mut crate::ui::Ui| String::from_utf8(scpi.exec_line(ui, &canvas, ":SYSTem:STATe?")).unwrap();
    let before = reads.load(Ordering::Relaxed);
    for _ in 0..100 {
        let text = query(&mut ui);
        assert!(text.contains("gpu_clock current_hz=200000000 min_hz=400000000 max_hz=600000000"), "{text}");
    }
    assert_eq!(reads.load(Ordering::Relaxed), before, "queries must not read the clock port");
    ui.step(0.0);
    let text = query(&mut ui);
    assert!(text.contains("a72 engaged reason none min_khz=1416000 gpu_floor engaged reason none"), "{text}");
    tree.set(GPU, "cur_freq", "400000000");
    tree.set(GPU, "max_freq", "400000000");
    tree.set(CPU, "scaling_min_freq", "816000");
    let before = reads.load(Ordering::Relaxed);
    clock.advance(thermal::SAMPLE_PERIOD - Duration::from_nanos(1));
    ui.step(0.0);
    for _ in 0..100 {
        assert!(query(&mut ui).contains("current_hz=200000000"));
    }
    assert_eq!(reads.load(Ordering::Relaxed), before, "no resampling before the deadline");
    clock.advance(Duration::from_nanos(1));
    assert!(query(&mut ui).contains("current_hz=200000000"));
    assert_eq!(reads.load(Ordering::Relaxed), before, "a due sample still requires an update, not a query");
    ui.step(0.0);
    assert!(reads.load(Ordering::Relaxed) > before, "a due update samples through the port");
    let before = reads.load(Ordering::Relaxed);
    for _ in 0..100 {
        let text = query(&mut ui);
        assert!(text.contains("gpu_clock current_hz=400000000 min_hz=600000000 max_hz=400000000"), "{text}");
        assert!(text.contains("a72 engaged reason none min_khz=816000"), "{text}");
    }
    assert_eq!(reads.load(Ordering::Relaxed), before);
    ui.m.run = crate::model::RunState::Stop;
    ui.step(0.0);
    let text = query(&mut ui);
    assert!(text.contains("a72 disabled reason idle min_khz=408000 gpu_floor disabled reason idle"), "{text}");
    assert!(text.contains("min_hz=400000000"), "{text}");
}

#[test]
fn status_uses_its_owned_snapshot_while_the_controller_is_locked() {
    let tree = Tree::new();
    let c = Handle::fixture(tree.controller(""));
    c.update(true);
    let expected = c.status().to_string();
    let _held = oos_hal::sync::lock(&c.controller);
    for _ in 0..100 {
        assert_eq!(c.status().to_string(), expected);
    }
    assert!(expected.contains("a72 engaged reason none min_khz=1416000"), "{expected}");
}

#[test]
fn cpu_minima_remain_readable_when_floor_controls_are_disabled() {
    let tree = Tree::new();
    policies::policies(&tree);
    tree.set(CPU, "scaling_min_freq", "816000");
    let c = Handle::fixture(tree.controller("perf.cpu_floor = off\nperf.gpu_floor = off"));
    for running in [false, true, false] {
        c.update(running);
        let text = c.status().to_string();
        assert!(text.contains("a53 disabled reason config-off min_khz=408000"), "{text}");
        assert!(text.contains("a72 disabled reason config-off min_khz=816000"), "{text}");
        assert_eq!(tree.minima(), ("816000".into(), "400000000".into()));
        assert!(!tree.0.join("saved").exists());
    }
}

#[test]
fn cpu_minimum_sampling_does_not_require_available_frequency_metadata() {
    for (policy, label, other) in [(policies::A53, "a53", "a72"), (CPU, "a72", "a53")] {
        let tree = Tree::new();
        policies::policies(&tree);
        std::fs::remove_file(tree.0.join(policy).join("scaling_available_frequencies")).unwrap();
        let mut c = tree.controller("");
        let text = c.status().to_string();
        assert!(text.contains(&format!("{label} disabled reason device-unavailable min_khz=408000")), "{text}");
        c.update(true);
        let text = c.status().to_string();
        assert!(text.contains(&format!("{label} disabled reason device-unavailable min_khz=408000")), "{text}");
        assert!(text.contains(&format!("{other} engaged reason none min_khz=")), "{text}");
        let receipt = std::fs::read_to_string(tree.0.join("saved")).unwrap();
        assert!(!receipt.contains(&format!("cpu-{label}")), "{receipt}");
    }
}

#[test]
fn absent_cpu_policy_reports_unknown_independently_of_other_policy() {
    for (policy, label, other) in [(policies::A53, "a53", "a72"), (CPU, "a72", "a53")] {
        let tree = Tree::new();
        policies::policies(&tree);
        std::fs::remove_dir_all(tree.0.join(policy)).unwrap();
        let mut c = tree.controller("");
        c.update(true);
        let text = c.status().to_string();
        assert!(text.contains(&format!("{label} disabled reason device-unavailable min_khz=unknown")), "{text}");
        assert!(text.contains(&format!("{other} engaged reason none min_khz=")), "{text}");
        assert!(!tree.0.join(policy).exists());
    }
}

#[test]
fn disabled_cpu_samples_refresh_at_the_deadline_and_clear_failed_readings() {
    let tree = Tree::new();
    policies::policies(&tree);
    let clock = ManualClock::new();
    let reads = Arc::new(AtomicUsize::new(0));
    let c = Handle::fixture(Controller::with_clock(
        Box::new(CountReads { inner: linux::Sysfs(tree.0.clone()), reads: reads.clone() }),
        Config::parse("perf.cpu_floor = off\nperf.gpu_floor = off"),
        Some(GPU),
        tree.0.join("saved"),
        clock.source(),
    ));
    let initial = c.status().to_string();
    for (policy, label) in [(policies::A53, "a53"), (CPU, "a72")] {
        for value in [Some("816000"), Some("bad"), None, Some("408000")] {
            match value {
                Some(value) => tree.set(policy, "scaling_min_freq", value),
                None => {
                    std::fs::remove_file(tree.0.join(policy).join("scaling_min_freq")).unwrap();
                    std::fs::create_dir(tree.0.join(policy).join("scaling_min_freq")).unwrap();
                }
            }
            let snapshot = c.status().to_string();
            let before = reads.load(Ordering::Relaxed);
            clock.advance(thermal::SAMPLE_PERIOD - Duration::from_nanos(1));
            c.poll();
            assert_eq!(c.status().to_string(), snapshot);
            assert_eq!(reads.load(Ordering::Relaxed), before);
            clock.advance(Duration::from_nanos(1));
            assert_eq!(c.status().to_string(), snapshot);
            c.poll();
            assert_eq!(reads.load(Ordering::Relaxed), before + 5);
            let text = c.status().to_string();
            let expected = value.filter(|value| *value != "bad").unwrap_or("unknown");
            assert!(text.contains(&format!("{label} disabled reason config-off min_khz={expected}")), "{text}");
            if value.is_none() {
                std::fs::remove_dir(tree.0.join(policy).join("scaling_min_freq")).unwrap();
            }
        }
    }
    assert_eq!(c.status().to_string(), initial);
}
