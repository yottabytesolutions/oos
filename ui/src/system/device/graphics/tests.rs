// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::{ClockMode, FlipDivisor, FrameClock, FrameSlot, Picture};
use std::{io::Write, os::fd::AsRawFd, os::unix::net::UnixStream, time::Duration};

fn picture(now: Instant) -> Picture {
    FrameClock::new(ClockMode::Flip, FlipDivisor::Three, now, Duration::from_millis(10)).seal()
}

#[test]
fn available_requires_both_helper_targets_and_panel_staging_to_be_free() {
    let ui = Ui::for_tests(wave::Source::Demo { t: 0.0 });
    let now = Instant::now();
    for (helper_panel, helper_monitor, panel_ready) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (true, true, false),
        (false, false, true),
        (true, true, true),
    ] {
        let mut graphics = Graphics::new(&ui);
        let mut panel = pacing::Panel::default();
        graphics.panel = helper_panel.then(|| PanelRender {
            ticket: gpu::Ticket::fixture(now),
            back: 0,
            picture: picture(now),
            pixels: Pixels::default(),
            cpu_hdmi: false,
        });
        graphics.monitor = helper_monitor.then(|| TargetRender { ticket: gpu::Ticket::fixture(now), back: 0 });
        if panel_ready {
            panel.stage(0, picture(now), now);
        }
        assert_eq!(graphics.slot(&panel) == FrameSlot::Ready, !(helper_panel || helper_monitor || panel_ready));
    }
}

#[test]
fn helper_failure_and_switching_to_cpu_release_both_targets_for_the_next_frame() {
    for helper_failed in [false, true] {
        let mut ui = Ui::for_tests(wave::Source::Demo { t: 0.0 });
        let mut out = Outputs::new(drm::Display::scanout_fixture(-1), false);
        let mut graphics = Graphics::new(&ui);
        let now = graphics.started;
        graphics.want = true;
        graphics.panel = Some(PanelRender {
            ticket: gpu::Ticket::fixture(now),
            back: 0,
            picture: picture(now),
            pixels: Pixels::default(),
            cpu_hdmi: false,
        });
        graphics.monitor = Some(TargetRender { ticket: gpu::Ticket::fixture(now), back: 0 });
        ui.sys.gpu = helper_failed;
        let switches = ui.switches().clone();
        if helper_failed {
            graphics.failed(&mut ui, "helper disconnected".into());
        } else {
            graphics.select(&mut ui, &out, &switches, now);
        }
        assert!(graphics.slot(&out.panel) == FrameSlot::Ready);

        let mut canvas = Canvas::new(ui.scr.w, ui.scr.h);
        let mut web = http::Http::open(net6::Port::Host, &switches);
        assert!(!graphics.frame(
            &mut ui,
            &mut canvas,
            &mut out,
            (&mut web, &readback::Readback::default()),
            &switches,
            picture(now)
        ));
        assert!(canvas.px.iter().any(|&pixel| pixel != 0));
        assert!(out.panel.wait_until().is_some());
    }
}

struct Script {
    ui: Ui,
    graphics: Graphics,
    canvas: Canvas,
    web: http::Http,
    readback: readback::Readback,
    out: Outputs,
    timers: Timers,
    helper: gpu::HelperFixture,
    drm: (UnixStream, UnixStream),
    refresh: Duration,
    start: Instant,
}

impl Script {
    fn new() -> Self {
        let mut ui = Ui::for_tests(wave::Source::Demo { t: 0.0 });
        let mut graphics = Graphics::new(&ui);
        ui.sys.gpu = true;
        graphics.want = true;
        let canvas = Canvas::new(ui.scr.w, ui.scr.h);
        let (gpu, helper) = gpu::HelperFixture::connect(canvas.w, canvas.h);
        graphics.gpu = Some(gpu);
        let drm = UnixStream::pair().unwrap();
        let out = Outputs::new(drm::Display::scanout_fixture(drm.0.as_raw_fd()), false);
        let refresh = out.disp.refresh_period();
        let web = http::Http::open(net6::Port::Host, ui.switches());
        let timers = Timers::new(None);
        let start = Instant::now();
        Self { ui, graphics, canvas, web, readback: Default::default(), out, timers, helper, drm, refresh, start }
    }

    fn render(&mut self, at: Instant) -> Option<usize> {
        assert_eq!(self.graphics.slot(&self.out.panel), FrameSlot::Ready);
        let picture = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, at, self.refresh).seal();
        let switches = self.ui.switches().clone();
        assert!(self.graphics.frame(
            &mut self.ui,
            &mut self.canvas,
            &mut self.out,
            (&mut self.web, &self.readback),
            &switches,
            picture,
        ));
        self.out.frame_n += 1;
        self.helper.frame()
    }

    fn service(&mut self, at: Instant) {
        self.graphics.service(&mut self.ui, &mut self.canvas, &mut self.out, &mut self.web, &mut self.timers, || at);
    }

    fn queue_for_target(&mut self, seal: Instant) {
        assert!(!self.out.panel.ready_for_picture());
        let request_at = seal + self.refresh * 2 - self.refresh / 2;
        self.out.service_panel(request_at, None);
        assert!(self.out.panel.ready_for_picture(), "flip must queue in time for the target");
        assert!(self.out.disp.poll_flip_fd(false, request_at).is_some());
    }

    fn flip_at_target(&mut self, seal: Instant, cookie: u32) {
        let event = [2u32, 32, cookie, 0, 0, 0, 3 * cookie + 2, 0].map(u32::to_ne_bytes);
        self.drm.1.write_all(event.as_flattened()).unwrap();
        let mut flip = self.out.read_panel_event().expect("the target flip was queued");
        flip.at = seal + self.refresh * 2;
        self.out.service_panel(flip.at, Some(flip));
    }
}

#[test]
fn every_gpu_submission_without_pixel_demand_has_readback_off() {
    let mut script = Script::new();
    for index in 0..32 {
        let seal = script.start + script.refresh * (3 * index);
        assert_eq!(script.render(seal), None, "picture {index}");
        script.helper.rendered();
        script.service(seal + script.refresh);
        script.queue_for_target(seal);
        script.flip_at_target(seal, index + 1);
    }
}

#[test]
fn pixel_reply_waits_for_scanout_and_precedes_the_next_render_reply() {
    let mut script = Script::new();
    let mut request = script.readback.request();
    let seal = script.start;
    let offset = script.render(seal).expect("a waiting reader enables readback");
    script.helper.rendered();
    script.helper.pixels(offset, 0x123456);
    script.service(seal + script.refresh);
    assert!(!request.ready(&script.readback));
    assert_eq!(script.canvas.px[0], 0);
    assert!(script.graphics.reply_fd(&script.out.panel).is_none());
    script.queue_for_target(seal);
    script.flip_at_target(seal, 1);
    assert!(!request.ready(&script.readback));

    let next = seal + script.refresh * 3;
    assert_eq!(script.render(next), None);
    script.helper.rendered();
    script.service(next + script.refresh);
    assert_eq!(request.pixels(&script.readback).unwrap().px[0], 0x123456);
    script.queue_for_target(next);
    script.flip_at_target(next, 2);
    assert!(script.graphics.gpu.is_some(), "the next render reply must decode as a render");
    assert!(cmd::stat_text(&script.ui).contains("last_read 11.0 ms"));
}

#[test]
fn staged_panel_suppresses_the_read_timeout_until_it_accepts_the_picture() {
    let mut script = Script::new();
    let mut request = script.readback.request();
    let seal = script.start;
    assert!(script.render(seal).is_some());
    script.helper.rendered();
    script.service(seal + script.refresh);
    let ticket = &script.graphics.read.as_ref().unwrap().ticket;
    let (read_deadline, end) = (ticket.deadline(), ticket.submitted() + Duration::from_secs(3));
    let mut poll_at = script.out.panel.wait_until().unwrap();
    while poll_at <= end {
        script.out.panel.service(poll_at, None, None, script.refresh, |_, _| pacing::presentation::FlipRequest::Busy);
        assert!(script.graphics.reply_fd(&script.out.panel).is_none());
        assert_eq!(script.graphics.deadline(&script.out.panel), None);
        script.service(poll_at);
        assert!(script.graphics.gpu.is_some());
        let panel_deadline = script.out.panel.wait_until().unwrap();
        assert_eq!(panel_deadline - poll_at, script.refresh);
        poll_at = panel_deadline;
    }
    script.out.panel.service(poll_at, None, None, script.refresh, |_, _| pacing::presentation::FlipRequest::Modeset);
    assert_eq!(script.graphics.deadline(&script.out.panel), Some(read_deadline));
    script.service(poll_at);
    assert!(script.graphics.gpu.is_none());
    assert!(!request.ready(&script.readback));
    assert!(script.readback.take().is_some(), "the failed read makes its readers eligible for CPU capture");
}
