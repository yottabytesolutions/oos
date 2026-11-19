// SPDX-License-Identifier: GPL-2.0-or-later

//! Deterministic change storm for the test-only allocation observer. No allocator lives here.

use super::HostScript;
use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
use crate::{cmd, drawpipe, ui::Ui, wave::Source};
use std::sync::atomic::Ordering;

pub fn run(mode: &str, steps: usize, mut sample: impl FnMut(usize, [u64; 6])) -> Result<(), String> {
    run_observed(mode, steps, |step, counters, _| sample(step, counters))
}

pub fn run_observed(mode: &str, steps: usize, mut sample: impl FnMut(usize, [u64; 6], &Ui)) -> Result<(), String> {
    let kind = BatchKind::parse(if mode == "average" { "raw" } else { mode })?;
    oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some("DHO924S")));
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
    ui.store_dir = "store".into();
    let mut host = HostScript::new(&mut ui, None, Some(kind), false)?;
    let time = match mode {
        "pairs" => "200 µs",
        "deep-pairs" => "1 ms",
        "codes" => "5 µs",
        _ => "200 ns",
    };
    commands(&mut ui, &format!("menu Horizontal\nset Time/div = {}\n", time))?;
    if mode == "average" {
        commands(&mut ui, "menu Acquire\nset Mode = Average\nset Averages = 16\n")?;
    }
    println!(
        "soak config mode={} tdiv={} workers=2 measurements=off schedule=27,3,13,3,27,13",
        mode,
        ui.m.time_per_div()
    );
    let mut planned = 0;
    // Process totals can include earlier host scripts and demo frames.
    let before = drawpipe::counters().read();
    let measured_before = drawpipe::counters().measured.load(Ordering::Relaxed);
    for step in 1..=steps {
        super::super::super::change::intake(&mut ui, |ui| storm(ui, (step - 1) % 100))?;
        host.render_step(&mut ui)?;
        if step % 100 == 0 {
            old_stamp(&mut ui)?;
            planned += 3;
        }
        let (acquired, used, dropped) = drawpipe::counters().read();
        let (acquired, used, dropped) = (acquired - before.0, used - before.1, dropped - before.2);
        let measured = drawpipe::counters().measured.load(Ordering::Relaxed) - measured_before;
        if dropped != planned || acquired != used + dropped || ui.zone_drops != 0 {
            return Err(format!("soak step {} accounting: {:?}, planned {}", step, (acquired, used, dropped), planned));
        }
        sample(step, [acquired, used, dropped, measured, planned, ui.zone_drops], &ui);
    }
    Ok(())
}

fn storm(ui: &mut Ui, phase: usize) -> Result<(), String> {
    let text = match phase {
        0 => "menu Display\nset Persistence = Min\n",
        10 => "menu CH2\nset Display = On\nmenu CH3\nset Display = On\nmenu CH4\nset Display = On\nkey ch1\n",
        20 => "knob vscale -1\nknob vpos +2\nknob tlevel +1\n",
        30 => "menu Display\nset Persistence = 500 ms\n",
        40 => "knob hscale push\n",
        50 => "key clear\n",
        60 => "key runstop\n",
        65 => "key runstop\n",
        70 => "knob hscale push\n",
        80 => "knob vscale +1\nknob vpos -2\nknob tlevel -1\n",
        90 => "menu CH2\nset Display = Off\nmenu CH3\nset Display = Off\nmenu CH4\nset Display = Off\n",
        _ => return Ok(()),
    };
    commands(ui, text)
}

fn commands(ui: &mut Ui, text: &str) -> Result<(), String> {
    for line in text.lines() {
        // Named setters are validated, rather than relying on cmd::run's diagnostic-only error handling.
        if let Some(name) = line.strip_prefix("menu ") {
            ui.script_menu(name)?;
        } else if let Some(setting) = line.strip_prefix("set ") {
            let (name, value) = setting.split_once(" = ").ok_or("invalid soak setting")?;
            ui.script_set(name, value)?;
        } else {
            cmd::run(ui, line);
        }
    }
    Ok(())
}

fn old_stamp(ui: &mut Ui) -> Result<(), String> {
    let pipe = drawpipe::pipe(ui.switches()).ok_or("soak needs workers")?;
    pipe.set_identity(super::super::super::change::acquisition::identity(ui));
    pipe.set_view(Some(&ui.m), ui.ph.w, ui.ph.h, ui.switches().interp);
    let mut source = scripted::source(BatchKind::Raw, 123);
    let batch = source.next_batch(&ui.m, ui.switches(), 3, None)?;
    let (fb, decode) = (batch.frame_bytes(), batch.decode());
    let before = drawpipe::counters().read();
    drawpipe::Counters::add(&drawpipe::counters().acquired, 3);
    pipe.feed_with(Box::new(batch.into_bytes()), fb, 0..3, Default::default(), decode, None, None);
    drop(drawpipe::host::finish_batch(pipe, 0)?);
    let after = drawpipe::counters().read();
    if (after.0 - before.0, after.1 - before.1, after.2 - before.2) != (3, 0, 3) {
        return Err(format!("planned old-stamp drop: {:?} -> {:?}", before, after));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_reports_its_own_counters_after_a_demo_step() {
        let _scope = drawpipe::test_scope::Scope::new();
        // A preceding soak leaves nonzero acquisition, usage and planned-drop totals.
        run("raw", 100, |_, _| {}).expect("the first scripted soak balances its own frames");
        let mut demo = Ui::new(Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        demo.m.meas.clear();
        demo.m.meas2.clear();
        let before = drawpipe::counters().read();
        demo.step(0.04);
        let after = drawpipe::counters().read();
        assert_eq!(after.0, before.0, "demo frames are not acquired hardware frames");
        assert!(after.1 > before.1, "the demo step must draw a frame");
        // Model measurements collected before this measurements-off soak.
        drawpipe::Counters::add(&drawpipe::counters().measured, 7);
        let mut samples = 0;
        run_observed("raw", 100, |step, counters, _| {
            samples += 1;
            if step == 1 {
                assert_eq!(counters, [27, 27, 0, 0, 0, 0]);
            }
            assert_eq!(counters[0], counters[1] + counters[2]);
            assert_eq!(counters[2], if step == 100 { 3 } else { 0 });
            assert_eq!(counters[3], 0, "measurements from before the soak must be excluded");
            assert_eq!(counters[4], counters[2]);
            assert_eq!(counters[5], 0);
        })
        .expect("the soak excludes accounting and measurements from before its start");
        assert_eq!(samples, 100);
    }
}
