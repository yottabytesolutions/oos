// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Submit a 20-frame batch and return once its first decoder is blocked on the gate.
pub(super) fn one_worker_held(pipe: &Pipe) -> Arc<(Mutex<bool>, std::sync::Condvar)> {
    let (bytes, fb, s, p) = batch(20, 12_512, false);
    let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let (blocked_tx, blocked) = std::sync::mpsc::channel::<()>();
    let first = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let inner = dec(&s, &p);
    let (g, tx) = (gate.clone(), Mutex::new(blocked_tx));
    let decode: DecodeInto = Arc::new(move |c: &[u8], f: &mut Frame| {
        if first.swap(false, Ordering::AcqRel) {
            let _ = tx.lock().unwrap().send(());
            let mut open = g.0.lock().unwrap();
            while !*open {
                open = g.1.wait(open).unwrap();
            }
        }
        inner(c, f)
    });
    pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, decode);
    blocked.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).expect("a worker in its first decode");
    gate
}

#[test]
fn shutdown_leaves_a_busy_deposit_workers_buffers_owned() {
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&model(1e-6, false, false)), 970, 448, Interp::Exact);
    let gate = one_worker_held(&pipe);
    let now = std::time::Instant::now();
    assert!(shutdown::close(&pipe, now) >= 1);
    assert!(!pipe.accepting());
    // The detached worker still owns its Batch, view and mailbox after the
    // UI's pipe is gone. Releasing the gate lets that owner finish safely.
    drop(pipe);
    *oos_hal::sync::lock(&gate.0) = true;
    gate.1.notify_all();
}

#[test]
#[ignore = "perf-guard latency: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_shutdown_retains_busy_workers_within_budget() {
    crate::perfguard::latency::check(
        std::time::Duration::from_millis(250),
        shutdown_leaves_a_busy_deposit_workers_buffers_owned,
    );
}
