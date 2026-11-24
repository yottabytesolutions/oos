// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::{ClockMode, FlipDivisor, FrameClock, Picture};
use std::time::Duration;

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
            shot: None,
            cpu_hdmi: false,
        });
        graphics.monitor = helper_monitor.then(|| TargetRender { ticket: gpu::Ticket::fixture(now), back: 0 });
        if panel_ready {
            panel.stage(0, picture(now));
        }
        assert_eq!(graphics.available(&panel), !(helper_panel || helper_monitor || panel_ready));
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
            shot: None,
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
        assert!(graphics.available(&out.panel));

        let mut canvas = Canvas::new(ui.scr.w, ui.scr.h);
        let mut web = http::Http::open(net6::Port::Host, &switches);
        assert!(!graphics.frame(&mut ui, &mut canvas, &mut out, &mut web, &switches, picture(now)));
        assert!(canvas.px.iter().any(|&pixel| pixel != 0));
        assert!(out.panel.wait_until().is_some());
    }
}
