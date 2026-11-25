// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::super::{publication, roll as output};
use super::super::{
    lifecycle,
    roll::{roll_la_t0, scan_gap, scan_la_pos, scan_write},
};
use super::{status, Io, Port};
use crate::{
    board::{Frame, HwShared, Slow},
    model::NCH,
    perfguard::deep::acq_lock as lock,
    ui::la::LaRec,
};
use oos_core::change::Identity;
use oos_hal::dho::{
    bus::{self, Bus},
    hz, la, roll, samples,
    seq::{self, Settings},
};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Stream {
    rolling: Option<roll::Params>,
    ring: [VecDeque<f32>; NCH],
    ring_la: VecDeque<u16>,
    sweep: [Vec<f32>; NCH],
    sweep_la: Vec<f32>,
    cursor: usize,
    last_info: roll::ScanInfo,
    halted: bool,
    done_seen: Option<Instant>,
    published: Instant,
    writes: Vec<oos_hal::dho::Write>,
}

impl Stream {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            rolling: None,
            ring: Default::default(),
            ring_la: VecDeque::new(),
            sweep: Default::default(),
            sweep_la: Vec::new(),
            cursor: 0,
            last_info: Default::default(),
            halted: false,
            done_seen: None,
            published: now,
            writes: Vec::new(),
        }
    }
}

struct Sweep<'a> {
    settings: &'a Settings,
    params: &'a hz::Params,
    tdiv_fs: i128,
    scan: Option<(i128, bool)>,
    la: Option<la::Timing>,
}

impl Sweep<'_> {
    fn start(&self, io: &mut Io<impl Port>, rp: &roll::Params) -> Vec<oos_hal::dho::Write> {
        let mut writes = match self.scan {
            Some((delay_fs, _)) => {
                let pos = roll::scan_trig_pos(delay_fs, self.tdiv_fs, self.params.dot_fs);
                roll::config_scan(&mut io.state.sh, rp, self.params.memsel, pos)
            }
            None => roll::config(&mut io.state.sh, rp, self.params.memsel),
        };
        io.bar.apply(&writes);
        if let Some(timing) = &self.la {
            let logic = roll::la_config(rp, timing);
            io.bar.apply(&logic);
            writes.extend(logic);
        }
        let start = lifecycle::roll_start(&mut io.state.sh, self.settings.mask);
        io.bar.apply(&start);
        writes.extend(start);
        writes
    }
}

struct Geometry {
    dt: f64,
    la_dt: f64,
    keep: usize,
    keep_la: usize,
}

impl Stream {
    pub(super) fn boundary(&mut self, applied: Option<(u64, u64)>, identity: Identity) -> bool {
        output::boundary(
            applied,
            identity,
            &mut self.ring,
            &mut self.sweep,
            &mut self.ring_la,
            &mut self.sweep_la,
            &mut self.cursor,
        )
    }

    pub(super) fn restart_scan(&mut self, io: &mut Io<impl Port>, mode: Slow) {
        if output::restart_scan(mode, self.halted, &mut self.rolling) {
            io.bar.apply(&lifecycle::roll_stop(&mut io.state.sh));
        }
    }

    pub(super) fn stop(&mut self, io: &mut Io<impl Port>) {
        if self.rolling.take().is_some() {
            io.bar.apply(&lifecycle::roll_stop(&mut io.state.sh));
        }
    }

    pub(super) fn poll_control(&mut self, io: &mut Io<impl Port>, out: &HwShared, mode: Slow) -> bool {
        output::poll(mode, out, &mut self.rolling, &mut self.ring, &mut self.ring_la, &mut io.state.sh, &mut io.bar)
    }

    /// Returns false if a read error requires the acquisition to be configured again.
    pub(super) fn poll<P: Port>(
        &mut self,
        io: &mut Io<P>,
        out: &HwShared,
        (settings, params, mode): (&Settings, &hz::Params, Slow),
        (seq, identity): (&mut u64, Identity),
        disarm: impl FnOnce(&mut Io<P>),
    ) -> bool {
        if !self.poll_control(io, out, mode) {
            std::thread::sleep(Duration::from_millis(5));
            return true;
        }
        disarm(io);
        let scan = match mode {
            Slow::Scan { delay, single } => Some(((delay * 1e15) as i128, single)),
            _ => None,
        };
        let sweep = Sweep {
            settings,
            params,
            tdiv_fs: seq::tdiv_fs(settings.tdiv),
            scan,
            la: settings.la.is_some().then(|| la::timing(params, settings.mask)),
        };
        let rp = self.start(io, out, &sweep);
        if self.halted {
            if !std::mem::take(&mut lock(out).rearm) {
                std::thread::sleep(Duration::from_millis(20));
                return true;
            }
            self.writes = sweep.start(io, &rp);
            (self.cursor, self.halted) = (0, false);
        }
        let dt = rp.spacing_fs as f64 / 1e15;
        let la_dt = if sweep.la.is_some() { roll::la_spacing_fs(&rp) as f64 / 1e15 } else { dt };
        let g = Geometry {
            dt,
            la_dt,
            keep: ((10.0 * settings.tdiv / dt).round() as usize).max(2),
            keep_la: ((10.0 * settings.tdiv / la_dt).round() as usize).max(2),
        };
        if !self.read_chunks(io, out, &sweep, &g) {
            self.rolling = None;
            return false;
        }
        let done = scan.is_some_and(|(_, single)| self.scan_done(io, out, single, g.keep));
        if done || self.published.elapsed().as_millis() >= 33 {
            self.published = Instant::now();
            *seq += 1;
            let frame = self.frame(&sweep, &g, done, *seq, identity);
            self.publish(out, frame, scan.is_some());
        }
        if done && !self.halted {
            std::thread::sleep(Duration::from_millis(500));
            self.writes = sweep.start(io, &rp);
            self.cursor = 0;
        }
        true
    }

    fn start(&mut self, io: &mut Io<impl Port>, out: &HwShared, sweep: &Sweep<'_>) -> roll::Params {
        if let Some(rp) = self.rolling {
            return rp;
        }
        let s = sweep.settings;
        let rp = roll::params(s.mask, sweep.tdiv_fs, s.depth, sweep.params);
        self.writes = sweep.start(io, &rp);
        self.ring.iter_mut().for_each(VecDeque::clear);
        self.sweep.iter_mut().for_each(Vec::clear);
        self.ring_la.clear();
        self.sweep_la.clear();
        (self.cursor, self.halted, self.done_seen, self.last_info) = (0, false, None, Default::default());
        let name = if sweep.scan.is_some() { "scan" } else { "roll" };
        status(
            out,
            format!(
                "{}: {:.1} us per point, chunk {:.2} ms",
                name,
                rp.spacing_fs as f64 / 1e9,
                rp.spacing_fs as f64 * 100.0 / rp.ratio as f64 / 1e12
            ),
        );
        self.rolling = Some(rp);
        crate::compress::note_batch_k(1);
        rp
    }

    fn read_chunks(&mut self, io: &mut Io<impl Port>, out: &HwShared, sweep: &Sweep<'_>, g: &Geometry) -> bool {
        let pulse = roll::fill_pulse(&io.state.sh);
        io.bar.apply(&pulse);
        let fifo = io.bar.rd(0x11e8);
        let chunks = roll::chunks(fifo);
        let has_la = sweep.la.is_some();
        let chunks = if has_la { roll::la_chunks(chunks) } else { chunks };
        if chunks == 0 {
            std::thread::sleep(Duration::from_millis(9));
            return true;
        }
        let asked = chunks * roll::CHUNK_BYTES;
        let mut received = 0;
        let mut dma = bus::DmaBuf::new();
        let data = match dma.read_with_progress(&mut io.c2h, asked, |_, _| {}, &mut received) {
            Ok(data) => data,
            Err(error) => {
                let name = if sweep.scan.is_some() { "scan" } else { "roll" };
                let (context, error) = if error.kind() == std::io::ErrorKind::TimedOut {
                    let writes = self.writes.iter().copied().chain(pulse);
                    let detail = seq::time::dma_timeout(&mut io.bar, (asked, received), writes);
                    (format!("{name} FIFO 0x11e8={fifo:#010x}"), seq::time::ExecuteError::DmaTimeout(detail))
                } else {
                    (format!("{name} read of {chunks} chunks"), seq::time::ExecuteError::DmaRead(error.to_string()))
                };
                io.report_read_error(out, &context, error);
                return false;
            }
        };
        io.dma_fault.read_succeeded();
        let s = sweep.settings;
        let (words, logic) =
            if has_la { roll::split_la(data, s.mask) } else { (roll::split(data, s.mask), Vec::new()) };
        if sweep.scan.is_some() {
            self.write_scan(g, s, (data, has_la), (&words, &logic));
        } else {
            self.ring_la.extend(logic);
            while self.ring_la.len() > g.keep_la {
                self.ring_la.pop_front();
            }
            for (ch, (ring, words)) in self.ring.iter_mut().zip(&words).enumerate() {
                if words.is_empty() {
                    ring.clear();
                } else {
                    ring.extend(samples::volts_f64(words, s.ch[ch].scale, s.ch[ch].offset));
                    while ring.len() > g.keep {
                        ring.pop_front();
                    }
                }
            }
        }
        true
    }

    fn write_scan(
        &mut self,
        g: &Geometry,
        s: &Settings,
        (data, has_la): (&[u8], bool),
        (words, logic): (&[Vec<u16>; NCH], &[u16]),
    ) {
        // The first analog chunk contains the sweep write position.
        let first = data
            .as_chunks::<{ roll::CHUNK_BYTES }>()
            .0
            .iter()
            .map(|chunk| chunk.as_slice())
            .find(|c| !roll::is_la_chunk(c))
            .unwrap_or(&data[..roll::CHUNK_BYTES]);
        let col = roll::scan_column(roll::counter(first), has_la);
        self.cursor = (col * roll::points_per_chunk(s.mask)) % g.keep;
        if !logic.is_empty() {
            let volts: Vec<f32> = logic.iter().map(|&x| x as f32).collect();
            scan_write(&mut self.sweep_la, g.keep_la, scan_la_pos(self.cursor, g.keep, g.keep_la), &volts);
        }
        let fresh = words.iter().map(Vec::len).max().unwrap_or(0);
        for (ch, (sweep, words)) in self.sweep.iter_mut().zip(words).enumerate() {
            if words.is_empty() {
                sweep.clear();
            } else {
                scan_write(sweep, g.keep, self.cursor, &samples::volts_f64(words, s.ch[ch].scale, s.ch[ch].offset));
            }
        }
        self.cursor = (self.cursor + fresh) % g.keep;
    }

    fn scan_done(&mut self, io: &mut Io<impl Port>, out: &HwShared, single: bool, keep: usize) -> bool {
        if self.done_seen.is_none() {
            let r = roll::SCAN_INFO_REGS.map(|a| io.bar.rd(a));
            let info = roll::scan_info(r[0], r[1]);
            if info.triggered != self.last_info.triggered || info.done {
                status(
                    out,
                    format!("scan: 0x1204 = {:#010x}, 0x40d0 = {:#x}, cursor {} of {}", r[0], r[1], self.cursor, keep),
                );
            }
            self.last_info = info;
            if info.done {
                self.done_seen = Some(Instant::now());
            }
        }
        // Done can arrive before the last chunks. Keep reading through the final column.
        let Some(seen) = self.done_seen.filter(|t| self.cursor == 0 || t.elapsed().as_millis() >= 100) else {
            return false;
        };
        status(
            out,
            format!(
                "scan: sweep complete {} ms after done, cursor {} of {}",
                seen.elapsed().as_millis(),
                self.cursor,
                keep
            ),
        );
        self.done_seen = None;
        io.bar.apply(&roll::scan_done(&mut io.state.sh));
        self.halted = single;
        true
    }

    fn frame(&self, sweep: &Sweep<'_>, g: &Geometry, done: bool, seq: u64, identity: Identity) -> Frame {
        let scan = sweep.scan.is_some();
        let (ch, pre, la, partial) = if scan {
            let mut ch = self.sweep.clone();
            if !done {
                scan_gap(&mut ch, self.cursor, g.keep / 50);
            }
            let la = sweep.la.map(|_| LaRec {
                words: Arc::new(self.sweep_la.iter().take_while(|x| !x.is_nan()).map(|&x| x as u16).collect()),
                t0: 0.0,
                dt: g.la_dt,
            });
            (ch, 0, la, !done)
        } else {
            let len = self.ring.iter().map(VecDeque::len).max().unwrap_or(0);
            let pre = (len as f64 - 1.0 - 5.0 * sweep.settings.tdiv / g.dt).max(0.0) as usize;
            let la = sweep.la.map(|_| LaRec {
                t0: roll_la_t0(len, pre, g.dt, self.ring_la.len(), g.la_dt),
                words: Arc::new(self.ring_la.iter().copied().collect()),
                dt: g.la_dt,
            });
            (self.ring.each_ref().map(|r| r.iter().copied().collect()), pre, la, len < g.keep)
        };
        Frame {
            seq,
            dt: g.dt,
            pre,
            ch,
            roll: !scan,
            envelope: true,
            scan,
            partial,
            stamp: identity.epoch(),
            cfg: identity.gen(),
            la,
            ..Frame::default()
        }
    }

    fn publish(&self, out: &HwShared, frame: Frame, scan: bool) {
        let sample = {
            let mut g = lock(out);
            let sample = if publication::current(&g, &frame) { output::publish(&mut g, frame, scan) } else { None };
            g.halted = self.halted;
            sample
        };
        if let Some(sample) = sample {
            sample.record();
        }
    }
}
