// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::super::publication;
use super::super::{
    comparator::{hw_merge, mask_map, unit_key},
    decode_with, lifecycle,
    plan::raw_for_ui,
    stop_plan,
};
use super::pacing::PacingPolicy;
use super::{status, AcqLoop, Io, Port, Request};
use crate::{
    board::{CounterWant, Slow},
    perfguard::deep::acq_lock as lock,
};
use oos_hal::dho::{
    hz,
    seq::{self, time::Clock, Settings},
};
use std::sync::atomic::Ordering;
use std::time::Duration;

const VIEW_POLL: Duration = Duration::from_millis(20);

impl<T: PacingPolicy> AcqLoop<T> {
    pub(super) fn service<P: Port>(&mut self, io: &mut Io<P>, counter: Option<CounterWant>) -> Option<Request> {
        if self.configured.current.is_some() {
            self.service.counter(io, &self.out, counter);
        }
        self.service.readings(io, &self.out);
        let request = {
            let mut g = lock(&self.out);
            match g.cal_req.take() {
                Some(calibration) => Err(calibration),
                None => Ok(Request::read(&g)),
            }
        };
        let request = match request {
            Ok(request) => request,
            Err(req) => {
                io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
                io.abort_segments(&mut self.segrun, &self.out);
                self.stream.stop(io);
                self.configured.current = None;
                self.replay.discard(&self.out);
                if P::calibrate(io, &self.options.cal_dir, &self.out, req) {
                    self.units.reset_trim();
                }
                return None;
            }
        };
        if self.service.generator_changed(request.generator) {
            io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
            io.abort_segments(&mut self.segrun, &self.out);
            self.service.generator(io, &self.out, request.generator);
        }
        Some(request)
    }

    pub(super) fn reconfigure<P: Port>(
        &mut self,
        io: &mut Io<P>,
        request: &Request,
        last_read: &mut bool,
    ) -> Option<(Settings, hz::Params)> {
        let Some(want) = request.want.as_ref() else {
            io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
            io.clock.wait(Duration::from_millis(50));
            return None;
        };
        let held = request.run.hold;
        if held && !self.replay.paused {
            self.enter_hold(io, request, *last_read);
        }
        self.stream_boundary(io, request);
        // STOP defers edits to an applied configuration and keeps the batch captured with it.
        // At startup or after calibration, restore a missing configuration before serving the hold.
        if self.configured.current.is_none() || !held {
            let mut want = want.clone();
            let blocked = self.apply_overrides(io, request, &mut want);
            let current = self.configured.current.as_ref().map(|c| (&c.0, self.configured.mode));
            if publication::reconfigure(current, &want, request.mode) {
                *last_read = false;
                if !self.configure(io, want, request.mode, request.avg_clear) {
                    return None;
                }
            }
            self.publish_unit_blocked(blocked);
            crate::pacing::APPLIED.store(request.identity.gen(), Ordering::Relaxed);
        }
        // Side service can disarm and drop the retained slot. Check for a view batch afterwards.
        self.poll_units(io, request);
        if self.poll_stop_view(io, request, *last_read) {
            return None;
        }
        if held {
            self.poll_held(io, request);
            return None;
        }
        self.configured.current.clone()
    }

    fn poll_units(&mut self, io: &mut Io<impl Port>, request: &Request) {
        let Some((settings, _)) = self.configured.current.as_ref() else { return };
        self.units.poll(
            io,
            self.segrun.as_ref(),
            &self.out,
            (settings, self.configured.mode),
            (request, self.service.generator_on()),
            |io| io.disarm(&mut self.batch.armed, &mut self.replay, &self.out),
        );
    }

    fn stream_boundary<P: Port>(&mut self, io: &mut Io<P>, request: &Request) {
        if self.stream.boundary(self.configured.stream_pair, request.identity) {
            #[cfg(test)]
            P::stream_cleared(io, request.identity);
            io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
            self.stream.restart_scan(io, request.mode);
            self.configured.stream_pair = Some(request.pair());
        }
    }

    fn apply_overrides(&self, io: &Io<impl Port>, request: &Request, want: &mut Settings) -> bool {
        want.afe = self.options.program_afe;
        raw_for_ui(want, request.needs_raw);
        let map = mask_map();
        let blocked = self.batch.guard.blocks(&unit_key(want, map), io.clock.now());
        if blocked {
            hw_merge(want, None, None, map);
        } else {
            hw_merge(want, request.mask.as_ref(), request.zone.as_ref(), map);
        }
        if let Some(la) = want.la.as_mut() {
            la.cal = io.la_cal;
        }
        // The flush batch leaves the hardware view in the UI.
        blocked && !self.batch.guard.flush
    }

    fn publish_unit_blocked(&mut self, blocked: bool) {
        if self.configured.unit_blocked != blocked {
            self.configured.unit_blocked = blocked;
            lock(&self.out).unit_blocked = blocked.then(|| {
                "the mask/zone unit stopped the acquisition with these settings: tested in software".to_string()
            });
        }
    }

    fn poll_held(&mut self, io: &mut Io<impl Port>, request: &Request) {
        let mode = self.configured.mode;
        self.stream.poll_control(io, &self.out, mode);
        // Roll and scan keep the acknowledgement from entering STOP.
        if mode == Slow::Off {
            lock(&self.out).final_ack = request.run.final_req;
        }
        io.clock.wait(Duration::from_millis(5));
    }

    fn enter_hold(&mut self, io: &mut Io<impl Port>, request: &Request, last_read: bool) {
        io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
        let last = self.replay.last_batch.as_ref().filter(|_| last_read).and_then(|read| {
            let settings = read.plan(request.run.deep)?;
            let params = &read.params;
            match seq::time::read_raw_again_with_clock(
                &mut io.state,
                &settings,
                params,
                seq::time::RawReadIo::new(&mut io.bar, &mut io.c2h, &mut self.batch.raw.dma, &io.clock),
                read.index,
                |_| {},
            ) {
                Ok(()) => {
                    self.seq += 1;
                    let frame = decode_with(
                        self.batch.raw.dma.data(),
                        &settings,
                        params,
                        self.seq,
                        self.switches.board.header_probe,
                        request.run.cores.then_some(&mut self.batch.raw.cores),
                    );
                    Some(read.tag(frame))
                }
                Err(e) => {
                    status(&self.out, format!("raw read of the deep window at STOP: {}", e));
                    None
                }
            }
        });
        publication::publish_held(&self.out, last, request.run.final_req);
        self.replay.paused = true;
    }

    fn poll_stop_view(&mut self, io: &mut Io<impl Port>, request: &Request, last_read: bool) -> bool {
        let Some(view) = request.run.stop_view else {
            self.replay.replayed = None;
            return false;
        };
        if !request.run.hold {
            // The UI publishes the view and hold separately. The pending view already blocks a new arm.
            self.replay.replayed = None;
            self.replay.fresh = last_read && self.replay.last_batch.is_some();
            io.clock.wait(VIEW_POLL);
            return true;
        }
        let batch = self.replay.last_batch.as_ref();
        let Some(batch) = batch.filter(|_| self.configured.mode == Slow::Off && request.mode == Slow::Off) else {
            self.replay.replayed = None;
            return false;
        };
        let cores = {
            let mut g = lock(&self.out);
            g.final_ack = g.final_req;
            g.cores
        };
        if self.replay.replayed != Some(view) {
            self.replay.replayed = Some(view);
            let params = &batch.params;
            let (settings, point) = stop_plan(&batch.settings, params, view.0, view.1);
            let steps = io.state.reread_steps(&settings, params, batch.index);
            match io.execute(&steps) {
                Ok(bytes) => {
                    let fb = seq::frame_bytes(&settings, params);
                    self.seq += 1;
                    let frame = decode_with(
                        &bytes[..fb.min(bytes.len())],
                        &settings,
                        params,
                        self.seq,
                        self.switches.board.header_probe,
                        cores.then_some(&mut self.batch.raw.cores),
                    );
                    let mut g = lock(&self.out);
                    if g.hold
                        && g.stop_view == Some(view)
                        && !publication::boundary(Some(request.pair()), g.acquisition.identity())
                    {
                        g.replay = Some(batch.view(frame, point));
                    } else {
                        self.replay.replayed = None;
                    }
                }
                Err(e) => status(&self.out, format!("STOP read: {}", e)),
            }
        } else {
            io.clock.wait(VIEW_POLL);
        }
        true
    }

    fn configure<P: Port>(&mut self, io: &mut Io<P>, want: Settings, mode: Slow, avg_clear: u64) -> bool {
        io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
        lock(&self.out).record_flow.serial = false;
        io.abort_segments(&mut self.segrun, &self.out);
        self.configured.current = None;
        self.replay.last_batch = None;
        self.stream.stop(io);
        let configured = lifecycle::configure(&mut io.state, &want, io.cal.as_ref(), io.cal_adc.as_ref())
            .map_err(|e| format!("settings: {}", e))
            .and_then(|(steps, params)| {
                P::apply_settings(io, &steps).map(|()| params).map_err(|e| format!("configure: {}", e))
            });
        let params = match configured {
            Ok(params) => params,
            Err(e) => {
                status(&self.out, e);
                io.clock.wait(Duration::from_millis(500));
                return false;
            }
        };
        status(&self.out, format!("configured: mask {:#x}, {} Sa/s, {} points", want.mask, params.rate, params.memsel));
        crate::pacing::configured(|| io.clock.now());
        self.batch.la_fails = 0;
        self.units.configured(io.state.mode.min(10), want.mask, self.switches.board.core_offset_tight, io.clock.now());
        self.batch.configured(want.mode, self.switches.board.avg_kind(), avg_clear);
        self.configured.mode = mode;
        self.configured.current = Some((want, params));
        true
    }
}
