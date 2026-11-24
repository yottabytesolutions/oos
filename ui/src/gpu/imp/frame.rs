// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use upload::UploadData::Pixels;

impl Gpu {
    /// Sends a frame and returns at once. Reuse its upload half after render
    /// completion. One read can occupy the separately reserved arena tail.
    pub fn submit(&mut self, index: usize, clear: Option<u32>, quads: &[Quad], read: bool) -> Result<Ticket, String> {
        let started = std::time::Instant::now();
        let (width, height) = self.targets.get(index).copied().flatten().ok_or("no such target")?;
        if quads.len() > MAX_QUADS {
            return Err("too many quads".into());
        }

        let half = usize::from(index >= self.monitor_from);
        let mut frame = FrameMsg::new(index, clear, half);
        let mut arena = self.upload_range(half, read.then_some((width, height)))?;
        if read {
            frame.readback = 1;
            frame.readback_off = arena.tail as u32;
        }

        self.pack_atlas(&mut frame, quads, &mut arena)?;
        let mut recordings = [[(NONE, NONE); 3]; MAX_QUADS];
        for (quad, slots) in quads.iter().zip(&mut recordings).filter(|(quad, _)| !quad.reuse) {
            *slots = arena.recordings(quad, &self.atlas)?;
        }

        self.spare_blocks.clear();
        let mut upload = BlockUpload {
            base: arena.base,
            at: arena.start,
            tail: arena.tail,
            known: true,
            previous: &self.blocks[half],
            blocks: &mut self.spare_blocks,
        };
        for (quad, recording) in quads.iter().zip(recordings) {
            frame.push_quad(quad, recording, &mut upload)?;
        }
        std::mem::swap(&mut self.blocks[half], &mut self.spare_blocks);

        let sent = std::time::Instant::now();
        self.sock.write_all(bytes(&frame)).map_err(|error| format!("frame: {error}"))?;
        let offset = frame.readback_off as usize;
        if read {
            self.pending_read = Some((half, offset));
        }
        self.times.copy = (sent - started).as_secs_f32() * 1e3;
        Ok(Ticket::new(read.then_some((offset, width, height)), sent, half == 1))
    }

    fn upload_range(&self, half: usize, read: Option<(i32, i32)>) -> Result<ArenaRange, String> {
        let size = ARENA / 2;
        let start = half * size;
        let mut arena = ArenaRange { base: self.arena, start, tail: start + size };
        if let Some((width, height)) = read {
            if self.pending_read.is_some() {
                return Err("readback already pending".into());
            }
            arena.reserve(width as usize * height as usize * 4)?;
        } else if let Some((_, offset)) = self.pending_read.filter(|(owner, _)| *owner == half) {
            arena.tail = offset;
        }
        Ok(arena)
    }
}

impl ArenaRange {
    fn recordings(&mut self, quad: &Quad, atlas: &[(usize, u32)]) -> Result<[(u32, u32); 3], String> {
        let layers = [quad.under, quad.over].map(|canvas| canvas.and_then(|c| c.rec.as_deref()));
        let mut slots = [(NONE, NONE); 3];
        slots[2] = self.images(layers)?;
        let mut image_base = 0;
        for (layer, slot) in layers.into_iter().zip(&mut slots) {
            if let Some(recording) = layer {
                *slot = self.commands(recording, atlas, image_base)?;
                image_base += recording.img.len() as u32;
            }
        }
        Ok(slots)
    }
}

impl FrameMsg {
    fn new(target: usize, clear: Option<u32>, half: usize) -> Self {
        Self {
            magic: MAGIC,
            kind: K_FRAME,
            target: target as u32,
            clear: clear.map_or(0, |colour| 0x0100_0000 | (colour & 0xff_ffff)),
            n_quads: 0,
            n_jobs: 0,
            readback: 0,
            readback_off: 0,
            atlas_off: 0,
            atlas_rows: 0,
            atlas_row: 0,
            set: half as u32,
            q: [QuadMsg::default(); MAX_QUADS],
            j: [JobMsg::default(); MAX_JOBS],
        }
    }

    fn push_quad(
        &mut self,
        quad: &Quad,
        [under, over, images]: [(u32, u32); 3],
        upload: &mut BlockUpload<'_>,
    ) -> Result<(), String> {
        let (under_off, under_y0, under_y1) = upload.layer(quad.under, under, quad.reuse)?;
        let (over_off, over_y0, over_y1) = upload.layer(quad.over, over, quad.reuse)?;
        let first_job = self.n_jobs;
        for job in quad.jobs {
            let slot = self.j.get_mut(self.n_jobs as usize).ok_or("too many plot windows")?;
            *slot = upload.job(quad, job)?;
            self.n_jobs += 1;
        }
        let (lw, lh) = quad.size;
        self.q[self.n_quads as usize] = QuadMsg {
            under_off,
            over_off,
            lw,
            lh,
            sx: quad.src.x,
            sy: quad.src.y,
            sw: quad.src.w,
            sh: quad.src.h,
            dx: quad.dst.x,
            dy: quad.dst.y,
            dw: quad.dst.w,
            dh: quad.dst.h,
            filter: u32::from(quad.filter),
            first_job,
            n_jobs: self.n_jobs - first_job,
            reuse: u32::from(quad.reuse),
            under_y0,
            under_y1,
            over_y0,
            over_y1,
            under_n: under.1,
            over_n: over.1,
            img_off: images.0,
            img_rows: if images.1 == NONE { 0 } else { images.1 },
        };
        self.n_quads += 1;
        Ok(())
    }
}

impl BlockUpload<'_> {
    fn layer(
        &mut self,
        canvas: Option<&Canvas>,
        (offset, count): (u32, u32),
        reuse: bool,
    ) -> Result<(u32, i32, i32), String> {
        match canvas {
            None => Ok((NONE, 0, 0)),
            Some(_) if count != NONE => Ok((offset, 0, 0)),
            Some(canvas) if !reuse && canvas.rec.is_none() => {
                self.put(Pixels { bytes: bytes(canvas.px.as_slice()), row_bytes: canvas.w as usize * 4 })
            }
            Some(_) => Ok((0, 0, 0)),
        }
    }

    fn job(&mut self, quad: &Quad, job: &TraceJob) -> Result<JobMsg, String> {
        let ph = if job.zoom { quad.phz } else { quad.ph };
        let mut msg = JobMsg {
            x: job.area.x,
            y: job.area.y,
            w: job.area.w,
            h: job.area.h,
            mw: ph.w as i32,
            mh: ph.h as i32,
            map_off: [NONE; NCH],
            graded: u32::from(job.graded),
            lut_off: 0,
            shift: job.shift,
            col: job.col,
            map_y0: [0; NCH],
            map_y1: [0; NCH],
        };
        for c in 0..NCH {
            if job.show[c] {
                let (off, y0, y1) =
                    if quad.reuse { (0, 0, 0) } else { self.put(Source::counts(ph, c, job.count_map(ph, c)))? };
                (msg.map_off[c], msg.map_y0[c], msg.map_y1[c]) = (off, y0, y1);
            }
        }
        if !quad.reuse {
            let (bytes, row_bytes) = if job.graded {
                (bytes(job.colours.as_slice()), job.colours.len() * 4)
            } else {
                (job.alpha.as_slice(), job.alpha.len())
            };
            msg.lut_off = self.put(Pixels { bytes, row_bytes })?.0;
        }
        Ok(msg)
    }
}
