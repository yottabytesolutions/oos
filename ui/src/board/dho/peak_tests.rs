// SPDX-License-Identifier: GPL-2.0-or-later

use super::{hw, publication};
use crate::board::{Frame, HwShared};
use oos_hal::dho::acq::Mode;
use oos_hal::sync::lock;
use std::sync::atomic::Ordering::Relaxed;

/// The ordinary worker's production admission/publication sequence without hardware IO.
pub(crate) fn peak_batch(shared: &HwShared, frames: Vec<Frame>) -> Result<(), Vec<Frame>> {
    if publication::peak_backpressure(&lock(shared), Mode::Peak) {
        return Err(frames);
    }
    assert!(frames.len() <= super::PEAK_BATCH_MAX, "the host producer must obey the real planner bound");
    publication::push_frames(shared, frames, Mode::Peak);
    Ok(())
}

pub(crate) fn peak_batch_frames(model: &crate::model::Model) -> usize {
    use crate::board::Board;
    use oos_hal::dho::{hz, seq};
    let s = crate::board::board().want(model, &Default::default());
    let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, 0);
    hw::batch_plan(&s, &p, 1).frames as usize
}

fn shared() -> HwShared {
    let shared = HwShared::default();
    let model = crate::model::Model { acq: crate::model::AcqMode::Peak, ..Default::default() };
    use crate::board::Board;
    lock(&shared).want = Some(crate::board::board().want(&model, &Default::default()));
    shared
}

#[test]
fn peak_backpressure_preserves_repeated_maximum_batches() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = shared();
    let max = super::PEAK_BATCH_MAX as u64;
    for batch in 0..3 {
        let frames = (1..=max).map(|seq| Frame { seq: batch * max + seq, ..Default::default() }).collect();
        assert!(peak_batch(&shared, frames).is_ok());
        for seq in 1..=max {
            assert!(peak_batch(&shared, vec![]).is_err(), "no next read while any capture remains");
            assert_eq!(lock(&shared).queue.pop_front().unwrap().seq, batch * max + seq);
        }
        assert!(!publication::peak_backpressure(&lock(&shared), Mode::Peak));
    }
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(Relaxed), 0);
    assert_eq!(crate::drawpipe::counters().dropped.load(Relaxed), 0);
}

#[test]
fn peak_keeps_count_and_sample_bounds_and_newest_exception() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = shared();
    publication::push_frames(&shared, vec![Frame::default(); super::PEAK_BATCH_MAX + 1], Mode::Peak);
    assert_eq!(lock(&shared).queue.len(), super::PEAK_BATCH_MAX);
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(Relaxed), 1);
    lock(&shared).queue.clear();
    // An oversized newest record retains the existing exception, without retaining older captures beside it.
    let large = Frame {
        seq: 2,
        ch: [vec![0.0; crate::board::QUEUE_SAMPLES + 1], vec![], vec![], vec![]],
        ..Default::default()
    };
    publication::push_frames(&shared, vec![Frame { seq: 1, ..Default::default() }, large], Mode::Peak);
    assert_eq!(lock(&shared).queue.len(), 1);
    assert_eq!(lock(&shared).queue.front().unwrap().seq, 2);
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(Relaxed), 2);
}

#[test]
fn peak_decodes_all_records_across_a_normal_to_peak_transition() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = shared();
    lock(&shared).take = 3;
    assert_eq!(publication::record_decode(&shared, Mode::Peak, 720, false, false), ((0, 0), false));
    assert_eq!(publication::record_decode(&shared, Mode::Normal, 720, false, false), ((0, 717), false));
    assert_eq!(publication::record_decode(&shared, Mode::Peak, 720, false, true), ((1, 0), false));
    assert_eq!(publication::record_decode(&shared, Mode::Average { count: 4 }, 720, true, false), ((0, 719), false));
    assert_eq!(crate::drawpipe::counters().read(), (0, 0, 0), "selection must not account losses");
    assert_eq!(publication::record_decode(&shared, Mode::Normal, 720, false, false), ((0, 717), false));
    assert_eq!(crate::drawpipe::counters().read(), (0, 0, 0), "repeated selection must remain side-effect free");
    lock(&shared).take = 0;
    assert_eq!(publication::record_decode(&shared, Mode::Normal, 720, false, false), ((0, 0), false));
}

#[test]
fn normal_publication_retains_its_overflow_policy_without_backpressure() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = HwShared::default();
    publication::push_frames(&shared, (1..=720).map(|seq| Frame { seq, ..Default::default() }).collect(), Mode::Normal);
    assert_eq!(lock(&shared).queue.len(), crate::board::QUEUE_MAX);
    assert_eq!(lock(&shared).queue.front().unwrap().seq, 465);
    assert_eq!(lock(&shared).frame.seq, 720);
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(Relaxed), 464);
    assert!(!publication::peak_backpressure(&lock(&shared), Mode::Normal));
}

#[test]
fn peak_planner_fits_a_single_bounded_publication() {
    use crate::board::Board;
    use crate::model::{tscales, AcqMode, Model};
    use oos_hal::dho::{hz, seq};
    let mut largest = 0;
    for channels in 1..=4 {
        for depth in 0..super::DEPTHS.len() {
            for tscale in 0..tscales().len() {
                let mut model = Model { acq: AcqMode::Peak, mdepth: depth, tscale, ..Default::default() };
                for (c, ch) in model.ch.iter_mut().enumerate() {
                    ch.on = c < channels;
                }
                let s = crate::board::board().want(&model, &Default::default());
                let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, 0);
                let b = hw::batch_plan(&s, &p, 1);
                assert!(b.frames as usize <= super::PEAK_BATCH_MAX);
                largest = largest.max(b.frames as usize);
                let mut normal = s.clone();
                normal.mode = Mode::Normal;
                let normal = hw::batch_plan(&normal, &p, 1);
                let by_size = (4_000_000 / seq::frame_bytes(&normal, &p).max(1)) as u32;
                assert_eq!(
                    normal.frames,
                    by_size.min(p.max_frames).min(super::BATCH_MAX as u32 / p.ratio.max(1)).max(1)
                );
                // f32 decoding expands each 16-bit input sample to one 4-byte sample.
                // A singleton keeps the existing oversized-newest exception.
                let samples = seq::frame_bytes(&b, &p) * b.frames as usize / 2;
                assert!(b.frames == 1 || samples <= crate::board::QUEUE_SAMPLES);
            }
        }
    }
    assert_eq!(largest, super::PEAK_BATCH_MAX, "the sweep includes the maximum-sized Peak batch");
}

#[test]
fn peak_rejects_old_identity_without_overflowing_current_batch() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = shared();
    let mut frames = vec![Frame { cfg: 1, ..Default::default() }];
    frames.extend((1..=super::PEAK_BATCH_MAX as u64).map(|seq| Frame { seq, ..Default::default() }));
    publication::push_frames(&shared, frames, Mode::Peak);
    assert_eq!(lock(&shared).queue.len(), super::PEAK_BATCH_MAX);
    assert_eq!(lock(&shared).queue.front().unwrap().seq, 1);
    assert_eq!(lock(&shared).frame.seq, super::PEAK_BATCH_MAX as u64);
    let losses = &crate::drawpipe::counters().losses;
    assert_eq!(losses.worker_push.load(Relaxed), 1);
    assert_eq!(losses.overflow.load(Relaxed), 0);
}

#[test]
fn publication_bound_follows_applied_mode_during_request_changes() {
    use crate::board::Board;
    use crate::model::{AcqMode, Model};
    let _scope = crate::drawpipe::test_scope::Scope::new();
    for (applied, requested, max) in
        [(Mode::Peak, AcqMode::Normal, super::PEAK_BATCH_MAX), (Mode::Normal, AcqMode::Peak, crate::board::QUEUE_MAX)]
    {
        let shared = HwShared::default();
        let model = Model { acq: requested, ..Default::default() };
        lock(&shared).want = Some(crate::board::board().want(&model, &Default::default()));
        let frames = (1..=max + 1).map(|seq| Frame { seq: seq as u64, ..Default::default() }).collect();
        publication::push_frames(&shared, frames, applied);
        assert_eq!(lock(&shared).queue.len(), max);
        assert_eq!(lock(&shared).queue.front().unwrap().seq, 2);
    }
}

#[test]
fn peak_wait_status_is_rate_limited_across_waits_and_resets_its_timer() {
    use std::time::{Duration, Instant};
    let shared = shared();
    let identity = lock(&shared).acquisition.identity();
    let mut wait = publication::PeakWait::default();
    let start = Instant::now();
    let mut reports = vec![];
    for ms in 0..=3001 {
        let report = wait.observe(start + Duration::from_millis(ms), identity, true, super::PEAK_BATCH_MAX);
        if let Some(message) = report {
            reports.push((ms, message));
        }
    }
    assert_eq!(reports.iter().map(|(ms, _)| *ms).collect::<Vec<_>>(), [1001, 2001, 3001]);
    assert!(reports[0].1.contains("event=peak_backpressure wait_ms=1001"));
    assert!(reports[0].1.contains(&format!(
        "queued={} gen={} epoch={}",
        super::PEAK_BATCH_MAX,
        identity.gen(),
        identity.epoch()
    )));
    let at = |ms| start + Duration::from_millis(ms);
    assert!(wait.observe(at(3002), identity, false, 0).is_none());
    assert!(wait.observe(at(3010), identity, true, 1).is_none());
    assert!(wait.observe(at(4010), identity, true, 1).is_none(), "each wait must exceed one second");
    assert!(wait.observe(at(4011), identity, true, 1).is_some());
    // Separate waits also keep the one-second threshold.
    assert!(wait.observe(at(4012), identity, false, 0).is_none());
    assert!(wait.observe(at(4013), identity, true, 1).is_none());
    assert!(wait.observe(at(5014), identity, true, 1).is_some());
}

#[test]
fn peak_wait_poll_releases_the_lock_for_status_and_bypasses_normal_or_empty_queues() {
    use std::time::{Duration, Instant};
    let shared = shared();
    let identity = lock(&shared).acquisition.identity();
    lock(&shared).queue.push_back(Frame::default().into());
    let mut wait = publication::PeakWait::default();
    wait.observe(
        Instant::now().checked_sub(Duration::from_secs(2)).expect("monotonic clock past 2 s"),
        identity,
        true,
        1,
    );
    assert!(wait.poll(&shared, Mode::Peak, |message| lock(&shared).status = message));
    assert!(lock(&shared).status.starts_with("event=peak_backpressure wait_ms="));
    assert!(!wait.poll(&shared, Mode::Normal, |_| panic!("Normal must not log Peak waits")));
    // A new Peak wait starts over after Normal, while the shared rate limit survives.
    assert!(wait.poll(&shared, Mode::Peak, |_| panic!("new wait must not log immediately")));
    lock(&shared).queue.clear();
    assert!(!wait.poll(&shared, Mode::Peak, |_| panic!("an empty queue must not log")));
}

#[test]
fn peak_wait_timer_restarts_on_an_identity_change() {
    use oos_core::change::{Request, Shadow, UiAcq};
    use std::time::{Duration, Instant};
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let old = shadow.identity();
    let new = shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity();
    let start = Instant::now();
    let mut wait = publication::PeakWait::default();
    assert!(wait.observe(start, old, true, 1).is_none());
    let changed = start + Duration::from_secs(2);
    assert!(wait.observe(changed, new, true, 1).is_none(), "previous identity's wait must not carry over");
    assert!(wait.observe(changed + Duration::from_millis(1001), new, true, 1).is_some());
}
