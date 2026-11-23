// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{batch_timing, dma_source::ArmPlan};
use super::{read::Capture, *};
use crate::drawpipe::{self, admission::Grant, test_scope::Scope};
use oos_hal::{
    dho::{acq::Mode, tpu},
    sync::lock,
};
use sim::{Event, FixedPacing, Frame, Pause, Run};

mod bounds;
mod deposit;
mod picture;
mod raw;
mod side_service;
mod sim;
mod stopped_raw;

#[derive(Debug, PartialEq)]
struct Outcome {
    events: Vec<Event>,
    screens: Vec<Vec<Frame>>,
    queue: Vec<Frame>,
    latest: Option<Frame>,
    final_ack: u64,
    decoded: u64,
    dropped: u64,
    counters: RecordCounts,
    elapsed: Duration,
}

impl Outcome {
    fn snapshot(trace: &mut sim::Trace, shared: &crate::board::Shared, elapsed: Duration) -> Self {
        Self {
            events: std::mem::take(&mut trace.events),
            screens: std::mem::take(&mut trace.screens),
            queue: shared.queue.iter().map(|f| Frame::read(f)).collect(),
            latest: (shared.frame.seq != 0).then(|| Frame::read(&shared.frame)),
            final_ack: shared.final_ack,
            decoded: shared.decoded,
            dropped: shared.dropped,
            counters: RecordCounts::read(),
            elapsed,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RecordCounts {
    read: u64,
    deposited: u64,
    dropped: u64,
    losses: Vec<(String, u64)>,
}

impl RecordCounts {
    fn loss(&self, name: &str) -> u64 {
        self.losses.iter().find(|(key, _)| key == name).map_or(0, |(_, count)| *count)
    }

    fn read() -> Self {
        use std::sync::atomic::Ordering;
        let counters = drawpipe::counters();
        let (read, _, dropped) = counters.read();
        let losses = counters
            .records_text()
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .filter(|(name, _)| name.starts_with("lost.") || name.starts_with("rejected."))
            .filter_map(|(name, count)| {
                let count = count.parse().unwrap();
                (count != 0).then(|| (name.to_owned(), count))
            })
            .collect();
        Self { read, deposited: counters.deposited.load(Ordering::Relaxed), dropped, losses }
    }
}

fn run(
    script: Run,
    (records, available): (u32, u32),
    settings: Settings,
    (on_trigger, keep): (bool, bool),
    deposit: bool,
) -> Outcome {
    let scope = Scope::new();
    let options = Spawn { mask: settings.mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let out = HwShared::default();
    let id = Identity::default();
    {
        let mut g = lock(&out);
        g.want = Some(settings.clone());
        g.acquisition = crate::board::identity::Publication::new(id, deposit);
        if matches!(script, Run::StopDuring) {
            g.record_flow.cost.set_identity(id);
            g.record_flow.cost.observe(id, 1, Duration::from_millis(50), Duration::ZERO);
        }
    }
    let settings_text = if keep { "acq.frame0 = keep" } else { "" };
    let switches = Switches::parse(&|_| None, "acq.work_budget_ms = 12", settings_text, Default::default());
    let intake = deposit.then(|| {
        let (intake, model) = if script.draws() {
            (scope.gated_worker(), picture::model(settings.tdiv))
        } else {
            (scope.install_intake(), crate::model::Model::default())
        };
        let (w, h) = picture::SIZE;
        drawpipe::pipe().unwrap().publish_view(&model, w, h, switches.interp, id);
        intake
    });
    let (mut io, trace) = sim::fixture(&settings, out.clone(), (script, records, on_trigger), intake);
    let started = io.clock.now();
    let mut source = AcqLoop::new(options, switches, out.clone(), Service::default(), None, started, FixedPacing);
    match script {
        Run::Selection | Run::DepositBackpressure => {
            if available < records {
                let request = Request::read(&lock(&out));
                let (settings, params) = source.reconfigure(&mut io, &request, &mut false).unwrap();
                short_buffer(&mut source, &mut io, (&settings, &params), &request, (records, available));
            } else {
                capture(&mut source, &mut io);
                if matches!(script, Run::DepositBackpressure) {
                    capture(&mut source, &mut io);
                }
            }
            io.clock.take_submission();
        }
        Run::Pictures(stop) => picture::script(&mut source, &mut io, stop, records),
        Run::CompressedTap => raw::script(&mut source, &mut io),
        _ => source.run(&mut io),
    }
    let mut trace = trace.borrow_mut();
    let shared = lock(&out);
    Outcome::snapshot(&mut trace, &shared, io.clock.now().duration_since(started))
}

fn capture(source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>) {
    let request = Request::read(&lock(&source.out));
    let mut last_read = std::mem::take(&mut source.replay.fresh);
    let (settings, params) = source.reconfigure(io, &request, &mut last_read).unwrap();
    source.batch(io, (&settings, &params), request, last_read);
}

fn requested(want: &Settings, identity: Identity, deposit: bool) -> HwShared {
    let out = HwShared::default();
    {
        let mut g = lock(&out);
        g.want = Some(want.clone());
        g.acquisition = crate::board::identity::Publication::new(identity, deposit);
    }
    out
}

fn drive(
    out: &HwShared,
    (mut io, trace): sim::Fixture,
    advance: impl FnMut(&sim::Virtual, Pause) + 'static,
) -> Outcome {
    let switches = Switches::parse(&|_| None, "acq.work_budget_ms = 12", "acq.frame0 = keep", Default::default());
    let mask = lock(out).want.as_ref().unwrap().mask;
    let options = Spawn { mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let started = io.clock.now();
    io.clock.drive(advance);
    let mut source = AcqLoop::new(options, switches, out.clone(), Service::default(), None, started, FixedPacing);
    source.run(&mut io);
    Outcome::snapshot(&mut trace.borrow_mut(), &lock(out), io.clock.now().duration_since(started))
}

// C2H requires a complete transfer. Truncate after the real fill to exercise a short completed DMA buffer.
fn short_buffer(
    source: &mut AcqLoop<FixedPacing>,
    io: &mut Io<sim::Sim>,
    (settings, params): (&Settings, &hz::Params),
    request: &Request,
    (records, available): (u32, u32),
) {
    use super::dispatch::{Handoff, Published};
    let settings = super::super::batch_plan(settings, params, source.switches.board.zone_batch);
    let (deposit, uncapped) = super::super::batch::deposit_plan(&settings, params, request.identity, request.route);
    let plan = ArmPlan::new(&settings, request.identity, request.route, deposit, false);
    let capture = Capture { settings: &settings, params, request, uncapped, first: 0 };
    let timing = batch_timing::SourceTiming::new(None, Grant::Overlapping(records));
    let mut io = io.batch_io();
    let batch = &mut source.batch;
    let completed =
        batch.fill(&mut io, &capture, (false, timing, &source.pacing), (&source.out, false, false), &plan).unwrap();
    assert_eq!(completed.count, records);
    batch.dma.destination().truncate(available as usize * seq::frame_bytes(&settings, params));
    let readout = batch.read(&mut io, &capture, &completed, (&source.out, source.switches.board.header_probe), None);
    let handoff = Handoff { capture, completed, plan: &plan, deposit, tap: None, readout };
    let published = batch.publish(&mut io, handoff, (&source.out, &source.switches), &mut source.seq, || false);
    assert!(matches!(published, Published::Running));
}

fn settings() -> Settings {
    Settings { tdiv: 100e-9, depth: 1000, window: None, level: 0.0, ..Settings::default() }
}

fn twice<T: std::fmt::Debug + PartialEq>(test: impl Fn() -> T) -> T {
    let first = test();
    assert_eq!(first, test());
    first
}

#[test]
fn auto_dispatches_each_live_record_once_in_batch_order() {
    let batches = 3;
    let result = twice(|| run(Run::Live(batches), (4, 4), settings(), (false, true), false));
    let expected: Vec<Vec<Frame>> = (0..batches)
        .map(|batch| (0..4).map(|i| Frame::live((batch * 4 + i + 1) as u64, i as u16, (batch * 4) as u16)).collect())
        .collect();
    assert_eq!(result.screens, expected);
    assert_eq!(result.decoded, 12);
    assert_eq!(result.dropped, 0);
    assert!(result.queue.is_empty());
}

#[test]
fn stop_during_serial_wait_publishes_the_completed_record() {
    let result = twice(|| run(Run::StopDuring, (1, 1), settings(), (false, false), false));
    assert_eq!(result.final_ack, 7);
    assert_eq!(result.queue, [Frame::stamp(1, 0)]);
    assert_eq!(result.latest, Some(Frame::stamp(1, 0)));
    assert_eq!((result.decoded, result.dropped), (1, 0));
    let stop = result.events.iter().position(|event| *event == Event::Stop).unwrap();
    let dma = result.events.iter().position(|event| matches!(event, Event::Dma(_))).unwrap();
    assert!(stop < dma);
}

#[test]
fn stop_between_batches_acknowledges_and_keeps_the_shallow_publication() {
    let result = twice(|| run(Run::StopBetween, (4, 4), settings(), (false, true), false));
    assert_eq!(result.final_ack, 7);
    assert_eq!(result.queue, [Frame::stamp(1, 0), Frame::stamp(2, 1), Frame::stamp(3, 2), Frame::stamp(4, 3)]);
    assert_eq!(result.latest, Some(Frame::stamp(4, 3)));
    assert_eq!(result.events.iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
    assert!(matches!(result.events.as_slice(), [.., Event::Park(_), Event::Stop, Event::Wait(_)]));
}

fn selected(
    (records, available): (u32, u32),
    (on_trigger, keep): (bool, bool),
    settings: Settings,
    expected: &[Frame],
    skipped: u64,
) {
    let result = twice(|| run(Run::Selection, (records, available), settings.clone(), (on_trigger, keep), false));
    assert_eq!(result.queue, expected);
    assert_eq!((result.decoded, result.dropped), (u64::from(records), skipped));
}

#[test]
fn first_short_singleton_is_dropped_after_decode() {
    selected((1, 1), (false, false), settings(), &[], 0);
}

#[test]
fn two_records_keep_frame_one_on_its_edge() {
    selected((2, 2), (true, false), settings(), &[Frame::stamp(2, 1)], 1);
}

#[test]
fn two_records_drop_both_when_frame_one_misses_its_edge() {
    selected((2, 2), (false, false), settings(), &[], 2);
}

#[test]
fn four_and_six_records_skip_zero_and_off_trigger_one() {
    for count in [4, 6] {
        let expected: Vec<_> = (2..count).map(|i| Frame::stamp(u64::from(i + 1), i as u16)).collect();
        selected((count, count), (false, false), settings(), &expected, 2);
    }
}

#[test]
fn one_available_record_from_a_multi_record_batch_is_treated_as_a_singleton() {
    selected((4, 1), (false, false), settings(), &[], 0);
}

#[test]
fn edge_coupling_and_non_edge_trigger_control_frame_one_rejection() {
    for (coupling, kept) in [(tpu::Coupling::Dc, false), (tpu::Coupling::Ac, true), (tpu::Coupling::LfReject, true)] {
        let settings = Settings { trig_couple: coupling, ..settings() };
        let expected: Vec<_> = kept.then(|| Frame::stamp(2, 1)).into_iter().collect();
        selected((2, 2), (false, false), settings, &expected, if kept { 1 } else { 2 });
    }
    let non_edge = Settings { trigger: seq::Trig::Timeout { time: 1e-6 }, ..settings() };
    selected((2, 2), (false, false), non_edge, &[Frame::stamp(2, 1)], 1);
}

#[test]
fn frame_zero_keep_also_preserves_off_trigger_frame_one_and_the_first_singleton() {
    selected((2, 2), (false, true), settings(), &[Frame::stamp(1, 0), Frame::stamp(2, 1)], 0);
    selected((1, 1), (false, true), settings(), &[Frame::stamp(1, 0)], 0);
}
