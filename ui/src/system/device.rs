// SPDX-License-Identifier: GPL-2.0-or-later
//! Scope startup, main intake and ordered process teardown.
mod flip;
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
    started: Instant,
    inp: input::Inputs,
    cv: Canvas,
    remote: remote::Remote,
    outputs: outputs::Outputs,
    timers: timers::Timers,
    frame_clock: pacing::FrameClock,
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
    placement::irq::startup(&sw);
    let inp = startup::prepare(&mut ui, &mut disp, args, &state);
    if let Err(error) = ui::lifecycle::storage::get(&mut ui, state) {
        ui.persistence_status(false);
        eprintln!("oos-ui: storage: {}", error);
    }
    let started = Instant::now();
    let cv = Canvas::new(ui.scr.w, ui.scr.h);
    let remote = remote::Remote::new(args, &sw, &mut ui);
    crate::ui::step_draw::owner::set_render_owner_wake(&mut ui, remote.web.wake_handle());
    let graphics = graphics::Graphics::new(&ui);
    let refresh = disp.refresh_period();
    let now = Instant::now();
    let frame_clock = pacing::FrameClock::new(sw.perf.pacing_clock, sw.perf.flip_divisor, now, refresh);
    pacing::publish_deadline(&frame_clock, now);
    let mut device = Device {
        ui,
        sw,
        started,
        inp,
        cv,
        remote,
        graphics,
        frame_clock,
        outputs: outputs::Outputs::new(disp, !args.iter().any(|a| a == "--no-hdmi")),
        timers: timers::Timers::new(arg(args, "--start-up")),
        power_request: None,
        buffers_released: false,
    };
    while !device.ui.quit && !input::stop_requested() {
        let vblank = device.intake();
        if device.ui.quit || input::stop_requested() {
            break;
        }
        device.frame(vblank);
        if device.power_request.is_some() {
            break;
        }
    }
    device.shutdown();
}

impl Device {
    fn available(&self) -> bool {
        self.graphics.available(&self.outputs.panel)
    }

    fn intake(&mut self) -> Option<pacing::Flip> {
        self.timers.startup_status(&mut self.ui);
        let now = Instant::now();
        let clock = self.sw.perf.pacing_clock == pacing::ClockMode::Flip;
        let display_fds = [self.outputs.disp.poll_flip_fd(clock, now), self.graphics.reply_fd()];
        let deadline = [
            self.outputs.vblank.map(|_| now),
            self.available().then(|| self.frame_clock.wait_until()),
            self.graphics.deadline(),
            self.outputs.panel.wait_until(),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(now);
        let wait_ms = deadline.saturating_duration_since(now).as_nanos().div_ceil(1_000_000) as i32;
        let input = self.inp.poll(wait_ms, self.remote.wake(), display_fds);
        let flipped = self.outputs.read_panel_event();
        self.outputs.service_panel(Instant::now(), flipped);
        self.service_graphics();
        let vblank = self.outputs.vblank.take();
        if vblank.is_some() {
            self.outputs.disp.poll_flip_fd(clock, Instant::now());
        }
        for line_or_ev in input {
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
            return None;
        }
        let remote_started = Instant::now();
        self.remote.poll(&mut self.ui, &self.cv, &self.sw);
        pacing::note_remote_intake(remote_started.elapsed());
        crate::ui::step_draw::owner::intake_render_owner(&mut self.ui);
        if self.available() {
            self.outputs.poll(&mut self.ui, &mut self.graphics);
        }
        vblank
    }

    fn service_graphics(&mut self) {
        self.graphics.service(&mut self.ui, &mut self.cv, &mut self.outputs, &mut self.remote.web, &mut self.timers);
    }

    fn frame(&mut self, vblank: Option<pacing::Flip>) {
        let t0 = Instant::now();
        let (elapsed, fell_back) = self.frame_clock.advance_available(t0, vblank, self.available());
        if fell_back {
            pacing::note_flip_fallback();
        }
        pacing::publish_deadline(&self.frame_clock, t0);
        let Some(elapsed) = elapsed else { return };
        self.timers.picture(&mut self.ui, t0);
        let mut picture = self.frame_clock.seal();
        pacing::begin_picture();
        self.ui.step(elapsed.as_secs_f64().min(0.2));
        let t1 = Instant::now();
        picture.composed(t1);
        crate::ui::step_draw::capture::sample(&mut self.ui, t1);
        let stopped = self.ui.memory_stopped();
        if stopped && !self.buffers_released {
            self.remote.release_buffers();
        }
        self.buffers_released = stopped;
        if self.ui.quit {
            return;
        }
        self.ui.set_hdmi_present(self.outputs.hdmi.is_some());
        let hdmi_frame = self.outputs.hdmi.is_some() && (self.outputs.frame_n + 1).is_multiple_of(2);
        self.ui.hdmi_snap_want = hdmi_frame && self.ui.m.hdmi == model::HdmiMode::Waveform;
        let drawn =
            self.graphics.frame(&mut self.ui, &mut self.cv, &mut self.outputs, &mut self.remote.web, &self.sw, picture);
        let t2 = Instant::now();
        self.outputs.frame_n += 1;
        if drawn {
            self.service_graphics();
        } else {
            self.outputs.cpu_hdmi(&mut self.ui, &self.cv, hdmi_frame);
            let shot = self.ui.shot_request.take();
            self.timers.shot(&mut self.ui, &self.cv, shot);
        }
        let t3 = Instant::now();
        self.frame_clock.step_finished(t3);
        pacing::pass(t0, t1, t2, t3);
        let a = 0.05;
        self.ui.t_step += a * ((t1 - t0).as_secs_f32() * 1000.0 - self.ui.t_step);
        self.ui.t_render += a * ((t2 - t1).as_secs_f32() * 1000.0 - self.ui.t_render);
        self.ui.t_present += a * ((t3 - t2).as_secs_f32() * 1000.0 - self.ui.t_present);
        self.timers.panel(&mut self.ui, &mut self.inp);
        self.power_request = self.ui.power_action.take();
        if self.power_request.is_some() || self.ui.exit_code() != 0 {
            return;
        }
        let text = store::setup_text(self.ui.setup_model());
        if let Err(e) = ui::lifecycle::storage::get(&mut self.ui, None)
            .and_then(|worker| worker.observe(text, self.started.elapsed()))
        {
            eprintln!("oos-ui: persist: {}", e);
        }
        self.timers.network(&mut self.ui);
    }

    fn shutdown(&mut self) {
        let Self { ui, started, remote, graphics, outputs, inp, power_request, .. } = self;
        let result = super::shutdown::run(
            std::time::Instant::now(),
            ui,
            |deadline| remote.close(deadline),
            |ui, deadline| ui.stop_workers(deadline),
            |ui| ui::lifecycle::storage::finish(ui, started.elapsed(), power_request.is_some()),
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
