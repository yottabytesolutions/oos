// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{identity::Publication, Frame, HwShared};
use oos_core::change::{Request, Shadow, UiAcq};
use oos_hal::sync::lock;

#[test]
fn epoch_only_boundary_rejects_old_work() {
    let mut model = Model::default();
    let config = crate::model::config();
    let mut changes = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let shared = HwShared::default();
    for generation in [false, true] {
        let old = changes.identity();
        if generation {
            model.tscale += 1;
        }
        let requests = if generation { &[][..] } else { &[Request::Clear][..] };
        let id = changes.observe(&model, UiAcq::default(), requests, &config).unwrap().identity();
        let applied = Some((old.gen(), old.epoch()));
        assert!(publication::boundary(applied, id), "prearmed work and replay must be invalidated");
        assert!(!publication::boundary(Some((id.gen(), id.epoch())), id));
        if !generation {
            assert_eq!(id.gen(), old.gen(), "epoch alone does not configure settings");
        }
        let want = crate::board::board().want(&model, &Default::default());
        {
            let mut g = lock(&shared);
            g.want = Some(want.clone());
            g.acquisition = Publication::new(id, generation);
        }
        let (_, published, _, _, _, _, _, tagged, route) = publication::snapshot(&shared);
        assert_eq!(published, Some(want));
        assert_eq!(tagged, id);
        assert_eq!(route, generation);
        publication::push_frames(
            &shared,
            vec![
                Frame { seq: 1, cfg: old.gen(), stamp: old.epoch(), ..Default::default() },
                Frame { seq: 2, cfg: id.gen(), stamp: id.epoch(), ..Default::default() },
                Frame { seq: 3, cfg: id.gen(), stamp: id.epoch() + 1, ..Default::default() },
            ],
        );
        let mut g = lock(&shared);
        assert_eq!(g.queue.len(), 1, "reject both old and different newer pairs, retain valid work");
        assert_eq!(g.queue.pop_front().unwrap().seq, 2);
        assert_eq!(g.frame.seq, 2);
    }
}

#[test]
fn segmented_and_roll_restart_counts_follow_hardware_changes_only() {
    use crate::board::{Slow, Want};
    use oos_hal::dho::{bus::Bus, roll, seg, seq};
    let mut model = Model { segments: 4, ..Default::default() };
    let mut want = crate::board::board().want(&model, &Default::default());
    let mut state = seq::State::handover(1, None);
    let p = state.configure(&want, None, None).unwrap().1;
    let (_, mut bus, mut c2h) = seg::sim::engine(seg::sim::Engine::new(0, 1_000_000_000_000, 1, p.ratio));
    let mut starts = 0;
    let mut roll_counts = (0, 0);
    seg::start(&mut state, &want, &p, &mut bus, &mut c2h).unwrap();
    starts += 1;
    bus.apply(&roll::start(&mut state.sh, want.mask));
    roll_counts.1 += 1;
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    for boundary in 0..10 {
        let old = shadow.identity();
        let mut needs = UiAcq::default();
        let request = restart_boundary(&mut model, &mut needs, boundary);
        let requests: Vec<_> = request.into_iter().collect();
        let id = shadow.observe(&model, needs, &requests, &config).unwrap().identity();
        let next = if boundary == 7 { crate::board::board().want(&model, &Default::default()) } else { want.clone() };
        let mode = if boundary == 9 { Slow::Roll } else { Slow::Off };
        let reconfigure = publication::reconfigure(Some((&want, Slow::Off)), &next, mode);
        if reconfigure {
            seg::stop(&mut state, &mut bus, &mut c2h).unwrap();
            bus.apply(&roll::stop(&mut state.sh));
            roll_counts.0 += 1;
            let p = state.configure(&next, None, None).unwrap().1;
            seg::start(&mut state, &next, &p, &mut bus, &mut c2h).unwrap();
            starts += 1;
            bus.apply(&roll::start(&mut state.sh, next.mask));
            roll_counts.1 += 1;
        }
        assert_eq!(reconfigure, boundary == 7 || boundary == 9, "boundary {boundary}");
        if boundary < 7 {
            assert!(publication::boundary(Some((old.gen(), old.epoch())), id));
            assert_eq!(
                (starts, roll_counts),
                (1, (0, 1)),
                "boundary {boundary} must preserve recording and STOP browse"
            );
        }
        want = next;
    }
    assert_eq!((starts, roll_counts), (3, (2, 3)));
    let _: Want = want;
}

fn restart_boundary(model: &mut Model, needs: &mut UiAcq, boundary: usize) -> Option<Request> {
    match boundary {
        0 => Some(Request::Clear),
        1 => Some(Request::Run),
        2 => Some(Request::Single),
        3 | 4 => Some(Request::Epoch(oos_core::change::Token::new(
            if boundary == 3 { oos_core::change::Requester::Autoset } else { oos_core::change::Requester::Bode },
            1,
            1,
        ))),
        5 => {
            model.mask_src = 1;
            None
        }
        6 => {
            *needs = UiAcq::of(false, true, false, false, Default::default());
            None
        }
        7 => {
            model.tscale += 1;
            None
        }
        _ => None,
    }
}

#[test]
fn worker_identity_rejection_and_queue_overflow_have_distinct_counts() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let counters = crate::drawpipe::counters();
    let worker = counters.losses.worker_push.load(std::sync::atomic::Ordering::Relaxed);
    let overflow = counters.losses.overflow.load(std::sync::atomic::Ordering::Relaxed);
    let shared = HwShared::default();
    let mut frames = vec![Frame { cfg: 1, ..Default::default() }; 2];
    frames.extend((0..crate::board::QUEUE_MAX + 3).map(|_| Frame::default()));
    publication::push_frames(&shared, frames);
    assert_eq!(lock(&shared).queue.len(), crate::board::QUEUE_MAX);
    assert_eq!(counters.losses.worker_push.load(std::sync::atomic::Ordering::Relaxed) - worker, 2);
    assert_eq!(counters.losses.overflow.load(std::sync::atomic::Ordering::Relaxed) - overflow, 3);
}
