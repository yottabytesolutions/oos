// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::scpi::Scpi;
use oos_hal::bootcard;

const DEGRADED: &str =
    "mode=hs\nexpected=sdr104\nstate=degraded\nreason=none\nceiling=sdr104\ndegraded_in_a_row=1\nkexec=0\n";

fn status(text: &str) -> Status {
    let file = std::env::temp_dir().join(format!("oos-power-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::write(&file, text).unwrap();
    let status = bootcard::read_state(&file);
    std::fs::remove_file(file).unwrap();
    status
}

fn fixture(text: fn(&str) -> Option<String>) -> Ui {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::fixture_with_text(text));
    ui.host.runtime_mut().sd_card = status(DEGRADED);
    ui
}

fn armed(path: &str) -> Option<String> {
    (path == JUDGED_PATH).then(|| "persist".into())
}

fn one_shot(path: &str) -> Option<String> {
    (path == JUDGED_PATH).then(|| "oneshot".into())
}

fn poll_fixture(ui: &mut Ui) {
    refresh(ui);
    poll_with_busy(ui, Ui::update_busy);
}

#[test]
fn slow_warning_waits_for_every_gate_and_shows_only_once() {
    type Change = fn(&mut Ui);
    let gates: [(Change, Change); 8] = [
        (|u: &mut Ui| u.host.set_text_reader(|_| None), |u: &mut Ui| u.host.set_text_reader(armed)),
        (|u: &mut Ui| u.start_up_begin(None), |u: &mut Ui| u.start_up = None),
        (|u: &mut Ui| u.menu_open = true, |u: &mut Ui| u.menu_open = false),
        (|u: &mut Ui| u.dialog = Some(vec![]), |u: &mut Ui| u.dialog = None),
        (|u: &mut Ui| u.power_dlg = true, |u: &mut Ui| u.power_dlg = false),
        (|u: &mut Ui| u.meas_cfg = Some(0), |u: &mut Ui| u.meas_cfg = None),
        (|u: &mut Ui| u.wlan.osk = Some(wlan::Osk::for_label(0, "")), |u: &mut Ui| u.wlan.osk = None),
        (|u: &mut Ui| u.touch.down = true, |u: &mut Ui| u.touch.down = false),
    ];
    for (block, release) in gates {
        let mut ui = fixture(armed);
        block(&mut ui);
        poll_fixture(&mut ui);
        assert!(ui.upd.power.warning.is_none());
        release(&mut ui);
        poll_fixture(&mut ui);
        assert!(ui.upd.power.warning.is_some());
        ui.upd.power.warning = None;
        poll_fixture(&mut ui);
        assert!(ui.upd.power.warning.is_none());
    }
    let mut ui = fixture(armed);
    refresh(&mut ui);
    poll_with_busy(&mut ui, |_| true);
    assert!(ui.upd.power.warning.is_none());
    poll_with_busy(&mut ui, |_| false);
    assert!(ui.upd.power.warning.is_some());
}

#[test]
fn slow_warning_skips_kexec_unlearned_healthy_unknown_and_previously_shown_starts() {
    for (field, replacement) in [
        ("kexec=0", "kexec=1"),
        ("state=degraded", "state=unverified"),
        ("state=degraded", "state=ok"),
        ("state=degraded", "state=unknown"),
    ] {
        let mut ui = fixture(armed);
        ui.host.runtime_mut().sd_card = status(&DEGRADED.replace(field, replacement));
        poll_fixture(&mut ui);
        assert!(ui.upd.power.warning.is_none());
    }
    let mut ui = fixture(|path| if path == SHOWN_PATH { Some("later\n".into()) } else { armed(path) });
    restore_warning_marker(&mut ui);
    poll_fixture(&mut ui);
    assert!(ui.upd.power.warning.is_none());
    assert!(ui.upd.power.deferred());
    let mut ui = fixture(|path| (path == JUDGED_PATH).then(|| "stock".into()));
    poll_fixture(&mut ui);
    assert!(ui.upd.power.warning.is_none());
}

#[test]
fn warning_text_and_action_follow_speed_streak_and_boot_mode() {
    let sd = status(DEGRADED);
    let warning = Warning::new(sd, false);
    assert_eq!(warning.action, PowerAction::Restart);
    assert_eq!(warning.lines.join(" "), "The card runs in high speed (50 MHz), expected SDR104. Storage is about three times slower; performance numbers from this start are not representative. Restart now? If that does not help, power off and remove power for 10 s.");
    let warning = Warning::new(sd, true);
    assert_eq!(warning.action, PowerAction::Off);
    assert_eq!(warning.lines.join(" "), "The card runs in high speed (50 MHz), expected SDR104. Storage is about three times slower; performance numbers from this start are not representative. Power off and remove power for 10 s before starting again.");
    let sd = status(&DEGRADED.replace("mode=hs", "mode=legacy").replace("state=degraded", "state=worse"));
    assert!(Warning::new(sd, false).lines.join(" ").contains("default speed (25 MHz), expected SDR104."));
    let sd = status(&DEGRADED.replace("a_row=1", "a_row=3"));
    for one_shot in [false, true] {
        let warning = Warning::new(sd, one_shot);
        assert_eq!(warning.action, PowerAction::Off);
        assert!(warning.lines.join(" ").ends_with(
            "three starts in a row. Power off and remove power for 10 s. If it stays slow, set sd.mode = hs."
        ));
    }
}

fn touch(ui: &mut Ui, button: usize, phase: Phase) {
    let r = panel::layout(ui, ui.upd.power.warning.as_ref().unwrap()).1[button];
    ui.handle(Ev::Touch { id: 0, phase, x: r.x + 5, y: r.y + 5 });
}

#[test]
fn warning_preserves_scpi_and_acquisition_and_later_leaves_a_system_note() {
    let mut ui = fixture(armed);
    poll_fixture(&mut ui);
    let screen = Canvas::new(ui.scr.w, ui.scr.h);
    let response = Scpi::disconnected().exec_line(&mut ui, &screen, ":SYSTem:STATe?");
    assert!(String::from_utf8(response).unwrap().contains("sd.degraded_in_a_row=1"));
    Scpi::disconnected().exec_line(&mut ui, &screen, "*RST");
    assert!(ui.upd.power.warning.is_some());
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run);
    assert!(ui.last[0].is_some());
    touch(&mut ui, 0, Phase::Up);
    assert!(ui.power_action.is_none());
    touch(&mut ui, 1, Phase::Down);
    touch(&mut ui, 1, Phase::Up);
    assert!(ui.power_action.is_none());
    assert!(ui.upd.power.warning.is_none());
    assert_system_note(&mut ui);
}

fn assert_system_note(ui: &mut Ui) {
    assert!(ui
        .sysinfo_rows()
        .iter()
        .flatten()
        .any(|row| matches!(row, sysinfo::Row::Note(text, _) if text.contains("SD card is in slow mode"))));
}

#[test]
fn warning_draws_over_gpu_traces_and_on_the_cpu_canvas() {
    let mut ui = fixture(armed);
    let mut cpu = Canvas::new(ui.scr.w, ui.scr.h);
    let mut under = Canvas::new(ui.scr.w, ui.scr.h);
    let mut over = Canvas::new_layer(ui.scr.w, ui.scr.h);
    ui.render(&mut cpu);
    ui.render_layers(&mut Layers { under: &mut under, over: Some(&mut over), jobs: Vec::new() });
    let (plain_cpu, plain_under, plain_over) = (cpu.px.clone(), under.px.clone(), over.px.clone());

    poll_fixture(&mut ui);
    ui.render(&mut cpu);
    ui.render_layers(&mut Layers { under: &mut under, over: Some(&mut over), jobs: Vec::new() });
    assert_ne!(cpu.px, plain_cpu);
    assert_eq!(under.px, plain_under);
    assert_ne!(over.px, plain_over);
}

#[test]
fn next_due_frame_draws_the_warning_while_acquisition_is_stopped() {
    use crate::pacing::{ClockMode, Flip, FlipDivisor, FrameClock};
    use std::time::{Duration, Instant};

    let mut ui = fixture(|_| None);
    ui.host.enable_fixture_services();
    ui.m.run = RunState::Stop;
    ui.step(0.0);
    let acquired = ui.wfm_total;
    let mut under = Canvas::new(ui.scr.w, ui.scr.h);
    let mut over = Canvas::new_layer(ui.scr.w, ui.scr.h);
    ui.render_layers(&mut Layers { under: &mut under, over: Some(&mut over), jobs: Vec::new() });
    let plain_corner = over.px[0];

    ui.host.set_text_reader(armed);
    assert!(refresh(&mut ui)); // Judgment notification, with no panel input or acquisition event.
    let start = Instant::now();
    let refresh_period = Duration::from_micros(31_250);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, start, refresh_period);
    for frame in 1..=8 {
        let at = start + clock.period() * frame;
        let elapsed = clock.advance(at, Some(Flip { sequence: frame * 2, at })).0.unwrap();
        ui.step(elapsed.as_secs_f64());
        assert_eq!(ui.upd.power.warning.is_some(), frame == 8);
    }
    ui.render_layers(&mut Layers { under: &mut under, over: Some(&mut over), jobs: Vec::new() });
    assert_ne!(over.px[0], plain_corner);
    assert_eq!((ui.m.run, ui.wfm_total), (RunState::Stop, acquired));
}

#[test]
fn warning_defers_to_front_panel_keys_and_knobs() {
    let mut ui = fixture(armed);
    poll_fixture(&mut ui);
    ui.handle(Ev::Key(0));
    assert!(ui.upd.power.warning.is_some());
    ui.handle(Ev::Key(key::RUNSTOP));
    assert_eq!(ui.m.run, RunState::Stop);
    assert_system_note(&mut ui);

    let mut ui = fixture(armed);
    poll_fixture(&mut ui);
    let before = ui.m.time_per_div();
    ui.handle(Ev::Key(key::HSCALE_CW));
    assert_ne!(ui.m.time_per_div(), before);
    assert!(ui.upd.power.warning.is_none());
    assert_system_note(&mut ui);

    let mut ui = fixture(armed);
    poll_fixture(&mut ui);
    ui.handle(Ev::Key(key::TMENU));
    assert_eq!(ui.menu, MenuId::Trigger);
    assert!(ui.menu_open);
    assert!(ui.upd.power.warning.is_none());
    assert_system_note(&mut ui);
}

fn remote_command(ui: &mut Ui, line: &str) {
    assert!(crate::cmd::remote_ok(line));
    assert!(matches!(crate::cmd::run(ui, line), crate::cmd::Out::None));
}

#[test]
fn web_view_touches_the_warning_and_keys_and_knobs_defer_it() {
    let mut ui = fixture(armed);
    poll_fixture(&mut ui);
    let run = ui.run_pill();
    let tap_run = format!("tap {} {}", run.x + run.w / 2, run.y + run.h / 2);
    remote_command(&mut ui, &tap_run);
    assert_eq!(ui.m.run, RunState::Run);
    assert!(ui.upd.power.warning.is_some());

    let [act, later] = panel::layout(&ui, ui.upd.power.warning.as_ref().unwrap()).1;
    remote_command(&mut ui, &format!("drag {} {} {} {}", later.x + 5, later.y + 5, act.x + 5, act.y + 5));
    assert!(ui.upd.power.warning.is_some());
    assert!(ui.power_action.is_none());
    remote_command(&mut ui, &format!("tap {} {}", later.x + 5, later.y + 5));
    assert!(ui.upd.power.warning.is_none());
    assert_system_note(&mut ui);
    remote_command(&mut ui, &tap_run);
    assert_eq!(ui.m.run, RunState::Stop);

    for line in ["key runstop", "knob hscale +1", "workflow settings"] {
        let mut ui = fixture(armed);
        poll_fixture(&mut ui);
        let timebase = ui.m.time_per_div();
        remote_command(&mut ui, line);
        assert!(ui.upd.power.warning.is_none());
        assert_system_note(&mut ui);
        match line {
            "key runstop" => assert_eq!(ui.m.run, RunState::Stop),
            "knob hscale +1" => assert_ne!(ui.m.time_per_div(), timebase),
            _ => assert!(ui.menu_open),
        }
    }
}

#[test]
fn a_deliberate_ui_stop_releases_the_power_restart_gate() {
    let mut ui = fixture(|path| (path == JUDGED_PATH).then(|| "ui-off".into()));
    poll_fixture(&mut ui);
    assert!(ui.upd.power.warning.is_none());
    assert_eq!(restart_hint(&ui, 0.0, true), Some(MAY_BOOT_STOCK));
}

#[test]
fn warning_power_buttons_use_the_existing_actions() {
    type Marker = fn(&str) -> Option<String>;
    let cases: [(Marker, u32, PowerAction); 3] =
        [(armed, 1, PowerAction::Restart), (one_shot, 1, PowerAction::Off), (armed, 3, PowerAction::Off)];
    for (marker, streak, action) in cases {
        let mut ui = fixture(marker);
        ui.host.runtime_mut().sd_card = status(&DEGRADED.replace("a_row=1", &format!("a_row={streak}")));
        poll_fixture(&mut ui);
        touch(&mut ui, 0, Phase::Down);
        assert!(ui.power_action.is_none());
        touch(&mut ui, 0, Phase::Up);
        assert_eq!(ui.power_action, Some(action));
    }
}

#[test]
fn menu_power_restart_waits_for_marker_but_has_a_bounded_fallback() {
    for (marker, elapsed, expected, action) in [
        (None, 0.0, Some(WAITING), None),
        (None, RESTART_WAIT_S - 0.01, Some(WAITING), None),
        (None, RESTART_WAIT_S, Some(MAY_BOOT_STOCK), Some(PowerAction::Restart)),
        (Some(Judgment::Armed), 0.0, None, Some(PowerAction::Restart)),
        (Some(Judgment::OneShot), 0.0, None, Some(PowerAction::Restart)),
    ] {
        let mut ui = fixture(if marker == Some(Judgment::Armed) { armed } else { |_| None });
        ui.activate(Item::PowerAct);
        if marker == Some(Judgment::OneShot) {
            ui.host.set_text_reader(one_shot);
            refresh(&mut ui);
        }
        assert!(ui.power_dlg);
        assert_eq!(ui.upd.power.judgment, marker);
        let hint = restart_hint(&ui, elapsed, true);
        assert_eq!(hint, expected);
        let restart = layout(&ui).1[1];
        tap_with_hint(&mut ui, restart.x + 5, restart.y + 5, hint);
        assert_eq!(ui.power_action, action);
        assert_eq!(ui.power_dlg, action.is_none());
    }
}
