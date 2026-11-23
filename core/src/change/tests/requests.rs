// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn requests_resolve_together_and_unresolved_accepts_nothing() {
    let mut g = Generations::default();
    let a = Token::new(Requester::Autoset, 1, 1);
    let b = Token::new(Requester::Bode, 1, 1);
    let first = g.resolve(Classes::NONE, &[]).unwrap();
    assert!(!first.accepts(a, 0));
    let c = g.resolve(Classes::ACQ, &[Request::Epoch(a), Request::Epoch(b)]).unwrap();
    assert_eq!((c.identity().gen(), c.identity().epoch(), c.identity().stats_gen()), (1, 1, 0));
    assert!(c.accepts(a, 1) && c.accepts(b, 1));
    assert!(!c.accepts(a, 0) && !c.accepts(a, 2));
    assert!(!c.accepts(Token::new(Requester::Autoset, 0, 1), 1));
    let retry = Token::new(Requester::Autoset, 1, 2);
    let c = g.resolve(Classes::NONE, &[Request::Epoch(retry)]).unwrap();
    assert_eq!((c.identity().gen(), c.identity().epoch()), (1, 2));
    assert!(!c.accepts(a, 1));
    assert!(c.accepts(retry, 2));
    let id = g.identity();
    assert_eq!(g.resolve(Classes::ACQ, &[Request::Epoch(a), Request::Epoch(a)]), Err(Error::DuplicateToken));
    assert_eq!(g.identity(), id);
}

#[test]
fn equal_settings_requests_repeat_and_overflow_is_explicit() {
    let mut g = Generations::default();
    for r in [Request::Clear, Request::Single, Request::Run, Request::CreateMask] {
        let c = g.resolve(Classes::NONE, &[r]).unwrap();
        assert_eq!(c.classes(), r.classes());
        let d = g.resolve(Classes::NONE, &[r]).unwrap();
        assert_eq!(d.classes(), c.classes());
        assert_eq!(d.identity().gen(), 0);
        assert_eq!(d.identity().epoch() - c.identity().epoch(), u64::from(r != Request::CreateMask));
    }
    let mut outbox = Outbox::default();
    for i in 0..64 {
        outbox.push(Command::Request(Request::Epoch(Token::new(Requester::Autoset, 1, i)))).unwrap();
    }
    let full = outbox.clone();
    assert_eq!(outbox.push(Command::Run(RunState::Stop)), Err(Error::Full));
    assert_eq!(outbox, full);
    let mut requests = [Request::Clear; 64];
    assert_eq!(outbox.requests(&mut requests), 64);
    let c = g.resolve(Classes::NONE, &requests).unwrap();
    for i in 0..64 {
        assert_eq!(c.epoch(Token::new(Requester::Autoset, 1, i)), Some(c.identity().epoch()));
    }
    outbox.clear();
    assert!(outbox.entries().is_empty());
}

#[test]
fn internal_writes_apply_before_external_inputs() {
    let mut outbox = Outbox::default();
    outbox.push(Command::Timebase(3, 0.1)).unwrap();
    let mut m = Model::default();
    outbox.apply(&mut m);
    // Existing external intake is applied afterwards, before normalize and token resolution.
    m.tscale = 8;
    assert_eq!((m.tscale, m.delay), (8, 0.1));
}

#[test]
fn retained_snapshot_capacities_cannot_cross_the_byte_cap() {
    let mut first = Model::default();
    first.ch_label[0] = "a".repeat(40_000);
    let mut shadow = Shadow::new(&first, UiAcq::default(), &config(4)).unwrap();
    let mut second = Model::default();
    second.ch_label[1] = "b".repeat(40_000);
    let id = shadow.identity();
    assert_eq!(shadow.observe(&second, UiAcq::default(), &[], &config(4)), Err(Error::SnapshotTooLarge));
    assert_eq!(shadow.identity(), id);
    let (a, b) = shadow.bytes();
    assert!(a < 64 * 1024 && b < 64 * 1024);
    let mut outbox = Outbox::default();
    outbox.push(Command::Run(RunState::Stop)).unwrap();
    let queued = outbox.clone();
    second.bode_addr = "c".repeat(64 * 1024);
    assert_eq!(outbox.capture(&first, &second), Err(Error::PayloadTooLarge));
    assert_eq!(outbox, queued);
}

#[test]
fn a_segmented_run_request_adds_history_without_a_settings_diff() {
    let model = Model { segments: 3, ..Model::default() };
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config(4)).unwrap();
    let changes = shadow.observe(&model, UiAcq::default(), &[Request::Run], &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::HW_AVERAGE | Classes::HISTORY);
    assert_eq!(changes.identity().gen(), 0);
    assert_eq!(changes.identity().epoch(), 1);
}

#[test]
fn requested_sweep_changes_are_companions_even_when_accepted_run_is_equal() {
    let initial = Model { trig_mode: TrigMode::Single, ..Model::default() };
    let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
    let mut current = initial.clone();
    current.trig_mode = TrigMode::Auto;
    let requests = [Request::Run, Request::ResumeSweep(TrigMode::Auto)];
    let changes = shadow.observe(&current, UiAcq::default(), &requests, &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::HW_AVERAGE);
    assert_eq!((changes.identity().gen(), changes.identity().epoch()), (0, 1));
    current.trig_mode = TrigMode::Normal;
    current.trig_level += 1.0;
    let changes = shadow.observe(&current, UiAcq::default(), &[Request::Run], &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::ACQ | Classes::STATS | Classes::HW_AVERAGE);
    assert_eq!((changes.identity().gen(), changes.identity().epoch()), (1, 2));
    current.trig_mode = TrigMode::Auto;
    let changes = shadow.observe(&current, UiAcq::default(), &[], &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::ACQ | Classes::STATS, "ordinary sweep changes retain their classes");
}

#[test]
fn run_restores_either_single_sweep_without_reclassifying_settings() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        let initial = Model { trig_mode: TrigMode::Single, ..Model::default() };
        let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
        let current = Model { trig_mode: mode, ..initial };
        let requests = [Request::Run, Request::ResumeSweep(mode)];
        let changes = shadow.observe(&current, UiAcq::default(), &requests, &config(4)).unwrap();
        assert_eq!(changes.classes(), Classes::HW_AVERAGE);
        assert_eq!((changes.identity().gen(), changes.identity().epoch()), (0, 1));
    }
}

#[test]
fn armed_single_restores_its_sweep_without_a_run_state_edge() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        let initial = Model { trig_mode: TrigMode::Single, run: RunState::Run, ..Model::default() };
        let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
        let current = Model { trig_mode: mode, ..initial };
        let changes = shadow.observe(&current, UiAcq::default(), &[Request::ResumeSweep(mode)], &config(4)).unwrap();
        assert_eq!(changes.classes(), Classes::NONE);
        assert_eq!(changes.identity(), Identity::default());
    }
}

#[test]
fn requested_normal_sweep_is_not_a_run_companion() {
    let initial = Model::default();
    let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
    let current = Model { trig_mode: TrigMode::Normal, ..initial };
    let changes = shadow.observe(&current, UiAcq::default(), &[Request::Run], &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::ACQ | Classes::STATS | Classes::HW_AVERAGE);
}

#[test]
fn view_reset_is_repeatable_without_advancing_acquisition_or_statistics() {
    let mut g = Generations::default();
    for _ in 0..2 {
        let c = g.resolve(Classes::NONE, &[Request::ViewReset]).unwrap();
        assert_eq!(c.classes(), Classes::VIEW);
        assert_eq!(c.identity(), crate::change::Identity::default());
    }
}

#[test]
fn playback_origin_classifies_only_the_exact_driver_position_delta() {
    for zoom in [false, true] {
        for user_edit in [0, 1, 2, 3] {
            let initial = Model::default();
            let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
            let mut current = initial.clone();
            let before = if zoom { initial.zdelay } else { initial.delay };
            let request = Request::PlaybackPosition {
                zoom,
                before: (before + if user_edit == 1 { 0.0001 } else { 0.0 }).to_bits(),
                after: (before + 0.0002).to_bits(),
            };
            if zoom {
                current.zdelay = before + 0.0002;
            } else {
                current.delay = before + 0.0002;
            }
            if user_edit == 2 {
                current.delay += 0.0003;
            }
            if user_edit == 3 {
                current.ch[0].offset += 0.0001;
            }
            let changes = shadow.observe(&current, UiAcq::default(), &[request], &config(4)).unwrap();
            if user_edit == 0 {
                assert_eq!(changes.classes(), Classes::VIEW);
                assert_eq!(changes.identity(), Identity::default());
            } else {
                assert!(changes.classes().contains(Classes::MAPPING));
                if !zoom || user_edit != 1 {
                    assert!(changes.classes().contains(Classes::ACQ | Classes::STATS));
                    assert_eq!((changes.identity().gen(), changes.identity().stats_gen()), (1, 1));
                }
            }
        }
    }
}

#[test]
fn consecutive_playback_requests_preserve_statistics_and_expire_after_acceptance() {
    let mut model = Model::default();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config(4)).unwrap();
    let requests = [
        Request::PlaybackPosition { zoom: false, before: model.delay.to_bits(), after: 0.0001f32.to_bits() },
        Request::PlaybackPosition { zoom: false, before: 0.0001f32.to_bits(), after: 0.0002f32.to_bits() },
    ];
    model.delay = 0.0002;
    let changes = shadow.observe(&model, UiAcq::default(), &requests, &config(4)).unwrap();
    assert_eq!(changes.classes(), Classes::VIEW);
    model.delay = 0.0003;
    let changes = shadow.observe(&model, UiAcq::default(), &[], &config(4)).unwrap();
    assert!(changes.classes().contains(Classes::ACQ | Classes::STATS));
    assert_eq!(changes.identity().stats_gen(), 1);
}

#[test]
fn explicit_sweep_with_run_from_single_retains_its_classes() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        let initial = Model { trig_mode: TrigMode::Single, ..Model::default() };
        let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
        let current = Model { trig_mode: mode, ..initial };
        let changes = shadow.observe(&current, UiAcq::default(), &[Request::Run], &config(4)).unwrap();
        assert_eq!(changes.classes(), Classes::ACQ | Classes::STATS | Classes::HW_AVERAGE);
        assert_eq!((changes.identity().gen(), changes.identity().stats_gen()), (1, 1));
    }
}

#[test]
fn explicit_sweep_after_restoration_retains_its_classes() {
    for (restored, selected) in [(TrigMode::Auto, TrigMode::Normal), (TrigMode::Normal, TrigMode::Auto)] {
        let initial = Model { trig_mode: TrigMode::Single, ..Model::default() };
        let mut shadow = Shadow::new(&initial, UiAcq::default(), &config(4)).unwrap();
        let current = Model { trig_mode: selected, ..initial };
        let requests = [Request::Run, Request::ResumeSweep(restored)];
        let changes = shadow.observe(&current, UiAcq::default(), &requests, &config(4)).unwrap();
        assert_eq!(changes.classes(), Classes::ACQ | Classes::STATS | Classes::HW_AVERAGE);
        assert_eq!((changes.identity().gen(), changes.identity().stats_gen()), (1, 1));
    }
}

#[test]
fn software_average_single_route_changes_preserve_the_acquisition_class_boundary() {
    use Request::{ResumeSweep, Run, Single};

    for sweep in [TrigMode::Auto, TrigMode::Normal] {
        let single = Model { acq: AcqMode::Average, avg_hw: false, trig_mode: TrigMode::Single, ..Model::default() };
        let continuous = Model { trig_mode: sweep, ..single.clone() };
        let resume = ResumeSweep(sweep);
        let cases: [(bool, &[Request], Classes, u64, u64); 5] = [
            (true, &[Single], Single.classes(), 0, 1),
            (false, &[resume], Classes::NONE, 0, 0),
            (false, &[Run, resume], Run.classes(), 0, 1),
            (true, &[], Classes::ACQ | Classes::STATS, 1, 0),
            (false, &[], Classes::ACQ | Classes::STATS, 1, 0),
        ];
        for (arm, requests, classes, gen, epoch) in cases {
            let (initial, current) = if arm { (&continuous, &single) } else { (&single, &continuous) };
            let batch = UiAcq::of(false, true, false, false, Layout::default());
            let record = UiAcq::default();
            let (before, after) = if arm { (batch, record) } else { (record, batch) };
            let cfg = config(4);
            let mut shadow = Shadow::new(initial, before, &cfg).unwrap();
            let changes = shadow.observe(current, after, requests, &cfg).unwrap();
            let id = changes.identity();
            assert_eq!((changes.classes(), id.gen(), id.epoch()), (classes, gen, epoch), "{sweep:?}, {requests:?}");
        }
    }
}
