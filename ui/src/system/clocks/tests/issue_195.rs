// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn rk3399(tree: &Tree) {
    policies::policies(tree);
    tree.set("class/thermal/thermal_zone7", "trip_point_0_type", "passive\n");
    tree.set("class/thermal/thermal_zone7", "trip_point_1_type", "passive\n");
    tree.set("class/thermal/thermal_zone7", "trip_point_1_temp", "80000\n");
    tree.set("class/thermal/thermal_zone2", "trip_point_0_type", "critical\n");
    tree.set("class/thermal/thermal_zone2", "trip_point_0_temp", "95000\n");
}

#[test]
fn cpu_passive_trips_protect_both_run_floors_on_rk3399() {
    let tree = Tree::new();
    rk3399(&tree);
    let clock = ManualClock::new();
    let mut c = tree.controller_with_clock(&clock);
    c.update(true);
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    for (temperature, raised) in [("65000", false), ("60000", false), ("59999", true), ("bad", false)] {
        tree.set("class/thermal/thermal_zone2", "temp", temperature);
        clock.advance(Duration::from_secs(1));
        c.update(true);
        let expected = if raised { ("1416000", "600000000") } else { ("408000", "400000000") };
        assert_eq!(tree.minima(), (expected.0.into(), expected.1.into()));
    }
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn no_passive_trips_in_either_zone_keeps_both_floors_disabled() {
    let tree = Tree::new();
    rk3399(&tree);
    for index in 0..=1 {
        tree.set("class/thermal/thermal_zone7", &format!("trip_point_{index}_type"), "critical");
    }
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn lowest_passive_trip_is_shared_independent_of_zone_or_trip_order() {
    let tree = Tree::new();
    rk3399(&tree);
    tree.set("class/thermal/thermal_zone7", "trip_point_0_temp", "80000");
    tree.set("class/thermal/thermal_zone7", "trip_point_1_temp", "70000");
    tree.set("class/thermal/thermal_zone2", "trip_point_3_type", "passive");
    tree.set("class/thermal/thermal_zone2", "trip_point_3_temp", "60000");
    tree.set("class/thermal/thermal_zone7", "temp", "55000");
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn state_query_reports_gpu_readbacks_floors_and_upload_quantiles() {
    let tree = Tree::new();
    rk3399(&tree);
    tree.set(GPU, "cur_freq", "400000000\n");
    tree.set(GPU, "max_freq", "600000000\n");
    let clock = ManualClock::new();
    let host = crate::ui::host::Host::fixture_with_clocks(Handle::fixture(tree.controller_with_clock(&clock)));
    let mut ui = crate::ui::Ui::new(crate::wave::Source::Hw { shared: Default::default(), seen: 0 }, host);
    let mut scpi = crate::scpi::Scpi::open(0, false);
    let canvas = crate::gfx::Canvas::new(1, 1);
    let query = |ui: &mut crate::ui::Ui, scpi: &mut crate::scpi::Scpi| {
        String::from_utf8(scpi.exec_line(ui, &canvas, ":SYSTem:STATe?")).unwrap()
    };
    let text = query(&mut ui, &mut scpi);
    assert!(text.contains("gpu_clock current_hz=400000000 min_hz=400000000 max_hz=600000000"), "{text}");
    assert!(text.contains("gpu_floor disabled reason idle"), "{text}");
    assert!(text.contains("gpu.upload n=0 total=0 p50_ns=pending p99_ns=pending"), "{text}");
    ui.step(0.0);
    for i in 1..=400 {
        ui.record_gpu_upload(Duration::from_nanos(i));
    }
    let text = query(&mut ui, &mut scpi);
    assert!(
        text.contains("min_hz=600000000 max_hz=600000000 cpu_floor a53 engaged reason none min_khz=1200000 a72 engaged reason none min_khz=1416000 gpu_floor engaged reason none"),
        "{text}"
    );
    assert!(text.contains("gpu.upload n=256 total=400 p50_ns=272 p99_ns=398 max_ns=400"), "{text}");
    tree.set("class/thermal/thermal_zone2", "temp", "65000");
    clock.advance(Duration::from_secs(1));
    ui.step(0.0);
    let text = query(&mut ui, &mut scpi);
    assert!(text.contains("min_hz=400000000"), "{text}");
    assert!(text.contains("gpu_floor disabled reason thermal-limit"), "{text}");
    ui.m.run = crate::model::RunState::Stop;
    ui.step(0.0);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn state_query_distinguishes_verified_and_unverified_pending_restoration() {
    for case in ["verified-raise", "unverified-raise", "startup-recovery"] {
        let tree = Tree::new();
        let receipt = "cpu-a72 408000\n";
        if case == "startup-recovery" {
            tree.set(CPU, "scaling_min_freq", "1416000");
            std::fs::write(tree.0.join("saved"), receipt).unwrap();
        }
        let failures = Arc::new(Mutex::new(usize::from(case != "verified-raise")));
        let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
        let clock = ManualClock::new();
        let mut c = Controller::with_clock(
            Box::new(port),
            Config::parse("perf.gpu_floor = off"),
            None,
            tree.0.join("saved"),
            clock.source(),
        );
        c.update(true);
        *oos_hal::sync::lock(&failures) = 3;
        c.update(false);
        let host = crate::ui::host::Host::fixture_with_clocks(Handle::fixture(c));
        let mut ui = crate::ui::Ui::new(crate::wave::Source::Hw { shared: Default::default(), seen: 0 }, host);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let canvas = crate::gfx::Canvas::new(1, 1);
        let text = String::from_utf8(scpi.exec_line(&mut ui, &canvas, ":SYSTem:STATe?")).unwrap();
        let state = if case == "verified-raise" { "engaged" } else { "disabled" };
        assert!(text.contains(&format!("a72 {state} reason restore-pending")), "{case}: {text}");
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
    }
}

#[test]
fn state_reasons_distinguish_unavailable_off_failed_and_idle_floors() {
    let tree = Tree::new();
    let mut c = tree.controller("perf.gpu_floor = off");
    c.update(true);
    assert!(c.status().to_string().contains("gpu_floor disabled reason config-off"));
    drop(c);
    let mut c = tree.controller("");
    tree.set(GPU, "available_frequencies", "200000000 400000000");
    c.update(true);
    assert!(c.status().to_string().contains("gpu_floor disabled reason not-engaged"));
    drop(c);
    for zone in ["thermal_zone2", "thermal_zone7"] {
        tree.set(&format!("class/thermal/{zone}"), "trip_point_0_type", "critical");
    }
    let mut c = tree.controller("");
    c.update(true);
    assert!(c.status().to_string().contains("gpu_floor disabled reason thermal-unavailable:no-passive-trip"));
    drop(c);
    std::fs::remove_dir_all(tree.0.join(GPU)).unwrap();
    let c = tree.controller("");
    assert!(c.status().to_string().contains("gpu_clock current_hz=unknown min_hz=unknown max_hz=unknown"));
    assert!(c.status().to_string().contains("gpu_floor disabled reason device-unavailable"));
}

#[test]
fn failed_raise_does_not_report_an_engaged_floor() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: Arc::new(Mutex::new(1)) };
    let mut c =
        Controller::with_clock(Box::new(port), Config::parse(""), Some(GPU), tree.0.join("saved"), clock.source());
    c.update(true);
    let text = c.status().to_string();
    assert!(text.contains("a72 disabled reason not-engaged"), "{text}");
    assert!(text.contains("gpu_floor engaged reason none"), "{text}");
    let receipt = std::fs::read(tree.0.join("saved")).unwrap();
    clock.advance(thermal::SAMPLE_PERIOD);
    c.update(true);
    assert_eq!(tree.minima(), ("1416000".into(), "600000000".into()));
    assert!(c.status().to_string().contains("a72 engaged reason none"));
    assert_eq!(std::fs::read(tree.0.join("saved")).unwrap(), receipt);
    c.update(false);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
}

#[test]
fn stop_reports_idle_after_thermal_limit_or_read_error() {
    for (temperature, reason) in [("65000", "thermal-limit"), ("bad", "thermal-read-error")] {
        let tree = Tree::new();
        let clock = ManualClock::new();
        let mut c = tree.controller_with_clock(&clock);
        c.update(true);
        tree.set("class/thermal/thermal_zone2", "temp", temperature);
        clock.advance(Duration::from_secs(1));
        c.update(true);
        for label in ["a72", "gpu_floor"] {
            assert!(c.status().to_string().contains(&format!("{label} disabled reason {reason}")));
        }
        c.update(false);
        let text = c.status().to_string();
        for label in ["a72", "gpu_floor"] {
            assert!(text.contains(&format!("{label} disabled reason idle")), "{text}");
        }
        assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    }
}

#[test]
fn no_controller_status_uses_the_shared_clock_format() {
    let ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let expected = " gpu_clock current_hz=unknown min_hz=unknown max_hz=unknown cpu_floor a53 disabled reason no-controller min_khz=unknown a72 disabled reason no-controller min_khz=unknown gpu_floor disabled reason no-controller";
    assert_eq!(Status::unavailable().to_string(), expected);
    assert!(ui.lifecycle_text().ends_with(expected));
}

#[test]
fn discovery_reasons_preserve_missing_zone_and_io_details() {
    for (file, value, detail) in [
        ("type", Some("other"), "missing-cpu-gpu-zones"),
        ("type", None, "io-NotFound"),
        ("trip_point_0_temp", Some("bad"), "io-Other"),
    ] {
        let tree = Tree::new();
        if let Some(value) = value {
            tree.set("class/thermal/thermal_zone2", file, value);
        } else {
            std::fs::remove_file(tree.0.join("class/thermal/thermal_zone2").join(file)).unwrap();
        }
        let mut c = tree.controller("");
        c.update(true);
        let text = c.status().to_string();
        assert!(
            text.ends_with(&format!(
                "cpu_floor a53 disabled reason device-unavailable min_khz=unknown a72 disabled reason thermal-unavailable:{detail} min_khz=408000 gpu_floor disabled reason thermal-unavailable:{detail}"
            )),
            "{text}"
        );
        c.update(false);
        assert!(c.status().to_string().contains("gpu_floor disabled reason idle"));
    }
}

#[test]
fn gpu_readbacks_survive_disabled_floors_and_report_missing_attributes_independently() {
    let tree = Tree::new();
    tree.set(GPU, "cur_freq", "200000000\n");
    tree.set(GPU, "max_freq", "600000000\n");
    for receipt in [None, Some("other 1\n")] {
        if let Some(receipt) = receipt {
            std::fs::write(tree.0.join("saved"), receipt).unwrap();
        }
        let c = tree.controller("perf.cpu_floor = off\nperf.gpu_floor = off");
        let text = c.status().to_string();
        assert!(text.contains("gpu_clock current_hz=200000000 min_hz=400000000 max_hz=600000000"), "{text}");
        let reason = if receipt.is_some() { "recovery-failed" } else { "config-off" };
        for label in ["a53", "a72", "gpu_floor"] {
            assert!(text.contains(&format!("{label} disabled reason {reason}")), "{text}");
        }
    }
    std::fs::remove_file(tree.0.join("saved")).unwrap();
    let clock = ManualClock::new();
    let mut c = tree.controller_with_clock(&clock);
    tree.set(GPU, "cur_freq", "bad");
    clock.advance(thermal::SAMPLE_PERIOD);
    c.update(false);
    assert!(c.status().to_string().contains("gpu_clock current_hz=unknown min_hz=400000000 max_hz=600000000"));
    tree.set(GPU, "cur_freq", "200000000");
    std::fs::remove_file(tree.0.join(GPU).join("max_freq")).unwrap();
    clock.advance(thermal::SAMPLE_PERIOD);
    c.update(false);
    assert!(c.status().to_string().contains("gpu_clock current_hz=200000000 min_hz=400000000 max_hz=unknown"));
}
