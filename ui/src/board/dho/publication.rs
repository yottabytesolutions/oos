// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::{Frame, Gen, HwShared, MaskWant, Slow, Want, ZoneWant, QUEUE_MAX, QUEUE_SAMPLES};
use crate::perfguard::deep::acq_lock as lock;
use oos_core::change::Identity;
use oos_hal::dho::acq::Mode;

pub(super) struct Request {
    pub(super) generator: Option<Gen>,
    pub(super) want: Option<Want>,
    pub(super) mode: Slow,
    pub(super) avg_clear: u64,
    pub(super) mask: Option<MaskWant>,
    pub(super) zone: Option<ZoneWant>,
    pub(super) needs_raw: bool,
    pub(super) identity: Identity,
    pub(super) route: bool,
    pub(super) run: Run,
    pub(super) dvm: Option<crate::board::DvmWant>,
    pub(super) slots: Option<u64>,
}

pub(super) struct Run {
    pub(super) hold: bool,
    pub(super) stop_view: Option<crate::board::identity::StopView>,
    pub(super) final_req: u64,
    pub(super) cores: bool,
    pub(super) deep: Option<crate::deepmeas::Want>,
    pub(super) segment_bound: Option<u32>,
    pub(super) plot_columns: usize,
}

impl Request {
    pub(super) fn pair(&self) -> (u64, u64) {
        (self.identity.gen(), self.identity.epoch())
    }
}

#[cfg(any(target_os = "linux", test))]
impl Run {
    pub(super) fn batch_bound(&self, bound: u32) -> u32 {
        self.segment_bound.map_or(bound, |remaining| bound.min(remaining))
    }
}

/// Copy the batch inputs together. Live control changes are still polled at IO and hand-off boundaries.
impl Request {
    pub(super) fn read(g: &crate::board::Shared) -> Self {
        Self {
            generator: g.afg[0],
            want: g.want.clone(),
            mode: g.slow,
            avg_clear: g.avg_clear,
            mask: g.mask_want.clone(),
            zone: g.zone_want.clone(),
            needs_raw: g.needs_raw,
            identity: g.acquisition.identity(),
            route: g.acquisition.route(),
            run: Run {
                hold: g.hold,
                stop_view: g.stop_view,
                final_req: g.final_req,
                cores: g.cores,
                deep: g.deep_raw,
                segment_bound: g.record_flow.segment_bound(),
                plot_columns: g.acquisition.plot_columns(),
            },
            dvm: g.dvm_want,
            slots: g.slots_want,
        }
    }
}

#[cfg(test)]
pub(super) fn snapshot(out: &HwShared) -> Request {
    Request::read(&lock(out))
}

/// Publish one bounded batch after the acquisition loop has waited for the previous hand-off.
#[cfg(test)]
pub(super) fn push_frames(out: &HwShared, frames: Vec<Frame>, mode: Mode) {
    if frames.is_empty() {
        return;
    }
    finish_batch(out, frames, mode, |_| {});
}

/// Publish the readings and any current frames in the same Shared transaction.
pub(super) fn finish_batch(
    out: &HwShared,
    mut frames: Vec<Frame>,
    mode: Mode,
    readings: impl FnOnce(&mut crate::board::Shared),
) {
    let sample = {
        let mut g = lock(out);
        readings(&mut g);
        frames.retain(|f| current(&g, f));
        enqueue(&mut g, frames, mode)
    };
    if let Some(sample) = sample {
        sample.record();
    }
}

fn enqueue(g: &mut crate::board::Shared, frames: Vec<Frame>, mode: Mode) -> Option<crate::perfguard::deep::CopySample> {
    if frames.is_empty() {
        return None;
    }
    let max = if mode == Mode::Peak { super::PEAK_BATCH_MAX } else { QUEUE_MAX };
    let samples = frames
        .iter()
        .map(|frame| frame.ch.iter().map(Vec::len).sum::<usize>() + frame.la.as_ref().map_or(0, |la| la.words.len()))
        .sum::<usize>();
    let handoff = &crate::drawpipe::counters().losses.record_handoff;
    let fits = frames.len() <= max && (frames.len() == 1 || samples <= QUEUE_SAMPLES);
    let empty = g.queue.is_empty();
    if !fits {
        crate::drawpipe::Counters::add(&handoff.sample_mismatch, 1);
    }
    if !empty {
        crate::drawpipe::Counters::add(&handoff.unexpected_queue, 1);
    }
    debug_assert!(fits, "record plan exceeds one hand-off");
    debug_assert!(empty, "the record producer waits for the previous hand-off");
    g.queue.extend(frames.into_iter().map(Into::into));
    g.record_flow.segments_published();
    let copied = g.queue.back().map(crate::perfguard::deep::clone_frame);
    copied.map(|(frame, sample)| {
        g.frame = frame;
        sample
    })
}

/// Count and sample capacity for the ordinary record hand-off, before arm.
#[cfg(any(target_os = "linux", test))]
pub(super) fn record_grant(
    out: &HwShared,
    identity: Identity,
    bound: u32,
    settings: &oos_hal::dho::seq::Settings,
    params: &oos_hal::dho::hz::Params,
    budget: impl Into<crate::drawpipe::admission::GrantBudget>,
) -> crate::drawpipe::admission::Grant {
    let (per, newest) = record_sizes(settings, params);
    // A singleton may exceed the sample bound. With no analog channels, only the newest LA record costs samples.
    let by_samples = QUEUE_SAMPLES.saturating_sub(newest).checked_div(per).map_or(QUEUE_MAX, |extra| extra + 1);
    let bound = bound.min(by_samples as u32).min(crate::board::RECORDS_PER_STEP as u32);
    lock(out).record_flow.cost.grant(identity, bound, budget)
}

#[cfg(any(target_os = "linux", test))]
fn record_sizes(settings: &oos_hal::dho::seq::Settings, params: &oos_hal::dho::hz::Params) -> (usize, usize) {
    use oos_hal::dho::{hz, la, seq};
    let analog = |s: &seq::Settings| {
        let bytes = seq::frame_bytes(s, params) - usize::from(s.header) * 16;
        let points = bytes / (2 * hz::ratio(s.mask) as usize);
        points * (s.mask & 0xf).count_ones() as usize
    };
    let per = analog(settings);
    let raw = super::hw::raw_replay_settings(settings, params, true);
    let logic = settings.la.as_ref().map_or(0, |_| la::window(&la::timing(params, settings.mask)).0 as usize);
    (per, raw.as_ref().map_or(per, analog).max(per) + logic)
}

/// Check once after controls and STOP handling, then return to the source loop on any event.
#[derive(Default)]
pub(super) struct RecordWait {
    started: Option<(Identity, std::time::Instant)>,
    logged: Option<std::time::Instant>,
}

impl RecordWait {
    pub(super) fn poll(
        &mut self,
        out: &HwShared,
        deposit: Option<&crate::drawpipe::Pipe>,
        clock: &impl super::clock::LoopClock,
        obsolete: impl FnOnce() -> bool,
        status: impl FnOnce(String),
    ) -> bool {
        use super::clock::Wake;
        use std::time::Duration;
        if deposit.is_some() {
            self.started = None;
            return false;
        }
        let pipe_event = crate::drawpipe::source_event();
        let (local, seen, identity, queued, segment_drawing, segment_exhausted) = {
            let g = lock(out);
            let local = g.record_flow.source.clone();
            let seen = Wake::generation(pipe_event.unwrap_or(&local));
            let drawing = g.record_flow.segments_pending();
            let exhausted = !drawing && g.record_flow.segment_bound() == Some(0);
            (local, seen, g.acquisition.identity(), g.queue.len(), drawing, exhausted)
        };
        let event = pipe_event.unwrap_or(&local);
        let waiting = queued != 0 || segment_drawing;
        if let Some(message) = self.observe(clock.now(), identity, waiting, queued) {
            status(message);
        }
        if obsolete() {
            return true;
        }
        if waiting {
            if let Some(elapsed) = clock.park(event, seen, Duration::from_millis(100)) {
                crate::drawpipe::counters().metrics.source_wait(identity, elapsed);
            }
        }
        if segment_exhausted {
            // The UI has already queued STOP for the completed recording.
            clock.park(event, seen, Duration::from_millis(100));
        }
        waiting || segment_exhausted
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
            "event=record_backpressure wait_ms={} queued={} gen={} epoch={}",
            elapsed.as_millis(),
            queued,
            id.gen(),
            id.epoch()
        ))
    }
}

/// Preserve frame-zero rejection and hardware-average selection. The source budgets records before the read.
pub(super) fn record_decode(avail: usize, average: bool, frame0_drop: bool) -> (usize, usize) {
    let take = if average { avail.min(1) } else { avail };
    let stale = (avail > 1 && !average && frame0_drop) as usize;
    let skip = (avail - take).max(stale);
    let frame0 = usize::from(skip > 0 && stale > 0);
    (frame0, skip - frame0)
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
pub(super) fn read(counters: &crate::drawpipe::Counters, count: u32, shown: u32, avail: usize) {
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
        read(crate::drawpipe::counters(), 10, 8, 6);
        let skipped = record_decode(6, false, true);
        assert_eq!(skipped, (1, 0));
        assert_eq!(account_skipped(skipped, false), 1);
        let counters = crate::drawpipe::counters();
        counters.lose(crate::drawpipe::Lost::Settings, 1);
        assert_eq!(counters.read(), (10, 0, 0));
        let text = counters.records_text();
        for bucket in ["lost.average=2", "lost.error=2", "lost.frame0=1", "lost.settings=1"] {
            assert!(text.contains(bucket), "{text}");
        }
        read(crate::drawpipe::counters(), 3, 3, 3);
        let skipped = record_decode(3, true, true);
        assert_eq!(skipped, (0, 2), "average selection does not discard frame zero separately");
        assert_eq!(account_skipped(skipped, true), 2);
        assert_eq!(counters.read(), (13, 0, 0));
        assert!(counters.records_text().contains("lost.average=4"));
    }

    #[test]
    fn incomplete_average_and_boundary_queue_clear_have_explicit_buckets() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        read(crate::drawpipe::counters(), 64, 0, 0);
        let mut shared = crate::board::Shared::default();
        shared.queue.extend([Frame::default(), Frame::default()].map(Into::into));
        read(crate::drawpipe::counters(), 2, 2, 2);
        shared.clear_queue(crate::drawpipe::Lost::Settings);
        let counters = crate::drawpipe::counters();
        assert_eq!(counters.read(), (66, 0, 0));
        let text = counters.records_text();
        assert!(text.contains("lost.average=64"));
        assert!(text.contains("lost.settings=2"));
        assert!(shared.queue.is_empty());
    }
}

#[cfg(test)]
mod tests;
