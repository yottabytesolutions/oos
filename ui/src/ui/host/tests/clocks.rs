// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::system::clocks::{Config, Controller, Handle, Port};
use std::sync::{Arc, Mutex};

struct Fake(Arc<Mutex<u64>>);
impl Port for Fake {
    fn read(&self, path: &str) -> std::io::Result<String> {
        Ok(if path.ends_with("/type") {
            if path.contains("zone7") {
                "cpu-thermal".into()
            } else {
                "gpu-thermal".into()
            }
        } else if path.ends_with("_type") {
            "passive".into()
        } else if path.ends_with("_temp") {
            "70000".into()
        } else if path.ends_with("/temp") {
            "45000".into()
        } else if path.ends_with("scaling_available_frequencies") {
            "408000 1416000".into()
        } else {
            oos_hal::sync::lock(&self.0).to_string()
        })
    }
    fn entries(&self, path: &str) -> std::io::Result<Vec<String>> {
        Ok(if path == "class/thermal" {
            vec!["thermal_zone7".into(), "thermal_zone2".into()]
        } else {
            vec!["trip_point_0_type".into()]
        })
    }
    fn write(&mut self, _: &str, value: u64) -> std::io::Result<()> {
        *oos_hal::sync::lock(&self.0) = value;
        Ok(())
    }
}

fn fixture() -> (Host, Arc<Mutex<u64>>) {
    let value = Arc::new(Mutex::new(408000));
    let c = Controller::new(Box::new(Fake(value.clone())), Config::parse("perf.gpu_floor = off"), None, journal());
    (Host::fixture_with_clocks(Handle::fixture(c)), value)
}

fn journal() -> PathBuf {
    static ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "oos-host-clocks-{}-{}",
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn memory_hold_restores_floors_before_the_early_step_return() {
    let (host, min) = fixture();
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host);
    ui.step(0.04);
    assert_eq!(*oos_hal::sync::lock(&min), 1416000);
    ui.host.runtime_mut().memory = crate::system::memory::Watchdog::new(crate::system::memory::Limits::parse(
        "mem.soft_mb = 1\nmem.hard_mb = 2\n",
    ));
    ui.host.text = |path| (path == "/proc/self/status").then(|| "VmRSS: 4096 kB".into());
    ui.step(0.04);
    assert!(ui.memory_stopped());
    assert_eq!(*oos_hal::sync::lock(&min), 408000);
}

#[test]
fn demo_run_does_not_request_hardware_clock_floors() {
    let (host, min) = fixture();
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host);
    ui.step(0.04);
    assert_eq!(*oos_hal::sync::lock(&min), 408000);
}

struct RejectRestore(Fake, Arc<Mutex<usize>>);
impl Port for RejectRestore {
    fn read(&self, path: &str) -> std::io::Result<String> {
        self.0.read(path)
    }
    fn entries(&self, path: &str) -> std::io::Result<Vec<String>> {
        self.0.entries(path)
    }
    fn write(&mut self, path: &str, value: u64) -> std::io::Result<()> {
        if value == 408000 {
            *oos_hal::sync::lock(&self.1) += 1;
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        self.0.write(path, value)
    }
}

#[test]
fn memory_hold_does_not_force_restore_retries_every_frame() {
    let attempts = Arc::new(Mutex::new(0));
    let receipt = journal();
    let port = RejectRestore(Fake(Arc::new(Mutex::new(408000))), attempts.clone());
    let c = Controller::new(Box::new(port), Config::parse("perf.gpu_floor = off"), None, receipt.clone());
    let host = Host::fixture_with_clocks(Handle::fixture(c));
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host);
    ui.step(0.04);
    ui.host.runtime_mut().memory = crate::system::memory::Watchdog::new(crate::system::memory::Limits::parse(
        "mem.soft_mb = 1\nmem.hard_mb = 2\n",
    ));
    ui.host.text = |path| (path == "/proc/self/status").then(|| "VmRSS: 4096 kB".into());
    ui.step(0.04);
    for _ in 0..100 {
        ui.step(0.001);
    }
    assert!(ui.memory_stopped());
    let count = *oos_hal::sync::lock(&attempts);
    drop(ui);
    std::fs::remove_file(receipt).unwrap();
    assert_eq!(count, 1, "memory hold uses the same one-second retry cadence as polling");
}
