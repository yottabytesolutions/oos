// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Self-calibration page (System > Self-cal): channels, AFE zero on/off, start (pressed twice: disconnect all inputs
//! first), cancel, progress and result. The work runs in the acquisition thread of the board (`board/dho/hw.rs`); the
//! results go to OOS's own files in /oos/cal, never to /rigol/data.

use super::*;

/// Minutes the scope should run before a calibration.
const WARM_UP_MIN: f64 = 30.0;

/// UI state of the page.
#[derive(Default)]
pub struct State {
    /// 0 = all channels, 1..4 = CHn
    pub ch: usize,
    pub zero: bool,
    /// ADC core gain/offset calibration first (all inputs of the set disconnected)
    pub adc: bool,
    /// Start was pressed once at this UI time (the second press within 10 s starts)
    pub armed: Option<f64>,
    pub last: Option<String>,
}

impl Ui {
    fn cal_shared(&self) -> Option<crate::board::HwShared> {
        match &self.src {
            Source::Hw { shared, .. } => Some(shared.clone()),
            _ => None,
        }
    }

    /// Text about the warm-up for the dialog before the start.
    fn cal_warmup(minutes: f64) -> String {
        if minutes < WARM_UP_MIN {
            format!("NOT WARM YET: on for {:.0} of {:.0} minutes. The result will drift.", minutes, WARM_UP_MIN)
        } else {
            format!("Warmed up: on for {:.0} minutes.", minutes)
        }
    }

    /// A self-calibration is requested or running.
    pub(super) fn cal_running(&self) -> bool {
        self.cal_shared().is_some_and(|sh| {
            let g = crate::perfguard::deep::ui_lock(&sh);
            g.cal_progress.is_some() || g.cal_req.is_some()
        })
    }

    /// Progress or the last result.
    pub(super) fn cal_status(&self) -> String {
        let Some(sh) = self.cal_shared() else { return "needs the hardware source".into() };
        let g = crate::perfguard::deep::ui_lock(&sh);
        if let Some((d, t, s)) = &g.cal_progress {
            return format!("{:.0} % {}", 100.0 * *d as f64 / (*t).max(1) as f64, s);
        }
        if g.cal_req.is_some() {
            return "starting ...".into();
        }
        self.cal.last.clone().unwrap_or_else(|| "idle".into())
    }

    pub(super) fn cal_adjust(&mut self, it: Item, d: i32) -> bool {
        self.cal_adjust_for(it, d, n_ch())
    }

    fn cal_adjust_for(&mut self, it: Item, d: i32, count: usize) -> bool {
        match it {
            Item::CalCh => self.cal.ch = (self.cal.ch as i32 + d).rem_euclid(count as i32 + 1) as usize,
            Item::CalZero => self.cal.zero = !self.cal.zero,
            Item::CalAdc => self.cal.adc = !self.cal.adc,
            Item::CalStatus => {}
            _ => return false,
        }
        true
    }

    pub(super) fn cal_activate(&mut self, it: Item) -> bool {
        self.cal_activate_for(it, n_ch())
    }

    fn cal_activate_for(&mut self, it: Item, count: usize) -> bool {
        match it {
            Item::CalStart => {
                let Some(sh) = self.cal_shared() else {
                    self.toast("Self-cal needs the hardware source");
                    return true;
                };
                if self.cal.armed.is_some_and(|t| self.now - t < 10.0) {
                    self.cal.armed = None;
                    let chans = channels(self.cal.ch, count);
                    let mut g = crate::perfguard::deep::ui_lock(&sh);
                    (g.cal_req, g.cal_result, g.cal_cancel) = (Some((chans, self.cal.zero, self.cal.adc)), None, false);
                    g.record_flow.wake();
                    drop(g);
                    self.toast("Self-cal started");
                } else {
                    self.cal.armed = Some(self.now);
                    self.dialog = Some(vec![
                        "Self-calibration".into(),
                        "Disconnect ALL inputs (no probes, no cables).".into(),
                        Self::cal_warmup(
                            self.host
                                .text("/proc/uptime")
                                .and_then(|t| t.split(' ').next()?.parse::<f64>().ok())
                                .unwrap_or(self.now)
                                / 60.0,
                        ),
                        "Takes about 5 minutes per channel and pass (4 passes).".into(),
                        "The result goes to /oos/cal; /rigol/data stays as it is.".into(),
                        "Press Start again within 10 s to begin.".into(),
                    ]);
                }
            }
            Item::CalCancel => {
                if let Some(sh) = self.cal_shared() {
                    crate::perfguard::deep::ui_lock(&sh).cal_cancel = true;
                    self.toast("Cancelling self-cal ...");
                }
            }
            _ => return false,
        }
        true
    }

    /// SCPI: the status line of the self-calibration.
    pub fn cal_status_text(&self) -> String {
        self.cal_status()
    }

    /// Remote test hook (`/tmp/oos-ui.ctl`: `selfcal 0..4 [zero]`, `selfcal cancel`): starts without the dialog.
    pub fn selfcal_remote(&mut self, arg: &str, zero: bool) {
        self.selfcal_remote_full(arg, zero, false)
    }

    /// `selfcal_remote` with the ADC core calibration.
    pub fn selfcal_remote_full(&mut self, arg: &str, zero: bool, adc: bool) {
        self.cal.adc = adc;
        if arg == "cancel" {
            self.cal_activate(Item::CalCancel);
            return;
        }
        self.cal.ch = arg.parse::<usize>().unwrap_or(0).min(n_ch());
        self.cal.zero = zero;
        self.cal.armed = Some(self.now);
        self.cal_activate(Item::CalStart);
    }

    /// Per frame: pick up a finished run.
    pub(super) fn cal_poll(&mut self) {
        let Some(sh) = self.cal_shared() else { return };
        let r = crate::perfguard::deep::ui_lock(&sh).cal_result.take();
        if let Some(r) = r {
            let s = match r {
                Ok(s) => format!("done: {}", s),
                Err(e) => format!("failed: {}", e),
            };
            self.toast(&format!("Self-cal {}", s.split(':').next().unwrap_or("")));
            self.cal.last = Some(s);
        }
    }

    pub(super) fn calibration_capture_note(&self) -> Option<&'static str> {
        let (_, sequence) = self.stopped_capture()?;
        (self.src.hw_stop_loss(sequence) == Some(crate::board::identity::ReplayLoss::Calibration))
            .then_some("Captured before calibration. RUN for new data.")
    }

    #[cfg(test)]
    pub(crate) fn test_calibration_note(&self) -> Option<&'static str> {
        self.calibration_capture_note()
    }

    pub(super) fn draw_capture_calibration_note(&self, cv: &mut Canvas) {
        if let Some(note) = self.calibration_capture_note() {
            backed_text(cv, &self.fonts().s, self.grid.x + 8, self.grid.y + 15, note, AMBER);
        }
    }

    /// Progress banner over the plot while a run is active.
    pub(super) fn draw_cal_banner(&self, cv: &mut Canvas) {
        let Some(sh) = self.cal_shared() else { return };
        let p = crate::perfguard::deep::ui_lock(&sh).cal_progress.clone();
        let Some((d, t, s)) = p else { return };
        let a = self.area;
        let f = self.fonts();
        let txt = format!(
            "Self-cal {:.0} %  {}  (inputs disconnected; Cancel in System > Self-cal)",
            100.0 * d as f64 / t.max(1) as f64,
            s
        );
        let r = Rect::new(a.x + 20, a.y + a.h / 2 - 20, a.w - 40, 40);
        cv.round(r, 6, CAL_BANNER, 240);
        let bar = Rect::new(r.x, r.y + r.h - 4, (r.w as f64 * d as f64 / t.max(1) as f64) as i32, 4);
        cv.fill(bar, ACCENT);
        cv.text_in(&f.sb, r, 1, &txt, TEXT);
    }
}

fn channels(selected: usize, count: usize) -> Vec<usize> {
    if selected == 0 {
        (0..count).collect()
    } else {
        vec![selected - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit0073_selfcal_all_uses_model_channels() {
        for (name, count) in [("DHO802", 2), ("DHO804", 4)] {
            let caps = oos_hal::model::lookup(name).unwrap();
            assert_eq!(caps.channels, count);
            let shared: crate::board::HwShared = Default::default();
            let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
            assert!(ui.cal_adjust_for(Item::CalCh, -1, caps.channels));
            assert_eq!(ui.cal.ch, count);
            assert!(ui.cal_adjust_for(Item::CalCh, 1, caps.channels));
            assert_eq!(ui.cal.ch, 0);
            for selected in 0..=count {
                ui.cal.armed = Some(ui.now);
                assert!(ui.cal_activate_for(Item::CalStart, caps.channels));
                let want = if selected == 0 { (0..count).collect() } else { vec![selected - 1] };
                assert_eq!(shared.lock().unwrap().cal_req.as_ref().unwrap().0, want);
                ui.cal_adjust_for(Item::CalCh, 1, caps.channels);
            }
        }
    }
}
