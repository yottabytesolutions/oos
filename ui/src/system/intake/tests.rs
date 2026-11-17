// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use std::sync::mpsc;

fn id(connection: u64, request: u64) -> Id {
    Id::new(connection, request)
}

fn port<T>(entries: usize, bytes: usize) -> Arc<Port<T>> {
    Arc::new(Port::new(entries, bytes, Arc::new(Wake::new().unwrap())))
}

#[test]
fn admission_bounds_capacity_and_count_and_preserves_fifo() {
    let p = port(2, 6);
    let mut first = Vec::with_capacity(4);
    first.push(1);
    let retained = first.capacity();
    p.submit(id(1, 0), first, retained).unwrap();
    assert_eq!(p.submit(id(2, 0), vec![2; 3], 3), Err(vec![2; 3]));
    p.submit(id(2, 0), vec![2; 2], 2).unwrap();
    assert_eq!(p.submit(id(3, 0), Vec::new(), 0), Err(Vec::new()));
    assert_eq!(p.take(1), vec![vec![1]]);
    p.submit(id(3, 0), vec![3; 4], 4).unwrap();
    assert_eq!(p.take(8), vec![vec![2; 2], vec![3; 4]]);
    assert!(p.is_empty());
    assert!(p.submit(id(4, 0), vec![0; 7], 7).is_err());
}

#[test]
fn enqueue_before_during_and_after_wait_has_no_lost_wake() {
    let p = port(3, 3);
    p.submit(id(1, 0), 1, 1).unwrap();
    let (ready, waiting) = mpsc::channel();
    let (done, completion) = mpsc::channel();
    let worker = p.clone();
    let thread = std::thread::spawn(move || {
        for expected in 1..=3 {
            ready.send(expected).unwrap();
            assert!(worker.wake().wait(HANG_GUARD.as_millis() as i32));
            worker.wake().drain();
            assert_eq!(worker.take(1), vec![expected]);
            // Clear capacity notification before entering the next wait.
            worker.wake().drain();
            done.send(expected).unwrap();
        }
    });
    assert_eq!(waiting.recv_timeout(HANG_GUARD).unwrap(), 1);
    assert_eq!(completion.recv_timeout(HANG_GUARD).unwrap(), 1);
    assert_eq!(waiting.recv_timeout(HANG_GUARD).unwrap(), 2);
    p.submit(id(2, 0), 2, 1).unwrap();
    assert_eq!(completion.recv_timeout(HANG_GUARD).unwrap(), 2);
    assert_eq!(waiting.recv_timeout(HANG_GUARD).unwrap(), 3);
    p.submit(id(3, 0), 3, 1).unwrap();
    assert_eq!(completion.recv_timeout(HANG_GUARD).unwrap(), 3);
    thread.join().unwrap();
}

#[test]
fn quota_rearms_remaining_work_and_cancellation_uses_both_id_parts() {
    let p = port(4, 4);
    for key in [id(1, 0), id(1, 1), id(2, 0), id(3, 0)] {
        p.submit(key, key, 1).unwrap();
    }
    p.wake().drain();
    assert_eq!(p.take(1), vec![id(1, 0)]);
    assert!(p.wake().wait(HANG_GUARD.as_millis() as i32));
    p.wake().drain();
    p.cancel(id(1, 1));
    assert_eq!(p.take(1), vec![id(2, 0)]);
    assert!(p.wake().wait(HANG_GUARD.as_millis() as i32));
    p.wake().drain();
    assert_eq!(p.take(1), vec![id(3, 0)]);
}

#[test]
fn full_reply_port_preserves_owned_reply_and_does_not_block_frames() {
    let wake = Arc::new(Wake::new().unwrap());
    let replies = Arc::new(Port::new(1, 4, wake.clone()));
    replies.submit(id(1, 0), vec![1; 4], 4).unwrap();
    let (frame_done, completed) = mpsc::channel();
    let frame_replies = replies.clone();
    let frame = std::thread::spawn(move || {
        let pending = frame_replies.submit(id(2, 0), vec![2; 4], 4).unwrap_err();
        // The frame can proceed with input/render work while retaining this exact reply.
        frame_done.send(pending).unwrap();
    });
    let pending = completed.recv_timeout(HANG_GUARD).expect("full output must not block the frame thread");
    frame.join().unwrap();
    wake.drain();
    assert_eq!(replies.take(1), vec![vec![1; 4]]);
    assert!(wake.wait(HANG_GUARD.as_millis() as i32));
    wake.drain();
    replies.submit(id(2, 0), pending, 4).unwrap();
    assert_eq!(replies.take(1), vec![vec![2; 4]]);
}

#[test]
fn disconnect_and_shutdown_wake_waiters_and_drop_owned_requests() {
    let p = port(1, 1);
    let (tx, rx) = mpsc::sync_channel::<()>(1);
    p.submit(id(1, 0), tx, 1).unwrap();
    p.wake().drain();
    p.cancel(id(1, 0));
    assert!(p.wake().wait(HANG_GUARD.as_millis() as i32));
    assert!(matches!(rx.recv_timeout(HANG_GUARD), Err(mpsc::RecvTimeoutError::Disconnected)));
    let (tx, rx) = mpsc::sync_channel::<()>(1);
    p.submit(id(2, 0), tx, 1).unwrap();
    p.wake().drain();
    p.clear(true);
    assert!(p.wake().wait(HANG_GUARD.as_millis() as i32));
    assert!(matches!(rx.recv_timeout(HANG_GUARD), Err(mpsc::RecvTimeoutError::Disconnected)));
    assert!(p.submit(id(3, 0), mpsc::sync_channel(1).0, 1).is_err());
    let weak = Arc::downgrade(&p.wake);
    drop(p);
    assert!(weak.upgrade().is_none(), "shutdown leaves no owner of either wake descriptor");
}

#[test]
fn cancellation_cannot_retract_a_request_already_taken() {
    let p = port(1, 1);
    p.submit(id(1, 0), 1, 1).unwrap();
    assert!(p.cancel(id(1, 0)));
    assert!(!p.cancel(id(1, 0)));
    p.submit(id(2, 0), 2, 1).unwrap();
    assert_eq!(p.take(1), vec![2]);
    assert!(!p.cancel(id(2, 0)), "intake owns completion once it takes the request");
}

#[test]
fn empty_intake_quiesces_after_capacity_notification() {
    let p = port(1, 1);
    p.submit(id(1, 0), 1, 1).unwrap();
    p.wake().drain();
    assert_eq!(p.take(8), vec![1]);
    assert!(p.wake().wait(0));
    p.wake().drain();
    assert!(p.take(8).is_empty());
    assert!(!p.wake().wait(0), "idle intake must return to a blocking wait");
}

#[test]
fn saturated_notifications_coalesce_without_blocking_or_losing_work() {
    let p = port(1, 1);
    let worker = p.clone();
    let (done, completed) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        for _ in 0..100_000 {
            worker.wake().notify();
        }
        worker.submit(id(1, 0), 1, 1).unwrap();
        done.send(()).unwrap();
    });
    completed.recv_timeout(HANG_GUARD).expect("a full wake socket must coalesce notifications");
    thread.join().unwrap();
    assert!(p.wake().wait(0));
    p.wake().drain();
    assert_eq!(p.take(1), vec![1]);
}

#[test]
fn request_identity_preserves_connection_incarnation_and_request() {
    let id = Id::new(u64::MAX, 42);
    assert_eq!(id.connection(), u64::MAX);
    assert_eq!(id.request(), 42);
    assert_eq!(id, Id::new(id.connection(), id.request()));
    assert_ne!(id, Id::new(0, id.request()));
    assert_ne!(id, Id::new(id.connection(), 0));
}
