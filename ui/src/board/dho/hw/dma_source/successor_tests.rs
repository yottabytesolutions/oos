// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use oos_hal::dho::{hz, seg::sim, seq};
use std::{cell::RefCell, rc::Rc};

struct Unit {
    pipe: Pipe,
    settings: Settings,
    id: oos_core::change::Identity,
    state: seq::State,
    params: hz::Params,
    engine: Rc<RefCell<sim::Engine>>,
    bus: sim::SimBus,
    input: sim::SimC2h,
    source: DmaSource<oos_hal::dho::bus::DmaBuf>,
    armed: Option<ArmedBatch>,
    // Arm-only execution must never materialize DMA bytes in this empty test port.
    arm_scratch: oos_hal::dho::bus::DmaBuf,
}
impl Unit {
    fn new(count: u32) -> Self {
        let settings = Settings { tdiv: 1e-6, frames: count, window: None, ..Default::default() };
        let mut state = seq::State::handover(1, None);
        let params = state.configure(&settings, None, None).unwrap().1;
        let (engine, bus, input) = sim::engine(sim::Engine::new(0, 1_000_000_000, u32::MAX, params.ratio));
        Self {
            pipe: Pipe::start(0, 0, Box::leak(Box::default())),
            settings,
            id: Default::default(),
            state,
            params,
            engine,
            bus,
            input,
            source: DmaSource::default(),
            armed: None,
            arm_scratch: oos_hal::dho::bus::DmaBuf::new(),
        }
    }
    fn offer(&mut self, count: u32, loan: bool) -> Option<oos_hal::dho::bus::DmaBuf> {
        let Self { pipe, settings, id, state, params, bus, input, source, armed, arm_scratch, .. } = self;
        let plan = ArmPlan::new(settings, *id, true, Some(pipe), false);
        plan.prearm(
            source,
            armed,
            loan.then_some(pipe),
            Some(Grant::Overlapping(count)),
            || false,
            |next, _| {
                assert_eq!(next.frames, count);
                assert_eq!(seq::frame_bytes(next, params), seq::frame_bytes(settings, params));
                seq::execute_bus(&state.arm_only_steps(next, params), bus, input, arm_scratch, |_| {}).is_ok()
            },
        )
    }
    fn read(&mut self, proposed: u32) -> u32 {
        let Self { pipe, settings, id, state, params, bus, input, source, armed, .. } = self;
        let mut current = Settings { frames: proposed, ..settings.clone() };
        let pair = (id.gen(), id.epoch());
        limit_frames(&mut current, proposed, armed.as_ref(), (pair, true, Some(pipe)));
        let plan = ArmPlan::new(&current, *id, true, Some(pipe), false);
        let (ahead, _) = plan
            .prepare(
                source,
                armed,
                &oos_hal::dho::seq::time::Real,
                || false,
                |_| panic!("resize cannot discard a captured ring"),
            )
            .unwrap();
        assert!(ahead);
        let got =
            seq::acquire_batch(state, &current, params, bus, input, source.destination(), ahead, 0, |_| {}).unwrap();
        assert_eq!(got, current.frames);
        assert_complete(source.destination().data(), got, seq::frame_bytes(&current, params));
        *settings = current;
        got
    }
}
fn assert_complete(bytes: &[u8], count: u32, fb: usize) {
    assert_eq!(bytes.len(), count as usize * fb);
    for index in 0..count as usize {
        assert_eq!(
            u16::from_le_bytes(bytes[index * fb..index * fb + 2].try_into().unwrap()),
            sim::Engine::word(index as u32, 0)
        );
    }
}

#[test]
fn production_prearm_consumes_old_then_new_and_steady_without_losing_geometry_or_records() {
    // These are injected planner outputs, not a production cap or a calibrated service model.
    for (old, new) in [(128, 88), (128, 32), (32, 128)] {
        let mut unit = Unit::new(old);
        assert!(unit.offer(old, false).is_none());
        let mut captured = 0;
        for (index, expected) in [old, new, new].into_iter().enumerate() {
            assert_eq!(unit.read(new), expected);
            captured += expected;
            if index == 2 {
                break;
            }
            let before = unit.source.destination().data().to_vec();
            let spare = unit.offer(new, true).unwrap();
            assert_eq!(unit.settings.frames, expected, "successor never mutates current capture settings");
            assert_eq!(unit.source.destination().data(), before, "completed DMA is intact until its normal handoff");
            assert_eq!(unit.armed.as_ref().unwrap().settings.frames, new);
            let completed = unit.source.completed().take().unwrap();
            unit.pipe.return_buffer(Box::new(completed));
            unit.source.switch(&unit.pipe, spare);
        }
        assert_eq!(captured, old + 2 * new);
        assert_eq!(unit.engine.borrow().triggers, captured as u64);
        assert_eq!(unit.source.made, 2, "actual source pool returned/reused warmed destinations");
        assert!(unit.arm_scratch.data().is_empty(), "no arm-only read or extra materialized destination");
    }
}

#[test]
fn canceled_successor_does_not_arm_or_lose_current_completed_bytes() {
    use std::cell::Cell;
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let settings = Settings::default();
    let id = oos_core::change::Identity::default();
    let plan = ArmPlan::new(&settings, id, true, Some(&pipe), false);
    let mut source = DmaSource::<oos_hal::dho::bus::DmaBuf>::default();
    source.destination().read(&mut &b"completed"[..], 9, |_, _| {}).unwrap();
    let mut armed = None;
    let checks = Cell::new(0);
    let spare = plan.prearm(
        &mut source,
        &mut armed,
        Some(&pipe),
        Some(Grant::Overlapping(88)),
        || {
            let n = checks.get();
            checks.set(n + 1);
            n != 0
        },
        |_, _| panic!("cancellation must precede hardware"),
    );
    assert!(armed.is_none());
    assert_eq!(source.destination().data(), b"completed");
    pipe.return_buffer(Box::new(spare.unwrap()));
    assert!(source.spare(&pipe).is_some());
    assert_eq!(source.made, 2);
    let mut changed = Settings { mask: settings.mask ^ 2, frames: 88, ..settings.clone() };
    let old = ArmedBatch { settings: settings.clone(), pair: (0, 0), route: true, deposit: false, tap: None };
    assert!(!old.preserve_frames(&mut changed, (0, 0), true, None));
    assert_eq!(changed.frames, 88, "real geometry changes keep normal cancellation semantics");
}

mod renderer_tests;
mod serial_tests;
