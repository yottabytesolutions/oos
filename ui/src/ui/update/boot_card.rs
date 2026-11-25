// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::update::sd::Mode;

pub(super) const JOB: &str = "Restage boot card mode";
pub(in crate::ui) const MODES: [Mode; 2] = [Mode::Sdr104, Mode::Hs];

pub(super) struct Confirmation {
    mode: Mode,
    restage_pressed: bool,
}

pub(in crate::ui) fn requested_index(ui: &Ui) -> Option<usize> {
    let requested = ui.sys.card_mode()?;
    MODES.iter().position(|&mode| mode == requested)
}

pub(in crate::ui) fn refresh(ui: &mut Ui) {
    if let Some(text) = ui.host.text(crate::sysconf::PATH) {
        ui.sys.replace_card_mode(&text);
    }
}

pub(in crate::ui) fn negotiated(ui: &Ui) -> String {
    use oos_hal::bootcard::Mode as Timing;

    let status = ui.host.runtime().sd_card;
    let label = match status.mode() {
        Timing::Hs => "HS",
        Timing::Unknown if !ui.host.services() => "not reported",
        Timing::Unknown => status.mode_unavailable_reason().unwrap_or("No timing reported"),
        mode => mode.label(),
    };
    if ui.upd.job.as_ref().is_some_and(|(_, task)| task == JOB) {
        format!("{label} (restaging ...)")
    } else {
        label.into()
    }
}

fn blocker(ui: &Ui) -> Option<String> {
    if !ui.host.services() {
        return Some("Boot card mode requires the scope".into());
    }
    if ui.update_busy() {
        return Some("An update job is running".into());
    }
    ui.update_state().as_ref().and_then(update::sd::restage_blocker)
}

pub(in crate::ui) fn select(ui: &mut Ui, label: &str) {
    if let Some(mode) = MODES.into_iter().find(|mode| mode.label() == label) {
        request(ui, mode);
    }
}

pub(in crate::ui) fn request(ui: &mut Ui, mode: Mode) {
    if let Some(reason) = blocker(ui) {
        return ui.toast(&reason);
    }
    ui.upd.card_confirmation = Some(Confirmation { mode, restage_pressed: false });
}

fn layout(ui: &Ui) -> (Rect, Rect, Rect) {
    let panel = ui.scr.rect().center(560, 224);
    let button = |offset| Rect::new(panel.x + panel.w / 2 + offset, panel.y + 140, 140, 64);
    (panel, button(-150), button(10))
}

pub(in crate::ui) fn draw_confirmation(ui: &Ui, cv: &mut Canvas) -> bool {
    let Some(confirmation) = &ui.upd.card_confirmation else { return false };
    let (panel, restage, cancel) = layout(ui);
    let title = format!("Set boot card mode to {}?", confirmation.mode.label());
    power::draw_box(ui, cv, panel, &title);
    for (index, line) in
        ["The boot image will be restaged.", "The change takes effect at the next OOS start."].into_iter().enumerate()
    {
        let row = Rect::new(panel.x, panel.y + 60 + index as i32 * 30, panel.w, 28);
        cv.text_in(&ui.fonts().m, row, 1, line, TEXT);
    }
    power::draw_button(ui, cv, restage, "Restage", ACCENT);
    power::draw_button(ui, cv, cancel, "Cancel", TEXT);
    true
}

pub(in crate::ui) fn handle_confirmation(ui: &mut Ui, event: &Ev) -> bool {
    if ui.upd.card_confirmation.is_none() {
        return false;
    }
    if let Ev::Touch { id: 0, phase, x, y } = *event {
        let hit = layout(ui).1.contains(x, y);
        if let Some(confirmation) = &mut ui.upd.card_confirmation {
            match phase {
                Phase::Down => confirmation.restage_pressed = hit,
                Phase::Move => {}
                Phase::Up if confirmation.restage_pressed || !hit => {
                    finish_confirmation(ui, hit);
                }
                Phase::Up => {}
            }
        }
    }
    !matches!(event, Ev::Key(_))
}

pub(in crate::ui) fn cancel_confirmation(ui: &mut Ui) -> bool {
    ui.upd.card_confirmation.take().is_some()
}

fn finish_confirmation(ui: &mut Ui, confirmed: bool) {
    let Some(confirmation) = ui.upd.card_confirmation.take() else { return };
    if confirmed {
        if let Some(reason) = blocker(ui) {
            ui.toast(&reason);
        } else {
            // The updater checks the same blockers again under its exclusive lock.
            ui.update_spawn(&["sd-mode", confirmation.mode.as_str(), "--execute"], JOB);
        }
    }
}

#[cfg(test)]
mod tests;
