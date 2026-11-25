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
fn absent_device_does_not_block_startup_recovery_or_the_other_floor() {
    for (absent, present, minimum, raised, saved) in
        [(GPU, CPU, "scaling_min_freq", "1416000", "408000"), (CPU, GPU, "min_freq", "600000000", "400000000")]
    {
        let tree = Tree::new();
        let receipt = tree.0.join("saved");
        std::fs::write(&receipt, "gpu 400000000\ncpu 408000\n").unwrap();
        std::fs::remove_dir_all(tree.0.join(absent)).unwrap();
        tree.set(present, minimum, raised);

        let mut controller = tree.controller("");
        let read_minimum = || std::fs::read_to_string(tree.0.join(present).join(minimum)).unwrap();
        assert_eq!(read_minimum(), saved);
        assert!(!receipt.exists());
        assert!(!tree.0.join(absent).exists());
        controller.update(true);
        assert_eq!(read_minimum(), raised);
        let state = status_text(&controller);
        let (present, absent) = if absent == GPU { ("cpu", "gpu") } else { ("gpu", "cpu") };
        assert_eq!(std::fs::read_to_string(&receipt).unwrap(), format!("{present} {saved}\n"));
        assert!(state.contains(&format!("clock.{absent}.reason=absent")));
        controller.restore();
        assert_eq!(read_minimum(), saved);
        assert!(!receipt.exists());
    }
}

#[test]
fn existing_gpu_with_unreadable_frequencies_keeps_recovery_pending() {
    let tree = Tree::new();
    let receipt = tree.0.join("saved");
    let contents = "gpu 400000000\ncpu 408000\n";
    std::fs::write(&receipt, contents).unwrap();
    std::fs::remove_file(tree.0.join(GPU).join("available_frequencies")).unwrap();
    tree.set(CPU, "scaling_min_freq", "1416000");
    tree.set(GPU, "min_freq", "600000000");
    let clock = ManualClock::new();
    let mut controller = tree.controller_with_clock(&clock);

    for _ in 0..2 {
        controller.update(true);
        assert_eq!(tree.minima(), ("408000".into(), "600000000".into()));
        assert_eq!(std::fs::read_to_string(&receipt).unwrap(), contents);
        assert!(status_text(&controller).contains("clock.gpu.reason=restore_failed"));
        clock.advance(SAMPLE_PERIOD);
    }
    tree.set(GPU, "available_frequencies", "400000000 600000000");
    controller.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    assert!(!receipt.exists());
    assert!(status_text(&controller).contains("clock.cpu.reason=recovery_failed"));
}

#[test]
fn failed_journal_write_never_raises_and_invalid_receipt_is_kept() {
    let tree = Tree::new();
    std::fs::create_dir(tree.0.join("saved.new")).unwrap();
    let mut c = tree.controller("");
    c.update(true);
    assert_eq!(tree.minima(), ("408000".into(), "400000000".into()));
    drop(c);
    for text in
        ["", "cpu bad\n", "cpu 408000\ncpu 816000\n", "other 1\n", "CPU 408000\n", "cpu\n", "gpu 400000000 extra\n"]
    {
        std::fs::write(tree.0.join("saved"), text).unwrap();
        let mut c = tree.controller("");
        c.update(true);
        drop(c);
        assert_eq!(std::fs::read_to_string(tree.0.join("saved")).unwrap(), text);
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
