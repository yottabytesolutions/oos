// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    batch,
    dma_source::ArmPlan,
    header::dump::{BatchInput, Pending},
    stop, Settings,
};
use super::{
    batch::Reserved,
    publication,
    read::{Capture, Completed, ReadIo, Readout},
    status, BatchState, LoopClock, SingleCompletion, SingleHandoff,
};
use crate::{
    board::{
        dho::switches::{FrameZero, HeaderProbe},
        Frame, HwShared, Slow,
    },
    drawpipe::{self, Pipe},
    perfguard::deep::acq_lock as lock,
    sysconf::Switches,
};
use oos_hal::dho::{avg, bus::Bus, seq};
use std::io::Read;

pub(super) struct Handoff<'a> {
    pub(super) capture: Capture<'a>,
    pub(super) completed: Completed,
    pub(super) plan: &'a ArmPlan<'a>,
    pub(super) deposit: Option<&'static Pipe>,
    pub(super) tap: Reserved,
    pub(super) readout: Readout,
}

pub(super) enum Published {
    Running,
    Held,
    Rejected,
}

struct Selection {
    captured: Settings,
    avail: usize,
    fb: usize,
    first_unreliable: bool,
    frame0: FrameZero,
    average: Option<avg::After>,
}

impl BatchState {
    pub(super) fn publish(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        handoff: Handoff<'_>,
        (out, switches): (&HwShared, &Switches),
        seq: &mut u64,
        obsolete: impl Fn() -> bool,
    ) -> Published {
        let count = handoff.completed.count;
        let selection = self.select(io, &handoff.capture, (count, handoff.readout.failed), switches.board.frame0);
        // Extra playback and latch writes occurred after these bytes were captured.
        if handoff.readout.wrote_after_dma {
            self.first.proven = None;
        }
        let pipe = batch::readout_pipe(
            handoff.deposit,
            selection.avail,
            selection.average.is_some(),
            !selection.first_unreliable,
        );
        let next = self.successor(io, &handoff, pipe, (out, switches), &obsolete);
        let request = handoff.capture.request;
        let (stale, held) = self.serial_handoff(&handoff, out);
        if stale {
            self.first.proven = None;
            drawpipe::counters().discard(drawpipe::Lost::Settings, selection.avail as u64);
            if let Some((pipe, spare)) = pipe.zip(next.ok().flatten()) {
                pipe.return_buffer(Box::new(spare));
            }
            return Published::Rejected;
        }
        if let Some(final_req) = held {
            self.first.proven = None;
            self.publish_held(io, handoff, &selection, (out, switches.board.header_probe), (seq, final_req));
            if let Some((pipe, spare)) = pipe.zip(next.ok().flatten()) {
                pipe.return_buffer(Box::new(spare));
            }
            return Published::Held;
        }
        let Ok(next) = next else {
            self.first.proven = None;
            drawpipe::counters().discard(drawpipe::Lost::Settings, selection.avail as u64);
            publication::finish_batch(out, Vec::new(), handoff.capture.settings.mode, |g| {
                handoff.readout.readings.publish(g, request.identity, io.state.mask_cols.as_deref(), None);
            });
            return Published::Rejected;
        };
        self.publish_running(io, handoff, selection, (out, switches), (pipe, next), (seq, &obsolete));
        if obsolete() {
            self.first.proven = None;
        }
        Published::Running
    }

    fn serial_handoff(&self, handoff: &Handoff<'_>, out: &HwShared) -> (bool, Option<u64>) {
        if !handoff.plan.serial() {
            return (false, None);
        }
        let request = handoff.capture.request;
        let mut g = lock(out);
        if publication::boundary(Some(request.pair()), g.acquisition.identity()) {
            if g.hold {
                // The accepted boundary discarded the final record. Complete any pending STOP request.
                g.final_ack = g.final_ack.max(g.final_req);
            }
            return (true, None);
        }
        let asked = (request.want.as_ref(), request.mode);
        (false, stop::held_request(&g, handoff.plan, asked, handoff.completed.count, self.armed.is_some()))
    }

    fn publish_running(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        handoff: Handoff<'_>,
        selection: Selection,
        (out, switches): (&HwShared, &Switches),
        (pipe, next): (Option<&Pipe>, Option<oos_hal::dho::bus::DmaBuf>),
        (seq, obsolete): (&mut u64, &dyn Fn() -> bool),
    ) {
        let Capture { settings: b, request, .. } = handoff.capture;
        let count = handoff.completed.count;
        let Handoff { tap, readout: Readout { readings, raw_frame, mut la, .. }, completed, .. } = handoff;
        // Trigger status and raw availability must be visible before the pool can hand off a deposit.
        let mut readings = Some(readings);
        if let Some(readings) = pipe.and_then(|_| readings.take()) {
            readings.publish(&mut lock(out), request.identity, io.state.mask_cols.as_deref(), Some((count, 0)));
        }
        let (mut frames, skip) = self.dispatch(
            (&selection, &completed),
            &handoff.capture,
            switches.board.header_probe,
            (pipe, tap, raw_frame),
            (&mut la, seq),
            obsolete,
        );
        self.keep_first(&selection, &mut frames);
        self.first.settings = Some(selection.captured);
        self.clear_completed_average(io, out, selection.average);
        if let (Some(la), Some(frame)) = (la, frames.last_mut().filter(|frame| frame.seq == *seq)) {
            frame.la = Some(la);
        }
        batch::summarize(&mut frames);
        if let Some((pipe, next)) = pipe.zip(next) {
            self.dma.switch(pipe, next);
        }
        let single_completed = b.single && !frames.is_empty();
        if readings.is_some() || !frames.is_empty() {
            publication::finish_batch(out, frames, b.mode, |g| {
                if let Some(readings) = readings {
                    readings.publish(g, request.identity, io.state.mask_cols.as_deref(), Some((count, skip)));
                }
            });
        }
        if single_completed {
            self.single_done =
                Some(SingleCompletion { pair: request.pair(), handoff: SingleHandoff::AwaitingUi(io.clock.now()) });
        }
    }

    fn publish_held(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        handoff: Handoff<'_>,
        selection: &Selection,
        (out, probe): (&HwShared, HeaderProbe),
        (sequence, final_req): (&mut u64, u64),
    ) {
        let Handoff { capture, completed, readout, tap, .. } = handoff;
        let Capture { settings: b, params: p, request, .. } = capture;
        let Readout { readings, raw_frame, la, .. } = readout;
        let avail = selection.avail;
        let frame = avail.checked_sub(1).map(|index| {
            *sequence += avail as u64;
            let raw = raw_frame.or_else(|| {
                let settings = super::super::window::raw_replay_settings(b, p, request.run.deep.is_some())?;
                self.read_raw(io, &capture, (&settings, completed.count), (out, probe), (tap.as_ref(), false))
                    .ok()
                    .flatten()
            });
            let (mut frame, uncorrected) = raw.unwrap_or_else(|| {
                batch::decode_record(
                    &self.dma.destination().data()[index * selection.fb..avail * selection.fb],
                    (b, p, probe),
                    request.run.cores.then_some(&mut self.raw.cores),
                    *sequence,
                )
            });
            if uncorrected {
                drawpipe::counters().metrics.source_uncorrected(request.identity, 1);
            }
            frame.seq = *sequence;
            frame.la = la;
            publication::tag(frame, request.identity)
        });
        let skipped = avail.saturating_sub(1);
        drawpipe::counters().discard(drawpipe::Lost::Settings, skipped as u64);
        readings.publish(
            &mut lock(out),
            request.identity,
            io.state.mask_cols.as_deref(),
            Some((completed.count, skipped)),
        );
        let single_completed = b.single && frame.is_some();
        publication::publish_held(out, frame, final_req);
        if single_completed {
            self.single_done = Some(SingleCompletion { pair: request.pair(), handoff: SingleHandoff::TakenOver });
        }
    }

    fn clear_completed_average(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        out: &HwShared,
        average: Option<avg::After>,
    ) {
        if average.is_some_and(|a| a.clear) {
            let steps = io.state.avg_clear_steps();
            if let Err(e) = io.execute(&steps, self.dma.destination()) {
                status(out, format!("average clear: {}", e));
            }
        }
    }

    fn select(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        capture: &Capture<'_>,
        (count, read_failed): (u32, bool),
        policy: FrameZero,
    ) -> Selection {
        let (b, p) = (capture.settings, capture.params);
        let fb = seq::frame_bytes(b, p);
        let average = self.avg.as_mut().map(|(tracker, _, logged)| {
            let after = tracker.batch(count);
            if *logged < 32 {
                *logged += 1;
                let bytes = self.dma.destination().data();
                let at = (count.max(1) as usize - 1) * fb;
                let words = bytes.get(at..).unwrap_or_default().as_chunks::<2>().0;
                let progress = avg::progress(&mut io.bar);
                log_average(tracker.kind, (*logged, count, &after), progress, words);
            }
            after
        });
        let shown = if average.is_some_and(|a| !a.show) { 0 } else { count };
        let avail = self.dma.destination().data().len().checked_div(fb).map_or(0, |len| (shown as usize).min(len));
        publication::read(crate::drawpipe::counters(), count, shown, avail);
        let frame0 = self.frame_zero(capture, policy, count != 0 && avail == count as usize && !read_failed);
        let captured = Settings { frames: 0, ..b.clone() };
        // The FPGA can retain frame zero from the settings that preceded the arm.
        const FIRST_LEFT_MAX_S: f64 = 0.05;
        let first_unreliable = self.first.settings.as_ref() != Some(&captured)
            && !self.first.dropped
            && (p.memsel as f64) < FIRST_LEFT_MAX_S * p.rate as f64
            && !b.single
            && policy != FrameZero::Keep;
        Selection { captured, avail, fb, first_unreliable, frame0, average }
    }

    fn frame_zero(&mut self, capture: &Capture<'_>, policy: FrameZero, complete: bool) -> FrameZero {
        let b = capture.settings;
        let request = capture.request;
        let ordinary = !b.single
            && self.avg.is_none()
            && b.segments.is_none()
            && request.run.segment_bound.is_none()
            && request.mode == Slow::Off
            && !request.run.hold;
        let previous = self.first.proven.take();
        if policy != FrameZero::Steady {
            return policy;
        }
        let pair = request.pair();
        if complete && ordinary {
            self.first.proven = Some(pair);
            if previous == Some(pair) {
                return FrameZero::Steady;
            }
        }
        FrameZero::Drop
    }

    fn keep_first(&mut self, selection: &Selection, frames: &mut Vec<Frame>) {
        self.first.dropped = selection.first_unreliable
            && selection.avail == 1
            && selection.average.is_none()
            && frames.len() == 1
            && frames.first().is_some_and(|f| !f.roll && !f.scan);
        if self.first.dropped {
            drawpipe::counters().lose(drawpipe::Lost::Settings, frames.len() as u64);
            frames.clear();
            // Read another raw deep window at the next opportunity.
            self.raw.deep = None;
        }
    }

    fn dispatch(
        &mut self,
        (selection, completed): (&Selection, &Completed),
        capture: &Capture<'_>,
        probe: HeaderProbe,
        (pipe, tap, raw_frame): (Option<&Pipe>, Reserved, Option<batch::DecodedRecord>),
        (la, seq): (&mut Option<crate::ui::la::LaRec>, &mut u64),
        obsolete: &dyn Fn() -> bool,
    ) -> (Vec<Frame>, usize) {
        let (b, p, request) = (capture.settings, capture.params, capture.request);
        let Selection { fb, avail, average, frame0, .. } = *selection;
        let id = request.identity;
        let raw_frame = raw_frame.filter(|_| avail as u32 == completed.count);
        let cores = &mut self.raw.cores;
        let capture = self.raw.dump.as_mut().and_then(|dump| {
            let input = BatchInput::new((b, p), (completed.count as usize, avail, frame0), (cores, request.run.cores));
            let newest = (raw_frame.as_ref().map(|(frame, _)| frame), la.as_ref().map(|la| la.words.as_slice()));
            dump.capture_live(request.pair(), input, self.dma.destination().data(), newest)
        });
        if let Some(pipe) = pipe {
            *seq += avail as u64;
            let readout = batch::Readout::new(
                pipe,
                (b, p),
                (fb, avail, frame0),
                (*seq, probe, raw_frame.map(|(frame, _)| frame)),
                (id, completed.clock.with_admission()),
            )
            .reserved(tap)
            .trace(capture.map(Pending::trace));
            let frames = readout.dispatch(
                self.dma.completed(),
                request.run.cores.then_some(cores),
                |mut newest| {
                    if let Some(la) = la.take() {
                        newest.la = Some(la);
                    }
                    publication::tag(newest, id)
                },
                obsolete,
            );
            (frames, 0)
        } else {
            if let Some(pending) = capture {
                pending.finish_records();
            }
            tap.inspect(|charge| charge.fallback());
            batch::decode_records(
                (request.run.cores, id),
                (b, p, probe),
                (self.dma.destination().data(), fb, avail),
                (frame0, average.is_some()),
                raw_frame,
                (seq, cores),
            )
        }
    }
}

fn log_average(kind: avg::Kind, (logged, count, after): (u32, u32, &avg::After), progress: u32, words: &[[u8; 2]]) {
    let words: Vec<_> = words.iter().take(4).map(|&word| format!("{:04x}", u16::from_le_bytes(word))).collect();
    eprintln!(
        "oos-ui hw: average {:?} batch {} of {} triggers, {} in, show {}, 0x10b0 = {}, words {}",
        kind,
        logged,
        count,
        after.triggers,
        after.show,
        progress,
        words.join(" ")
    );
}
