// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::{Frame, Gen, HwShared, MaskWant, Slow, Want, ZoneWant, QUEUE_MAX, QUEUE_SAMPLES};
use crate::perfguard::deep::acq_lock as lock;
use oos_core::change::Identity;
use oos_hal::dho::acq::Mode;

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

/// Peak holds one complete batch; its producer waits for this queue before reading another.
/// Other modes keep the newest `QUEUE_MAX` frames. All modes retain the sample bound and newest-frame exception.
pub(super) fn push_frames(out: &HwShared, mut frames: Vec<Frame>, mode: Mode) {
    if frames.is_empty() {
        return;
    }
    let mut g = lock(out);
    frames.retain(|f| current(&g, f));
    let max = if mode == Mode::Peak { super::PEAK_BATCH_MAX } else { QUEUE_MAX };
    g.queue.extend(frames.into_iter().map(Into::into));
    let size = |f: &std::sync::Arc<Frame>| f.ch.iter().map(Vec::len).sum::<usize>();
    let mut held: usize = g.queue.iter().map(size).sum();
    let mut over = 0u64;
    while g.queue.len() > 1 && (g.queue.len() > max || held > QUEUE_SAMPLES) {
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

/// Called after controls and STOP handling, before reading the next ordinary batch.
/// A prearmed batch may fill its bounded FPGA ring while the UI drains the preceding batch.
pub(super) fn peak_backpressure(g: &crate::board::Shared, mode: Mode) -> bool {
    mode == Mode::Peak && !g.queue.is_empty()
}

/// Worker-owned wait timer. Log at most once per second, including across separate waits.
#[derive(Default)]
pub(super) struct PeakWait {
    started: Option<(Identity, std::time::Instant)>,
    logged: Option<std::time::Instant>,
}

impl PeakWait {
    pub(super) fn poll(&mut self, out: &HwShared, mode: Mode, status: impl FnOnce(String)) -> bool {
        let (waiting, identity, queued) = {
            let g = lock(out);
            (peak_backpressure(&g, mode), g.acquisition.identity(), g.queue.len())
        };
        if let Some(message) = self.observe(std::time::Instant::now(), identity, waiting, queued) {
            status(message);
        }
        if waiting {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        waiting
    }

    pub(super) fn observe(
        &mut self,
        now: std::time::Instant,
        identity: Identity,
        waiting: bool,
        queued: usize,
    ) -> Option<String> {
        if !waiting {
            self.started = None;
            return None;
        }
        let (id, start) = self.started.get_or_insert((identity, now));
        if *id != identity {
            (*id, *start) = (identity, now);
        }
        let elapsed = now.duration_since(*start);
        let interval = std::time::Duration::from_secs(1);
        if elapsed <= interval || self.logged.is_some_and(|last| now.duration_since(last) < interval) {
            return None;
        }
        self.logged = Some(now);
        Some(format!(
            "event=peak_backpressure wait_ms={} queued={} gen={} epoch={}",
            elapsed.as_millis(),
            queued,
            id.gen(),
            id.epoch()
        ))
    }
}

/// Peak must decode every capture even when the previous Normal step requested only a few.
/// Frame-zero rejection and hardware-average selection keep their existing policies.
pub(super) fn record_decode(
    out: &HwShared,
    mode: Mode,
    avail: usize,
    average: bool,
    frame0_drop: bool,
) -> ((usize, usize), bool) {
    let g = lock(out);
    let take = if mode == Mode::Peak || g.take == 0 { avail } else { g.take.min(avail) };
    let take = if average { take.min(1) } else { take };
    let stale = (avail > 1 && !average && frame0_drop) as usize;
    let skip = (avail - take).max(stale);
    let frame0 = usize::from(skip > 0 && stale > 0);
    ((frame0, skip - frame0), g.cores)
}

/// Account for a selected batch's skips once, when the caller commits to decoding it.
pub(super) fn account_skipped((frame0, other): (usize, usize), average: bool) -> usize {
    let counters = crate::drawpipe::counters();
    counters.lose(crate::drawpipe::Lost::Frame0, frame0 as u64);
    counters.lose(if average { crate::drawpipe::Lost::Average } else { crate::drawpipe::Lost::Budget }, other as u64);
    frame0 + other
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

/// The frame read again at STOP into the screen and the UI queue when it is still current, and the UI's final-frame
/// request answered; the copy sample is recorded after Shared is released.
#[cfg(any(target_os = "linux", test))]
pub(super) fn publish_held(out: &HwShared, last: Option<Frame>, final_req: u64) {
    let mut g = lock(out);
    let sample = last.filter(|f| current(&g, f)).map(|f| {
        let f = std::sync::Arc::new(f);
        let (copy, sample) = crate::perfguard::deep::clone_frame(&f);
        g.frame = copy;
        g.queue.push_back(f);
        sample
    });
    g.final_ack = final_req;
    drop(g);
    if let Some(sample) = sample {
        sample.record();
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn publication_shares_samples_and_preserves_queue_and_latest() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = HwShared::default();
        let frame = Frame { ch: std::array::from_fn(|_| vec![2.0; 625_000]), ..Default::default() };
        push_frames(&shared, vec![frame], Mode::Normal);
        let text = crate::perfguard::deep::counters().text();
        assert!(text.contains("lock.acq.wait n=1 total=1"));
        assert!(text.contains("lock.acq.held n=1 total=1"));
        assert!(text.contains("producer.clone n=1 total=1"));
        let g = oos_hal::sync::lock(&shared);
        assert_eq!(g.queue.len(), 1);
        assert_eq!(g.frame.ch, g.queue[0].ch);
        assert!(std::sync::Arc::ptr_eq(&g.frame, &g.queue[0]));
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
                let started = crate::ui::lifecycle::test_wait::Wait::new(
                    "publication_releases_shared_before_waiting_to_record_copy completion",
                );
                loop {
                    if shared.try_lock().is_ok_and(|g| g.frame.seq == 7) {
                        break;
                    }
                    started.check();
                    started.poll();
                }
            });
            ready_rx.recv().unwrap();
            push_frames(shared, vec![Frame { seq: 7, ..Default::default() }], Mode::Normal);
        });
        assert!(diagnostics.text().contains("producer.clone n=1 total=1"));
    }
    #[test]
    fn held_frame_is_published_only_when_current_and_always_acknowledged() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = HwShared::default();
        let id = oos_hal::sync::lock(&shared).acquisition.identity();
        let held = tag(Frame { seq: 3, ..Default::default() }, id);
        publish_held(&shared, Some(held), 5);
        {
            let g = oos_hal::sync::lock(&shared);
            assert_eq!((g.frame.seq, g.queue.len(), g.final_ack), (3, 1, 5));
        }
        let stale = Frame { seq: 4, cfg: id.gen().wrapping_add(1), ..Default::default() };
        publish_held(&shared, Some(stale), 6);
        let g = oos_hal::sync::lock(&shared);
        assert_eq!((g.frame.seq, g.queue.len(), g.final_ack), (3, 1, 6));
    }
}

/// Completed read before average selection or short-buffer rejection.
pub(super) fn read(count: u32, shown: u32, avail: usize) {
    let counters = crate::drawpipe::counters();
    crate::drawpipe::Counters::add(&counters.acquired, count as u64);
    counters.lose(crate::drawpipe::Lost::Average, (count - shown) as u64);
    counters.lose(crate::drawpipe::Lost::Error, shown as u64 - avail as u64);
}

#[cfg(test)]
mod conservation_tests {
    use super::*;

    #[test]
    fn completed_reads_account_for_average_short_buffer_selection_and_settings_loss() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = HwShared::default();
        oos_hal::sync::lock(&shared).take = 1;
        read(10, 8, 6);
        let (skipped, _) = record_decode(&shared, Mode::Normal, 6, false, true);
        assert_eq!(skipped, (1, 4));
        assert_eq!(account_skipped(skipped, false), 5);
        let counters = crate::drawpipe::counters();
        counters.lose(crate::drawpipe::Lost::Settings, 1);
        assert_eq!(counters.read(), (10, 0, 0));
        let text = counters.records_text();
        for bucket in ["lost.average=2", "lost.error=2", "lost.frame0=1", "lost.budget=4", "lost.settings=1"] {
            assert!(text.contains(bucket), "{text}");
        }
        read(3, 3, 3);
        let (skipped, _) = record_decode(&shared, Mode::Average { count: 4 }, 3, true, true);
        assert_eq!(skipped, (0, 2), "average selection does not discard frame zero separately");
        assert_eq!(account_skipped(skipped, true), 2);
        assert_eq!(counters.read(), (13, 0, 0));
        assert!(counters.records_text().contains("lost.average=4"));
    }

    #[test]
    fn incomplete_average_and_boundary_queue_clear_have_explicit_buckets() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        read(64, 0, 0);
        let mut shared = crate::board::Shared::default();
        shared.queue.extend([Frame::default(), Frame::default()].map(Into::into));
        read(2, 2, 2);
        shared.clear_queue(crate::drawpipe::Lost::Settings);
        let counters = crate::drawpipe::counters();
        assert_eq!(counters.read(), (66, 0, 0));
        let text = counters.records_text();
        assert!(text.contains("lost.average=64"));
        assert!(text.contains("lost.settings=2"));
        assert!(shared.queue.is_empty());
    }
}
