// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn assert_minima(tree: &Tree, cpu: &str, gpu: &str) {
    assert_eq!(tree.minima(), (cpu.into(), gpu.into()));
}

#[test]
fn critical_only_gpu_zone_with_both_devices_bound_to_cpu_zone() {
    let tree = Tree::new();
    tree.unbind(GPU_ZONE, 1);
    tree.set(GPU_ZONE, "trip_point_0_type", "critical");
    tree.set(GPU_ZONE, "trip_point_0_temp", "115000");
    tree.set(CPU_ZONE, "trip_point_1_type", "passive");
    tree.set(CPU_ZONE, "trip_point_1_temp", "85000");
    tree.bind(CPU_ZONE, 1, 5, "devfreq-ff9a0000.gpu", 0);
    tree.bind(CPU_ZONE, 2, 5, "devfreq-ff9a0000.gpu", 1);
    tree.bind(CPU_ZONE, 3, 8, "cpufreq-cpu0", 0);
    tree.bind(CPU_ZONE, 5, 9, "cpufreq-cpu4", 1);

    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);
    controller.update(true);
    assert_minima(&tree, "1416000", "600000000");
    for (temp, cpu, gpu) in
        [("70000", "408000", "400000000"), ("60000", "408000", "400000000"), ("59999", "1416000", "600000000")]
    {
        tree.set(CPU_ZONE, "temp", temp);
        clock.advance(Duration::from_secs(1));
        controller.update(true);
        assert_minima(&tree, cpu, gpu);
    }
    for (temp, cpu, gpu, reason) in [
        ("109999", "1416000", "600000000", "none"),
        ("110000", "408000", "400000000", "thermal"),
        ("105000", "408000", "400000000", "thermal"),
        ("104999", "1416000", "600000000", "none"),
        ("bad", "408000", "400000000", "sensor_error"),
    ] {
        tree.set(GPU_ZONE, "temp", temp);
        clock.advance(Duration::from_secs(1));
        controller.update(true);
        assert_minima(&tree, cpu, gpu);
        assert_eq!(status_text(&controller).matches(&format!("reason={reason}")).count(), 2);
        assert_eq!(tree.0.join("saved").exists(), reason == "none");
    }
    controller.update(false);
    assert_minima(&tree, "408000", "400000000");
}

#[test]
fn unreadable_critical_trip_disables_both_floors() {
    let tree = Tree::new();
    tree.set(GPU_ZONE, "trip_point_3_type", "critical");
    tree.set(GPU_ZONE, "trip_point_3_temp", "unreadable");
    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);
    controller.update(true);
    assert_minima(&tree, "408000", "400000000");
    assert_eq!(status_text(&controller).matches("reason=sensor_error").count(), 2);
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn separate_zones_drop_only_the_bound_floor_and_keep_only_its_active_peer_in_receipt() {
    let tree = Tree::new();
    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);
    controller.update(true);
    assert_minima(&tree, "1416000", "600000000");
    for (temp, gpu, reason) in [
        ("65000", "400000000", "thermal"),
        ("62000", "400000000", "thermal"),
        ("59999", "600000000", "none"),
        ("bad", "400000000", "sensor_error"),
    ] {
        tree.set(GPU_ZONE, "temp", temp);
        let previous = tree.minima();
        clock.advance(Duration::from_secs(1) - Duration::from_nanos(1));
        controller.update(true);
        assert_eq!(tree.minima(), previous);
        clock.advance(Duration::from_nanos(1));
        controller.update(true);
        assert_minima(&tree, "1416000", gpu);
        let receipt = if reason == "none" { "cpu 408000\ngpu 400000000\n" } else { "cpu 408000\n" };
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), receipt);
        assert!(status_text(&controller).contains(&format!("clock.gpu.reason={reason}")));
    }
}

#[test]
fn only_the_trip_bound_to_the_target_device_can_protect_it() {
    let tree = Tree::new();
    tree.unbind(CPU_ZONE, 4);
    tree.bind(CPU_ZONE, 0, 8, "cpufreq-cpu0", 0);
    tree.bind(CPU_ZONE, 1, 6, "devfreq-another.gpu", 0);
    tree.set(GPU_ZONE, "type", "board-stage");
    tree.set(GPU_ZONE, "trip_point_1_type", "active");
    tree.set(GPU_ZONE, "trip_point_1_temp", "85000");
    tree.unbind(GPU_ZONE, 1);
    tree.bind(GPU_ZONE, 1, 5, "devfreq-ff9a0000.gpu", 1);
    tree.set(GPU_ZONE, "temp", "70000");
    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);
    controller.update(true);
    assert_minima(&tree, "408000", "600000000");
    assert!(status_text(&controller).contains("clock.cpu.reason=no_cooling_binding"));
    tree.set(GPU_ZONE, "temp", "80000");
    clock.advance(Duration::from_secs(1));
    controller.update(true);
    assert_minima(&tree, "408000", "400000000");
    assert!(!tree.0.join("saved").exists());
}

#[test]
fn no_binding_or_critical_only_binding_leaves_floors_off() {
    let tree = Tree::new();
    tree.unbind(CPU_ZONE, 4);
    tree.unbind(GPU_ZONE, 1);
    for critical in [false, true] {
        if critical {
            tree.set(GPU_ZONE, "trip_point_0_type", "critical");
            tree.bind(GPU_ZONE, 1, 5, "devfreq-ff9a0000.gpu", 0);
        }
        let mut controller = tree.controller("");
        controller.update(true);
        assert_minima(&tree, "408000", "400000000");
        assert_eq!(status_text(&controller).matches("reason=no_cooling_binding").count(), 2);
        assert!(!tree.0.join("saved").exists());
    }
}

#[test]
fn failed_thermal_release_is_visible_and_retried_before_rearming() {
    let tree = Tree::new();
    let failures = Arc::new(Mutex::new(0));
    let port = Failing { inner: linux::Sysfs(tree.0.clone()), failures: failures.clone() };
    let clock = ManualClock::new();
    let mut controller =
        Controller::with_clock(Box::new(port), Config::parse(""), Some(GPU), tree.0.join("saved"), clock.source());
    controller.update(true);
    *oos_hal::sync::lock(&failures) = 2;
    tree.set(CPU_ZONE, "temp", "65000");
    clock.advance(Duration::from_secs(1));
    controller.update(true);
    assert_minima(&tree, "1416000", "600000000");
    assert!(status_text(&controller)
        .contains("clock.cpu.applied=1 clock.cpu.current_hz=816000000 clock.cpu.reason=restore_failed"));

    tree.set(CPU_ZONE, "temp", "59999");
    clock.advance(Duration::from_secs(1));
    controller.update(true);
    assert!(status_text(&controller).contains("clock.cpu.reason=restore_failed"));
    clock.advance(Duration::from_secs(1));
    controller.update(true);
    assert_minima(&tree, "1416000", "600000000");
    assert!(status_text(&controller).contains("clock.cpu.reason=none"));
    assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), "cpu 408000\ngpu 400000000\n");
}

#[test]
fn unreadable_bound_trip_disables_only_its_device() {
    let tree = Tree::new();
    tree.set(GPU_ZONE, "cdev1_trip_point", "unreadable");
    let mut controller = tree.controller("");
    controller.update(true);
    assert_minima(&tree, "1416000", "400000000");
    assert!(status_text(&controller).contains("clock.gpu.reason=cooling_binding_error"));
}

#[test]
fn state_tracks_read_back_minimum_and_tolerates_missing_current_frequency() {
    let tree = Tree::new();
    let mut controller = tree.controller("perf.gpu_floor = off");
    controller.update(true);
    tree.set(CPU, "scaling_min_freq", "816000");
    std::fs::remove_file(tree.0.join(GPU).join("cur_freq")).unwrap();
    let state = status_text(&controller);
    assert!(state.contains("clock.cpu.applied=0 clock.cpu.current_hz=816000000 clock.cpu.reason=minimum_changed"));
    assert!(state.contains("clock.gpu.applied=0 clock.gpu.current_hz=unknown clock.gpu.reason=disabled"));
}
