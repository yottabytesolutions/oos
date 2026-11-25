// SPDX-License-Identifier: GPL-2.0-or-later

/// Load the prepared payload and start it. The running recovery image survives a failed start.
#[cfg(target_os = "linux")]
pub(crate) fn start_kexec(sp: &std::path::Path, s: oos_hal::update::Staged) {
    use super::messages;
    use crate::{notice, quiesce, restore, stop_watcher, tell, UI_LOOP};
    use oos_hal::system::DataShutdown;
    use oos_hal::update::{self, kexec};

    let short = update::short(&s.build).to_string();
    let payload = s.payload_size();
    let unload = || kexec::undo::warn("unload kexec image", kexec::unload());
    let clear = |why: &str| {
        let event = format!("kexec start of build {short} failed ({why}): nothing installed");
        kexec::undo::warn("clear kexec state", update::kexec_clear(sp, &event));
    };
    let fail = |why: &str| -> ! {
        notice(&messages::failed_before_start(&short, why));
        std::process::exit(1)
    };
    let undo_loading = |why: String| -> ! {
        unload();
        clear(&why);
        fail(&why)
    };
    let cmd = s.cmdline(&kexec::running_cmdline());
    let loaded = match &s.tree {
        Some(t) => kexec::load_with_tree(&s.kernel, &s.initrd, t, &cmd),
        None => kexec::load(&s.kernel, &s.initrd, &cmd),
    };
    if let Err(e) = loaded {
        undo_loading(e);
    }
    let supplied_tree = s.tree.is_some();
    drop(s);
    tell(&format!("build {short} loaded ({payload} bytes payload). Restarting into it by kexec."));
    // under the update lock: a write of the start-up watcher (a rollback, the persist arm) finishes first, then the
    // watcher is stopped, so no card write runs into the restart
    let lock = kexec::lock_for_start(sp).unwrap_or_else(|why| undo_loading(why));
    if let Err(why) = stop_ui() {
        drop(lock);
        undo_loading(why);
    }
    stop_watcher();
    let q = quiesce();
    // The writable lock descriptor must close before unmount or read-only remount.
    let jump = supplied_tree.then(kexec::prepare_jump);
    drop(lock);
    let data = DataShutdown::prepare();
    let why = attempt_kexec(
        data,
        || jump.map_or_else(kexec::start, |j| j.start()),
        |data, why| {
            unload();
            data.restore(why)
        },
        clear,
        || restore(&q),
        || kexec::undo::remove_marker("/tmp/oos-ui.off"),
        || kexec::undo::resume_ui(UI_LOOP),
    );
    fail(&why);
}

/// A returned kexec needs writable data for the update state and the next UI.
pub(crate) fn attempt_kexec<T>(
    data: T,
    jump: impl FnOnce() -> String,
    restore_data: impl FnOnce(T, String) -> String,
    clear_kexec: impl FnOnce(&str),
    restore_devices: impl FnOnce(),
    remove_stop_marker: impl FnOnce(),
    resume_ui: impl FnOnce(),
) -> String {
    let why = jump();
    let why = restore_data(data, why);
    clear_kexec(&why);
    restore_devices();
    remove_stop_marker();
    resume_ui();
    why
}

/// rcS is waiting for this call and starts the UI after recovery and the notice.
pub(crate) fn restart<T>(
    prepare: impl FnOnce() -> T,
    reboot: impl FnOnce() -> String,
    restore: impl FnOnce(T, String) -> String,
    notice: impl FnOnce(&str),
) {
    let data = prepare();
    let why = reboot();
    notice(&restore(data, why));
}

#[cfg(target_os = "linux")]
fn stop_ui() -> Result<(), String> {
    use crate::{ui_pid, UI_LOOP};
    use oos_hal::update::kexec::undo;

    const STOP: &str = "/tmp/oos-ui.off";
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(STOP)
        .map_err(|error| format!("cannot inhibit the UI supervisor: {error}"))?;
    if stop_ui_with(
        || ui_pid(None),
        |pid, signal| match oos_hal::system::signal_process(pid as i32, signal) {
            Err(error) if error.raw_os_error() != Some(libc::ESRCH) => {
                undo::warn("signal UI", Err::<(), _>(error));
            }
            _ => {}
        },
        || std::thread::sleep(std::time::Duration::from_millis(100)),
    ) {
        return Ok(());
    }
    undo::remove_marker(STOP);
    undo::resume_ui(UI_LOOP);
    Err("the UI did not stop after TERM and KILL".into())
}

fn stop_ui_with(
    mut pid: impl FnMut() -> Option<u32>,
    mut signal: impl FnMut(u32, i32),
    mut pause: impl FnMut(),
) -> bool {
    for attempt in 0..50 {
        let Some(pid) = pid() else { return true };
        signal(pid, if attempt < 30 { libc::SIGTERM } else { libc::SIGKILL });
        pause();
    }
    pid().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn returned_syscall_restores_data_clears_state_and_unblocks_the_supervisor_in_order() {
        let calls = RefCell::new(Vec::new());
        let result = attempt_kexec(
            "mount",
            || {
                calls.borrow_mut().push("reboot returned");
                "reboot returned".into()
            },
            |data, why| {
                assert_eq!(data, "mount");
                calls.borrow_mut().push("restore data");
                format!("{why}; restore /oos failed")
            },
            |why| {
                assert_eq!(why, "reboot returned; restore /oos failed");
                calls.borrow_mut().push("clear kexec mark");
            },
            || calls.borrow_mut().push("restore display and links"),
            || calls.borrow_mut().push("remove stop marker"),
            || calls.borrow_mut().push("resume UI"),
        );
        assert_eq!(result, "reboot returned; restore /oos failed");
        assert_eq!(
            *calls.borrow(),
            [
                "reboot returned",
                "restore data",
                "clear kexec mark",
                "restore display and links",
                "remove stop marker",
                "resume UI",
            ]
        );
    }

    #[test]
    fn trial_restart_restores_and_reports_before_returning_to_rcs() {
        for (restore_error, expected) in [(false, "reboot refused"), (true, "reboot refused; restore /oos failed")] {
            let calls = RefCell::new(Vec::new());
            restart(
                || {
                    calls.borrow_mut().push("close data");
                    "mount"
                },
                || {
                    calls.borrow_mut().push("restart");
                    "reboot refused".into()
                },
                |data, mut why| {
                    assert_eq!(data, "mount");
                    calls.borrow_mut().push("restore data");
                    if restore_error {
                        why.push_str("; restore /oos failed");
                    }
                    why
                },
                |why| {
                    assert_eq!(why, expected);
                    calls.borrow_mut().push("notice");
                },
            );
            assert_eq!(*calls.borrow(), ["close data", "restart", "restore data", "notice"]);
        }
    }

    #[test]
    fn a_ui_that_survives_the_bounded_stop_cannot_begin_data_cleanup() {
        let mut signals = Vec::new();
        assert!(!stop_ui_with(|| Some(42), |_, signal| signals.push(signal), || {}));
        assert_eq!(signals[..30], [libc::SIGTERM; 30]);
        assert_eq!(signals[30..], [libc::SIGKILL; 20]);
    }

    #[test]
    fn stop_ui_escalates_to_sigkill_and_stops_polling_when_ui_exits() {
        let mut polls = 0;
        let mut signals = Vec::new();
        let mut pauses = 0;
        assert!(stop_ui_with(
            || {
                polls += 1;
                (polls <= 31).then_some(42)
            },
            |pid, signal| signals.push((pid, signal)),
            || pauses += 1,
        ));
        assert_eq!((polls, pauses), (32, 31));
        assert_eq!(signals[..30], [(42, libc::SIGTERM); 30]);
        assert_eq!(signals[30..], [(42, libc::SIGKILL)]);
    }
}
