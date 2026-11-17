// SPDX-License-Identifier: GPL-2.0-or-later

//! A held non-fresh SIM capture that never reaches acceptance changes nothing that is shown.
use super::*;

// Break it catches: a qualified non-fresh hit installed at finish instead of held until its StatsReset is accepted.
// STOP before that acceptance drops the held capture, so the reference, the Eye and its picture stay the accepted
// ones through the accepting step and later polls.
#[test]
fn a_held_nonfresh_sim_hit_dropped_by_stop_leaves_the_reference_and_eye() {
    for asynchronous in [false, true] {
        let (mut ui, _old, fresh) = fixture();
        ui.m.eye_rate = ui.sim_lane().bits_per_s;
        change::reset(&mut ui);
        let accepted = Accepted::of(&ui);
        let records = eye_records(&ui);
        let _held = finish_nonfresh(&mut ui, fresh, asynchronous);
        accepted.assert_held(&ui);
        ui.key(key::RUNSTOP);
        change::begin_step(&mut ui, 0.0).unwrap();
        change::finish_step(&mut ui);
        for _ in 0..3 {
            ui.sim_poll();
        }
        let reference = ui.refs[0].as_ref().expect("the accepted reference stays");
        assert!(Arc::ptr_eq(&accepted.samples, &reference.v), "held data became visible ({asynchronous})");
        assert_eq!(ui.replay.captures, accepted.captures);
        assert_eq!(ui.replay.sim_revision, accepted.revision);
        assert_eq!(eye_records(&ui).to_bits(), records.to_bits());
        assert_eq!(ui.azr.page.img, accepted.image);
    }
}
