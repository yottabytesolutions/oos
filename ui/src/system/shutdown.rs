// SPDX-License-Identifier: GPL-2.0-or-later
//! A timed-out worker retains its captures until it actually finishes.

use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The main loop has ended before entering this sequence. Storage errors still
/// allow helper teardown and display release, while the final exit policy handles an explicit power request.
pub fn run<T>(
    context: &mut T,
    remote: impl FnOnce(Instant),
    workers: impl FnOnce(&mut T, Instant),
    persist: impl FnOnce(&mut T) -> std::io::Result<()>,
    helper: impl FnOnce(Instant) -> bool,
    release: impl FnOnce(),
) -> std::io::Result<()> {
    let start = Instant::now();
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
    fn total_shutdown_wait_stays_below_two_and_a_half_seconds() {
        let start = Instant::now();
        let wait = |deadline: Instant| std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        run(
            &mut (),
            wait,
            |_, deadline| wait(deadline),
            |_| Ok(()),
            |deadline| {
                wait(deadline + Duration::from_millis(100));
                true
            },
            || {},
        )
        .unwrap();
        assert!(start.elapsed() < Duration::from_millis(2500), "elapsed {:?}", start.elapsed());
    }

    #[test]
    fn failed_save_still_sends_power_without_an_abort() {
        let sent = std::cell::Cell::new(false);
        let code = finish(Err(std::io::Error::other("full")), 0, Some(|| sent.set(true)));
        assert!(sent.get(), "MCU power command must follow failed persistence");
        assert_eq!(code, 0);
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
        assert_eq!(join(vec![worker], start + Duration::from_millis(20)), 1);
        assert!(start.elapsed() < Duration::from_millis(250));
        assert!(weak.upgrade().is_some());
        tx.send(()).unwrap();
    }

    #[test]
    fn completed_workers_are_joined() {
        let worker = std::thread::spawn(|| {});
        assert_eq!(join(vec![worker], Instant::now() + Duration::from_secs(1)), 0);
    }

    #[test]
    fn existing_workers_stop_in_order_even_when_persistence_fails() {
        let events = std::cell::RefCell::new(vec!["main stopped"]);
        let result = run(
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
