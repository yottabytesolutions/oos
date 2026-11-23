// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::identity::Publication;
use crate::drawpipe::budget::WORK_BUDGET;
use std::sync::Barrier;

#[test]
fn bounded_record_handoff_waits_for_the_ui_and_yields_to_stop_or_settings() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let out = HwShared::default();
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut settings = oos_core::change::Shadow::new(&model, Default::default(), &config).unwrap();
    let old = settings.identity();
    let new =
        settings.observe(&model, Default::default(), &[oos_core::change::Request::Clear], &config).unwrap().identity();
    let mut source = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let mut wait = RecordWait::default();
    let modes = [Mode::Normal, Mode::Peak, Mode::Average { count: 4 }];
    for (index, mode) in modes.into_iter().enumerate() {
        let seq = index as u64 + 1;
        let frame = Frame {
            seq,
            dt: 1e-6,
            pre: 4,
            ch: [vec![0.0; 8], Vec::new(), Vec::new(), Vec::new()],
            ..Default::default()
        };
        push_frames(&out, vec![tag(frame, old)], mode);
        let record = notify_while_waiting(&mut wait, &out, old, || match index {
            0 => source
                .acquire_hw(&crate::model::Model::default(), |_, _| crate::drawpipe::TapRead::Empty)
                .and_then(|(record, ..)| record.raw.map(|raw| raw.frame.seq)),
            1 => {
                let mut g = lock(&out);
                g.hold = true;
                g.record_flow.wake();
                None
            }
            _ => {
                let mut g = lock(&out);
                g.acquisition = Publication::new(new, false);
                g.record_flow.wake();
                None
            }
        });
        assert_eq!(record, (index == 0).then_some(seq));
        let mut g = lock(&out);
        assert_eq!(g.queue.len(), usize::from(index != 0));
        g.queue.clear();
        g.hold = false;
    }
    assert_eq!(crate::drawpipe::counters().losses.overflow.load(std::sync::atomic::Ordering::Relaxed), 0);
}

fn notify_while_waiting(
    wait: &mut RecordWait,
    out: &HwShared,
    identity: Identity,
    notify: impl FnOnce() -> Option<u64> + Send,
) -> Option<u64> {
    let boundary = Barrier::new(2);
    std::thread::scope(|threads| {
        let boundary = &boundary;
        let action = threads.spawn(move || {
            boundary.wait();
            let record = notify();
            boundary.wait();
            record
        });
        // Deliver the event after checking the queue and before parking.
        assert!(wait.poll(
            out,
            None,
            &oos_hal::dho::seq::time::Real,
            || {
                boundary.wait();
                boundary.wait();
                let g = lock(out);
                g.hold || g.acquisition.identity() != identity
            },
            |_| {}
        ));
        action.join().unwrap()
    })
}

#[test]
fn a_queued_fallback_does_not_gate_the_deposit_source() {
    let out = HwShared::default();
    lock(&out).queue.push_back(Frame::default().into());
    let pipe = crate::drawpipe::Pipe::start(0, 0, Box::leak(Box::default()));
    assert!(!RecordWait::default().poll(
        &out,
        Some(&pipe),
        &oos_hal::dho::seq::time::Real,
        || panic!("deposit bypasses the record wait"),
        |_| {}
    ));
    assert_eq!(lock(&out).queue.len(), 1);
}

#[test]
fn publication_counts_both_invariants_before_debug_asserting() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let out = HwShared::default();
    lock(&out).queue.push_back(Frame { seq: 1, ..Default::default() }.into());
    let frames: Vec<_> =
        (0..=super::super::PEAK_BATCH_MAX).map(|n| Frame { seq: n as u64 + 2, ..Default::default() }).collect();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| push_frames(&out, frames, Mode::Peak)));
    assert_eq!(result.is_err(), cfg!(debug_assertions));
    let text = crate::drawpipe::counters().records_text();
    assert!(text.contains("records.unexpected_queue=1 records.sample_mismatch=1"), "{text}");
    let queue = &lock(&out).queue;
    assert_eq!(queue.front().unwrap().seq, 1);
    assert_eq!(queue.len(), if cfg!(debug_assertions) { 1 } else { super::super::PEAK_BATCH_MAX + 2 });
}

#[test]
fn record_grant_is_cold_then_uses_draw_cost_with_the_same_handoff_bound() {
    use std::time::Duration;
    let out = HwShared::default();
    let id = Identity::default();
    let settings = oos_hal::dho::seq::Settings::default();
    let params = oos_hal::dho::hz::prep(1, 1_000_000_000, 4096, 0);
    lock(&out).record_flow.cost.set_identity(id);
    assert_eq!(record_grant(&out, id, 100, &settings, &params, WORK_BUDGET).records(), 12);
    lock(&out).record_flow.cost.observe(id, 20, Duration::from_millis(2), Duration::ZERO);
    assert_eq!(
        record_grant(&out, id, 100, &settings, &params, WORK_BUDGET).records(),
        crate::board::RECORDS_PER_STEP as u32
    );
    assert_eq!(record_grant(&out, id, 7, &settings, &params, WORK_BUDGET).records(), 7);
    let budget = Duration::from_millis(4);
    assert_eq!(record_grant(&out, id, 100, &settings, &params, budget).records(), 40);
    lock(&out).record_flow.cost.observe(id, 1, Duration::from_millis(8), Duration::ZERO);
    assert!(record_grant(&out, id, 100, &settings, &params, budget).serial());
}

#[test]
fn grant_sizes_match_decoding_of_pairs_raw_replay_and_la() {
    use super::super::hw;
    use oos_hal::dho::{hz, la, seq};

    for mask in [0, 1, 5, 7, 15] {
        let params = hz::prep(mask, 1_000_000_000, 4096, 0);
        let settings = seq::Settings { mask, header: true, compress: 4, ..Default::default() };
        let decode = |s: &seq::Settings| hw::decode(&vec![0; seq::frame_bytes(s, &params)], s, &params, 0);
        let count = |frame: &Frame| frame.ch.iter().map(Vec::len).sum::<usize>();
        let pairs = count(&decode(&settings));
        let raw = count(&decode(&seq::raw_settings(&settings)));
        assert_eq!(record_sizes(&settings, &params), (pairs, raw));
        let settings = seq::Settings { la: Some(la::Set::default()), ..settings };
        let n = la::window(&la::timing(&params, mask)).0 as usize;
        let words = la::words(&vec![0; n * 2], n);
        let logic = hw::la_record(words, &settings, &params);
        assert_eq!(record_sizes(&settings, &params), (pairs, raw + logic.words.len()));
    }
}

#[test]
fn batch_result_updates_readings_and_filters_frames_with_the_live_identity() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let out = HwShared::default();
    lock(&out).cores = true;
    let request = snapshot(&out);
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut settings = oos_core::change::Shadow::new(&model, Default::default(), &config).unwrap();
    let id =
        settings.observe(&model, Default::default(), &[oos_core::change::Request::Clear], &config).unwrap().identity();
    {
        let mut g = lock(&out);
        g.cores = false;
        g.acquisition = Publication::new(id, false);
    }
    let frames = vec![
        tag(Frame { seq: 1, ..Default::default() }, request.identity),
        tag(Frame { seq: 2, ..Default::default() }, id),
    ];
    finish_batch(&out, frames, Mode::Normal, |g| g.decoded += 2);
    let g = lock(&out);
    assert!(request.run.cores && !g.cores);
    assert_eq!(g.acquisition.identity(), id);
    assert_eq!((g.decoded, g.queue.len(), g.frame.seq), (2, 1, 2));
}
