// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;
use std::time::Instant;

pub(super) struct Outputs {
    pub(super) disp: drm::Display,
    pub(super) hdmi: Option<drm::Display>,
    hdmi_cv: Option<Canvas>,
    hdmi_t: Option<Instant>,
    hdmi_on: bool,
    pub(super) frame_n: u64,
}
impl Outputs {
    pub(super) fn new(disp: drm::Display, hdmi_on: bool) -> Self {
        Self { disp, hdmi_on, hdmi: None, hdmi_cv: None, hdmi_t: None, frame_n: 0 }
    }
    pub(super) fn cpu_hdmi(&mut self, ui: &mut Ui, cv: &Canvas, hdmi_frame: bool, hdmi_done: bool) {
        if let Some(h) = self.hdmi.as_mut().filter(|_| hdmi_frame && !hdmi_done) {
            // the monitor's picture: its own layout, the workspace, or the panel's
            let size = match ui.m.hdmi {
                model::HdmiMode::Layout => ui::Screen::for_monitor(h.width, h.height),
                model::HdmiMode::Waveform if !ui.hdmi_snap.is_empty() => Some(ui::Screen { w: h.width, h: h.height }),
                _ => None,
            };
            match size {
                Some(s) => {
                    let hc = self.hdmi_cv.get_or_insert_with(|| Canvas::new(s.w, s.h));
                    if (hc.w, hc.h) != (s.w, s.h) {
                        *hc = Canvas::new(s.w, s.h);
                    }
                    if ui.m.hdmi == model::HdmiMode::Layout {
                        ui.render_view(hc);
                    } else {
                        hdmi_workspace(ui, hc);
                    }
                    h.present_scaled(hc);
                }
                None => h.present_scaled(cv),
            }
        }
    }
    pub(super) fn poll(&mut self, ui: &mut Ui, graphics: &mut super::graphics::Graphics) {
        if self.hdmi_on && self.hdmi_t.is_none_or(|t| t.elapsed().as_secs() >= 3) {
            self.hdmi_t = Some(Instant::now());
            match self.hdmi.take() {
                Some(h) if !h.connected() => {
                    eprintln!("oos-ui: HDMI disconnected");
                    h.close();
                    self.hdmi_cv = None;
                    // a frame of it still in work is collected, not shown
                    graphics.monitor_changed();
                }
                Some(h) => self.hdmi = Some(h),
                None => match self.disp.hdmi() {
                    Ok(h) => {
                        eprintln!("oos-ui: HDMI {}x{}", h.width, h.height);
                        self.hdmi = Some(h);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        // e.g. a mode the second VOP refuses: try again in 30 s, not every 3 s
                        eprintln!("oos-ui: HDMI: {}", e);
                        self.hdmi_t = Some(Instant::now() + std::time::Duration::from_secs(27));
                    }
                },
            }
        }
        ui.hdmi_out = self.hdmi.as_ref().map(|h| (h.width as u32, h.height as u32));
    }
    pub(super) fn release(&mut self) {
        if let Some(h) = self.hdmi.take() {
            h.close();
        }
        self.disp.restore();
    }
}
