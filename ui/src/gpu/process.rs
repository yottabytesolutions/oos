// SPDX-License-Identifier: GPL-2.0-or-later
//! A helper must not hold up process exit, including after SIGKILL.

use std::process::Child;
use std::time::{Duration, Instant};

pub(super) fn stop(child: &mut Child, deadline: Instant) -> bool {
    stop_with(child, deadline, wait, Child::kill, Instant::now)
}

fn stop_with<C>(
    child: &mut C,
    deadline: Instant,
    mut wait: impl FnMut(&mut C, Instant) -> bool,
    kill: impl FnOnce(&mut C) -> std::io::Result<()>,
    now: impl FnOnce() -> Instant,
) -> bool {
    if wait(child, deadline) {
        return true;
    }
    eprintln!("oos-ui: GPU helper shutdown deadline; killing helper");
    if let Err(e) = kill(child) {
        eprintln!("oos-ui: GPU helper kill: {}", e);
    }
    // Reaping is bounded too. Never use blocking wait after a kill request.
    wait(child, now() + Duration::from_millis(100))
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
    fn uncooperative_helper_is_killed_at_the_deadline() {
        let mut child = std::process::Command::new("cat").stdin(std::process::Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        // An already-expired grace period deterministically requests a kill. Reaping is an event.
        let _ = stop(&mut child, Instant::now());
        drop(stdin);
        assert!(!child.wait().unwrap().success());
    }

    #[test]
    fn unreaped_helper_returns_after_one_bounded_reap_attempt() {
        let start = Instant::now();
        let killed_at = start + Duration::from_secs(5);
        let mut polls = Vec::new();
        assert!(!stop_with(
            &mut false,
            start,
            |killed, deadline| {
                polls.push((*killed, deadline));
                false
            },
            |killed| {
                *killed = true;
                Ok(())
            },
            || killed_at,
        ));
        assert_eq!(polls, [(false, start), (true, killed_at + Duration::from_millis(100))]);
    }
}
