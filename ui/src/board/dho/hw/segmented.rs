// SPDX-License-Identifier: GPL-2.0-or-later

//! Hardware segment phases shared with the host HAL engine fixture.

use super::{decode_probed, seg_read_plan};
use crate::board::dho::switches::HeaderProbe as Probe;
use crate::board::dho::{
    lifecycle,
    publication::{self, tag},
};
use crate::board::{HwShared, SegPhase};
use crate::perfguard::deep::acq_lock as lock;
use oos_core::change::Identity;
use oos_hal::dho::{
    bus::{Bus, DmaBuf},
    hz, seg,
    seq::{Settings, State},
};

/// A recording retains the identity captured before its first hardware arm.
pub(super) struct SegRun {
    run: u64,
    identity: Identity,
    n: u32,
    recording: bool,
    count: u32,
    next: u32,
    deliver: bool,
    tl: seg::Timeline,
}

impl SegRun {
    /// A recording runs in the FPGA (upstream 85e20164: the measure unit and the core trim wait for its end).
    pub(super) fn recording(&self) -> bool {
        self.recording
    }
}

/// End the hardware recording and discard its readback state before replacing its pair.
pub(super) fn seg_abort(
    run: &mut Option<SegRun>,
    state: &mut State,
    bar: &mut impl Bus,
    c2h: &mut impl std::io::Read,
    out: &HwShared,
) {
    let Some(r) = run.take() else { return };
    if r.recording {
        if let Err(e) = lifecycle::seg_stop(state, bar, c2h, true) {
            eprintln!("oos-ui hw: segments stop: {}", e);
        }
    }
    let mut g = lock(out);
    (g.seg.phase, g.seg.shown, g.seg.want) = (SegPhase::Idle, None, None);
    (g.seg.recorded, g.seg.read) = (0, 0);
    g.seg.times.clear();
}

/// The borrowed IO of one segment poll. Hardware and the HAL simulator use the same phases.
pub(super) struct SegmentIo<'a, B, R> {
    state: &'a mut State,
    bar: &'a mut B,
    c2h: &'a mut R,
    dma: &'a mut DmaBuf,
    out: &'a HwShared,
    seq: &'a mut u64,
}

impl<'a, B: Bus, R: std::io::Read> SegmentIo<'a, B, R> {
    pub(super) fn new(
        state: &'a mut State,
        bar: &'a mut B,
        c2h: &'a mut R,
        dma: &'a mut DmaBuf,
        out: &'a HwShared,
        seq: &'a mut u64,
    ) -> Self {
        Self { state, bar, c2h, dma, out, seq }
    }

    pub(super) fn step(
        &mut self,
        run: &mut Option<SegRun>,
        s: &Settings,
        p: &hz::Params,
        id: Identity,
    ) -> Result<(), String> {
        if run.as_ref().is_some_and(|r| {
            (r.recording || r.next < r.count) && publication::boundary(Some((r.identity.gen(), r.identity.epoch())), id)
        }) {
            seg_abort(run, self.state, self.bar, self.c2h, self.out);
        }
        let (cmd, running, want) = {
            let g = lock(self.out);
            (g.seg.run, g.seg.running && !g.hold, g.seg.want)
        };
        if running && run.as_ref().is_none_or(|r| r.run != cmd) {
            if run.as_ref().is_some_and(|r| r.recording) {
                lifecycle::seg_stop(self.state, self.bar, self.c2h, true)?;
            }
            let total = lifecycle::seg_start(self.state, s, p, self.bar, self.c2h)?;
            *run = Some(SegRun {
                run: cmd,
                identity: id,
                n: total,
                recording: true,
                count: 0,
                next: 0,
                deliver: true,
                tl: seg::Timeline::new(None),
            });
            let mut g = lock(self.out);
            g.seg = crate::board::HwSeg {
                run: g.seg.run,
                running: g.seg.running,
                done_run: cmd,
                phase: SegPhase::Recording,
                total,
                ..Default::default()
            };
            return Ok(());
        }
        let Some(r) = run.as_mut() else {
            std::thread::sleep(std::time::Duration::from_millis(10));
            return Ok(());
        };
        if r.recording {
            return self.record(r, running);
        }
        let rs = seg_read_plan(s, p);
        if r.next < r.count {
            self.read(r, s, &rs, p, running)
        } else {
            self.show(r, s, &rs, p, want, id)
        }
    }

    fn record(&mut self, r: &mut SegRun, running: bool) -> Result<(), String> {
        let x = seg::status(self.bar, r.n);
        lock(self.out).seg.recorded = x.recorded;
        if !x.complete(r.n) && running {
            std::thread::sleep(std::time::Duration::from_millis(10));
            return Ok(());
        }
        let tag = seg::first_trigger(self.bar);
        lifecycle::seg_stop(self.state, self.bar, self.c2h, !x.complete(r.n))?;
        (r.recording, r.count, r.next, r.deliver, r.tl) = (false, x.recorded, 0, running, seg::Timeline::new(tag));
        eprintln!(
            "oos-ui hw: segments: {} of {} recorded, 0x4034 {:#x}, first-trigger stamp {:?}",
            x.recorded, r.n, x.fsm, tag
        );
        lock(self.out).seg.phase = if r.count == 0 { SegPhase::Done } else { SegPhase::Reading };
        Ok(())
    }

    fn read(
        &mut self,
        r: &mut SegRun,
        s: &Settings,
        rs: &Settings,
        p: &hz::Params,
        running: bool,
    ) -> Result<(), String> {
        r.deliver &= running;
        let cnt = rs.frames.min(r.count - r.next);
        if r.deliver && lock(self.out).queue.len() + cnt as usize > crate::board::QUEUE_MAX {
            std::thread::sleep(std::time::Duration::from_millis(5));
            return Ok(());
        }
        let st = seg::stamps(self.state, rs, p, self.bar, self.c2h, r.next, cnt)?;
        if r.next == 0 {
            for (k, x) in st.iter().take(8).enumerate() {
                eprintln!("oos-ui hw: segment {} latch {:?} header {:?}", k, x.latch, x.header);
            }
        }
        let times: Vec<Option<f64>> = st.iter().map(|x| r.tl.push(*x)).collect();
        let mut frames = Vec::new();
        if r.deliver {
            seg::read(self.state, rs, p, self.bar, self.c2h, self.dma, r.next, cnt)?;
            frames = self
                .dma
                .data()
                .chunks_exact(oos_hal::dho::seq::frame_bytes(rs, p))
                .take(cnt as usize)
                .map(|chunk| {
                    *self.seq += 1;
                    tag(decode_probed(chunk, rs, p, *self.seq, Probe::of_header(s.header)), r.identity)
                })
                .map(std::sync::Arc::new)
                .collect();
        }
        let mut g = lock(self.out);
        frames.retain(|f| publication::current(&g, f));
        // A new publication can arrive during DMA. Its timestamps and status are obsolete too.
        if publication::boundary(Some((r.identity.gen(), r.identity.epoch())), g.acquisition.identity())
            || g.seg.run != r.run
        {
            return Ok(());
        }
        r.next += cnt;
        g.seg.times.extend(times);
        g.seg.read = r.next;
        let sample = frames.last().map(crate::perfguard::deep::clone_frame).map(|(frame, sample)| {
            g.frame = frame;
            sample
        });
        g.queue.extend(frames);
        if r.next >= r.count {
            g.seg.phase = SegPhase::Done;
        }
        drop(g);
        if let Some(sample) = sample {
            sample.record();
        }
        Ok(())
    }

    fn show(
        &mut self,
        r: &SegRun,
        s: &Settings,
        rs: &Settings,
        p: &hz::Params,
        want: Option<u32>,
        id: Identity,
    ) -> Result<(), String> {
        match want {
            Some(k) if k < r.count => {
                lock(self.out).seg.want = None;
                let one = seg::read_settings(rs, 1);
                seg::read(self.state, &one, p, self.bar, self.c2h, self.dma, k, 1)?;
                *self.seq += 1;
                let fb = oos_hal::dho::seq::frame_bytes(rs, p);
                let f = tag(
                    decode_probed(
                        &self.dma.data()[..fb.min(self.dma.data().len())],
                        &one,
                        p,
                        *self.seq,
                        Probe::of_header(s.header),
                    ),
                    // Completed segments survive pair changes without a hardware reconfigure.
                    // This new browse read belongs to the snapshot at its request, not the recording arm.
                    id,
                );
                let mut g = lock(self.out);
                if publication::current(&g, &f) && g.seg.run == r.run {
                    g.seg.shown = Some((k, f));
                } else if g.seg.run == r.run && g.seg.want.is_none() {
                    // Retry an interrupted browse with the next current snapshot.
                    g.seg.want = Some(k);
                }
            }
            Some(_) => lock(self.out).seg.want = None,
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
        Ok(())
    }
}
