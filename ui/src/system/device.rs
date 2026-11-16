// SPDX-License-Identifier: GPL-2.0-or-later
//! Scope startup, main intake and ordered process teardown.
mod graphics;
mod outputs;
mod remote;
mod startup;
mod timers;
use crate::*;
use std::time::Instant;

struct Device {
    ui: Ui,
    sw: sysconf::Switches,
    startup: Option<store::startup::Startup>,
    started: Instant,
    inp: input::Inputs,
    cv: Canvas,
    remote: remote::Remote,
    outputs: outputs::Outputs,
    timers: timers::Timers,
    graphics: graphics::Graphics,
    power_request: Option<ui::PowerAction>,
    buffers_released: bool,
}

pub fn run(mut ui: Ui, args: &[String], sw: sysconf::Switches, state: Option<store::startup::Startup>) {
    let mut disp = match drm::Display::open("/dev/dri/card0") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("oos-ui: display: {}", e);
            std::process::exit(1);
        }
    };
    eprintln!("oos-ui: display {}x{}", disp.width, disp.height);
    let inp = startup::prepare(&mut ui, &mut disp, args, &state);
    let started = Instant::now();
    let cv = Canvas::new(ui.scr.w, ui.scr.h);
    let remote = remote::Remote::new(args, &sw, &mut ui);
    let graphics = graphics::Graphics::new(&ui);
    let mut device = Device {
        ui,
        sw,
        startup: state,
        started,
        inp,
        cv,
        remote,
        graphics,
        outputs: outputs::Outputs::new(disp, !args.iter().any(|a| a == "--no-hdmi")),
        timers: timers::Timers::new(arg(args, "--start-up")),
        power_request: None,
        buffers_released: false,
    };
    while !device.ui.quit && !input::stop_requested() {
        device.intake();
        if device.ui.quit || input::stop_requested() {
            break;
        }
        device.frame();
        if device.power_request.is_some() {
            break;
        }
    }
    device.shutdown();
}

impl Device {
    fn intake(&mut self) {
        self.timers.startup_status(&mut self.ui);
        let elapsed = self.timers.last.elapsed().as_secs_f64();
        let wait_ms = ((1.0 / 30.0 - elapsed) * 1000.0).max(0.0) as i32;
        for line_or_ev in self.inp.poll(wait_ms) {
            match line_or_ev {
                input::In::Ev(ev) => self.ui.handle(ev),
                input::In::Cmd(line) => {
                    if let cmd::Out::Shot(p) = cmd::run(&mut self.ui, &line) {
                        self.ui.shot_request = Some(p);
                    }
                }
            }
            if self.ui.quit || input::stop_requested() {
                break;
            }
        }
        if self.ui.quit || input::stop_requested() {
            return;
        }
        self.remote.poll(&mut self.ui, &self.cv, &self.sw);
    }

    fn frame(&mut self) {
        if self.timers.last.elapsed().as_secs_f64() < 1.0 / 30.0 {
            return;
        }
        let dt = self.timers.last.elapsed().as_secs_f64().min(0.2);
        self.timers.last = Instant::now();
        let t0 = Instant::now();
        self.ui.step(dt);
        let stopped = self.ui.memory_stopped();
        if stopped && !self.buffers_released {
            self.remote.release_buffers();
        }
        self.buffers_released = stopped;
        if self.ui.quit {
            return;
        }
        let t1 = Instant::now();
        self.ui.set_hdmi_present(self.outputs.hdmi.is_some());
        let hdmi_frame = self.outputs.hdmi.is_some() && (self.outputs.frame_n + 1).is_multiple_of(2);
        self.ui.hdmi_snap_want = hdmi_frame && self.ui.m.hdmi == model::HdmiMode::Waveform;
        let drawn = self.graphics.frame(&mut self.ui, &mut self.cv, &mut self.outputs, &mut self.remote.web, &self.sw);
        let t2 = Instant::now();
        if !drawn {
            self.outputs.disp.present(&self.cv);
        }
        self.outputs.frame_n += 1;
        self.outputs.cpu_hdmi(&mut self.ui, &self.cv, hdmi_frame, drawn && self.graphics.monitor_drawn());
        let t3 = Instant::now();
        self.outputs.poll(&mut self.ui, &mut self.graphics);
        pacing::pass(t0, t1, t2, t3);
        let a = 0.05;
        self.ui.t_step += a * ((t1 - t0).as_secs_f32() * 1000.0 - self.ui.t_step);
        self.ui.t_render += a * ((t2 - t1).as_secs_f32() * 1000.0 - self.ui.t_render);
        self.ui.t_present += a * ((t3 - t2).as_secs_f32() * 1000.0 - self.ui.t_present);
        self.timers.panel(&mut self.ui, &self.cv, &mut self.inp);
        self.power_request = self.ui.power_action.take();
        if self.power_request.is_some() || self.ui.exit_code() != 0 {
            return;
        }
        if let Some(state) = self.startup.as_mut() {
            if let Err(e) = state.observe(store::setup_text(self.ui.setup_model()), self.started.elapsed()) {
                eprintln!("oos-ui: persist: {}", e);
            }
        }
        self.timers.network(&mut self.ui);
    }

    fn shutdown(&mut self) {
        let Self { ui, startup, started, remote, graphics, outputs, inp, power_request, .. } = self;
        let result = super::shutdown::run(
            ui,
            |deadline| remote.close(deadline),
            |ui, deadline| ui.stop_workers(deadline),
            |ui| {
                if ui.exit_code() != 0 {
                    return Ok(());
                }
                if let Some(state) = startup.as_mut() {
                    let text = store::setup_text(ui.setup_model());
                    if power_request.is_some() {
                        state.power_off(text, started.elapsed())
                    } else {
                        state.clean(text, started.elapsed(), false)
                    }
                } else {
                    Ok(())
                }
            },
            |deadline| graphics.shutdown(deadline),
            || outputs.release(),
        );
        inp.set_leds(0);
        let code = super::shutdown::finish(
            result,
            ui.exit_code(),
            power_request.take().map(|action| move || power(inp, action)),
        );
        if code != 0 {
            std::process::exit(code);
        }
    }
}
