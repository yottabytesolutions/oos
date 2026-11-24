// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct CaptureStatus {
    start: Option<Sample>,
    label: Option<(String, bool)>,
    share: Option<f64>,
}

#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    shown: u64,
    lost: u64,
    pair: (u64, u64),
}

impl CaptureStatus {
    fn observe(&mut self, sample: Sample, geometry: impl FnOnce() -> Option<(f64, usize)>) {
        let Some(start) = &self.start else {
            self.start = Some(sample);
            return;
        };
        let elapsed = sample.at.saturating_duration_since(start.at);
        if elapsed < Duration::from_secs(1) {
            return;
        }
        let shown = sample.shown.saturating_sub(start.shown);
        let rate = shown as f64 / elapsed.as_secs_f64();
        // A window spanning an acquisition change can contain records with different lengths.
        self.share = if shown > 0 && start.pair == sample.pair {
            geometry().and_then(|(sample_rate, points)| {
                (sample_rate.is_finite() && sample_rate > 0.0 && points > 0)
                    .then(|| (rate * (points as f64 / sample_rate)).clamp(0.0, 1.0))
            })
        } else {
            None
        };
        let loss = sample.lost > start.lost;
        self.label = Some((caption(rate, self.share, loss), loss));
        self.start = Some(sample);
    }
}

fn caption(rate: f64, share: Option<f64>, loss: bool) -> String {
    let rate = if rate < 999.5 { format!("{rate:.0}") } else { si_digits(rate, "", 3).replace(' ', "") };
    let percent = share.map_or_else(
        || "--".into(),
        |share| {
            let value = share * 100.0;
            if value < 10.0 {
                format!("{value:.1}")
            } else {
                format!("{value:.0}")
            }
        },
    );
    let mark = if loss { "!" } else { "" };
    format!("{rate} wfm/s {percent}%{mark}")
}

fn running(ui: &Ui) -> bool {
    ui.m.run == RunState::Run && ui.m.status != TrigStatus::Stop
}

fn visible_label(ui: &Ui) -> Option<&(String, bool)> {
    ui.batch_picture.capture.label.as_ref().filter(|_| running(ui))
}

fn capture_active(ui: &Ui) -> bool {
    running(ui) && ui.src.is_hw() && !ui.record_mode()
}

pub(crate) fn share(ui: &Ui) -> Option<f64> {
    ui.batch_picture.capture.share.filter(|_| capture_active(ui))
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn sample(ui: &mut Ui, at: Instant) {
    if !capture_active(ui) {
        ui.batch_picture.capture = CaptureStatus::default();
        return;
    }
    let id = change::acquisition::identity(ui);
    let pair = (id.gen(), id.epoch());
    let counters = drawpipe::counters();
    let (shown, lost) = counters.capture_counts();
    ui.batch_picture.capture.observe(Sample { at, shown, lost, pair }, || ui.src.hw_rate_depth_matching(Some(pair)));
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
    fn fast_signal_uses_stored_depth_and_shares_the_result_with_scpi() {
        let _scope = drawpipe::test_scope::Scope::new();
        let shared = crate::board::HwShared::default();
        let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host::Host::headless());
        (ui.m.run, ui.m.status) = (RunState::Run, TrigStatus::Triggered);
        let id = change::acquisition::identity(&ui);
        let frame = crate::board::Frame {
            seq: 1,
            cfg: id.gen(),
            stamp: id.epoch(),
            acq: (625e6, 3500),
            dt: 0.01,
            ch: [vec![0.0; 16], Vec::new(), Vec::new(), Vec::new()],
            ..Default::default()
        };
        oos_hal::sync::lock(&shared).frame = std::sync::Arc::new(frame);
        let at = Instant::now();
        sample(&mut ui, at);
        drawpipe::counters().picture_drawn(10_700, 10_700, 0);
        sample(&mut ui, at + Duration::from_secs(1));
        assert_eq!(visible_label(&ui), Some(&("10.7k wfm/s 6.0%".into(), false)));

        let mut scpi = crate::scpi::Scpi::open(0, false);
        let reply = scpi.exec_line(&mut ui, &Canvas::new(1, 1), ":SYSTem:STATe?");
        let reply = String::from_utf8(reply).unwrap();
        let (_, source) = reply.split_once(" source.v1 ").unwrap();
        let (source, _) = source.split_once(" fallback.v1 ").unwrap();
        assert!(source.contains(" duty=0.000/0.000 "));
        assert!(source.contains(" capture_share=0.060 "));
    }

    #[test]
    fn slow_signal_uses_elapsed_wall_time() {
        let at = Instant::now();
        let start = Sample { at, shown: 0, lost: 0, pair: (0, 0) };
        let mut status = CaptureStatus::default();
        status.observe(start, || unreachable!());
        status.observe(Sample { at: at + Duration::from_secs(2), shown: 2, ..start }, || Some((1e6, 62_000)));
        assert_eq!(status.label, Some(("1 wfm/s 6.2%".into(), false)));
    }

    #[test]
    fn coverage_over_one_hundred_percent_clamps() {
        let at = Instant::now();
        let start = Sample { at, shown: 0, lost: 0, pair: (0, 0) };
        let mut status = CaptureStatus::default();
        status.observe(start, || unreachable!());
        status.observe(Sample { at: at + Duration::from_secs(1), shown: 2, ..start }, || Some((1e6, 600_000)));
        assert_eq!(status.share, Some(1.0));
        assert_eq!(status.label, Some(("2 wfm/s 100%".into(), false)));
    }

    #[test]
    fn shown_rate_uses_counter_deltas_and_marks_new_losses() {
        let at = Instant::now();
        let start = Sample { at, shown: 100, lost: 4, pair: (0, 0) };
        let mut status = CaptureStatus::default();
        status.observe(start, || unreachable!());
        status.observe(Sample { at: at + Duration::from_millis(999), shown: 1939, ..start }, || unreachable!());
        assert!(status.label.is_none());
        let next = at + Duration::from_secs(1);
        let end = Sample { at: next, shown: 1940, lost: 5, ..start };
        status.observe(end, || Some((1e6, 337)));
        assert_eq!(status.label, Some(("1.84k wfm/s 62%!".into(), true)));
        status.observe(Sample { at: next + Duration::from_secs(1), ..end }, || unreachable!());
        assert_eq!(status.share, None);
        assert_eq!(status.label, Some(("0 wfm/s --%".into(), false)));
    }

    #[test]
    fn a_window_spanning_an_acquisition_change_has_no_share() {
        let at = Instant::now();
        let start = Sample { at, shown: 0, lost: 0, pair: (0, 0) };
        let mut status = CaptureStatus::default();
        status.observe(start, || unreachable!());
        let next = Sample { at: at + Duration::from_secs(1), shown: 1, lost: 1, pair: (1, 0) };
        status.observe(next, || unreachable!());
        assert_eq!(status.label, Some(("1 wfm/s --%!".into(), true)));
        status.observe(Sample { at: at + Duration::from_secs(2), shown: 2, ..next }, || Some((1e6, 100_000)));
        assert_eq!(status.label, Some(("1 wfm/s 10%".into(), false)));
    }

    #[test]
    fn settings_changes_leave_the_capture_label_unmarked() {
        use std::sync::atomic::Ordering::Relaxed;
        let counters = drawpipe::Counters::default();
        let at = Instant::now();
        let mut status = CaptureStatus::default();
        let mut observe = |seconds| {
            let (shown, lost) = counters.capture_counts();
            status.observe(Sample { at: at + Duration::from_secs(seconds), shown, lost, pair: (0, 0) }, || None);
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
    fn normal_panel_status_fits_beside_single_with_loss_mark_and_full_share() {
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
