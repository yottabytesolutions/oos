// SPDX-License-Identifier: GPL-2.0-or-later

use super::{hw, publication};
use crate::board::{Frame, HwShared};
use oos_hal::dho::acq::Mode;
use oos_hal::sync::lock;
use std::sync::atomic::Ordering::Relaxed;

/// The ordinary worker's production admission/publication sequence without hardware IO.
pub(crate) fn peak_batch(shared: &HwShared, frames: Vec<Frame>) -> Result<(), Vec<Frame>> {
    if !lock(shared).queue.is_empty() {
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
        assert!(lock(&shared).queue.is_empty());
    }
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(Relaxed), 0);
    assert_eq!(crate::drawpipe::counters().dropped.load(Relaxed), 0);
}

#[test]
fn record_decode_preserves_frame_zero_and_average_selection() {
    assert_eq!(publication::record_decode(720, false, false), (0, 0));
    assert_eq!(publication::record_decode(720, false, true), (1, 0));
    assert_eq!(publication::record_decode(720, true, false), (0, 719));
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
fn peak_wait_status_is_rate_limited_across_waits_and_resets_its_timer() {
    use std::time::{Duration, Instant};
    let shared = shared();
    let identity = lock(&shared).acquisition.identity();
    let mut wait = publication::RecordWait::default();
    let start = Instant::now();
    let mut reports = vec![];
    for ms in 0..=3001 {
        let report = wait.observe(start + Duration::from_millis(ms), identity, true, super::PEAK_BATCH_MAX);
        if let Some(message) = report {
            reports.push((ms, message));
        }
    }
    assert_eq!(reports.iter().map(|(ms, _)| *ms).collect::<Vec<_>>(), [1001, 2001, 3001]);
    assert!(reports[0].1.contains("event=record_backpressure wait_ms=1001"));
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
fn peak_wait_timer_restarts_on_an_identity_change() {
    use oos_core::change::{Request, Shadow, UiAcq};
    use std::time::{Duration, Instant};
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let old = shadow.identity();
    let new = shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity();
    let start = Instant::now();
    let mut wait = publication::RecordWait::default();
    assert!(wait.observe(start, old, true, 1).is_none());
    let changed = start + Duration::from_secs(2);
    assert!(wait.observe(changed, new, true, 1).is_none(), "previous identity's wait must not carry over");
    assert!(wait.observe(changed + Duration::from_millis(1001), new, true, 1).is_some());
}
