// SPDX-License-Identifier: GPL-2.0-or-later
//! A timed-out worker retains its captures until it actually finishes.

use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The main loop has ended before entering this sequence. Storage errors still
/// allow helper teardown and display release, while the final exit policy handles an explicit power request.
pub fn run<T>(
    start: Instant,
    context: &mut T,
    remote: impl FnOnce(Instant),
    workers: impl FnOnce(&mut T, Instant),
    persist: impl FnOnce(&mut T) -> std::io::Result<()>,
    helper: impl FnOnce(Instant) -> bool,
    release: impl FnOnce(),
) -> std::io::Result<()> {
    // Shared offsets prevent slow groups from each obtaining another full budget. Leave 100 ms
    // for the helper's kill/reap, and 300 ms below the updater's 2.5 s healthy-exit margin.
    let deadline = start + Duration::from_millis(2100);
    remote(start + Duration::from_millis(500));
    workers(context, start + Duration::from_millis(1000));
    let stored = persist(context);
    if helper(deadline) {
        release();
    } else {
        eprintln!("oos-ui: helper still owns scan-out buffers; leaving DRM to process exit");
    }
    stored
}

/// Resolve the final exit after teardown, with the MCU request supplied by device wiring.
pub fn finish(stored: std::io::Result<()>, exit_code: i32, power: Option<impl FnOnce()>) -> i32 {
    if let Err(e) = stored {
        eprintln!("oos-ui: shutdown save failed: {}", e);
        if power.is_none() {
            return 1;
        }
    }
    if exit_code != 0 {
        return exit_code;
    }
    if let Some(power) = power {
        power();
    }
    0
}

/// If the MCU leaves the process alive, recover before the supervisor can start a fresh UI.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn panel_power<T>(
    failure: String,
    prepare: impl FnOnce() -> T,
    send: impl FnOnce(),
    grace: impl FnOnce(),
    restore: impl FnOnce(T, String) -> String,
    notice: impl FnOnce(&str),
) {
    let data = prepare();
    send();
    grace();
    notice(&restore(data, failure));
}

/// Wait only until the shared deadline, including when several workers overrun.
/// Dropping a JoinHandle detaches the thread; its owned buffers remain alive.
pub fn join(handles: Vec<JoinHandle<()>>, deadline: Instant) -> usize {
    let mut pending = handles;
    loop {
        let mut i = 0;
        while i < pending.len() {
            if pending[i].is_finished() {
                if pending.swap_remove(i).join().is_err() {
                    eprintln!("oos-ui: worker panicked during shutdown");
                }
            } else {
                i += 1;
            }
        }
        if pending.is_empty() || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())));
    }
    let count = pending.len();
    if count > 0 {
        eprintln!("oos-ui: shutdown deadline: {} workers still own their buffers", count);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};

    #[test]
    fn total_shutdown_deadlines_share_one_time_origin() {
        let start = Instant::now();
        run(
            start,
            &mut (),
            |deadline| assert_eq!(deadline, start + Duration::from_millis(500)),
            |_, deadline| assert_eq!(deadline, start + Duration::from_millis(1000)),
            |_| Ok(()),
            |deadline| {
                assert_eq!(deadline, start + Duration::from_millis(2100));
                true
            },
            || {},
        )
        .unwrap();
    }

    #[test]
    fn failed_save_does_not_block_power_and_an_ignored_command_recovers_before_exit() {
        let events = std::cell::RefCell::new(vec!["persist failed"]);
        let code = finish(
            Err(std::io::Error::other("full")),
            0,
            Some(|| {
                panel_power(
                    "MCU did not power off".into(),
                    || {
                        events.borrow_mut().push("close data");
                        "mount"
                    },
                    || events.borrow_mut().push("MCU power"),
                    || events.borrow_mut().push("power grace"),
                    |data, failure| {
                        assert_eq!(data, "mount");
                        events.borrow_mut().push("restore data");
                        failure
                    },
                    |failure| {
                        assert_eq!(failure, "MCU did not power off");
                        events.borrow_mut().push("panel notice");
                    },
                )
            }),
        );
        assert_eq!(
            *events.borrow(),
            ["persist failed", "close data", "MCU power", "power grace", "restore data", "panel notice"]
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn process_only_exit_keeps_the_data_mount() {
        assert_eq!(finish(Ok(()), 0, None::<fn()>), 0);
        assert_eq!(finish(Ok(()), 1, Some(|| panic!("must not send MCU power"))), 1);
    }

    #[test]
    fn overrun_continues_without_freeing_live_buffers() {
        let buffer = Arc::new(vec![42u8; 1024]);
        let weak = Arc::downgrade(&buffer);
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            rx.recv().unwrap();
            assert_eq!(buffer[0], 42);
        });
        let start = Instant::now();
        assert_eq!(join(vec![worker], start), 1);
        assert!(weak.upgrade().is_some());
        tx.send(()).unwrap();
    }

    #[test]
    fn completed_workers_are_joined() {
        let worker = std::thread::spawn(|| {});
        assert_eq!(join(vec![worker], Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD), 0);
    }

    #[test]
    fn existing_workers_stop_in_order_even_when_persistence_fails() {
        let events = std::cell::RefCell::new(vec!["main stopped"]);
        let result = run(
            Instant::now(),
            &mut (),
            |_| events.borrow_mut().push("remote"),
            |_, _| events.borrow_mut().push("deposit and measure"),
            |_| {
                events.borrow_mut().push("persist");
                Err(std::io::Error::other("full"))
            },
            |_| {
                events.borrow_mut().push("helper");
                true
            },
            || events.borrow_mut().push("DRM"),
        );
        assert!(result.is_err());
        assert_eq!(*events.borrow(), ["main stopped", "remote", "deposit and measure", "persist", "helper", "DRM"]);
    }
}
