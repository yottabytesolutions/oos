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
    for step in 1..=steps {
        super::super::super::change::intake(&mut ui, |ui| storm(ui, (step - 1) % 100))?;
        host.render_step(&mut ui)?;
        if step % 100 == 0 {
            old_stamp(&mut ui)?;
            planned += 3;
        }
        let (acquired, used, dropped) = drawpipe::counters().read();
        if dropped != planned || acquired != used + dropped || ui.zone_drops != 0 {
            return Err(format!("soak step {} accounting: {:?}, planned {}", step, (acquired, used, dropped), planned));
        }
        sample(
            step,
            [acquired, used, dropped, drawpipe::counters().measured.load(Ordering::Relaxed), planned, ui.zone_drops],
            &ui,
        );
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
    let mut view = ui.m.clone();
    view.hw_stamp += 1;
    pipe.set_view(Some(&view), ui.ph.w, ui.ph.h, ui.switches().interp);
    let mut source = scripted::source(BatchKind::Raw, 123);
    let batch = source.next_batch(&ui.m, ui.switches(), 3, None)?;
    let (fb, decode) = (batch.frame_bytes(), batch.decode());
    let before = drawpipe::counters().read();
    drawpipe::Counters::add(&drawpipe::counters().acquired, 3);
    pipe.feed_with(Box::new(batch.into_bytes()), fb, 3, ui.m.hw_stamp, decode, None, Box::new(|| {}));
    drop(drawpipe::host::finish_batch(pipe, 0)?);
    pipe.release();
    let after = drawpipe::counters().read();
    if (after.0 - before.0, after.1 - before.1, after.2 - before.2) != (3, 0, 3) {
        return Err(format!("planned old-stamp drop: {:?} -> {:?}", before, after));
    }
    Ok(())
}
