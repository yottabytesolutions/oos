// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn signal(code: u8, frame: &mut Frame) {
    fill_signal(code, frame, 1);
}

pub(super) fn dense_signal(code: u8, frame: &mut Frame) {
    fill_signal(code, frame, 40);
}

fn fill_signal(code: u8, frame: &mut Frame, repeat: usize) {
    decode(0, frame);
    if repeat > 1 {
        frame.dt = 0.5e-9;
        frame.pre *= repeat;
        frame.ch[0].resize(frame.ch[0].len() * repeat, 0.0);
    }
    frame.fixed_ti = Some(frame.pre as f64);
    for (i, sample) in frame.ch[0].iter_mut().enumerate() {
        let i = i / repeat;
        let phase = (i / 5) % 2;
        let noise = match code {
            0 => 1.0,
            1 => -1.0,
            _ => 0.0,
        };
        let level = if (250..310).contains(&i) { 0.375 } else { 0.125 };
        *sample = level + if phase == 0 { noise } else { -noise };
    }
}

fn push(ui: &Ui, frame: Frame) {
    let Source::Hw { shared, .. } = &ui.src else { panic!("requires fake hardware") };
    oos_hal::sync::lock(shared).queue.push_back(Arc::new(frame));
}

fn waveform(ui: &mut Ui, mode: &str, query: &str) -> Vec<u8> {
    let mut scpi = crate::scpi::Scpi::disconnected();
    let cmd = format!(":WAV:MODE {mode};:WAV:FORM ASC;:WAV:{query}?");
    let mut program = crate::scpi::Program::new(&cmd);
    scpi.poll_program(ui, &Canvas::new(1, 1), &mut program).expect("immediate waveform query")
}

fn volts(ui: &mut Ui, mode: &str) -> Vec<f32> {
    let bytes = waveform(ui, mode, "DATA");
    assert!(bytes.starts_with(b"#9"));
    std::str::from_utf8(&bytes[11..]).unwrap().split(',').map(|value| value.parse().unwrap()).collect()
}

fn check_transfer(ui: &mut Ui, display: &[f32], raw: &[f32], native_max: bool) {
    for mode in ["NORM", "MAX", "RAW"] {
        let expected = if mode == "RAW" || mode == "MAX" && native_max { raw } else { display };
        let actual = volts(ui, mode);
        assert_eq!(actual.len(), expected.len(), "{mode}");
        assert!(actual.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-6), "{mode}");
        let preamble = waveform(ui, mode, "PRE");
        let preamble = std::str::from_utf8(&preamble).unwrap();
        let points = preamble.split(',').nth(2).unwrap().parse::<usize>().unwrap();
        assert_eq!(points, expected.len(), "{mode} preamble");
    }
}

#[test]
fn noisy_batch_feeds_measurements_statistics_cursor_dvm_and_waveform_from_the_average() {
    for (tdiv, signal) in [("0.0000001", signal as fn(u8, &mut Frame)), ("0.000001", dense_signal)] {
        check_noisy_batch_readers(tdiv, signal);
    }
}

fn check_noisy_batch_readers(tdiv: &str, signal: fn(u8, &mut Frame)) {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    crate::store::set(&mut ui.m, "tdiv", tdiv).unwrap();
    ui.m.meas = vec![(Meas::Vpp, WaveId::Ch(0))];
    ui.m.meas_full_record = true;
    ui.m.dvm_on = true;
    ui.step(0.04);
    submit_with(&ui, 0, &[0, 1], signal);
    ui.step(0.04);
    let record = ui.last[0].as_ref().unwrap();
    assert!(!record.pairs);
    assert_eq!(record.ch[0].len(), crate::board::record::HW_POINTS);
    let view = record.ch[0].clone();
    let raw = record.raw.as_ref().unwrap().frame.ch[0].clone();
    let vpp = ui.meas(0, Meas::Vpp);
    assert!((0.2..0.4).contains(&vpp), "averaged Vpp = {vpp}");
    assert_eq!((ui.mstat[0].n, ui.mstat[0].max), (1, vpp));
    assert!((ui.wave_at_source(WaveId::Ch(0), 0.0).unwrap() - 0.375).abs() < 1e-6);
    let mean = crate::board::BasicStats::of(&view).reading(Meas::Vavg).unwrap();
    assert!((ui.dvm_value() - mean).abs() < 1e-6);
    check_transfer(&mut ui, &view, &raw, false);

    ui.m.run = RunState::Stop;
    ui.step(0.04);
    check_transfer(&mut ui, &view, &raw, false);
    assert_eq!(ui.meas(0, Meas::Vpp), vpp);

    ui.m.run = RunState::Run;
    ui.m.acq = AcqMode::Normal;
    ui.step(0.04);
    let mut noisy = frame(&ui, 3, 0);
    signal(1, &mut noisy);
    push(&ui, noisy);
    ui.step(0.04);
    assert!(ui.meas(0, Meas::Vpp) > 2.0);
}

#[test]
fn stopped_average_pan_zoom_draws_between_grid_samples_and_keeps_raw() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    submit_with(&ui, 0, &[0, 1], signal);
    ui.step(0.04);
    let before = ui.last[0].as_ref().unwrap();
    let samples = before.ch[0].clone();
    let clock = (before.t0, before.dt);
    let native = ui.raw_volts(0);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    ui.m.tscale -= 5;
    ui.m.delay = 4e-8;
    ui.set_zoom(true);
    ui.m.zscale = ui.m.tscale - 3;
    ui.m.zdelay = ui.m.delay;
    ui.step(0.04);
    assert!(ui.ph.channel_max(0) > 0);
    let grid = ui.last[0].as_ref().unwrap();
    assert_eq!((&grid.ch[0], (grid.t0, grid.dt)), (&samples, clock));
    assert_eq!(ui.raw_volts(0), native);
    assert!((ui.wave_at_source(WaveId::Ch(0), ui.m.zdelay as f64).unwrap() - 0.375).abs() < 1e-6);

    let mut expected = crate::wave::Phosphor::new(ui.phz.w, ui.phz.h);
    expected.draw_ex(
        0,
        &ui.zoom_display_model(),
        ui.view(),
        clock.0,
        clock.1,
        &samples,
        step_draw::STOPPED_TRACE_WEIGHT,
        None,
        false,
    );
    assert_eq!(ui.phz.maps()[0], expected.maps()[0]);
    assert!(ui.phz.maps()[0].iter().any(|&pixel| pixel > 0));
}

#[test]
fn hardware_average_uses_native_samples_for_measurement_cursor_stopped_zoom_and_max() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    ui.m.avg_hw = true;
    ui.m.meas_full_record = true;
    ui.step(0.04);
    let mut averaged = frame(&ui, 1, 0);
    signal(2, &mut averaged);
    averaged.hw_avg = true;
    averaged.ch[0][0] = 5.0;
    let raw = averaged.ch[0].clone();
    push(&ui, averaged);
    ui.step(0.04);
    let screen = ui.last[0].as_ref().unwrap().ch[0].clone();
    let displayed_max = screen.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!((0.375..0.5).contains(&displayed_max));
    assert_eq!(ui.meas(0, Meas::Vmax), 5.0);
    let rec = ui.displayed_reader_record().unwrap();
    let window = ui.reader_raw(rec).unwrap();
    assert_eq!(ui.meas_slice(rec, 0), (&raw[window.i0..window.i1], window.frame.dt));
    let (samples, dt, t0) = ui.deskew_samples(WaveId::Ch(0)).unwrap();
    assert_eq!(samples, raw);
    assert_eq!(ui.wave_at_source(WaveId::Ch(0), t0), Some(5.0));
    assert_eq!(dt, 2e-9);
    check_transfer(&mut ui, &screen, &raw, false);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    let Source::Hw { shared, .. } = &ui.src else { panic!("requires fake hardware") };
    assert!(oos_hal::sync::lock(shared).stop_view.is_none());
    check_transfer(&mut ui, &screen, &raw, true);
    ui.set_zoom(true);
    ui.m.zscale = ui.m.tscale - 3;
    ui.step(0.04);
    let rec = ui.last[0].as_ref().unwrap();
    let detail = ui.reader_capture_view(rec, ui.view()).unwrap();
    assert!(Arc::ptr_eq(&detail.raw.as_ref().unwrap().frame, &rec.raw.as_ref().unwrap().frame));
    let mut expected = crate::wave::Phosphor::new(ui.phz.w, ui.phz.h);
    expected.draw_ex(
        0,
        &ui.zoom_display_model(),
        ui.view(),
        detail.t0,
        detail.dt,
        &detail.ch[0],
        step_draw::STOPPED_TRACE_WEIGHT,
        None,
        detail.pairs,
    );
    assert_eq!(ui.phz.maps()[0], expected.maps()[0]);
    assert_eq!(ui.meas(0, Meas::Vmax), 5.0);
}

#[test]
fn readers_wait_for_the_new_grid_after_a_running_reseed() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    advance(&mut ui, 0, &[4, 8]);
    ui.m.tscale += 1;
    ui.step(0.04);
    assert!(ui.meas(0, Meas::Vpp).is_nan());
    assert!(ui.wave_at_source(WaveId::Ch(0), 0.0).is_none());
    assert_eq!(waveform(&mut ui, "NORM", "POIN"), b"0");
    assert_eq!(advance(&mut ui, 2, &[12]), 0.75);
    assert!((ui.meas(0, Meas::Vmax) - 0.75).abs() < 1e-6);
    assert!((ui.wave_at_source(WaveId::Ch(0), 0.0).unwrap() - 0.75).abs() < 1e-6);
}

#[test]
fn software_average_rise_time_at_a_slow_time_base_uses_screen_grid_crossings() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    crate::store::set(&mut ui.m, "tdiv", "0.00001").unwrap();
    ui.m.mask_on = true;
    ui.m.meas_full_record = true;
    ui.step(0.04);
    let mut averaged = frame(&ui, 1, 0);
    averaged.dt = 10e-9;
    averaged.pre = 6000;
    averaged.fixed_ti = Some(6000.0);
    averaged.ch[0] = (0..12_000).map(|i| if i / 200 % 2 == 0 { -1.0 } else { 1.0 }).collect();
    let measure = |values: &[f32], dt: f64| oos_dsp::measure::analyze_refs(values, dt, 0.4, 0.1, 0.9).get(Meas::Rise);
    let native = measure(&averaged.ch[0], averaged.dt);
    push(&ui, averaged);
    ui.step(0.04);

    let grid = ui.last[0].as_ref().unwrap();
    let expected = measure(&grid.ch[0], grid.dt);
    let measured = ui.meas(0, Meas::Rise);
    assert!((measured - expected).abs() < 1e-12, "{measured} != {expected}");
    assert!(measured > native, "grid rise {measured}, native rise {native}");
    assert!(ui.meas(0, Meas::Vrms).is_finite());
}

#[test]
fn dense_software_average_matches_between_worker_and_records_routes() {
    let inputs = [0, 1, 0, 1, 2];
    let capture = |workers: bool| {
        let _scope = drawpipe::test_scope::Scope::new();
        let mut ui = ui();
        crate::store::set(&mut ui.m, "tdiv", "0.000001").unwrap();
        ui.m.mask_on = !workers;
        ui.step(0.04);
        assert_eq!(ui.fast_ok(), workers);
        if workers {
            submit_with(&ui, 0, &inputs, dense_signal);
        } else {
            for (index, code) in inputs.into_iter().enumerate() {
                let mut rec = frame(&ui, (index + 1) as u64, code);
                dense_signal(code, &mut rec);
                push(&ui, rec);
            }
        }
        ui.step(0.04);
        let rec = ui.last[0].take().unwrap();
        assert!(!rec.pairs);
        assert!(rec.dt / rec.raw.as_ref().unwrap().frame.dt >= 2.0);
        assert_eq!(rec.ch[0].len(), crate::board::record::HW_POINTS);
        rec
    };
    let worker = capture(true);
    let record = capture(false);
    assert_eq!((worker.t0, worker.dt), (record.t0, record.dt));
    let max_error = worker.ch[0].iter().zip(&record.ch[0]).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
    assert!(max_error <= 0.5 / 7500.0, "maximum grid error: {} ADC counts", max_error * 7500.0);
}
