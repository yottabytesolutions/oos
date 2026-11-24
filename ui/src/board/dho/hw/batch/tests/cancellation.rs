// SPDX-License-Identifier: GPL-2.0-or-later
//! The default production Readout must service commands with every legacy grant held.
use super::*;
use crate::board::dho::hw::dma_source::ArmPlan;
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use oos_hal::sync::lock;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

// Success depends on events and ownership, never elapsed speed.

#[derive(Clone, Copy)]
enum Command {
    Calibration,
    Force,
    Hold,
    Settings,
}
impl Command {
    fn publish(self, shared: &crate::board::HwShared, settings: &Settings) {
        let mut state = lock(shared);
        match self {
            Self::Calibration => state.cal_req = Some((vec![0], false, false)),
            Self::Force => state.force = true,
            Self::Hold => state.hold = true,
            Self::Settings => state.want = Some(settings.clone()),
        }
        // Same lock/wake order as actual command publishers.
        crate::drawpipe::wake_source();
    }
}

#[test]
fn legacy_full_capacity_calibration_returns_with_all_grants_held() {
    check(Command::Calibration, false, false);
}
#[test]
fn legacy_full_capacity_force_returns_with_all_grants_held() {
    check(Command::Force, false, false);
}
#[test]
fn legacy_full_capacity_hold_returns_with_all_grants_held() {
    check(Command::Hold, false, false);
}
#[test]
fn legacy_full_capacity_settings_returns_with_all_grants_held() {
    check(Command::Settings, false, false);
}

#[test]
fn legacy_full_capacity_command_between_predicate_and_park_is_not_lost() {
    check(Command::Calibration, true, false);
}

#[test]
fn legacy_completed_single_read_cancels_without_rearming_or_returning_dma_early() {
    check(Command::Calibration, false, true);
}

fn check(command: Command, race: bool, single: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (m, sw, mut b, p) = normal_lines_plan(10e-6, false);
    (b.header, b.single) = (false, single);
    let (batch, count, _) = scripted::draw_route_batch(&m, &sw, 0);
    let fb = batch.frame_bytes();
    let pipe = crate::drawpipe::start_pipe(&sw);
    let id = Identity::default();
    pipe.publish_view(&m, 128, 64, sw.interp, id);
    assert!(!pipe.staged_taps(), "the default plain route is essential to this regression");
    let held = hold_capacity(pipe, id);
    let shared: crate::board::HwShared = Default::default();
    let plan = ArmPlan::new(&b, id, false, Some(pipe), false);
    let first = AtomicBool::new(true);
    let (checked, check) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let (done, result) = mpsc::channel();
    let mut dma = Some(batch.into_bytes());
    let original = dma.as_ref().unwrap().bytes().to_vec();
    std::thread::scope(|threads| {
        let (plan, shared, b, first, dma) = (&plan, &shared, &b, &first, &mut dma);
        let worker = threads.spawn(move || {
            let canceled = || {
                let value = plan.obsolete(shared, (None, Default::default())) || lock(shared).force;
                if race && first.swap(false, Ordering::Relaxed) {
                    checked.send(()).unwrap();
                    resumed.recv_timeout(HANG_GUARD).unwrap();
                }
                value
            };
            let readout = Readout::new(
                pipe,
                (b, &p),
                (fb, count, false),
                (count as u64, HeaderProbe::Off, None),
                (id, Instant::now()),
            );
            done.send(dispatch_canceled(readout, dma, &canceled, count)).unwrap();
        });
        let checkpoint = if race {
            check.recv_timeout(HANG_GUARD)
        } else {
            pipe.wait_tap_sender();
            Ok(())
        };
        command.publish(shared, b);
        if race {
            resume.send(()).unwrap();
        }
        let completion = result.recv_timeout(HANG_GUARD);
        // Failure cleanup invalidates admission, without releasing any of the held grants.
        if completion.is_err() {
            stop_waiting(pipe);
        }
        worker.join().unwrap();
        assert!(checkpoint.is_ok(), "Readout must check the actual source predicate");
        assert!(completion.unwrap(), "command completion cannot depend on grant release");
    });
    assert_eq!(held.len(), crate::board::QUEUE_MAX);
    assert_eq!(dma.as_ref().unwrap().bytes(), original, "source still owns the untouched completed read");
    assert_eq!(pipe.tap_count(), 0);
    assert!(!pipe.has_tap_plan(id, [0; 6]));
}

fn stop_waiting(pipe: &crate::drawpipe::Pipe) {
    pipe.stage_tap_hold(true);
    crate::drawpipe::wake_source();
}

fn dispatch_canceled<T: RawBatch + 'static>(
    readout: Readout<'_>,
    dma: &mut Option<T>,
    canceled: &dyn Fn() -> bool,
    count: usize,
) -> bool {
    let counters = readout.pipe.batch_counters();
    crate::board::dho::publication::read(counters, count as u32, count as u32, count);
    let events = counters.acquisition.text();
    let frames = readout.dispatch(dma, None, |f| f, canceled);
    assert_eq!(counters.acquisition.text(), events, "no extra arm or read during cancellation");
    assert_eq!(counters.read(), (count as u64, 0, count as u64));
    assert!(
        counters.records_text().contains(&format!("lost.settings={count}")),
        "existing command invalidation accounting covers every read record exactly once"
    );
    frames.is_empty()
}

fn hold_capacity(pipe: &Pipe, id: Identity) -> Vec<Arc<TapReservation>> {
    let frame = Frame { ch: [vec![0.0], vec![], vec![], vec![]], ..Default::default() };
    (0..crate::board::QUEUE_MAX).map(|_| pipe.reserve_waveform(&frame, id).unwrap()).collect()
}
