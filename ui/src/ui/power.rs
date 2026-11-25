// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::bootcard::{CardState, Mode, Status};
use oos_hal::update::JUDGED_PATH;

pub(super) const SHOWN_PATH: &str = "/tmp/oos-sd.shown";
// The watcher can wait 180 s for its verdict, then still has to finish the persist arm.
const RESTART_WAIT_S: f64 = 190.0;
const WAITING: &str = "The unit is still completing its start.";
const MAY_BOOT_STOCK: &str = "The stock system may start after a restart.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Judgment {
    Armed,
    OneShot,
    StockPossible,
}

impl Judgment {
    fn read(host: &host::Host) -> Option<Self> {
        match host.text(JUDGED_PATH)?.trim() {
            "persist" => Some(Self::Armed),
            "oneshot" => Some(Self::OneShot),
            "stock" | "kexec" | "ui-off" => Some(Self::StockPossible),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(super) struct BootPower {
    judgment: Option<Judgment>,
    warning: Option<Warning>,
    shown: bool,
    later: bool,
    pressed: Option<usize>,
}

impl BootPower {
    pub(super) fn deferred(&self) -> bool {
        self.later
    }
}

struct Warning {
    lines: Vec<String>,
    action: PowerAction,
}

impl Warning {
    fn new(sd: Status, one_shot: bool) -> Self {
        let observed = match sd.mode() {
            Mode::Legacy | Mode::Sdr12 => "default speed (25 MHz)",
            Mode::Hs => "high speed (50 MHz)",
            mode => mode.label(),
        };
        let (action, advise) = if sd.degraded_in_a_row() >= 3 {
            (PowerAction::Off, "The card has been slow for three starts in a row. Power off and remove power for 10 s. If it stays slow, select HS in System.")
        } else if one_shot {
            (PowerAction::Off, "Power off and remove power for 10 s before starting again.")
        } else {
            (PowerAction::Restart, "Restart now? If that does not help, power off and remove power for 10 s.")
        };
        let expected = sd.expected().label();
        let text = format!(
            "The card runs in {observed}, expected {expected}. Storage is about three times slower; performance numbers from this start are not representative. {advise}"
        );
        Self { lines: super::update::wrap_lines(&text, 58), action }
    }
}

fn slow_card(ui: &Ui) -> Option<Status> {
    let sd = ui.host.runtime().sd_card;
    (!sd.is_kexec() && matches!(sd.state(), CardState::Degraded | CardState::Worse)).then_some(sd)
}

pub(super) fn open(ui: &mut Ui) {
    super::update::boot_card::cancel_confirmation(ui);
    ui.close_menu();
    ui.dialog = None;
    ui.power_dlg = true;
    refresh(ui);
}

pub(crate) fn refresh(ui: &mut Ui) -> bool {
    if ui.upd.power.judgment.is_none() {
        ui.upd.power.judgment = Judgment::read(&ui.host);
    }
    ui.upd.power.judgment.is_some()
}

pub(crate) fn restore_warning_marker(ui: &mut Ui) {
    if let Some(text) = ui.host.text(SHOWN_PATH) {
        ui.upd.power.shown = true;
        ui.upd.power.later = text.trim() == "later";
    }
}

pub(super) fn poll(ui: &mut Ui) {
    if ui.host.services() {
        poll_with_busy(ui, Ui::update_busy);
    }
}

fn poll_with_busy(ui: &mut Ui, update_busy: impl FnOnce(&Ui) -> bool) {
    let candidate = slow_card(ui).filter(|_| !ui.upd.power.shown);
    let (Some(sd), Some(judgment @ (Judgment::Armed | Judgment::OneShot))) = (candidate, ui.upd.power.judgment) else {
        return;
    };
    if ui.start_up_active()
        || ui.power_dlg
        || ui.dialog.is_some()
        || ui.menu_open
        || ui.meas_cfg.is_some()
        || ui.workspace.popup.is_some()
        || ui.wlan.osk.is_some()
        || ui.touch.down
        || update_busy(ui)
    {
        return;
    }
    ui.upd.power.shown = true;
    if ui.host.sd_shown(false) {
        let one_shot = ui.update_state().map_or(judgment == Judgment::OneShot, |state| !state.persist);
        ui.upd.power.warning = Some(Warning::new(sd, one_shot));
    }
}

fn restart_hint(ui: &Ui, elapsed: f64, gated: bool) -> Option<&'static str> {
    match ui.upd.power.judgment {
        Some(Judgment::Armed | Judgment::OneShot) => None,
        Some(Judgment::StockPossible) => Some(MAY_BOOT_STOCK),
        None if !gated => None,
        None if elapsed < RESTART_WAIT_S => Some(WAITING),
        None => Some(MAY_BOOT_STOCK),
    }
}

fn hint(ui: &Ui) -> Option<&'static str> {
    restart_hint(ui, ui.host.runtime().started.elapsed().as_secs_f64(), ui.host.services())
}

pub(super) fn layout(ui: &Ui) -> (Rect, [Rect; 3]) {
    let b = ui.scr.rect().center(500, 200);
    let btn = |dx: i32| Rect::new(b.x + 250 + dx, b.y + 90, 140, 64);
    (b, [btn(-230), btn(-70), btn(90)])
}

pub(super) fn draw(ui: &Ui, cv: &mut Canvas) {
    if let Some(warning) = &ui.upd.power.warning {
        panel::draw(ui, cv, warning);
        return;
    }
    if !ui.power_dlg {
        return;
    }
    let f = ui.fonts();
    let (r, [off, restart, cancel]) = layout(ui);
    let hint = hint(ui);
    draw_box(ui, cv, r, "Power");
    cv.text_in(
        &f.m,
        Rect::new(r.x, r.y + 52, r.w, 24),
        1,
        hint.unwrap_or("Power key again or tap outside to cancel"),
        DIM,
    );
    draw_button(ui, cv, off, "Power off", RED);
    if hint != Some(WAITING) {
        draw_button(ui, cv, restart, "Restart", ACCENT);
    }
    draw_button(ui, cv, cancel, "Cancel", TEXT);
}

pub(super) fn draw_box(ui: &Ui, cv: &mut Canvas, r: Rect, title: &str) {
    cv.blend(ui.scr.rect(), rgb(0, 0, 0), 160);
    cv.framed(r, 8, 2, ACCENT, 7, DLG_BG);
    cv.text_in(&ui.fonts().lb, Rect::new(r.x, r.y + 18, r.w, 28), 1, title, ACCENT);
}

pub(super) fn draw_button(ui: &Ui, cv: &mut Canvas, r: Rect, label: &str, color: Rgb) {
    btn(cv, r, mix(DLG_BG, color, 40), color);
    cv.text_in(&ui.fonts().mb, r, 1, label, color);
}

pub(super) fn tap(ui: &mut Ui, x: i32, y: i32) {
    tap_with_hint(ui, x, y, hint(ui));
}

fn tap_with_hint(ui: &mut Ui, x: i32, y: i32, hint: Option<&str>) {
    let (_, [off, restart, _]) = layout(ui);
    if restart.contains(x, y) && hint == Some(WAITING) {
        return;
    }
    ui.power_dlg = false;
    if off.contains(x, y) {
        act(ui, PowerAction::Off);
    } else if restart.contains(x, y) {
        act(ui, PowerAction::Restart);
    }
}

fn act(ui: &mut Ui, action: PowerAction) {
    if ui.update_busy() {
        // A cut while the card is being written can leave recovery half-written.
        ui.toast("Update running: wait until it is done");
    } else {
        ui.power_action = Some(action);
        ui.toast(match action {
            PowerAction::Off => "Powering off ...",
            PowerAction::Restart => "Restarting ...",
        });
    }
}

mod panel;
pub(crate) use panel::{defer, handle_warning};

#[cfg(test)]
mod tests;
