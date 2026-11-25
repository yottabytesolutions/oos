// SPDX-License-Identifier: GPL-2.0-or-later

//! Host producer fixture: production SINGLE gate, configure/arm steps and publication filters.
//! Batch IO uses the HAL engine; stream records use synthetic complete snapshots, not chunk IO.

use super::*;
use crate::board::dho::{lifecycle, publication, roll};
use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
use crate::board::{Frame, HwShared, Slow, Want};
use crate::model::{AcqMode, Model, TrigMode};
use oos_hal::dho::{
    bus::{Bus, DmaBuf},
    hz,
    seg::sim,
    seq,
};
use oos_hal::sync::lock;

mod record_route;
pub(crate) use record_route::RecordWorker;

pub(crate) struct SingleWorker {
    state: seq::State,
    current: Option<(Want, Slow)>,
    done: Option<(u64, u64)>,
    source: scripted::ScriptedBatchSource,
    rolling: Option<oos_hal::dho::roll::Params>,
    average: Option<oos_hal::dho::avg::Tracker>,
}

impl SingleWorker {
    pub(crate) fn new(kind: BatchKind) -> Self {
        Self {
            state: seq::State::handover(1, None),
            current: None,
            done: None,
            source: scripted::source(kind, 123),
            rolling: None,
            average: None,
        }
    }

    pub(crate) fn read(&mut self, shared: &HwShared, model: &Model, sw: &crate::sysconf::Switches) -> Option<Frame> {
        let request = publication::snapshot(shared);
        let want = request.want.unwrap();
        let (mode, id) = (request.mode, request.identity);
        let pair = (id.gen(), id.epoch());
        let changed = publication::reconfigure(self.current.as_ref().map(|(s, m)| (s, *m)), &want, mode);
        let p = if changed {
            self.rolling = None;
            self.average = match want.mode {
                oos_hal::dho::acq::Mode::Average { count } => {
                    Some(oos_hal::dho::avg::Tracker::new(sw.board.avg_kind(), count))
                }
                _ => None,
            };
            lifecycle::configure(&mut self.state, &want, None, None).unwrap().1
        } else {
            hz::prep(want.mask, seq::tdiv_fs(want.tdiv), want.depth, 0)
        };
        self.current = Some((want.clone(), mode));
        let (_, mut bus, mut c2h) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, 1, p.ratio));
        if mode != Slow::Off {
            let mut ring = std::array::from_fn(|_| Default::default());
            if !roll::poll(
                mode,
                shared,
                &mut self.rolling,
                &mut ring,
                &mut Default::default(),
                &mut self.state.sh,
                &mut bus,
            ) {
                return None;
            }
        }
        if mode == Slow::Off
            && single::progress(&want, self.done, pair, lock(shared).hold, 0).1 != single::Poll::Acquire
        {
            return None;
        }
        let count = if mode == Slow::Off {
            self.acquire(&want, &p, &mut bus, &mut c2h)
        } else {
            self.start_stream(&want, &p, &mut bus);
            1
        };
        let show = self.average.as_mut().is_none_or(|tr| tr.batch(count).show);
        if !show {
            return None;
        }
        let batch = self.source.next_batch(model, sw, if mode == Slow::Off { 1 } else { 6 }, None).unwrap();
        let mut frame = publication::tag(batch.newest(id.epoch()), id);
        frame.roll = mode == Slow::Roll;
        frame.scan = matches!(mode, Slow::Scan { .. });
        assert!(!frame.partial, "fixture returns a complete record");
        self.done = single::progress(&want, self.done, pair, false, 1).0;
        crate::drawpipe::Counters::add(&crate::drawpipe::counters().acquired, 1);
        Some(frame)
    }

    fn start_stream(&mut self, want: &Want, p: &hz::Params, bus: &mut impl Bus) {
        if self.rolling.is_none() {
            bus.apply(&lifecycle::roll_start(&mut self.state.sh, want.mask));
            self.rolling = Some(oos_hal::dho::roll::params(want.mask, seq::tdiv_fs(want.tdiv), want.depth, p));
        }
    }

    fn acquire(
        &mut self,
        want: &Want,
        p: &hz::Params,
        bus: &mut impl oos_hal::dho::bus::Bus,
        c2h: &mut impl std::io::Read,
    ) -> u32 {
        let n = lifecycle::batch(want.single, false, || {
            seq::acquire_batch_idle(
                &mut self.state,
                want,
                p,
                bus,
                c2h,
                &mut DmaBuf::new(),
                false,
                0,
                |_| {},
                || false,
                || false,
            )
        })
        .unwrap();
        assert_eq!(n, 1);
        n
    }

    pub(crate) fn deliver(shared: &HwShared, frame: Frame) {
        publication::push_frames(shared, vec![frame], oos_hal::dho::acq::Mode::Normal);
    }
}

pub(crate) struct SegmentWorker {
    state: seq::State,
    current: Option<Want>,
    run: Option<super::segmented::SegRun>,
    bus: sim::SimBus,
    c2h: SegmentReader,
    n: u64,
}

impl SegmentWorker {
    pub(crate) fn new() -> Self {
        let (_, bus, c2h) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, 50, 1));
        Self {
            state: seq::State::handover(1, None),
            current: None,
            run: None,
            bus,
            c2h: SegmentReader { inner: c2h, boundary: None },
            n: 0,
        }
    }

    pub(crate) fn change_pair_during_read(&mut self, shared: &HwShared, id: oos_core::change::Identity) {
        self.c2h.boundary = Some((shared.clone(), id));
    }

    pub(crate) fn tick(&mut self, shared: &HwShared) {
        let request = publication::snapshot(shared);
        let want = request.want.unwrap();
        let id = request.identity;
        if self.current.is_none() {
            self.state = seq::State::handover(want.mask, None);
        }
        if self.current.as_ref() != Some(&want) {
            super::segmented::seg_abort(&mut self.run, &mut self.state, &mut self.bus, &mut self.c2h, shared);
            let (steps, _) = lifecycle::configure(&mut self.state, &want, None, None).unwrap();
            seq::execute_bus(&steps, &mut self.bus, &mut self.c2h, &mut DmaBuf::new(), |_| {}).unwrap();
            self.current = Some(want.clone());
        }
        let p = hz::prep(want.mask, seq::tdiv_fs(want.tdiv), want.depth, 0);
        self.bus.0.borrow_mut().ratio = p.ratio;
        super::segmented::SegmentIo::new(
            &mut self.state,
            &mut self.bus,
            &mut self.c2h,
            &mut DmaBuf::new(),
            (shared, &mut self.n),
            &seq::time::Real,
            &mut || {},
        )
        .step(&mut self.run, &want, &p, id)
        .unwrap();
    }
}

struct SegmentReader {
    inner: sim::SimC2h,
    boundary: Option<(HwShared, oos_core::change::Identity)>,
}

impl std::io::Read for SegmentReader {
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        if let Some((shared, id)) = self.boundary.take() {
            lock(&shared).acquisition = crate::board::identity::Publication::new(id, false);
        }
        self.inner.read(data)
    }
}

#[test]
fn hardware_average_single_keeps_acquiring_until_a_frame_is_shown() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let model =
        Model { acq: AcqMode::Average, avg_hw: true, avg_count: 16, trig_mode: TrigMode::Single, ..Default::default() };
    let mut sw = crate::sysconf::Switches::default();
    sw.board.avg_hold = true;
    let shared = HwShared::default();
    lock(&shared).want = Some(settings(&model, false, &sw));
    let mut worker = SingleWorker::new(BatchKind::HardwareAverage);
    for batch in 1..16 {
        assert!(worker.read(&shared, &model, &sw).is_none(), "batch {batch} is hidden");
        assert_eq!(worker.done, None, "a hidden batch cannot latch SINGLE");
    }
    let frame = worker.read(&shared, &model, &sw).expect("the completed average publishes a frame");
    assert!(frame.hw_avg);
    assert_eq!(event_single_arms(), 16);
    for _ in 0..20 {
        assert!(worker.read(&shared, &model, &sw).is_none());
    }
    assert_eq!(event_single_arms(), 16);
}

fn event_single_arms() -> u64 {
    crate::drawpipe::counters()
        .acquisition
        .text()
        .split_whitespace()
        .find_map(|v| v.strip_prefix("acq_single_arm="))
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn scan_single_gen_only_mid_sweep_restarts_before_publishing_completion() {
    use oos_core::change::{Shadow, UiAcq};
    let mut model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let old = shadow.identity();
    let s = settings(&model, false, &Default::default());
    let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, 0);
    let rp = oos_hal::dho::roll::params(s.mask, seq::tdiv_fs(s.tdiv), s.depth, &p);
    let mut rolling = Some(rp);
    let mut sweep = std::array::from_fn(|_| vec![17.0; 8]);
    let mut cursor = 4;
    let mut hardware_column = 4;
    model.mask_src = 1;
    let id = shadow.observe(&model, UiAcq::default(), &[], &config).unwrap().identity();
    assert_ne!(id.gen(), old.gen());
    assert_eq!(id.epoch(), old.epoch());
    assert!(roll::boundary(
        Some((old.gen(), old.epoch())),
        id,
        &mut Default::default(),
        &mut sweep,
        &mut Default::default(),
        &mut Vec::new(),
        &mut cursor
    ));
    let mode = Slow::Scan { delay: 0.0, single: true };
    if roll::restart_scan(mode, false, &mut rolling) {
        hardware_column = 0;
    }
    // The fake hardware's column resets only when the worker starts another sweep.
    super::roll::scan_write(&mut sweep[0], 8, hardware_column, &vec![23.0; 8 - hardware_column]);
    let frame = publication::tag(Frame { scan: true, partial: false, ch: sweep, ..Default::default() }, id);
    assert!(frame.ch[0].iter().all(|&v| v == 23.0), "the completed record must contain an entire current-pair sweep");
    assert!(rolling.is_none(), "the next poll must restart the hardware sweep");
    for (mode, halted) in [(Slow::Roll, false), (Slow::Scan { delay: 0.0, single: false }, false), (mode, true)] {
        rolling = Some(rp);
        assert!(!roll::restart_scan(mode, halted, &mut rolling));
        assert!(rolling.is_some(), "RUN streams and completed SINGLE retain their lifecycle");
    }
}
