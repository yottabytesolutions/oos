// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct CaptureStatus {
    start: Option<Sample>,
    label: Option<(String, bool)>,
}

struct Sample {
    at: Instant,
    shown: u64,
    lost: u64,
}

impl CaptureStatus {
    fn observe(&mut self, sample: Sample, duty: impl FnOnce(Instant) -> Option<f64>) {
        let Some(start) = &self.start else {
            self.start = Some(sample);
            return;
        };
        let elapsed = sample.at.saturating_duration_since(start.at);
        if elapsed < Duration::from_secs(1) {
            return;
        }
        let rate = sample.shown.saturating_sub(start.shown) as f64 / elapsed.as_secs_f64();
        let loss = sample.lost > start.lost;
        self.label = Some((caption(rate, duty(start.at), loss), loss));
        self.start = Some(sample);
    }
}

fn caption(rate: f64, duty: Option<f64>, loss: bool) -> String {
    let rate = if rate < 999.5 { format!("{rate:.0}") } else { si_digits(rate, "", 3).replace(' ', "") };
    let duty = duty.map_or_else(|| "--".into(), |value| format!("{:.0}", value.clamp(0.0, 1.0) * 100.0));
    let mark = if loss { "!" } else { "" };
    format!("{rate} wfm/s {duty}%{mark}")
}

fn running(ui: &Ui) -> bool {
    ui.m.run == RunState::Run && ui.m.status != TrigStatus::Stop
}

fn visible_label(ui: &Ui) -> Option<&(String, bool)> {
    ui.batch_picture.capture.label.as_ref().filter(|_| running(ui))
}

#[cfg(target_os = "linux")]
pub(crate) fn sample(ui: &mut Ui, at: Instant) {
    if !running(ui) || !ui.src.is_hw() || ui.record_mode() {
        ui.batch_picture.capture = CaptureStatus::default();
        return;
    }
    let id = change::acquisition::identity(ui);
    let counters = drawpipe::counters();
    let (shown, lost) = counters.capture_counts();
    ui.batch_picture.capture.observe(Sample { at, shown, lost }, |since| counters.metrics.capture_duty(id, since));
}

pub(in crate::ui) fn run_pill(ui: &Ui) -> Rect {
    let fonts = ui.fonts();
    let width = 92.max(30 + fonts.mb.width("TRIG'D") + 12);
    let Some((label, _)) = visible_label(ui) else { return Rect::new(8, 6, width, 28) };
    let capture_width = fonts.s.width(label) + 12;
    Rect::new(8, 3, width.max(capture_width), 34)
}

pub(in crate::ui) fn draw_run_status(ui: &Ui, cv: &mut Canvas) {
    let (txt, col) = ui.run_caption();
    let rect = ui.run_pill();
    let fonts = ui.fonts();
    cv.round(rect, 6, mix(BAR, col, 80), 255);
    cv.round(rect.inset(1), 5, mix(BAR, col, 36), 255);
    let capture = visible_label(ui);
    let top_height = if capture.is_some() { 18 } else { rect.h };
    cv.circle(rect.x + 16, rect.y + top_height / 2, 4, col);
    cv.text_in(&fonts.mb, Rect::new(rect.x + 30, rect.y, rect.w - 30, top_height), 0, txt, col);
    if let Some((label, loss)) = capture {
        let bottom = Rect::new(rect.x, rect.y + top_height, rect.w, rect.h - top_height);
        cv.text_in(&fonts.s, bottom, 1, label, if *loss { AMBER } else { TEXT });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_rate_uses_counter_deltas_and_marks_new_losses() {
        let at = Instant::now();
        let mut status = CaptureStatus::default();
        status.observe(Sample { at, shown: 100, lost: 4 }, |_| unreachable!());
        status.observe(Sample { at: at + Duration::from_millis(999), shown: 1939, lost: 4 }, |_| unreachable!());
        assert!(status.label.is_none());
        let next = at + Duration::from_secs(1);
        status.observe(Sample { at: next, shown: 1940, lost: 5 }, |since| {
            assert_eq!(since, at);
            Some(0.62)
        });
        assert_eq!(status.label, Some(("1.84k wfm/s 62%!".into(), true)));
        status.observe(Sample { at: next + Duration::from_secs(1), shown: 1940, lost: 5 }, |_| None);
        assert_eq!(status.label, Some(("0 wfm/s --%".into(), false)));
    }

    #[test]
    fn settings_changes_leave_the_capture_label_unmarked() {
        use std::sync::atomic::Ordering::Relaxed;
        let counters = drawpipe::Counters::default();
        let at = Instant::now();
        let mut status = CaptureStatus::default();
        let mut observe = |seconds| {
            let (shown, lost) = counters.capture_counts();
            status.observe(Sample { at: at + Duration::from_secs(seconds), shown, lost }, |_| None);
            status.label.clone()
        };
        assert_eq!(observe(0), None);
        counters.lose(drawpipe::Lost::Settings, 2);
        counters.lose(drawpipe::Lost::Superseded, 3);
        counters.losses.worker_push.store(4, Relaxed);
        counters.losses.queue_pop.store(5, Relaxed);
        counters.losses.ui_publish.store(6, Relaxed);
        let old = oos_core::change::Identity::default();
        counters.reject_deposit(Some(old), None, 7);
        counters.reject_deposit(Some(old), Some(old), 8);
        assert_eq!(observe(1), Some(("0 wfm/s --%".into(), false)));
        counters.lose(drawpipe::Lost::Error, 1);
        assert_eq!(observe(2), Some(("0 wfm/s --%!".into(), true)));
        counters.losses.overflow.store(1, Relaxed);
        assert_eq!(observe(3), Some(("0 wfm/s --%!".into(), true)));
    }

    #[test]
    fn stopped_pill_hides_the_previous_capture_label() {
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
        (ui.m.run, ui.m.status) = (RunState::Run, TrigStatus::Triggered);
        ui.batch_picture.capture.label = Some((caption(1840.0, Some(1.0), false), false));
        assert!(visible_label(&ui).is_some());
        assert_eq!(ui.run_pill().h, 34);
        ui.m.run = RunState::Stop;
        assert!(visible_label(&ui).is_none());
        assert_eq!(ui.run_pill().h, 28);
    }

    #[test]
    fn normal_panel_status_fits_beside_single_with_loss_mark_and_full_duty() {
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
        let label = caption(1840.0, Some(0.62), false);
        assert_eq!(label, "1.84k wfm/s 62%");
        ui.batch_picture.capture.label = Some((caption(1840.0, Some(1.0), true), true));
        for size in [TextSize::Normal, TextSize::Large, TextSize::Larger] {
            ui.m.text_size = size;
            let rect = ui.run_pill();
            let label = &ui.batch_picture.capture.label.as_ref().unwrap().0;
            assert!(ui.fonts().s.width(label) <= rect.w - 12);
            assert!(rect.bottom() <= ui.frame().top.bottom() - 2);
            assert!(rect.right() < ui.single_btn().x);
        }
    }
}
