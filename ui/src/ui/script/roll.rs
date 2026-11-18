// SPDX-License-Identifier: GPL-2.0-or-later

//! Host diagnosis of growing roll snapshots. Select the analyser with OOS_ROLL_PROFILE_VIEW.

use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
use crate::board::Frame;
use crate::ui::*;
use std::time::Instant;

fn fixture() -> (Ui, Frame) {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    let mut batches = super::batch::BatchScript::new(&mut ui, BatchKind::Roll);
    ui.m.tscale = tscales().iter().position(|&t| t == 0.2).unwrap();
    ui.m.auto_roll = true;
    ui.m.view = match std::env::var("OOS_ROLL_PROFILE_VIEW").as_deref() {
        Ok("spectrum") => AView::Spectrum,
        Ok("eye") => AView::Eye,
        Ok("step") => AView::Step,
        _ => AView::Scope,
    };
    ui.step(1.0 / 30.0);
    batches.feed_step(&mut ui).unwrap();
    ui.step(1.0 / 30.0);
    batches.finish_step(&ui).unwrap();
    let mut source = scripted::source(BatchKind::Roll, 123);
    let template = source.next_batch(&ui.m, ui.switches(), 1, None).unwrap().newest(ui.m.hw_stamp);
    (ui, template)
}

#[test]
#[ignore = "host timing: select OOS_ROLL_PROFILE_VIEW"]
fn growing_roll_step_profile() {
    let (mut ui, template) = fixture();
    for (seq, len) in [1000, 25_000, 50_000, 100_000].into_iter().enumerate() {
        let mut frame = template.clone();
        frame.seq = 10_000 + seq as u64;
        frame.dt = 20e-6;
        frame.envelope = true;
        frame.acq = (0.0, 0);
        frame.partial = len < 100_000;
        frame.ch[0] = template.ch[0].iter().copied().cycle().take(len).collect();
        let start = Instant::now();
        let rec = crate::wave::hw_record(&ui.m, std::sync::Arc::new(frame.clone()));
        let convert = start.elapsed();
        let start = Instant::now();
        ui.ph.draw_ex(0, &ui.m, (ui.m.time_per_div(), 0.0), rec.t0, rec.dt, &rec.ch[0], 24, None, rec.pairs);
        let draw = start.elapsed();
        if let Source::Hw { shared, .. } = &ui.src {
            let sample = crate::board::dho::roll::publish_roll(&mut oos_hal::sync::lock(shared), frame);
            if let Some(sample) = sample {
                sample.record();
            }
        }
        ui.t_acq = 0.0;
        ui.t_draw = 0.0;
        ui.t_parts = [0.0; 4];
        let start = Instant::now();
        ui.step(1.0 / 30.0);
        assert_eq!(ui.last[0].as_ref().and_then(|r| r.raw.as_ref()).unwrap().frame.ch[0].len(), len);
        assert!(!ui.last[0].as_ref().unwrap().triggered);
        if let Source::Hw { shared, seen } = &ui.src {
            assert_eq!(*seen, 10_000 + seq as u64);
            assert!(oos_hal::sync::lock(shared).queue.is_empty());
        }
        println!(
            "roll points={len} step={:?} convert={convert:?} draw={draw:?} acq_ms={} draw_ms={} parts_ms={:?}",
            start.elapsed(),
            ui.t_acq / 0.05,
            ui.t_draw / 0.05,
            ui.t_parts.map(|v| v / 0.05)
        );
    }
}

fn budget_fixture(roll: bool) -> (Ui, Frame) {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    let tdiv = if roll { 0.2 } else { 0.002 };
    ui.m.tscale = tscales().iter().position(|&t| t == tdiv).unwrap();
    ui.m.auto_roll = roll;
    ui.m.mdepth = mdepths().iter().position(|d| d.1 == 10_000_000).unwrap();
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.eye_src = WaveId::Ch(1);
    ui.m.spec_src = WaveId::Ch(1);
    ui.m.bw_src = 1;
    ui.m.ch.iter_mut().enumerate().for_each(|(c, ch)| ch.on = !roll || c == 1);
    // Keep the real measurement handoff busy without scheduling work or relying on thread timing.
    ui.measure_in_background();
    ui.meas_bg.as_mut().unwrap().busy = true;
    let len = if roll { 100_000 } else { 2_500_000 };
    let dt = tdiv as f64 * HDIV as f64 / len as f64;
    let signal: Vec<f32> = (0..128).map(|i| if i % 32 < 16 { -1.0 } else { 1.0 }).collect();
    ui.stats.fill(stats(&signal, dt));
    let ch = std::array::from_fn(|c| {
        if ui.m.ch[c].on {
            signal.iter().copied().cycle().take(len).collect()
        } else {
            Vec::new()
        }
    });
    let frame = Frame {
        seq: 1,
        ch,
        dt,
        pre: if roll { 0 } else { len / 2 },
        roll,
        envelope: roll,
        acq: if roll { (0.0, 0) } else { (1.0 / dt, 10_000_000) },
        ..Default::default()
    };
    (ui, frame)
}

fn assert_bounded_analysis_steps(roll: bool) {
    let (mut ui, mut frame) = budget_fixture(roll);
    let mut scpi = crate::scpi::Scpi::open(crate::net6::Port::Host, false);
    let cv = Canvas::new(ui.scr.w, ui.scr.h);
    for (view, pane) in [
        (AView::Eye, 0),
        (AView::Step, 0),
        (AView::Spectrum, 0),
        (AView::Power, 0),
        (AView::Scope, tables::PANE_SPECTRUM),
        (AView::Scope, tables::PANE_RESULTS),
    ] {
        ui.m.view = view;
        ui.m.pane = pane;
        for step in 0..3 {
            frame.seq += 1;
            let id = super::super::change::acquisition::script_prepare(&mut ui);
            frame.stamp = id.epoch();
            frame.cfg = id.gen();
            if let Source::Hw { shared, .. } = &ui.src {
                let mut g = oos_hal::sync::lock(shared);
                let mut snapshot = frame.clone();
                if roll {
                    snapshot.ch[1].truncate([1000, 25_000, 100_000][step]);
                    snapshot.partial = step < 2;
                }
                let snapshot = std::sync::Arc::new(snapshot);
                g.frame = snapshot.clone();
                g.queue.push_back(snapshot);
            }
            let before = ui.azr.work.samples();
            let queued_before = ui.azr.work.queued_samples();
            let records = ui.records;
            ui.step(if step == 0 { 0.5 } else { 1.0 / 30.0 });
            let work = ui.azr.work.samples() - before;
            assert_eq!(work, 0, "{view:?}, roll={roll}: main copied analysis samples");
            let queued = ui.azr.work.queued_samples() - queued_before;
            assert!(queued > 0 || ui.meas_bg.as_ref().unwrap().analysis_busy || ui.azr.work.pending);
            assert!(queued <= 7200, "only the selected screen vectors may be copied on main");
            println!("{view:?} roll={roll} step={step}: main analysis samples={work}, queued screen samples={queued}");
            assert!(ui.last[0].is_some() && ui.records > records);
            assert!(ui.ph.maps()[1].iter().any(|&v| v != 0), "waveform keeps updating");
            assert!(matches!(ui.src, Source::Hw { seen, .. } if seen == frame.seq));
            assert!(scpi.exec_line(&mut ui, &cv, "*IDN?;*OPC?").starts_with(b"OOS,"));
        }
    }
    ui.stop_workers(Instant::now() + std::time::Duration::from_secs(2));
}

#[test]
fn eye_roll_at_200ms_has_bounded_analysis_work() {
    assert_bounded_analysis_steps(true);
}

#[test]
fn eye_four_channels_at_10m_has_bounded_analysis_work() {
    assert_bounded_analysis_steps(false);
}
