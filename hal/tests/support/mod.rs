// SPDX-License-Identifier: GPL-2.0-or-later
//! Wait for a child-published log, marker, or exit event. Only the hang guard reads real time.
#[allow(dead_code)] // Acquisition tests use this fixture; supervisor tests only use wait_for.
pub mod clock;
#[allow(dead_code)] // Each integration executable uses its own subset of the shared fixtures.
pub fn wait_for(event: &str, mut completed: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !completed() {
        assert!(std::time::Instant::now() < deadline, "waiting for {event}");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
