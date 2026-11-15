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
