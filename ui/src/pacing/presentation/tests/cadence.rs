// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{model::Model, pacing::CostView};

#[test]
fn measured_view_cost_changes_the_target_without_changing_the_step_rate() {
    let mut model = Model { zoom: true, ..Model::default() };
    let heavy = CostView::new(&model, true);
    model.zoom = false;
    let light = CostView::new(&model, true);
    let start = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, start, REFRESH);
    let mut panel = Panel::default();
    let mut previous = None;
    let mut first_heavy = None;
    let mut first_light = None;
    let mut last_target = 2;
    for index in 1..=260 {
        let sequence = index * 3;
        let seal = start + REFRESH * sequence;
        let edge = Flip { sequence, at: seal };
        assert!(clock.advance(seal, Some(edge)).0.is_some());
        let (view, ms) = match index {
            1..=40 => (light, 14),
            41..=74 => (heavy, if index.is_multiple_of(8) { 14 } else { 24 }),
            75..=190 => (heavy, if index.is_multiple_of(2) { 19 } else { 20 }),
            _ => (light, 14),
        };
        clock.select_view(view);
        let mut picture = clock.seal();
        let target = picture.target.sequence.unwrap() - sequence;
        if (41..=74).contains(&index) && target == 3 {
            first_heavy.get_or_insert(index);
        }
        if (75..=190).contains(&index) {
            assert_eq!(target, 3, "boundary jitter cannot lower the target");
        }
        if index > 190 && target == 2 {
            first_light.get_or_insert(index);
        }
        if index <= 40 {
            assert_eq!(target, 2);
        }
        if let Some(last) = previous.filter(|_| target == last_target) {
            assert_eq!(picture.target.sequence.unwrap() - last, 3);
        }
        previous = picture.target.sequence;
        last_target = target;
        let ready = seal + Duration::from_millis(ms);
        picture.helper_done(seal, ready);
        panel.stage(0, picture, ready);
        let (at, cost) = panel.take_ready_cost().unwrap();
        clock.observe_ready(at, cost);
        // Even when the last sample changes the policy, the staged picture keeps its own target.
        assert_eq!(panel.ready.take().unwrap().1.target.sequence, Some(sequence + target));
    }
    assert_eq!(first_heavy, Some(73));
    assert_eq!(first_light, Some(223));
    assert_eq!(clock.skipped(), 0);
}

#[test]
fn a_requested_readback_after_done_leaves_two_consecutive_flips_on_target() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    let readback = crate::system::readback::Readback::default();
    let mut request = readback.request();
    let capture = readback.take().unwrap();
    let mut first = script.picture(0, 2);
    first.helper_done(script.at(4), script.at(12));
    script.poll(10, None);
    script.panel.stage(1, first, script.at(12));
    script.poll(12, None);
    script.poll(20, Some(20));
    assert!(!request.ready(&readback));
    capture.complete(&crate::gfx::Canvas::new(1, 1)); // Pixels arrive at 23 ms, after this flip.
    assert!(request.ready(&readback));
    assert!(readback.take().is_none());
    let mut next = script.picture(30, 2);
    next.helper_done(script.at(34), script.at(42));
    script.poll(40, None);
    script.panel.stage(2, next, script.at(42));
    script.poll(42, None);
    script.poll(50, Some(50));
    assert_eq!((script.history.shown, script.history.late), (2, 0));
    assert!(script.requests.is_empty());
}
