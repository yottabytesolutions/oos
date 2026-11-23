// SPDX-License-Identifier: GPL-2.0-or-later
//! Default (including telemetry-off) proposal through actual prearm, HAL capture and warmed pool reuse.
use super::*;
use crate::drawpipe::budget::WORK_BUDGET;
use crate::model::{AcqMode, Model, RunState, TrigMode};
fn unit(mask: u32) -> Unit {
    let mut unit = Unit::new(142);
    let mut model = Model::default();
    model.run = RunState::Run;
    model.acq = AcqMode::Normal;
    model.trig_mode = TrigMode::Normal;
    model.tscale = crate::model::TIME_SCALES.iter().position(|&v| v == 1e-6).unwrap();
    model.cores = true;
    for c in 0..4 {
        model.ch[c].on = mask & (1 << c) != 0;
    }
    model.trig_src = 1;
    let original = crate::board::dho::hw::settings(&model, false, &Default::default());
    unit.params = hz::prep(original.mask, seq::tdiv_fs(original.tdiv), original.depth, 0);
    unit.settings = crate::board::dho::hw::batch_plan(&original, &unit.params, 1);
    unit.settings.frames = 142;
    assert_eq!(unit.params.rate, if mask == 3 { 625_000_000 } else { 312_500_000 });
    assert_eq!(unit.params.memsel, if mask == 3 { 7032 } else { 3520 });
    unit.state = seq::State::handover(mask, None);
    unit.state.configure(&unit.settings, None, None).unwrap();
    (unit.engine, unit.bus, unit.input) = sim::engine(sim::Engine::new(0, 1_000_000_000, u32::MAX, unit.params.ratio));
    unit.pipe.publish_view(&model, 970, 448, crate::sparse::Interp::Exact, unit.id);
    unit
}
#[test]
fn admission_preserves_the_armed_count_then_uses_the_new_grant() {
    for mask in [3, 15] {
        let mut unit = unit(mask);
        assert!(unit.offer(142, false).is_none());
        let proposed = crate::board::dho::hw::batch::read_grant(Some(&unit.pipe), unit.id, 142, WORK_BUDGET).records();
        assert_eq!(proposed, 12);
        let mut total = 0;
        for (index, expected) in [142, proposed, proposed].into_iter().enumerate() {
            assert_eq!(unit.read(proposed), expected, "an armed bank is consumed under its original geometry");
            total += expected;
            if index == 2 {
                break;
            }
            let bytes = unit.source.destination().data().to_vec();
            let next = unit.offer(proposed, true).unwrap();
            assert_eq!(unit.source.destination().data(), bytes, "new arm never overwrites the completed capture");
            let completed = unit.source.completed().take().unwrap();
            unit.pipe.return_buffer(Box::new(completed));
            unit.source.switch(&unit.pipe, next);
        }
        assert_eq!(unit.engine.borrow().triggers, total as u64);
        assert_eq!(unit.source.made, 2);
        assert!(unit.arm_scratch.data().is_empty());
    }
}
