// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) fn layout(ui: &Ui, warning: &Warning) -> (Rect, [Rect; 3]) {
    let width =
        (warning.lines.iter().map(|line| ui.fonts().m.width(line)).max().unwrap_or(0) + 48).clamp(560, ui.scr.w - 20);
    let height = 155 + warning.lines.len() as i32 * 26;
    let r = ui.scr.rect().center(width, height);
    let button = |dx| Rect::new(r.x + r.w / 2 + dx, r.y + r.h - 84, 172, 64);
    (r, [button(-270), button(-86), button(98)])
}

pub(super) fn draw(ui: &Ui, cv: &mut Canvas, warning: &Warning) {
    let (r, [act, card_mode, later]) = layout(ui, warning);
    draw_box(ui, cv, r, "SD card is in slow mode.");
    for (i, line) in warning.lines.iter().enumerate() {
        cv.text_in(&ui.fonts().m, Rect::new(r.x, r.y + 52 + i as i32 * 26, r.w, 26), 1, line, TEXT);
    }
    let (label, color) = match warning.action {
        PowerAction::Off => ("Power off", RED),
        PowerAction::Restart => ("Restart", ACCENT),
    };
    draw_button(ui, cv, act, label, color);
    draw_button(ui, cv, card_mode, "Open System", ACCENT);
    draw_button(ui, cv, later, "Later", TEXT);
}

pub(crate) fn handle_warning(ui: &mut Ui, event: &Ev) -> bool {
    let Some(warning) = &ui.upd.power.warning else { return false };
    if matches!(event, Ev::Key(_) | Ev::Wheel { .. }) {
        // The panel sends a zero code on key release.
        if !matches!(event, Ev::Key(0)) {
            defer(ui);
        }
        return false;
    }
    if let Ev::Touch { id: 0, phase, x, y } = *event {
        let hit = layout(ui, warning).1.iter().position(|r| r.contains(x, y));
        match phase {
            Phase::Down => ui.upd.power.pressed = hit,
            Phase::Move => {}
            Phase::Up => match ui.upd.power.pressed.take().filter(|pressed| Some(*pressed) == hit) {
                Some(0) => {
                    let action = warning.action;
                    ui.upd.power.warning = None;
                    act(ui, action);
                }
                Some(1) => {
                    defer(ui);
                    ui.open_menu(MenuId::Utility);
                    if let Some(index) = ui.menu_items(MenuId::Utility).iter().position(|it| *it == Item::BootCardMode)
                    {
                        ui.focus = index;
                    }
                }
                Some(2) => defer(ui),
                _ => {}
            },
        }
    }
    true
}

pub(crate) fn defer(ui: &mut Ui) {
    if ui.upd.power.warning.take().is_none() {
        return;
    }
    ui.upd.power.pressed = None;
    ui.upd.power.later = true;
    if !ui.host.sd_shown(true) {
        eprintln!("oos-ui: could not save the deferred SD card warning");
    }
}
