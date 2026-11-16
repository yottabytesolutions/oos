// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::{Frame, Gen, HwShared, MaskWant, Slow, Want, ZoneWant, QUEUE_MAX, QUEUE_SAMPLES};
use crate::perfguard::deep::acq_lock as lock;
use oos_core::change::Identity;

type Request = (Option<Gen>, Option<Want>, Slow, u64, Option<MaskWant>, Option<ZoneWant>, bool, Identity, bool);

pub(super) fn snapshot(out: &HwShared) -> Request {
    let g = lock(out);
    (
        g.afg[0],
        g.want.clone(),
        g.slow,
        g.avg_clear,
        g.mask_want.clone(),
        g.zone_want.clone(),
        g.needs_raw,
        g.acquisition.identity(),
        g.acquisition.route(),
    )
}

/// Frames of a batch into the UI queue, the oldest dropped beyond `QUEUE_MAX` frames or `QUEUE_SAMPLES` samples (the
/// newest frame stays whatever its size).
pub(super) fn push_frames(out: &HwShared, mut frames: Vec<Frame>) {
    if frames.is_empty() {
        return;
    }
    let mut g = lock(out);
    frames.retain(|f| current(&g, f));
    g.queue.extend(frames);
    let size = |f: &Frame| f.ch.iter().map(Vec::len).sum::<usize>();
    let mut held: usize = g.queue.iter().map(size).sum();
    let mut over = 0u64;
    while g.queue.len() > 1 && (g.queue.len() > QUEUE_MAX || held > QUEUE_SAMPLES) {
        if let Some(f) = g.queue.pop_front() {
            held -= size(&f);
        }
        over += 1;
    }
    g.dropped += over;
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.overflow, over);
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, over);
    let copied = g.queue.back().map(crate::perfguard::deep::clone_frame);
    let sample = copied.map(|(frame, sample)| {
        g.frame = frame;
        sample
    });
    drop(g);
    if let Some(sample) = sample {
        sample.record();
    }
}

pub(super) fn boundary(applied: Option<(u64, u64)>, identity: Identity) -> bool {
    applied != Some((identity.gen(), identity.epoch()))
}

pub(super) fn tag(mut frame: Frame, identity: Identity) -> Frame {
    (frame.cfg, frame.stamp) = (identity.gen(), identity.epoch());
    frame
}

/// Only effective hardware settings or mode changes restart segmented recording and roll.
pub(super) fn reconfigure(current: Option<(&Want, Slow)>, want: &Want, mode: Slow) -> bool {
    current != Some((want, mode))
}

/// Check a producer output under the publication lock, counting identity loss before any handoff.
pub(super) fn current(g: &crate::board::Shared, f: &Frame) -> bool {
    let id = g.acquisition.identity();
    if (f.cfg, f.stamp) == (id.gen(), id.epoch()) {
        return true;
    }
    let counters = crate::drawpipe::counters();
    crate::drawpipe::Counters::add(&counters.losses.worker_push, 1);
    crate::drawpipe::Counters::add(&counters.dropped, 1);
    false
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn publication_times_actual_clone_and_preserves_queue_and_latest() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = HwShared::default();
        let frame = Frame { ch: std::array::from_fn(|_| vec![2.0; 625_000]), ..Default::default() };
        push_frames(&shared, vec![frame]);
        let text = crate::perfguard::deep::counters().text();
        assert!(text.contains("lock.acq.wait n=1 total=1"));
        assert!(text.contains("lock.acq.held n=1 total=1"));
        assert!(text.contains("producer.clone n=1 total=1"));
        assert!(
            text.contains("producer.clone_size n=1 total=1 p50_bytes=10000000 p99_bytes=pending max_bytes=10000000")
        );
        let g = oos_hal::sync::lock(&shared);
        assert_eq!(g.queue.len(), 1);
        assert_eq!(g.frame.ch, g.queue[0].ch);
        assert_ne!(g.frame.ch[0].as_ptr(), g.queue[0].ch[0].as_ptr());
    }
    #[test]
    fn publication_releases_shared_before_waiting_to_record_copy() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = HwShared::default();
        let diagnostics = crate::perfguard::deep::counters();
        std::thread::scope(|scope| {
            let shared = &shared;
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            scope.spawn(move || {
                let _samples = diagnostics.hold_copies_for_test();
                ready_tx.send(()).unwrap();
                let started = std::time::Instant::now();
                loop {
                    if shared.try_lock().is_ok_and(|g| g.frame.seq == 7) {
                        break;
                    }
                    assert!(started.elapsed() < std::time::Duration::from_secs(5));
                    std::thread::yield_now();
                }
            });
            ready_rx.recv().unwrap();
            push_frames(shared, vec![Frame { seq: 7, ..Default::default() }]);
        });
        assert!(diagnostics.text().contains("producer.clone n=1 total=1"));
    }
}
