// SPDX-License-Identifier: GPL-2.0-or-later
//! Device presentation and helper recovery, independent of acquisition.
use super::{flip, outputs::Outputs, timers::Timers};
use crate::*;
use std::time::Instant;

struct PanelRender {
    ticket: gpu::Ticket,
    back: usize,
    picture: pacing::Picture,
    shot: Option<String>,
    cpu_hdmi: bool,
}

struct TargetRender {
    ticket: gpu::Ticket,
    back: usize,
}

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
    panel: Option<PanelRender>,
    monitor: Option<TargetRender>,
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
            panel: None,
            monitor: None,
        }
    }

    pub(super) fn monitor_changed(&mut self) {
        self.hdmi_gpu = None;
        if let Some(panel) = &mut self.panel {
            panel.cpu_hdmi = false;
        }
    }

    pub(super) fn slot(&self, panel: &pacing::Panel) -> pacing::FrameSlot {
        if self.deadline().is_some() {
            pacing::FrameSlot::HelperBusy
        } else if panel.ready_for_picture() {
            pacing::FrameSlot::Ready
        } else {
            pacing::FrameSlot::PanelBusy
        }
    }

    pub(super) fn reply_fd(&self) -> Option<i32> {
        self.deadline().and(self.gpu.as_ref()).map(gpu::Gpu::reply_fd)
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.panel.as_ref().map(|p| p.ticket.deadline()).or_else(|| self.monitor.as_ref().map(|h| h.ticket.deadline()))
    }

    fn select(&mut self, ui: &mut Ui, out: &Outputs, sw: &sysconf::Switches, now: Instant) {
        if ui.memory_stopped() {
            return;
        }
        if ui.sys.gpu != self.want {
            self.changed = out.frame_n > 0;
            self.want = ui.sys.gpu;
            self.clear_helper(ui);
            self.recovery = Default::default();
            if self.want {
                self.start(ui, out, sw);
            }
        } else if self.want && self.gpu.is_none() && self.recovery.ready(now.saturating_duration_since(self.started)) {
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
                ui.set_gpu_pinned_cpu(g.pinned_cpu());
                self.gpu = Some(g);
            }
            Err(e) => self.failed(ui, e),
        }
        self.hdmi_gpu = None;
    }

    fn clear_helper(&mut self, ui: &mut Ui) {
        self.gpu = None;
        self.panel = None;
        self.monitor = None;
        self.hdmi_gpu = None;
        ui.set_gpu_pinned_cpu(None);
    }

    fn failed(&mut self, ui: &mut Ui, error: String) {
        self.clear_helper(ui);
        self.recovery.failed(self.started.elapsed());
        ui.gpu_recovery_status(self.recovery.status());
        (ui.t_gpu, ui.t_gpu_screens) = (None, None);
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
        picture: pacing::Picture,
    ) -> bool {
        self.select(ui, out, sw, picture.composed_at());
        if self.gpu.is_some() {
            match self.draw(ui, (cv.w, cv.h), out, web, picture) {
                Ok(()) => return true,
                Err(error) => self.failed(ui, error),
            }
        }
        Self::cpu(ui, cv, out, web, picture);
        false
    }

    fn cpu(ui: &mut Ui, cv: &mut Canvas, out: &mut Outputs, web: &mut http::Http, picture: pacing::Picture) {
        web.before_render(cv, !ui.start_up_active());
        ui.render(cv);
        (ui.t_gpu, ui.t_gpu_screens) = (None, None);
        let back = out.disp.copy(cv);
        out.queue_panel(back, picture, Instant::now());
    }

    pub(super) fn service(
        &mut self,
        ui: &mut Ui,
        cv: &mut Canvas,
        out: &mut Outputs,
        web: &mut http::Http,
        timers: &mut Timers,
    ) {
        let Some(g) = &mut self.gpu else { return };
        if let Some(mut panel) = self.panel.take() {
            match g.try_done(&mut panel.ticket, Instant::now()) {
                Ok(Some(done)) => {
                    panel.picture.helper_done(panel.ticket.submitted(), flip::instant(done));
                    out.queue_panel(panel.back, panel.picture, Instant::now());
                    if let Err(error) = g.finish(&panel.ticket, Some(cv)) {
                        self.failed(ui, error);
                        return;
                    }
                    if panel.ticket.reads() {
                        web.picture_read_back(cv);
                    }
                    self.recovery.succeeded(self.started.elapsed());
                }
                Ok(None) => {
                    self.panel = Some(panel);
                    return;
                }
                Err(error) => {
                    self.failed(ui, error);
                    Self::cpu(ui, cv, out, web, panel.picture);
                    panel.cpu_hdmi |= out.frame_n.is_multiple_of(2);
                }
            }
            out.cpu_hdmi(ui, cv, panel.cpu_hdmi);
            timers.shot(ui, cv, panel.shot);
        }
        self.service_monitor(ui, out);
        if let (Some(g), Some(times)) = (&self.gpu, &mut ui.t_gpu) {
            let t = g.times;
            times[3..].copy_from_slice(&[t.upload, t.draw, t.read]);
            ui.t_gpu_screens = Some((t.panel, (self.hdmi_gpu == Some(true)).then_some(t.monitor)));
        }
    }

    fn service_monitor(&mut self, ui: &mut Ui, out: &mut Outputs) {
        let (Some(mut monitor), Some(g)) = (self.monitor.take(), self.gpu.as_mut()) else { return };
        match g
            .try_done(&mut monitor.ticket, Instant::now())
            .and_then(|done| done.map(|_| g.finish(&monitor.ticket, None)).transpose())
        {
            Ok(Some(())) => {
                if let Some(h) = out.hdmi.as_mut().filter(|_| self.hdmi_gpu == Some(true)) {
                    h.flip_scaled(monitor.back);
                }
            }
            Ok(None) => self.monitor = Some(monitor),
            Err(error) => self.failed(ui, error),
        }
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

    fn draw(
        &mut self,
        ui: &mut Ui,
        (w, h): (i32, i32),
        out: &Outputs,
        web: &mut http::Http,
        picture: pacing::Picture,
    ) -> Result<(), String> {
        self.monitor_targets(out)?;
        web.before_gpu_frame();
        let hdmi = out.hdmi.as_ref().filter(|_| (out.frame_n + 1).is_multiple_of(2));
        let cpu_hdmi = hdmi.is_some() && self.hdmi_gpu != Some(true);
        let read = ui.shot_request.is_some() || web.picture_wanted() || out.frame_n.is_multiple_of(15) || cpu_hdmi;
        let t = Instant::now();
        self.gl.draw(ui, w, h, false);
        let layers = t.elapsed().as_secs_f32() * 1e3;
        let g = self.gpu.as_mut().ok_or("helper unavailable")?;
        let back = out.disp.free();
        let full = gfx::Rect::new(0, 0, w, h);
        let ticket = g.submit(back, None, &[self.gl.quad(ui, full, full, false, false)], read)?;
        let t = g.times;
        if let Some(h) = hdmi.filter(|_| self.hdmi_gpu == Some(true)) {
            let back = h.free();
            let ticket = hdmi_gpu_frame(
                ui,
                g,
                &self.gl,
                &mut self.view,
                &mut self.head,
                h.width,
                h.height,
                out.disp.buffers() + back,
                false,
            )?;
            self.monitor = Some(TargetRender { ticket, back });
        }
        self.panel = Some(PanelRender { ticket, back, picture, shot: ui.shot_request.take(), cpu_hdmi });
        ui.t_gpu = Some([layers, t.copy, 0.0, t.upload, t.draw, t.read]);
        Ok(())
    }

    pub(super) fn shutdown(&mut self, deadline: Instant) -> bool {
        let done = self.gpu.as_mut().is_none_or(|g| g.shutdown_until(deadline));
        self.panel = None;
        self.monitor = None;
        done
    }
}

#[cfg(test)]
mod tests;
