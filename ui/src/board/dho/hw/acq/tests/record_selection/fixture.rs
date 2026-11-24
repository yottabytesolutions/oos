// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use super::{Case, Signal};
use crate::board::dho::switches::HeaderProbe;

pub(super) const RAW_SLOT_MARKER: u16 = 8;

/// Snapshot both routes at the same boundary, after intake and publication are observable.
pub(super) fn trial(case: &Case, deposit: bool) -> (Outcome, u64, Vec<u8>) {
    let scope = Scope::new();
    let id = Identity::default();
    let out = requested(&case.settings, id, deposit);
    lock(&out).cores = case.correct;
    let switches = Switches::parse(
        &|key| (case.average_hold && key == "OOS_HWAVG").then(|| "hold".into()),
        "acq.work_budget_ms = 12",
        if case.keep { "acq.frame0 = keep" } else { "" },
        Default::default(),
    );
    let intake = deposit.then(|| {
        let intake = scope.gated_worker();
        let (w, h) = picture::SIZE;
        drawpipe::pipe().unwrap().publish_view(&picture::model(case.settings.tdiv), w, h, switches.interp, id);
        intake
    });
    let (mut io, trace) = sim::fixture(&case.settings, out.clone(), (Run::Selection, case.count, false), intake);
    let start = io.clock.now();
    let options = Spawn { mask: case.settings.mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let mut source = AcqLoop::new(options, switches, out.clone(), Service::default(), None, start, FixedPacing);
    let bytes = dispatch(case, &mut source, &mut io);
    if deposit {
        io.clock.compose();
    }
    let outcome = Outcome::snapshot(&mut trace.borrow_mut(), &lock(&out), io.clock.now().duration_since(start));
    (outcome, source.seq, bytes)
}

fn dispatch(case: &Case, source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>) -> Vec<u8> {
    use super::super::super::dispatch::{Handoff, Published};
    let request = Request::read(&lock(&source.out));
    let (settings, params) = source.reconfigure(io, &request, &mut false).unwrap();
    let mut settings = super::super::super::super::batch_plan(&settings, &params, source.switches.board.zone_batch);
    assert!(settings.frames >= case.count);
    settings.frames = case.count;
    assert_eq!(i128::from(params.memsel), case.settings.depth);
    let deposit = drawpipe::pipe();
    let grant = Grant::Overlapping(case.count);
    let timing = batch_timing::SourceTiming::new(None, grant);
    let plan = ArmPlan::new(&settings, request.identity, request.route, deposit, case.held);
    let capture = Capture { settings: &settings, params: &params, request: &request, uncapped: case.count };
    let mut io = io.batch_io();
    let batch = &mut source.batch;
    let completed =
        batch.fill(&mut io, &capture, (false, timing, &source.pacing), (&source.out, false, false), &plan).unwrap();
    assert_eq!(completed.count, case.count);
    let dma = batch.dma.destination();
    let fb = seq::frame_bytes(&settings, &params);
    assert_eq!(dma.data().len(), fb * case.count as usize);
    let bytes = fixture_bytes(case, &capture, dma);
    let mut readout = batch.read(&mut io, &capture, &completed, (&source.out, HeaderProbe::Off), None);
    assert!(readout.raw_frame.is_none(), "the test provides any separate raw read at the decode boundary");
    if let Some(edge) = case.raw_edge {
        let mut raw = vec![0; fb];
        write_signal(&mut raw, &capture, if edge { Signal::Edge } else { Signal::Off });
        raw[..2].copy_from_slice(&(RAW_SLOT_MARKER << 8).to_le_bytes());
        let frame =
            super::super::super::super::batch::decode_record(&raw, (&settings, &params, HeaderProbe::Off), None, 0);
        readout.raw_frame = Some(frame);
    }
    if case.held {
        io.clock.stop();
    }
    let handoff = Handoff { capture, completed, plan: &plan, deposit, tap: None, readout };
    let published = batch.publish(&mut io, handoff, (&source.out, &source.switches), &mut source.seq, || false);
    assert!(matches!((case.held, published), (true, Published::Held) | (false, Published::Running)));
    bytes
}

fn fixture_bytes(case: &Case, capture: &Capture<'_>, dma: &mut DmaBuf) -> Vec<u8> {
    let fb = seq::frame_bytes(capture.settings, capture.params);
    if case.signal != Signal::Off {
        assert_eq!((capture.settings.mask, capture.settings.header, capture.settings.compress), (1, false, 1));
        let mut bytes = dma.data().to_vec();
        for (slot, record) in bytes.chunks_exact_mut(fb).enumerate() {
            if slot == 1 || case.signal == Signal::Periodic {
                write_signal(record, capture, case.signal);
            }
        }
        dma.read(&mut bytes.as_slice(), bytes.len(), |_, _| {}).unwrap();
    }
    if case.available < case.count {
        // Keep a partial slot after the last complete record. It must never become the newest.
        dma.truncate(case.available as usize * fb + fb / 2);
    }
    dma.data().to_vec()
}

fn write_signal(record: &mut [u8], capture: &Capture<'_>, signal: Signal) {
    let p = capture.params;
    let pre = p.pre.saturating_sub(seq::window(capture.settings, p).1) as usize;
    assert!((32..record.len() / 2 - 32).contains(&pre));
    if signal == Signal::Periodic {
        assert!(p.rate / 16 > 20_000_000);
    }
    // Leave the simulator's first-word slot marker intact. The comparator window is well inside the frame.
    for (i, word) in record.as_chunks_mut::<2>().0.iter_mut().enumerate().skip(1) {
        let code: u16 = match signal {
            Signal::Off => 25_000,
            Signal::Edge => {
                if i < pre {
                    25_000
                } else {
                    40_000
                }
            }
            Signal::Periodic => {
                if i % 16 < 8 {
                    25_000
                } else {
                    40_000
                }
            }
            Signal::CoreOffsets => {
                // The nominal step arms the comparator but stays below level. Phase offsets cross it.
                let base = if i < pre { 30_000 } else { 32_740 };
                let phase = [80, -80, 70, -70, 60, -60, 50, -50];
                (base + phase[i % phase.len()]) as u16
            }
        };
        *word = code.to_le_bytes();
    }
}
