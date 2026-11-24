// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    batch, batch_timing, la_record, lifecycle, plan::raw_newest, raw, stop, window::deep_raw_settings, Settings,
};
use super::pacing::PacingPolicy;
use super::{status, BatchState, Io, LoopClock, Port, Request};
use crate::{
    board::{HwShared, MaskRead, Shared, ZoneRead},
    drawpipe::{clock::StageClock, TapReservation},
    perfguard::deep::acq_lock as lock,
};
use oos_hal::dho::{
    bus::{Bus, DmaBuf},
    hz, mask,
    seq::{self, time::Clock},
    zone,
};
use std::io::Read;

/// Disjoint hardware borrows while a batch's register-settling policy is active.
pub(super) struct ReadIo<'a, B, R, C> {
    pub(super) bar: B,
    pub(super) c2h: &'a mut R,
    pub(super) clock: &'a C,
    pub(super) state: &'a mut seq::State,
}

impl<B: Bus, R: Read, C: Clock> ReadIo<'_, B, R, C> {
    pub(super) fn execute(&mut self, steps: &[seq::Step], dma: &mut DmaBuf) -> Result<(), String> {
        seq::time::execute_bus_with_clock(steps, &mut self.bar, self.c2h, dma, &mut |_| {}, &|| false, self.clock)
    }
}

impl<P: Port> Io<P> {
    pub(super) fn batch_io(&mut self) -> ReadIo<'_, impl Bus + '_, P::Stream, P::Clock> {
        ReadIo {
            bar: self.port.batch_bus(&mut self.bar),
            c2h: &mut self.c2h,
            state: &mut self.state,
            clock: &self.clock,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Capture<'a> {
    pub(super) settings: &'a Settings,
    pub(super) params: &'a hz::Params,
    pub(super) request: &'a Request,
    pub(super) uncapped: u32,
}

pub(super) struct Completed {
    pub(super) count: u32,
    pub(super) batch_ms: f32,
    pub(super) clock: StageClock,
}

pub(super) struct Readout {
    pub(super) readings: Readings,
    pub(super) raw_frame: Option<batch::DecodedRecord>,
    pub(super) la: Option<crate::ui::la::LaRec>,
}

pub(super) struct Readings {
    mask: Option<MaskRead>,
    zone: Option<ZoneRead>,
    fired: Option<bool>,
    deep_ok: bool,
}

impl Readings {
    pub(super) fn publish(self, g: &mut Shared, cols: Option<&[mask::Column]>, stats: Option<(u32, usize)>) {
        g.trig_fired = Some((g.trig_fired.map_or(1, |old| old.0 + 1), self.fired));
        g.deep_ok = self.deep_ok;
        let cols = cols.filter(|_| self.mask.is_some());
        g.mask = self.mask.map(|mut next| {
            next.n = g.mask.map_or(1, |old| old.n + 1);
            next
        });
        g.zone = self.zone.map(|mut next| {
            next.n = g.zone.map_or(1, |old| old.n + 1);
            next
        });
        match (cols, g.mask_table.as_ref()) {
            (Some(cols), Some(table)) if mask_entries(cols).eq(table.iter().copied()) => {}
            (Some(cols), _) => g.mask_table = Some(std::sync::Arc::new(mask_entries(cols).collect())),
            (None, _) => g.mask_table = None,
        }
        if let Some((decoded, skipped)) = stats {
            g.decoded += u64::from(decoded);
            g.dropped += skipped as u64;
        }
    }
}

impl BatchState {
    pub(super) fn fill(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        capture: &Capture<'_>,
        (ahead, mut timing, pacing): (bool, batch_timing::SourceTiming, &impl PacingPolicy),
        (out, reserved, prearm): (&HwShared, bool, bool),
        plan: &super::super::dma_source::ArmPlan<'_>,
    ) -> Result<Completed, String> {
        let (b, p, request) = (capture.settings, capture.params, capture.request);
        let asked = (request.want.as_ref(), request.mode);
        let wait = stop::FirstWait::default();
        let clock = io.clock;
        let changed = || wait.canceled(&lock(out), plan, asked, (b.auto, reserved), io.clock.now());
        let started = io.clock.now();
        let fill_ms = pacing.fill_ms(&mut self.fill, started);
        let allowed = |count| capture_read_allowed(plan, out, &mut self.record_wait, asked, count, clock);
        // The HAL owns the fill, ring readout and C2H sequence as one cancellation-aware operation.
        let count = lifecycle::batch(b.single, ahead, || {
            seq::time::acquire_batch_observed(
                io.state,
                b,
                p,
                &mut io.bar,
                io.c2h,
                self.dma.destination(),
                ahead,
                (timing.wait(fill_ms), clock),
                |_| {},
                changed,
                (|| plan.serial() && plan.capture_changed(&lock(out), asked), allowed),
                |phase| timing.observe(phase),
            )
        })
        .inspect_err(|message| {
            if stop::needs_disarm(message) {
                super::disarm(io.state, &mut io.bar, io.c2h, clock, out);
            }
        })?;
        let read_at = io.clock.now();
        let batch_ms = (read_at - started).as_secs_f32() * 1e3;
        let read_ms = timing.read_ms();
        crate::pacing::batch(|| clock.now(), batch_ms, count, seq::frames(b, p), fill_ms, read_ms);
        let pre_ms = timing.pre_ms(started);
        self.fill.done(pre_ms + read_ms);
        self.cap.done(
            count,
            read_ms,
            pre_ms,
            capture.uncapped,
            prearm && super::super::window::prearm_keeps_replay(b, p, None),
            || pacing.picture_period(),
        );
        Ok(Completed { count, batch_ms, clock: timing.complete(request.identity, read_at, count) })
    }

    pub(super) fn read(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        capture: &Capture<'_>,
        completed: &Completed,
        (out, probe): (&HwShared, crate::board::dho::switches::HeaderProbe),
        tap: Option<&std::sync::Arc<TapReservation>>,
    ) -> Readout {
        let (b, p) = (capture.settings, capture.params);
        let count = completed.count;
        let live = raw_newest(b, p);
        crate::compress::note_batch_k(if live { b.compress } else { 1 });
        let la = batch::read_la(io.state, (b, p), (&mut io.bar, io.c2h, io.clock), &mut self.la_fails, |message| {
            status(out, message);
        })
        .map(|words| la_record(words, b, p));
        let raw_frame = raw::replay_due(count, live, &mut self.raw.live, io.clock)
            .then(|| seq::raw_settings(b))
            .and_then(|settings| self.read_raw(io, capture, (&settings, count), (out, probe), (tap, false)));
        let deep_settings = deep_raw_settings(b, p);
        let deep_ok = deep_settings.is_some();
        let raw_frame = raw_frame.or_else(|| {
            let want = capture.request.run.deep.filter(|_| count > 0)?;
            let settings = deep_settings
                .filter(|ds| crate::deepmeas::due(want, self.raw.deep, io.clock.now(), seq::frame_bytes(ds, p)))?;
            self.raw.deep = Some(io.clock.now());
            self.read_raw(io, capture, (&settings, count), (out, probe), (tap, true))
        });
        let fired = batch::read_trigger(&mut io.bar);
        let mask = mask_read(io, b, p);
        let zone = zone_read(io, b, (count, completed.batch_ms));
        Readout { readings: Readings { mask, zone, fired, deep_ok }, raw_frame, la }
    }

    pub(super) fn read_raw(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        capture: &Capture<'_>,
        (b, count): (&Settings, u32),
        (out, probe): (&HwShared, crate::board::dho::switches::HeaderProbe),
        (tap, deep): (Option<&std::sync::Arc<TapReservation>>, bool),
    ) -> Option<batch::DecodedRecord> {
        let p = capture.params;
        let started = deep.then(|| io.clock.now());
        let index = count - 1;
        let read = seq::time::read_raw_again_with_clock(
            io.state,
            b,
            p,
            seq::time::RawReadIo::new(&mut io.bar, io.c2h, &mut self.raw.dma, io.clock),
            index,
            |_| {},
        );
        if let Err(e) = read {
            let what = if deep { "deep window" } else { "newest frame" };
            status(out, format!("raw read of the {}: {}", what, e));
            return None;
        }
        let decoded_at = started.map(|_| io.clock.now());
        let cores = capture.request.run.cores.then_some(&mut self.raw.cores);
        let frame = raw::decode_raw(self.raw.dma.data(), (b, p), probe, cores, tap);
        if let Some((started, at)) = started.zip(decoded_at) {
            use crate::deepmeas::cost;
            cost::set(&cost::DMA_US, at - started);
            cost::set(&cost::DECODE_US, io.clock.now().saturating_duration_since(at));
            cost::READS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        frame
    }
}

fn capture_read_allowed(
    plan: &super::super::dma_source::ArmPlan<'_>,
    out: &HwShared,
    record_wait: &mut super::publication::RecordWait,
    asked: stop::Asked<'_>,
    count: u32,
    clock: &impl LoopClock,
) -> bool {
    let interrupted = || stop::admission_interrupted(&lock(out), plan, asked);
    if plan.read_ahead() && plan.uses_record_queue() {
        while record_wait.poll(out, None, clock, interrupted, |message| status(out, message)) {
            if interrupted() {
                break;
            }
        }
    }
    let allowed = plan.read_allowed(clock, interrupted, || stop::read_obsolete(&lock(out), plan, asked));
    if !allowed {
        crate::drawpipe::counters().metrics.source_capture_discarded(count);
    }
    allowed
}

fn mask_entries(cols: &[mask::Column]) -> impl Iterator<Item = [u8; 4]> + '_ {
    cols.iter().map(|col| [col.lo, col.hi, col.lo_en as u8, col.hi_en as u8])
}

fn mask_read(
    io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
    b: &Settings,
    p: &hz::Params,
) -> Option<MaskRead> {
    let ((ctrl, m), run) = io.state.mask_ctrl().zip(b.hwmask.as_ref()).zip(io.state.mask_run)?;
    let raw = mask::read(&mut io.bar, ctrl);
    Some(MaskRead {
        ch: m.ch,
        slot: run.0,
        ctrl,
        raw: [raw.frames_lo, raw.frames_hi, raw.fails_lo, raw.fails_hi],
        frames: raw.frames(),
        fails: raw.fails(),
        n: 0,
        scale: b.ch[m.ch].scale,
        offset: b.ch[m.ch].offset,
        gain: io.state.gain[m.ch],
        points: p.memsel,
        chans: b.mask,
    })
}

fn zone_read(
    io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
    b: &Settings,
    (count, batch_ms): (u32, f32),
) -> Option<ZoneRead> {
    let ((ctrl, z), run) = io.state.zone_ctrl().zip(b.hwzone.as_ref()).zip(io.state.zone_run)?;
    let r = zone::read(&mut io.bar, ctrl);
    let c = r.counters;
    Some(ZoneRead {
        ch: z.ch,
        slot: run.0,
        ctrl,
        raw: [r.s4030, r.s4038, c.frames_lo, c.frames_hi, c.fails_lo, c.fails_hi],
        frames: c.frames(),
        armed: r.armed(),
        pass: r.pass(),
        in_cycle: r.in_cycle(),
        state: r.state(),
        n: 0,
        batch: (count, b.frames, batch_ms),
    })
}
