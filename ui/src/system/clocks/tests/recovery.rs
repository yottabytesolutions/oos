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
    std::fs::write(tree.0.join("saved"), "cpu 408000\ngpu 400000000\n").unwrap();
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
    for text in ["", "cpu bad\n", "cpu 408000\ncpu 816000\n", "other 1\n"] {
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
    let c = Handle::fixture(tree.controller(""));
    c.update(true);
    assert_eq!(tree.minima().0, "1416000");
    for (temp, raised) in [("65000", false), ("62000", false), ("59999", true), ("bad", false)] {
        tree.set("class/thermal/thermal_zone2", "temp", temp);
        std::thread::sleep(std::time::Duration::from_millis(1010));
        c.poll();
        assert_eq!(tree.minima().0, if raised { "1416000" } else { "408000" });
        assert_eq!(tree.0.join("saved").exists(), raised);
    }
}

#[test]
fn missing_passive_trip_or_zone_disables_floors() {
    for file in ["thermal_zone2/type", "thermal_zone7/trip_point_0_type"] {
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
    let _held = oos_hal::sync::lock(&handle.0);
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
    assert_eq!(text.matches("device=cpu absent; disabled").count(), 1);
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
