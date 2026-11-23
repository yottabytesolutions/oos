// SPDX-License-Identifier: GPL-2.0-or-later

//! One of three DMA destinations must be source-owned before the FPGA arms.

use super::super::clock::{LoopClock, Wake};
#[cfg(any(target_os = "linux", test))]
use crate::drawpipe::admission::Grant;
use crate::drawpipe::{self, Pipe, RawBatch, TapReservation};
use oos_hal::dho::seq::Settings;
use std::sync::Arc;
use std::time::Duration;

const POOL: usize = 3;
#[cfg(any(target_os = "linux", test))]
const HOUSEKEEPING: Duration = Duration::from_millis(100);

pub(super) struct DmaSource<T> {
    made: usize,
    current: Option<T>,
}

impl<T: Default + RawBatch + 'static> Default for DmaSource<T> {
    fn default() -> Self {
        Self { made: 1, current: Some(T::default()) }
    }
}

impl<T: Default + RawBatch + 'static> DmaSource<T> {
    fn take(&mut self, pipe: Option<&Pipe>) -> Option<T> {
        if let Some(buffer) = pipe.and_then(Pipe::take_buffer) {
            return Some(*buffer.downcast::<T>().expect("the source is the sole production DMA type"));
        }
        (self.made < POOL).then(|| {
            self.made += 1;
            T::default()
        })
    }

    /// The existing board loop rechecks mode/commands after any wake, including a ready-before-wait race.
    #[cfg(any(target_os = "linux", test))]
    pub(super) fn ready(
        &mut self,
        identity: oos_core::change::Identity,
        clock: &impl LoopClock,
        canceled: impl Fn() -> bool,
    ) -> bool {
        let pipe = self.current.is_none().then(drawpipe::pipe).flatten();
        self.ready_from(pipe, identity, clock, canceled, HOUSEKEEPING)
    }

    fn ready_from(
        &mut self,
        pipe: Option<&Pipe>,
        identity: oos_core::change::Identity,
        clock: &impl LoopClock,
        canceled: impl Fn() -> bool,
        deadline: Duration,
    ) -> bool {
        if self.current.is_some() {
            return !canceled();
        }
        let observed = pipe.map(Wake::generation);
        self.current = self.take(pipe);
        if canceled() {
            return false;
        }
        if self.current.is_some() {
            return true;
        }
        // All materialized destinations are leased through the process pipe.
        if let Some((pipe, observed)) = pipe.zip(observed) {
            pipe.note_source_wait(identity, clock.park(pipe, observed, deadline));
        }
        false
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn destination(&mut self) -> &mut T {
        self.current.as_mut().expect("ready or prearm reserved a DMA destination")
    }

    /// If the completed destination will be loaned, only a distinct spare can back prearm.
    pub(super) fn spare(&mut self, pipe: &Pipe) -> Option<T> {
        self.take(Some(pipe))
    }

    pub(super) fn completed(&mut self) -> &mut Option<T> {
        &mut self.current
    }

    pub(super) fn switch(&mut self, pipe: &Pipe, next: T) {
        if let Some(previous) = self.current.replace(next) {
            // A rejected handoff did not take the completed destination.
            pipe.return_buffer(Box::new(previous));
        }
    }
}

/// Settings and read authorities that were current when the FPGA armed ahead.
#[cfg(any(target_os = "linux", test))]
pub(super) struct ArmedBatch {
    settings: Settings,

    pair: (u64, u64),
    route: bool,
    deposit: bool,
    tap: Option<Arc<TapReservation>>,
}

#[cfg(any(target_os = "linux", test))]
impl ArmedBatch {
    pub(super) fn preserve_frames(
        &self,
        settings: &mut Settings,
        pair: (u64, u64),
        route: bool,
        pipe: Option<&Pipe>,
    ) -> bool {
        let proposed = settings.frames;
        settings.frames = self.settings.frames;
        if self.matches(settings, pair, route, pipe) {
            return true;
        }
        settings.frames = proposed;
        false
    }

    fn matches(
        &self,
        settings: &Settings,
        pair: (u64, u64),
        route: bool,
        pipe: Option<&crate::drawpipe::Pipe>,
    ) -> bool {
        self.settings == *settings
            && self.pair == pair
            && self.route == route
            && self.deposit == pipe.is_some()
            && match (pipe, &self.tap) {
                (Some(pipe), Some(tap)) => pipe.current_tap(tap),
                (Some(pipe), None) => !pipe.staged_taps(),
                (None, None) => true,
                (None, Some(_)) => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::Counters;
    use oos_hal::dho::bus::DmaBuf;

    #[test]
    fn three_owners_block_arm_then_return_allows_one_unless_a_command_is_pending() {
        let pipe = Pipe::start(0, 0, Box::leak(Box::new(Counters::default())));
        let mut source = DmaSource::<DmaBuf>::default();
        let one = source.completed().take().unwrap();
        let two = source.spare(&pipe).unwrap();
        let three = source.spare(&pipe).unwrap();
        let attempt = |source: &mut DmaSource<DmaBuf>, changed| {
            source.ready_from(
                Some(&pipe),
                Default::default(),
                &oos_hal::dho::seq::time::Real,
                || changed,
                Duration::ZERO,
            )
        };
        assert!(!attempt(&mut source, false), "no arm/read when all three destinations are owned");
        let observed = pipe.source_generation();
        pipe.return_buffer(Box::new(one));
        assert!(pipe.wait_source(observed, Duration::ZERO).is_none(), "a return before waiting cannot be lost");
        assert!(!attempt(&mut source, true), "STOP, route, settings, or another command wins a simultaneous return");
        assert!(attempt(&mut source, false));
        assert!(source.spare(&pipe).is_none(), "prearm cannot reuse the live destination while lending it");
        assert!(source.completed().take().is_some());
        assert!(!attempt(&mut source, false), "there is no fourth destination");
        drop((two, three));
    }

    #[test]
    fn rejected_handoff_recycles_the_current_destination_when_a_spare_was_reserved() {
        let pipe = Pipe::start(0, 0, Box::leak(Box::new(Counters::default())));
        let mut source = DmaSource::<DmaBuf>::default();
        let spare = source.spare(&pipe).unwrap();
        let loan = source.spare(&pipe).unwrap();
        let observed = pipe.source_generation();
        let settings = super::super::Settings::default();
        let p = oos_hal::dho::hz::prep(1, 1_000_000_000, 4096, 0);
        let id = oos_core::change::Identity::default();
        let readout = super::super::batch::Readout::new(
            &pipe,
            (&settings, &p),
            (0, 0, false),
            (0, crate::board::dho::switches::HeaderProbe::Off, None),
            (id, std::time::Instant::now()),
        );
        assert!(readout.dispatch(source.completed(), None, |f| f, &|| false).is_empty());
        assert!(source.completed().is_some(), "zero records leave the destination with the source");
        source.switch(&pipe, spare);
        assert!(
            pipe.wait_source(observed, Duration::ZERO).is_none(),
            "the rejected destination return wakes the source"
        );
        assert!(source.completed().is_some());
        assert!(source.spare(&pipe).is_some());
        assert!(source.spare(&pipe).is_none());
        drop(loan);
    }
}

#[cfg(test)]
#[test]
fn prearm_requires_the_captured_route_and_a_current_funded_plan() {
    use oos_core::change::Identity;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let profile = crate::drawpipe::funding::TEST_PROFILE;
    let sw = crate::sysconf::Switches::parse(&|_| None, profile, "", Default::default());
    let pipe = drawpipe::start_pipe(&sw);
    let identity = Identity::default();
    pipe.publish_view(&crate::model::Model::default(), 128, 64, sw.interp, identity);
    assert!(pipe.admit_taps(identity, [0; 6], 64, Duration::from_secs(1)));
    let settings = super::Settings::default();
    let pair = (identity.gen(), identity.epoch());
    let tap = pipe.try_prearm_tap(identity).unwrap();
    let armed = ArmedBatch { settings: settings.clone(), pair, route: true, deposit: true, tap: Some(tap) };
    assert!(armed.matches(&settings, pair, true, Some(pipe)));
    assert!(!armed.matches(&settings, pair, false, Some(pipe)));
    assert!(!armed.matches(&settings, (pair.0 + 1, pair.1), true, Some(pipe)));
    assert!(!armed.matches(&settings, pair, true, None));
    pipe.retire_tap_plan();
    assert!(!armed.matches(&settings, pair, true, Some(pipe)));
}

#[cfg(any(target_os = "linux", test))]
pub(super) struct ArmPlan<'a> {
    settings: &'a Settings,
    identity: oos_core::change::Identity,
    route: bool,
    deposit: Option<&'a Pipe>,
    serial: bool,
}

#[cfg(any(target_os = "linux", test))]
impl<'a> ArmPlan<'a> {
    pub(super) fn new(
        settings: &'a Settings,
        identity: oos_core::change::Identity,
        route: bool,
        deposit: Option<&'a Pipe>,
        serial: bool,
    ) -> Self {
        Self { settings, identity, route, deposit, serial }
    }

    pub(super) fn serial(&self) -> bool {
        self.serial
    }

    pub(super) fn read_ahead(&self) -> bool {
        self.serial && !self.settings.single && self.settings.mode == oos_hal::dho::acq::Mode::Normal
    }

    pub(super) fn uses_record_queue(&self) -> bool {
        self.deposit.is_none()
    }

    fn pair(&self) -> (u64, u64) {
        (self.identity.gen(), self.identity.epoch())
    }

    pub(super) fn obsolete(
        &self,
        out: &crate::board::HwShared,
        asked: (Option<&Settings>, crate::board::Slow),
    ) -> bool {
        let g = crate::perfguard::deep::acq_lock(out);
        g.cal_req.is_some() || self.handoff_obsolete(&g, asked)
    }

    pub(super) fn handoff_obsolete(
        &self,
        g: &crate::board::Shared,
        asked: (Option<&Settings>, crate::board::Slow),
    ) -> bool {
        self.capture_changed(g, asked) || g.hold || g.stop_view.is_some() || g.record_flow.shutdown
    }

    pub(super) fn capture_changed(
        &self,
        g: &crate::board::Shared,
        asked: (Option<&Settings>, crate::board::Slow),
    ) -> bool {
        self.settings_changed(g, asked)
            || g.acquisition.identity().epoch() != self.identity.epoch()
            || g.acquisition.route() != self.route
    }

    pub(super) fn wait_to_submit(&self, pipe: &Pipe, clock: &impl LoopClock, canceled: impl Fn() -> bool) -> bool {
        loop {
            let observed = Wake::generation(pipe);
            if canceled() || !pipe.accepting() {
                return false;
            }
            if !pipe.in_flight() {
                return true;
            }
            pipe.note_source_wait(self.identity, clock.park(pipe, observed, HOUSEKEEPING));
        }
    }

    /// Stop waiting for admission on a control request, then check if the read still has a destination.
    pub(super) fn read_allowed(
        &self,
        clock: &impl LoopClock,
        interrupted: impl Fn() -> bool,
        obsolete: impl Fn() -> bool,
    ) -> bool {
        if let Some(pipe) = self.deposit.filter(|_| self.serial()) {
            self.wait_to_submit(pipe, clock, interrupted);
        }
        !self.serial() || !obsolete()
    }

    pub(super) fn settings_changed(
        &self,
        shared: &crate::board::Shared,
        asked: (Option<&Settings>, crate::board::Slow),
    ) -> bool {
        (shared.want.as_ref(), shared.slow) != asked || shared.acquisition.identity().gen() != self.identity.gen()
    }

    /// Fill the successor during admission backpressure, retaining its distinct destination until dispatch.
    pub(super) fn prearm_and_wait(
        &self,
        loan: Option<&Pipe>,
        clock: &impl LoopClock,
        prearm: impl FnOnce() -> Option<oos_hal::dho::bus::DmaBuf>,
        canceled: impl Fn() -> bool,
    ) -> Result<Option<oos_hal::dho::bus::DmaBuf>, ()> {
        let next = prearm();
        if let Some(pipe) = loan.filter(|pipe| !self.wait_to_submit(pipe, clock, canceled)) {
            if let Some(spare) = next {
                pipe.return_buffer(Box::new(spare));
            }
            return Err(());
        }
        Ok(next)
    }

    pub(super) fn prepare(
        &self,
        dma: &mut DmaSource<oos_hal::dho::bus::DmaBuf>,
        armed: &mut Option<ArmedBatch>,
        clock: &impl LoopClock,
        obsolete: impl Fn() -> bool,
        disarm: impl FnOnce(&mut Option<ArmedBatch>),
    ) -> Option<(bool, Option<Arc<TapReservation>>)> {
        if !dma.ready(self.identity, clock, &obsolete) {
            return None;
        }
        let ahead = armed.as_ref().is_some_and(|a| a.matches(self.settings, self.pair(), self.route, self.deposit));
        let tap = if ahead {
            armed.as_ref().and_then(|a| a.tap.clone())
        } else {
            disarm(armed);
            super::batch::source_reservation(self.deposit, self.identity, &obsolete).ok()?
        };
        if obsolete() {
            return None;
        }
        *armed = None;
        Some((ahead, tap))
    }

    /// Read LA and raw replay before calling this. An owned destination and any funded tap are required before arming.
    /// When the current buffer will be loaned, keep a reserved spare available even if ahead arm is deferred.
    #[cfg(any(target_os = "linux", test))]
    pub(super) fn prearm(
        &self,
        dma: &mut DmaSource<oos_hal::dho::bus::DmaBuf>,
        armed: &mut Option<ArmedBatch>,
        loan: Option<&Pipe>,
        successor: Option<Grant>,
        obsolete: impl Fn() -> bool,
        arm: impl FnOnce(&Settings, Option<&TapReservation>) -> bool,
    ) -> Option<oos_hal::dho::bus::DmaBuf> {
        let Grant::Overlapping(frames) = successor? else { return None };
        if obsolete() {
            return None;
        }
        let next = loan.and_then(|pipe| dma.spare(pipe));
        if loan.is_some() && next.is_none() {
            // No free destination: defer prearm until a returned buffer is owned.
            return None;
        }
        let funded = self.deposit.filter(|pipe| pipe.staged_taps());
        let tap = funded.and_then(|pipe| pipe.try_prearm_tap(self.identity));
        if funded.is_none() || tap.is_some() {
            let mut successor = self.settings.clone();
            apply_count(&mut successor, frames);

            if !obsolete() && arm(&successor, tap.as_deref()) {
                *armed = Some(ArmedBatch {
                    settings: successor,
                    pair: self.pair(),
                    route: self.route,
                    deposit: self.deposit.is_some(),
                    tap,
                });
            }
        }
        next
    }
}

#[cfg(test)]
#[test]
fn calibration_cancels_prepare_with_every_funded_grant_held() {
    use crate::board::HwShared;
    use oos_core::change::Identity;
    let _scope = drawpipe::test_scope::Scope::new();
    let sw = crate::sysconf::Switches::default();
    let pipe = drawpipe::start_pipe(&sw);
    let id = Identity::default();
    let held = full_tap_plan(pipe, id);
    let shared: HwShared = Default::default();
    let settings = Settings::default();
    let plan = ArmPlan::new(&settings, id, false, Some(pipe), false);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|threads| {
        let worker = threads.spawn(|| {
            let mut dma = DmaSource::default();
            let mut armed = None;
            let result = plan.prepare(
                &mut dma,
                &mut armed,
                &oos_hal::dho::seq::time::Real,
                || plan.obsolete(&shared, (None, Default::default())),
                |_| {},
            );
            tx.send(result.is_none()).unwrap();
        });
        pipe.wait_tap_sender();
        oos_hal::sync::lock(&shared).cal_req = Some((vec![0], false, false));
        drawpipe::wake_source();
        let result = rx.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD);
        // Failure cleanup only: keep all grants through command delivery and the completion assertion.
        if result.is_err() {
            pipe.retire_tap_plan();
        }
        worker.join().unwrap();
        assert!(result.unwrap(), "calibration must return to its handler without tap release");
    });
    assert!(pipe.has_funded_taps(id), "command cancellation does not retire the plan");
    assert_eq!(held.len(), 8);
    assert!(held.iter().all(|grant| pipe.current_tap(grant)));
}

#[cfg(test)]
fn full_tap_plan(pipe: &Pipe, id: oos_core::change::Identity) -> Vec<Arc<TapReservation>> {
    pipe.publish_view(&crate::model::Model::default(), 128, 64, crate::sysconf::Switches::default().interp, id);
    pipe.fund_taps(
        crate::picture::deposit::admission::Share::checked(1 << 20, 0, 1 << 20, 16, Arc::new(())).unwrap(),
        Duration::from_secs(1),
    )
    .unwrap();
    assert!(pipe.admit_taps(id, [0; 6], 1, Duration::from_secs(1)));
    (0..8).map(|_| pipe.reserve_plan(id).unwrap()).collect()
}

#[cfg(test)]
#[test]
fn force_keeps_waiting_for_funded_capacity_before_prepare_can_arm() {
    let _scope = drawpipe::test_scope::Scope::new();
    let pipe = drawpipe::start_pipe(&crate::sysconf::Switches::default());
    let id = oos_core::change::Identity::default();
    let mut held = full_tap_plan(pipe, id);
    let shared: crate::board::HwShared = Default::default();
    let settings = Settings::default();
    let plan = ArmPlan::new(&settings, id, false, Some(pipe), false);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|threads| {
        let worker = threads.spawn(|| {
            let mut dma = DmaSource::default();
            let mut armed = None;
            let result = plan.prepare(
                &mut dma,
                &mut armed,
                &oos_hal::dho::seq::time::Real,
                || plan.obsolete(&shared, (None, Default::default())),
                |_| {},
            );
            tx.send(result.is_some()).unwrap();
        });
        pipe.wait_tap_sender();
        // Production force_hw publishes and wakes while Shared is still held.
        {
            let mut g = oos_hal::sync::lock(&shared);
            g.force = true;
            drawpipe::wake_source();
        }
        pipe.wait_tap_sender();
        assert!(rx.try_recv().is_err(), "force cannot bypass the full funded FIFO");
        assert!(oos_hal::sync::lock(&shared).force);
        assert!(held.iter().all(|grant| pipe.current_tap(grant)));
        drop(held.pop());
        assert!(rx.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap());
        worker.join().unwrap();
    });
    assert!(oos_hal::sync::lock(&shared).force, "only the armed handler consumes force");
}

#[cfg(test)]
#[test]
fn prearm_never_arms_without_a_distinct_destination_or_after_a_command() {
    use std::cell::Cell;
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let mut source = DmaSource::<oos_hal::dho::bus::DmaBuf>::default();
    let id = oos_core::change::Identity::default();
    let s = Settings::default();
    let plan = ArmPlan::new(&s, id, true, Some(&pipe), false);
    let calls = Cell::new(0);
    let mut armed = None;
    let first = plan
        .prearm(
            &mut source,
            &mut armed,
            Some(&pipe),
            Some(Grant::Overlapping(s.frames)),
            || false,
            |_, _| {
                calls.set(calls.get() + 1);
                true
            },
        )
        .unwrap();
    assert!(armed.as_ref().unwrap().matches(&s, (id.gen(), id.epoch()), true, Some(&pipe)));
    let second = source.spare(&pipe).unwrap();
    assert!(plan
        .prearm(
            &mut source,
            &mut armed,
            Some(&pipe),
            Some(Grant::Overlapping(s.frames)),
            || false,
            |_, _| panic!("no fourth destination")
        )
        .is_none());
    pipe.return_buffer(Box::new(first));
    assert!(plan
        .prearm(
            &mut source,
            &mut armed,
            Some(&pipe),
            Some(Grant::Overlapping(s.frames)),
            || true,
            |_, _| panic!("command wins")
        )
        .is_none());
    assert_eq!(calls.get(), 1);
    assert!(source.spare(&pipe).is_some(), "command cancellation leaves the returned buffer available");
    drop(second);
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn limit_frames(
    settings: &mut Settings,
    proposed: u32,
    armed: Option<&ArmedBatch>,
    (pair, route, pipe): ((u64, u64), bool, Option<&Pipe>),
) {
    apply_count(settings, proposed);
    if let Some(previous) = armed {
        previous.preserve_frames(settings, pair, route, pipe);
    }
}

#[cfg(any(target_os = "linux", test))]
fn apply_count(settings: &mut Settings, proposed: u32) {
    if !matches!(settings.mode, oos_hal::dho::acq::Mode::Average { .. }) {
        settings.frames = proposed;
    }
}

#[cfg(test)]
mod successor_tests;

#[cfg(test)]
#[test]
fn deposit_admission_rechecks_cancellation_and_pipe_shutdown() {
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    let id = oos_core::change::Identity::default();
    let settings = Settings::default();
    pipe.publish_view(&crate::model::Model::default(), 16, 16, crate::sparse::Interp::Exact, id);
    let plan = ArmPlan::new(&settings, id, true, Some(&pipe), false);
    assert!(plan.wait_to_submit(&pipe, &oos_hal::dho::seq::time::Real, || false));
    assert!(!plan.wait_to_submit(&pipe, &oos_hal::dho::seq::time::Real, || true));
    crate::drawpipe::shutdown::close(&pipe, std::time::Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    assert!(!plan.wait_to_submit(&pipe, &oos_hal::dho::seq::time::Real, || false));
}

#[cfg(test)]
#[test]
fn prearm_runs_before_the_admission_check_and_cancellation_returns_its_spare() {
    use std::cell::Cell;
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    let id = oos_core::change::Identity::default();
    let settings = Settings::default();
    pipe.publish_view(&crate::model::Model::default(), 16, 16, crate::sparse::Interp::Exact, id);
    let plan = ArmPlan::new(&settings, id, true, Some(&pipe), false);
    let mut dma = DmaSource::default();
    let mut armed = None;
    let did_arm = Cell::new(false);
    let result = plan.prearm_and_wait(
        Some(&pipe),
        &oos_hal::dho::seq::time::Real,
        || {
            plan.prearm(
                &mut dma,
                &mut armed,
                Some(&pipe),
                Some(Grant::Overlapping(2)),
                || false,
                |successor, _| {
                    assert_eq!(successor.frames, 2);
                    did_arm.set(true);
                    true
                },
            )
        },
        || {
            assert!(did_arm.get());
            true
        },
    );
    assert!(result.is_err());
    assert!(armed.take().is_some());
    assert!(dma.completed().is_some(), "the completed DMA has not been loaned");
    assert!(pipe.take_buffer().is_some());
}
