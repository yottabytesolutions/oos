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
use std::time::Duration;

impl<T: PacingPolicy> AcqLoop<T> {
    pub(super) fn service<P: Port>(&mut self, io: &mut Io<P>, counter: Option<CounterWant>) -> Option<Request> {
        self.service.readings(io, &self.out, counter);
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
                io.disarm(&mut self.batch.armed);
                io.abort_segments(&mut self.segrun, &self.out);
                self.stream.stop(io);
                if P::calibrate(io, &self.options.cal_dir, &self.out, req) {
                    self.units.reset_trim();
                }
                self.configured.current = None;
                return None;
            }
        };
        if self.service.generator_changed(request.generator) {
            io.disarm(&mut self.batch.armed);
            io.abort_segments(&mut self.segrun, &self.out);
            self.service.generator(io, &self.out, request.generator);
        }
        Some(request)
    }

    pub(super) fn reconfigure(
        &mut self,
        io: &mut Io<impl Port>,
        request: &Request,
        last_read: &mut bool,
    ) -> Option<(Settings, hz::Params)> {
        let Some(mut want) = request.want.clone() else {
            io.disarm(&mut self.batch.armed);
            io.clock.wait(Duration::from_millis(50));
            return None;
        };
        let want_mode = request.mode;
        if self.stream.boundary(self.configured.applied_pair, request.identity) {
            io.disarm(&mut self.batch.armed);
            self.stream.restart_scan(io, want_mode);
        }
        self.apply_overrides(io, request, &mut want);
        let held = request.run.hold;
        if held && !self.replay.paused {
            self.enter_hold(io, request, *last_read);
        }
        if self.stopped_view(io, request.run.stop_view, want_mode) {
            return None;
        }
        if held {
            self.poll_held(io, request, &want);
            return None;
        }
        if publication::reconfigure(
            self.configured.current.as_ref().map(|c| (&c.0, self.configured.mode)),
            &want,
            want_mode,
        ) {
            *last_read = false;
            if !self.configure(io, want, want_mode, request.avg_clear) {
                return None;
            }
        }
        let gen = request.identity.gen();
        self.configured.applied_pair = Some((gen, request.identity.epoch()));
        crate::pacing::APPLIED.store(gen, std::sync::atomic::Ordering::Relaxed);
        self.configured.current.clone()
    }

    fn apply_overrides(&mut self, io: &Io<impl Port>, request: &Request, want: &mut Settings) {
        want.afe = self.options.program_afe;
        raw_for_ui(want, request.needs_raw);
        let map = mask_map();
        let blocked = self.batch.guard.blocks(&unit_key(want, map), io.clock.now());
        if blocked {
            hw_merge(want, None, None, map);
        } else {
            hw_merge(want, request.mask.as_ref(), request.zone.as_ref(), map);
        }
        // The flush batch leaves the hardware view in the UI.
        let blocked = blocked && !self.batch.guard.flush;
        if self.configured.unit_blocked != blocked {
            self.configured.unit_blocked = blocked;
            lock(&self.out).unit_blocked = blocked.then(|| {
                "the mask/zone unit stopped the acquisition with these settings: tested in software".to_string()
            });
        }
        if let Some(la) = want.la.as_mut() {
            la.cal = io.la_cal;
        }
    }

    fn poll_held(&mut self, io: &mut Io<impl Port>, request: &Request, want: &Settings) {
        let settings = self.configured.current.as_ref().map_or(want, |(applied, _)| applied);
        self.units.poll(
            io,
            (&mut self.batch.armed, self.segrun.as_ref()),
            &self.out,
            (settings, self.configured.mode),
            (request, self.service.generator_on()),
        );
        self.stream.poll_control(io, &self.out, self.configured.mode);
        lock(&self.out).final_ack = request.run.final_req;
        io.clock.wait(Duration::from_millis(5));
    }

    fn enter_hold(&mut self, io: &mut Io<impl Port>, request: &Request, last_read: bool) {
        // An armed successor has already started overwriting the ring.
        let ahead = self.batch.armed.is_some();
        io.disarm(&mut self.batch.armed);
        if ahead {
            self.replay.last_batch = None;
        }
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

    fn stopped_view(&mut self, io: &mut Io<impl Port>, view: Option<(f64, f64)>, want_mode: Slow) -> bool {
        let Some(view) = view.filter(|_| self.configured.mode == Slow::Off && want_mode == Slow::Off) else {
            self.replay.replayed = None;
            return false;
        };
        let cores = {
            let mut g = lock(&self.out);
            g.final_ack = g.final_req;
            g.cores
        };
        if let Some(batch) = self.replay.last_batch.as_ref().filter(|_| self.replay.replayed != Some(view)) {
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
                    lock(&self.out).replay = Some(batch.view(frame, point));
                }
                Err(e) => status(&self.out, format!("STOP read: {}", e)),
            }
        } else {
            io.clock.wait(Duration::from_millis(20));
        }
        true
    }

    fn configure<P: Port>(&mut self, io: &mut Io<P>, want: Settings, mode: Slow, avg_clear: u64) -> bool {
        io.disarm(&mut self.batch.armed);
        lock(&self.out).record_flow.serial = false;
        io.abort_segments(&mut self.segrun, &self.out);
        self.configured.mode = mode;
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
        self.configured.current = Some((want, params));
        true
    }
}
