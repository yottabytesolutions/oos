// SPDX-License-Identifier: GPL-2.0-or-later
//! Device presentation and helper recovery, independent of acquisition.
use super::outputs::Outputs;
use crate::*;
use std::time::Instant;

type Pending = (gpu::Ticket, usize, Option<(gpu::Ticket, usize)>);

pub(super) struct Graphics {
    gpu: Option<gpu::Gpu>,
    want: bool,
    changed: bool,
    recovery: gpu::recovery::Recovery,
    started: Instant,
    gl: GpuLayers,
    view: GpuLayers,
    head: Option<Canvas>,
    hdmi_gpu: Option<bool>,
    pending: Option<Pending>,
}

impl Graphics {
    pub(super) fn new(ui: &Ui) -> Self {
        Self {
            gpu: None,
            want: false,
            changed: false,
            recovery: Default::default(),
            started: Instant::now(),
            gl: GpuLayers::new(ui.scr.w, ui.scr.h),
            view: GpuLayers::new(1, 1),
            head: None,
            hdmi_gpu: None,
            pending: None,
        }
    }

    pub(super) fn monitor_changed(&mut self) {
        self.hdmi_gpu = None;
    }

    fn select(&mut self, ui: &mut Ui, out: &Outputs, sw: &sysconf::Switches) {
        if ui.memory_stopped() {
            return;
        }
        if ui.sys.gpu != self.want {
            self.changed = out.frame_n > 0;
            self.want = ui.sys.gpu;
            self.gpu = None;
            self.recovery = Default::default();
            self.pending = None;
            self.hdmi_gpu = None;
            if self.want {
                self.start(ui, out, sw);
            }
        } else if self.want && self.gpu.is_none() && self.recovery.ready(self.started.elapsed()) {
            self.start(ui, out, sw);
        }
        ui.gpu_recovery_status(self.recovery.status());
    }

    fn start(&mut self, ui: &mut Ui, out: &Outputs, sw: &sysconf::Switches) {
        match start_gpu(&out.disp, sw) {
            Ok(g) => {
                eprintln!("oos-ui: GPU on");
                if self.changed {
                    ui.toast("Graphics: GPU");
                }
                self.gpu = Some(g);
            }
            Err(e) => self.failed(ui, e),
        }
        self.hdmi_gpu = None;
    }

    fn failed(&mut self, ui: &mut Ui, error: String) {
        self.gpu = None;
        self.pending = None;
        self.hdmi_gpu = None;
        self.recovery.failed(self.started.elapsed());
        ui.gpu_recovery_status(self.recovery.status());
        eprintln!("oos-ui: GPU failed: {}", error);
        ui.toast("GPU failed: drawing on the CPU");
    }

    pub(super) fn frame(
        &mut self,
        ui: &mut Ui,
        cv: &mut Canvas,
        out: &mut Outputs,
        web: &mut http::Http,
        sw: &sysconf::Switches,
    ) -> bool {
        self.select(ui, out, sw);
        if self.gpu.is_none() {
            web.before_render(cv, !ui.start_up_active());
            ui.render(cv);
            (ui.t_gpu, ui.t_gpu_screens) = (None, None);
            return false;
        }
        web.before_gpu_frame();
        gpu::recovery::render_or_cpu(
            &mut (self, ui, cv, out, web),
            |(g, ui, cv, out, web)| g.draw(ui, cv, out, web),
            |(_, ui, cv, _, _)| {
                (ui.t_gpu, ui.t_gpu_screens) = (None, None);
                ui.render(cv);
            },
            |(g, ui, _, _, _), error| g.failed(ui, error),
        )
    }

    fn collect(&mut self, cv: &mut Canvas, out: &mut Outputs, web: &mut http::Http) -> Result<f32, String> {
        let Some((tp, bp, th)) = self.pending.take() else { return Ok(0.0) };
        let g = self.gpu.as_mut().ok_or("helper unavailable")?;
        let back = tp.reads();
        g.collect(tp, Some(cv))?;
        if back {
            web.picture_read_back(cv);
        }
        let wait = g.times.wait;
        out.disp.flip(bp);
        if let Some((t2, b2)) = th {
            g.collect(t2, None)?;
            if let Some(h) = out.hdmi.as_mut() {
                h.flip_scaled(b2);
            }
        }
        self.recovery.succeeded(self.started.elapsed());
        Ok(wait)
    }

    fn monitor_targets(&mut self, out: &Outputs) -> Result<(), String> {
        if let (Some(h), None) = (out.hdmi.as_ref(), self.hdmi_gpu) {
            let g = self.gpu.as_mut().ok_or("helper unavailable")?;
            let mut ok = true;
            for i in 0..h.buffers() {
                let (fd, pitch) = h.export(i).map_err(|e| format!("HDMI export: {}", e))?;
                ok &= g
                    .target_dmabuf(out.disp.buffers() + i, h.width, h.height, pitch, fd)
                    .map_err(|e| eprintln!("oos-ui: GPU HDMI: {}", e))
                    .is_ok();
            }
            self.hdmi_gpu = Some(ok);
        }
        Ok(())
    }

    fn draw(&mut self, ui: &mut Ui, cv: &mut Canvas, out: &mut Outputs, web: &mut http::Http) -> Result<(), String> {
        let shot = ui.shot_request.is_some();
        let read = shot
            || web.picture_wanted()
            || out.frame_n.is_multiple_of(15)
            || (out.hdmi.is_some() && self.hdmi_gpu == Some(false));
        let t = Instant::now();
        self.gl.draw(ui, cv.w, cv.h, false);
        let layers = t.elapsed().as_secs_f32() * 1e3;
        let wait = self.collect(cv, out, web)?;
        self.monitor_targets(out)?;
        let g = self.gpu.as_mut().ok_or("helper unavailable")?;
        let b = out.disp.free();
        let full = gfx::Rect::new(0, 0, cv.w, cv.h);
        let tp = g.submit(b, None, &[self.gl.quad(ui, full, full, false, false)], read)?;
        let copy = g.times.copy;
        let th = match out.hdmi.as_ref().filter(|_| self.hdmi_gpu == Some(true) && (out.frame_n + 1).is_multiple_of(2))
        {
            Some(h) => {
                let b2 = h.free();
                Some((
                    hdmi_gpu_frame(
                        ui,
                        g,
                        &self.gl,
                        &mut self.view,
                        &mut self.head,
                        h.width,
                        h.height,
                        out.disp.buffers() + b2,
                        false,
                    )?,
                    b2,
                ))
            }
            None => None,
        };
        self.pending = Some((tp, b, th));
        if shot {
            self.collect(cv, out, web)?;
        }
        let g = self.gpu.as_ref().ok_or("helper unavailable")?;
        let t = g.times;
        ui.t_gpu = Some([layers, copy, wait, t.upload, t.draw, t.read]);
        ui.t_gpu_screens = Some((t.panel, (out.hdmi.is_some() && self.hdmi_gpu == Some(true)).then_some(t.monitor)));
        Ok(())
    }

    pub(super) fn monitor_drawn(&self) -> bool {
        self.gpu.is_some() && self.hdmi_gpu == Some(true)
    }

    pub(super) fn shutdown(&mut self, deadline: Instant) -> bool {
        let done = self.gpu.as_mut().is_none_or(|g| g.shutdown_until(deadline));
        self.pending = None;
        done
    }
}
