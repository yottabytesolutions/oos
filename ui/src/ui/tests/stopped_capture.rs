// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use crate::board::{identity::ReplayLoss, HwShared};
use oos_hal::sync::lock;

const FULL_RECORD_KINDS: [Meas; 3] = [Meas::Vmax, Meas::Vavg, Meas::Vpp];

fn measurement_bits(ui: &Ui) -> [u64; 3] {
    FULL_RECORD_KINDS.map(|kind| {
        let value = ui.meas(0, kind);
        if value.is_nan() {
            f64::NAN.to_bits()
        } else {
            value.to_bits()
        }
    })
}

fn scpi(ui: &mut Ui, commands: &str) -> Vec<u8> {
    let mut scpi = crate::scpi::Scpi::disconnected();
    let mut program = crate::scpi::Program::new(commands);
    scpi.poll_program(ui, &Canvas::new(1, 1), &mut program).expect("query answers in one pass")
}

fn settle_stop(ui: &mut Ui, shared: &HwShared) {
    ui.m.run = RunState::Stop;
    ui.step(0.02);
    {
        let mut g = lock(shared);
        g.final_ack = g.final_req;
    }
    ui.step(0.02);
    ui.step(0.02);
}

#[test]
fn full_record_and_raw_answers_survive_hardware_stop_rereads() {
    for _ in 0..2 {
        for (mode, pairs) in [
            (AcqMode::Normal, false),
            (AcqMode::Normal, true),
            (AcqMode::HiRes, false),
            (AcqMode::Peak, true),
            (AcqMode::Average, false),
        ] {
            exercise_full_record(mode, pairs);
        }
    }
}

fn full_record_fixture(mode: AcqMode, pairs: bool) -> (Ui, HwShared, crate::board::Frame) {
    let shared = HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.run = RunState::Run;
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.acq = mode;
    ui.m.tscale = tscales().iter().position(|&s| (s - 1e-3).abs() < 1e-9).unwrap();
    ui.m.meas = FULL_RECORD_KINDS.map(|kind| (kind, WaveId::Ch(0))).into();
    ui.m.meas2.clear();
    ui.m.meas_full_record = true;
    ui.m.ch.iter_mut().skip(1).for_each(|ch| ch.on = false);
    ui.step(0.02);
    let id = change::acquisition::identity(&ui);
    let mut values = vec![0.25; 6000];
    values[1000..1040].fill(0.75);
    values[3600..3640].fill(-0.5);
    let frame = crate::board::Frame {
        seq: 1,
        cfg: id.gen(),
        stamp: id.epoch(),
        dt: 2e-6,
        pre: 3000,
        fixed_ti: Some(3000.0),
        basic: [crate::board::BasicStats::of(&values), Default::default(), Default::default(), Default::default()]
            .into(),
        ch: [values, Vec::new(), Vec::new(), Vec::new()],
        envelope: pairs,
        hw_avg: mode == AcqMode::Average,
        ..Default::default()
    };
    (ui, shared, frame)
}

#[test]
fn run_without_a_trigger_keeps_the_original_capture_after_a_stopped_window() {
    for _ in 0..2 {
        no_trigger_after_stopped_window(false);
    }
}

#[test]
fn single_without_a_trigger_keeps_the_original_capture_after_a_stopped_window() {
    for _ in 0..2 {
        no_trigger_after_stopped_window(true);
    }
}

fn no_trigger_after_stopped_window(single: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, frame) = full_record_fixture(AcqMode::Normal, false);
    let full_len = frame.ch[0].len();
    lock(&shared).queue.push_back(frame.clone().into());
    ui.step(0.02);
    let full = measurement_bits(&ui);
    assert_eq!(ui.raw_info(0).unwrap().0, full_len);
    settle_stop(&mut ui, &shared);
    ui.m.tscale -= 3;
    ui.step(0.02);
    let window = crate::board::Frame {
        seq: 2,
        pre: 600,
        fixed_ti: Some(600.0),
        ch: [vec![-0.25; 1200], Vec::new(), Vec::new(), Vec::new()],
        basic: Default::default(),
        ..frame
    };
    lock(&shared).inject_stop_reply(window);
    ui.step(0.02);
    assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.ch[0].len(), 1200);
    let pixels = super::run_presentation::pixels(&ui);
    let geometry = (ui.m.time_per_div(), ui.m.delay);
    assert!(pixels[0].iter().any(|&pixel| pixel != 0));
    assert_eq!(measurement_bits(&ui), full);

    if single {
        ui.key(key::SINGLE);
    } else {
        ui.key(key::RUNSTOP);
    }
    ui.step(0.02);
    ui.step(0.02);
    assert_eq!(ui.m.run, RunState::Run);
    assert_eq!((ui.m.time_per_div(), ui.m.delay), geometry);
    let waiting_pixels = super::run_presentation::pixels(&ui);
    settle_stop(&mut ui, &shared);
    assert_eq!(measurement_bits(&ui), full);
    assert_eq!(ui.raw_info(0).unwrap().0, full_len);
    assert_eq!(lock(&shared).stop_view.unwrap().capture, 1);
    for mode in ["RAW", "MAX"] {
        assert_eq!(scpi(&mut ui, &format!(":WAV:MODE {mode};:WAV:POIN?")), full_len.to_string().into_bytes());
        let bytes = scpi(&mut ui, &format!(":WAV:MODE {mode};:WAV:FORM BYTE;:WAV:DATA?"));
        assert!(bytes.starts_with(format!("#9{full_len:09}").as_bytes()));
        assert_eq!(bytes.len(), full_len + 11);
    }
    if !single {
        assert!(waiting_pixels == pixels, "the waiting picture keeps the stopped view");
    }
}

#[test]
fn pairs_reread_draws_pairs_and_measures_the_raw_capture() {
    for _ in 0..2 {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, frame) = full_record_fixture(AcqMode::Normal, false);
        lock(&shared).queue.push_back(frame.clone().into());
        ui.step(0.02);
        let full = measurement_bits(&ui);
        settle_stop(&mut ui, &shared);
        ui.m.tscale -= 3;
        ui.m.meas_full_record = false;
        ui.step(0.02);
        let visible = measurement_bits(&ui);
        assert_ne!(visible, full);
        let raw_pixels = super::run_presentation::pixels(&ui);

        let mut pairs = frame;
        pairs.seq = 2;
        pairs.envelope = true;
        for pair in pairs.ch[0].as_chunks_mut::<2>().0 {
            *pair = [-1.0, 1.0];
        }
        lock(&shared).inject_stop_reply(pairs);
        ui.step(0.02);
        assert_eq!(measurement_bits(&ui), visible);
        let drawn = super::run_presentation::pixels(&ui);
        assert!(drawn != raw_pixels && drawn[0].iter().any(|&pixel| pixel != 0));
        ui.m.meas_full_record = true;
        ui.step(0.02);
        assert_eq!(measurement_bits(&ui), full);
    }
}

#[test]
fn pairs_measurements_follow_the_selected_record_after_a_raw_reread() {
    for _ in 0..2 {
        for depth in [100_000, 10_000_000] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let (mut ui, shared, mut frame) = full_record_fixture(AcqMode::Normal, true);
            frame.acq = (frame.dt.recip(), depth);
            lock(&shared).queue.push_back(frame.clone().into());
            ui.step(0.02);
            settle_stop(&mut ui, &shared);
            let pairs = measurement_bits(&ui);
            assert_eq!(pairs[1], f64::NAN.to_bits());

            ui.m.tscale -= 3;
            ui.m.meas_full_record = false;
            ui.step(0.02);
            frame.seq = 2;
            frame.envelope = false;
            frame.ch[0].fill(0.5);
            frame.basic = Default::default();
            lock(&shared).inject_stop_reply(frame);
            ui.step(0.02);
            assert_eq!(ui.meas(0, Meas::Vavg), 0.5);
            ui.m.meas_full_record = true;
            assert_eq!(measurement_bits(&ui), pairs);
        }
    }
}

fn exercise_full_record(mode: AcqMode, pairs: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, frame) = full_record_fixture(mode, pairs);
    lock(&shared).queue.push_back(frame.clone().into());
    ui.step(0.02);
    let run = measurement_bits(&ui);
    assert_ne!(run[0], f64::NAN.to_bits(), "{mode:?}, pairs {pairs}");
    let raw = (ui.raw_info(0), ui.raw_volts(0));
    let scpi_run = scpi(&mut ui, ":MEAS:ITEM? VMAX,CHAN1");
    let waves = ["RAW", "MAX"]
        .map(|mode| scpi(&mut ui, &format!(":WAV:MODE {mode};:WAV:FORM ASC;:WAV:STAR 1;:WAV:STOP 12;:WAV:DATA?")));
    assert!(waves.iter().all(|reply| reply.starts_with(b"#9")));
    settle_stop(&mut ui, &shared);
    assert_eq!(measurement_bits(&ui), run, "{mode:?}, pairs {pairs}, at STOP");
    assert_eq!(ui.stopped_capture().map(|capture| capture.1), Some(1), "{mode:?}, pairs {pairs}, STOP must settle");
    if mode == AcqMode::Normal && !pairs {
        assert_stop_refresh_keeps_statistics_trend_and_log_schedule(&mut ui, &shared, &frame);
    }
    let overview = ui.m.tscale;
    for (seq, scale, delay) in [(2, overview - 3, 0.0), (3, overview - 3, 8e-3), (4, overview, 0.0)] {
        ui.m.tscale = scale;
        ui.m.delay = delay;
        ui.step(0.02);
        assert_eq!(measurement_bits(&ui), run, "{mode:?}, pairs {pairs}, before reply {seq}");
        assert_eq!(scpi(&mut ui, ":MEAS:ITEM? VMAX,CHAN1"), scpi_run, "{mode:?}, before reply {seq}");
        assert!(lock(&shared).stop_replies.frame.is_none());
        let mut reply = frame.clone();
        reply.seq = seq;
        reply.envelope = false;
        reply.ch[0].fill(2.0);
        reply.basic = Default::default();
        lock(&shared).inject_stop_reply(reply);
        ui.step(0.02);
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, seq);
        assert_eq!(measurement_bits(&ui), run, "{mode:?}, pairs {pairs}, after reply {seq}");
        assert_eq!((ui.raw_info(0), ui.raw_volts(0)), raw);
        if pairs {
            for (mode, expected) in ["RAW", "MAX"].into_iter().zip(&waves) {
                assert_eq!(
                    &scpi(&mut ui, &format!(":WAV:MODE {mode};:WAV:FORM ASC;:WAV:STAR 1;:WAV:STOP 12;:WAV:DATA?")),
                    expected,
                    "{mode}, after reply {seq}"
                );
            }
        }
        assert!(ui.mstat.iter().all(|s| s.n == 0));
    }
}

fn assert_stop_refresh_keeps_statistics_trend_and_log_schedule(
    ui: &mut Ui,
    shared: &HwShared,
    frame: &crate::board::Frame,
) {
    ui.store_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target")
        .join(format!("stop-logger-{}", std::process::id()));
    ui.log_start();
    ui.step(0.02);
    let rows = ui.log.rows;
    assert_eq!(rows, 1);
    let measurements = ui.measurement_statistics_snapshot_for_test();
    assert!(measurements.iter().any(|(count, _)| *count > 0));
    let trend = format!("{:?}", ui.trend);
    assert!(!ui.trend.is_empty());
    let mut reread = frame.clone();
    reread.seq = 10;
    lock(shared).inject_stop_reply(reread);
    ui.step(0.02);
    assert_eq!(ui.measurement_statistics_snapshot_for_test(), measurements);
    assert_eq!(format!("{:?}", ui.trend), trend);
    assert_eq!(ui.log.rows, rows);
    std::fs::remove_file(ui.log.file.take().unwrap()).unwrap();
    std::fs::remove_dir(&ui.store_dir).unwrap();
}

#[test]
fn calibration_marks_the_capture_in_stop_only() {
    for _ in 0..2 {
        for analog in [true, false] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let shared = HwShared::default();
            let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
            ui.m.trig_mode = TrigMode::Normal;
            ui.m.la.on = !analog;
            ui.m.la.show = u16::from(!analog);
            ui.m.ch.iter_mut().enumerate().for_each(|(c, ch)| ch.on = analog && c == 0);
            ui.step(0.04);
            let accepted = |ui: &Ui, seq| {
                let id = change::acquisition::identity(ui);
                let ch = std::array::from_fn(|c| if analog && c == 0 { vec![0.25; 1024] } else { Vec::new() });
                let la = (!analog).then(|| la::LaRec { words: vec![1; 1024].into(), t0: -5e-6, dt: 1e-8 });
                crate::board::Frame {
                    seq,
                    ch,
                    la,
                    cfg: id.gen(),
                    stamp: id.epoch(),
                    dt: 1e-8,
                    pre: 512,
                    fixed_ti: Some(512.0),
                    ..Default::default()
                }
            };
            lock(&shared).queue.push_back(accepted(&ui, 1).into());
            ui.step(0.04);
            ui.m.run = RunState::Stop;
            ui.step(0.04);
            assert_eq!(ui.last[0].as_ref().unwrap().raw.is_some(), analog);
            assert_eq!(ui.calibration_capture_note(), None);
            lock(&shared).stop_replies.inject_loss(1, ReplayLoss::Calibration);
            ui.step(0.04);
            assert_eq!(ui.calibration_capture_note(), Some("Captured before calibration. RUN for new data."));
            let mut canvas = Canvas::new(ui.scr.w, ui.scr.h);
            ui.draw_capture_calibration_note(&mut canvas);
            assert!(canvas.px.iter().any(|&pixel| pixel != 0));

            ui.m.run = RunState::Run;
            ui.step(0.04);
            assert_eq!(ui.calibration_capture_note(), None);
            lock(&shared).queue.push_back(accepted(&ui, 2).into());
            ui.step(0.04);
            ui.m.run = RunState::Stop;
            ui.step(0.04);
            assert_eq!(ui.calibration_capture_note(), None);
        }
    }
}

#[test]
fn logic_only_capture_draws_the_same_after_calibration_and_ring_loss() {
    for _ in 0..2 {
        assert_eq!(logic_after_loss(ReplayLoss::Calibration), logic_after_loss(ReplayLoss::Successor));
    }
}

fn logic_after_loss(loss: ReplayLoss) -> Vec<u32> {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut frame) = full_record_fixture(AcqMode::Normal, false);
    ui.m.la.on = true;
    ui.m.la.show = 1;
    ui.m.ch[0].on = false;
    ui.step(0.02);
    let id = change::acquisition::identity(&ui);
    (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
    frame.ch[0].clear();
    frame.la = Some(la::LaRec {
        words: (0..6000).map(|i| u16::from(i / 150 % 2 == 0)).collect::<Vec<_>>().into(),
        t0: -6e-3,
        dt: 2e-6,
    });
    lock(&shared).queue.push_back(frame.into());
    ui.step(0.02);
    settle_stop(&mut ui, &shared);
    lock(&shared).stop_replies.inject_loss(1, loss);
    ui.m.delay += 0.5e-3;
    ui.m.tscale -= 2;
    ui.step(0.02);
    let mut drawn = Canvas::new(ui.scr.w, ui.scr.h);
    ui.draw_la(&mut drawn);
    let (band, _) = ui.la_band(1);
    let left = (band.x + MARK_W) as usize;
    let right = (band.x + band.w - MARK_W) as usize;
    assert!(
        drawn
            .px
            .chunks_exact(drawn.w as usize)
            .any(|row| { row[left..right].iter().any(|&pixel| pixel & 0xff_ffff == la::line_color(0)) }),
        "the logic trace still draws after loss"
    );
    drawn.px
}

#[test]
fn calibration_and_successor_loss_project_the_capture_at_the_requested_view() {
    for _ in 0..2 {
        for mode in [AcqMode::Normal, AcqMode::Peak] {
            assert_eq!(
                loss_projection(mode, ReplayLoss::Calibration, true),
                loss_projection(mode, ReplayLoss::Successor, true)
            );
        }
    }
}

#[test]
fn pan_and_zoom_project_the_capture_when_loss_precedes_the_first_reread() {
    for _ in 0..2 {
        for mode in [AcqMode::Normal, AcqMode::Peak] {
            assert_eq!(
                loss_projection(mode, ReplayLoss::Calibration, false),
                loss_projection(mode, ReplayLoss::Successor, false)
            );
        }
    }
}

fn loss_projection(mode: AcqMode, loss: ReplayLoss, reread: bool) -> [Vec<u32>; 2] {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.acq = mode;
    ui.m.tscale = tscales().iter().position(|&s| (s - 100e-6).abs() < 1e-9).unwrap();
    ui.m.meas = vec![(Meas::Vmax, WaveId::Ch(0))];
    ui.m.meas2.clear();
    ui.m.meas_full_record = false;
    ui.m.ch.iter_mut().skip(1).for_each(|ch| ch.on = false);
    ui.step(0.02);
    let id = change::acquisition::identity(&ui);
    let mut values = vec![0.25; 8000];
    values[4500..].fill(0.75);
    let frame = crate::board::Frame {
        seq: 1,
        cfg: id.gen(),
        stamp: id.epoch(),
        dt: 1e-6,
        pre: 4000,
        fixed_ti: Some(4000.0),
        ch: [values, Vec::new(), Vec::new(), Vec::new()],
        envelope: mode == AcqMode::Peak,
        ..Default::default()
    };
    lock(&shared).queue.push_back(frame.clone().into());
    ui.step(0.02);
    settle_stop(&mut ui, &shared);
    if reread {
        let mut reply = frame;
        reply.seq = 2;
        reply.envelope = false;
        reply.ch[0].fill(1.25);
        lock(&shared).inject_stop_reply(reply);
        ui.step(0.02);
        assert!((ui.meas(0, Meas::Vmax) - 1.25).abs() < 1e-6, "Screen gate follows the reread");
    }
    lock(&shared).stop_replies.inject_loss(1, loss);
    ui.m.delay = 2e-3;
    ui.set_zoom(true);
    ui.step(0.02);
    assert!((ui.meas(0, Meas::Vmax) - 0.75).abs() < 1e-6, "Screen gate follows the projected capture");
    let pixels = super::run_presentation::pixels(&ui);
    assert!(pixels.iter().all(|plot| plot.iter().any(|&pixel| pixel != 0)));
    ui.m.delay = 9e-3;
    ui.m.zdelay = ui.m.delay;
    ui.step(0.02);
    assert!(ui.last[0].as_ref().unwrap().ch[0].iter().all(|v| v.is_nan()));
    assert!(ui.meas(0, Meas::Vmax).is_nan());
    assert_eq!(ui.ph.channel_max(0), 0);
    assert_eq!(ui.phz.channel_max(0), 0);
    pixels
}
