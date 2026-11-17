// SPDX-License-Identifier: GPL-2.0-or-later
//! A helper must not hold up process exit, including after SIGKILL.

use std::process::Child;
use std::time::{Duration, Instant};

pub(super) fn stop(child: &mut Child, deadline: Instant) -> bool {
    if wait(child, deadline) {
        return true;
    }
    eprintln!("oos-ui: GPU helper shutdown deadline; killing helper");
    if let Err(e) = child.kill() {
        eprintln!("oos-ui: GPU helper kill: {}", e);
    }
    // Reaping is bounded too. Never use blocking wait after a kill request.
    wait(child, Instant::now() + Duration::from_millis(100))
}

fn wait(child: &mut Child, deadline: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(e) => {
                eprintln!("oos-ui: GPU helper status: {}", e);
                return false;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncooperative_helper_is_killed_within_bound() {
        let mut child = std::process::Command::new("sh").args(["-c", "exec sleep 60"]).spawn().unwrap();
        // An already-expired grace period deterministically requests a kill. Reaping is an event.
        let _ = stop(&mut child, Instant::now());
        assert!(!child.wait().unwrap().success());
    }
    #[test]
    #[ignore = "perf-guard latency: release, OOS_PERF_TESTS=1, one test thread"]
    fn perf_guard_uncooperative_helper_stop() {
        crate::perfguard::latency::check(Duration::from_millis(500), uncooperative_helper_is_killed_within_bound);
    }
}
