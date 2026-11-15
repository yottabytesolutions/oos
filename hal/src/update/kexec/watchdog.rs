// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Transfer the single-open watchdog from its supervisor, without ever magic-closing it.

trait Handoff {
    fn take(&mut self) -> Result<(), String>;
    fn ping(&mut self) -> Result<(), String>;
    fn restore(&mut self);
}

fn jump<W: Handoff>(watchdog: &mut W, marker: impl FnOnce(), reboot: impl FnOnce(&mut W) -> String) -> String {
    let result = match watchdog.take() {
        Ok(()) => {
            marker();
            reboot(watchdog)
        }
        Err(e) => e,
    };
    // A returned syscall or any preparation error closes our handle before the supervisor resumes.
    watchdog.restore();
    result
}

mod device;
#[cfg(target_os = "linux")]
mod native;
#[cfg(test)]
mod tests;

#[cfg(target_os = "linux")]
pub(super) fn preflight() -> Result<(), String> {
    let device = device::Device::new(native::Native);
    // Recovery is separate from the read-only check, before any teardown and under the update lock.
    device.recover_stale_request()?;
    device.preflight().map(|_| ())
}

fn reboot(watchdog: &mut impl Handoff, sync: impl FnOnce(), syscall: impl FnOnce() -> String) -> String {
    sync();
    match watchdog.ping() {
        Ok(()) => syscall(),
        Err(e) => e,
    }
}

#[cfg(target_os = "linux")]
pub(super) fn start(marker: impl FnOnce()) -> String {
    jump(&mut device::Device::new(native::Native), marker, |watchdog| {
        reboot(
            watchdog,
            || {
                // SAFETY: sync has no arguments; retain a final flush after all jump diagnostics, before the ping.
                unsafe { libc::sync() };
            },
            || {
                // SAFETY: the caller loaded the image and quiesced devices; the final ping immediately precedes this.
                let rc = unsafe { libc::reboot(libc::LINUX_REBOOT_CMD_KEXEC) };
                format!("reboot(KEXEC) returned {}: {}", rc, std::io::Error::last_os_error())
            },
        )
    })
}

#[cfg(test)]
mod ordering_tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake<'a> {
        calls: &'a RefCell<Vec<&'static str>>,
        fail: &'static str,
    }

    impl Handoff for Fake<'_> {
        fn take(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("take");
            if self.fail == "take" {
                Err("take failed".into())
            } else {
                Ok(())
            }
        }
        fn ping(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("ping");
            if self.fail == "ping" {
                Err("ping failed".into())
            } else {
                Ok(())
            }
        }
        fn restore(&mut self) {
            self.calls.borrow_mut().push("close then resume");
        }
    }

    #[test]
    fn both_paths_ping_after_diagnostics_and_restore_on_return() {
        for changed_tree in [false, true] {
            let calls = RefCell::new(vec![]);
            let mut fake = Fake { calls: &calls, fail: "" };
            let result = jump(
                &mut fake,
                || {
                    if changed_tree {
                        calls.borrow_mut().push("jump marker");
                    }
                },
                |watchdog| {
                    reboot(
                        watchdog,
                        || calls.borrow_mut().push("sync"),
                        || {
                            calls.borrow_mut().push("reboot");
                            "returned".into()
                        },
                    )
                },
            );
            let expected = if changed_tree {
                vec!["take", "jump marker", "sync", "ping", "reboot", "close then resume"]
            } else {
                vec!["take", "sync", "ping", "reboot", "close then resume"]
            };
            assert_eq!(*calls.borrow(), expected);
            assert_eq!(result, "returned");
        }
    }

    #[test]
    fn preparation_failure_never_jumps_and_always_restores() {
        for fail in ["take", "ping"] {
            let calls = RefCell::new(vec![]);
            let mut fake = Fake { calls: &calls, fail };
            let result = jump(
                &mut fake,
                || calls.borrow_mut().push("marker"),
                |watchdog| reboot(watchdog, || calls.borrow_mut().push("sync"), || panic!("must not reboot")),
            );
            assert_eq!(result, format!("{fail} failed"));
            assert_eq!(calls.borrow().last(), Some(&"close then resume"));
            assert_eq!(calls.borrow().contains(&"ping"), fail == "ping");
        }
    }
}
