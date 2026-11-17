// SPDX-License-Identifier: GPL-2.0-or-later
//! Lock release runs apart from subprocess-spawning tests: a concurrent spawn can inherit the locked descriptor
//! until exec, keeping the flock alive after the parent's descriptor is dropped, even with close-on-exec set.

#[test]
#[cfg(unix)] // the lock is flock(2): the scope only
fn busy_follows_the_lock() {
    use oos_hal::update::{busy, lock};

    let dir = std::env::temp_dir().join(format!("oos-update-lock-test-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let state = dir.join("state");
    assert!(!busy(&state), "no lock file yet");
    let held = lock(&state).unwrap();
    assert!(busy(&state));
    drop(held);
    assert!(!busy(&state));
    std::fs::remove_dir_all(dir).unwrap();
}
