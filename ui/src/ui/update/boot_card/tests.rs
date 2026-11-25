// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn panel() -> Ui {
    let mut ui = Ui::new(
        Source::Demo { t: 0.0 },
        host::Host::fixture_with_text(|path| (path == crate::sysconf::PATH).then(|| "sd.mode = sdr104".into())),
    );
    ui.open_menu(MenuId::Utility);
    ui
}

fn touch(ui: &mut Ui, button: Rect, phase: Phase) {
    ui.handle(Ev::Touch { id: 0, phase, x: button.x + button.w / 2, y: button.y + button.h / 2 });
}

fn trial(path: &str) -> Option<String> {
    (path == update::STATE_PATH).then(|| "trial 0\n".into())
}

#[test]
fn demo_shows_preference_and_refuses_changes() {
    let mut ui = panel();
    assert_eq!(ui.value(Item::BootCardMode), "SDR104");
    assert_eq!(active_choice::index(&ui, Item::BootCardMode), Some(0));
    assert_eq!(ui.value(Item::NegotiatedCardMode), "not reported");
    ui.select_option(Item::BootCardMode, "HS");
    assert_eq!(ui.toast.as_ref().unwrap().0, "Boot card mode requires the scope");
    assert!(ui.upd.card_confirmation.is_none());
}

#[test]
fn changing_with_the_knob_or_picker_waits_for_explicit_confirmation() {
    let mut ui = panel();
    ui.host.enable_fixture_services();
    assert_eq!(ui.value(Item::NegotiatedCardMode), "Boot record unavailable");
    ui.adjust(Item::BootCardMode, 1);
    assert_eq!(ui.upd.card_confirmation.as_ref().unwrap().mode, Mode::Hs);
    let (_, restage, cancel) = layout(&ui);
    touch(&mut ui, restage, Phase::Up);
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_some());
    touch(&mut ui, cancel, Phase::Down);
    touch(&mut ui, cancel, Phase::Up);
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_none());

    ui.open_picker(Item::BootCardMode);
    ui.activate(Item::PickOpt(1));
    assert_eq!(ui.upd.card_confirmation.as_ref().unwrap().mode, Mode::Hs);
    ui.key(key::HMENU);
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_none());
    assert_eq!(ui.value(Item::BootCardMode), "SDR104");
}

#[test]
fn pending_update_refuses_selection_and_rechecks_at_confirmation() {
    let mut ui = panel();
    ui.host.enable_fixture_services();
    ui.host.set_directory_reader(|path| Path::new(update::STATE_PATH).parent() == Some(path));
    ui.select_option(Item::BootCardMode, "HS");
    assert!(ui.upd.card_confirmation.is_some() && ui.upd.job.is_none());

    ui.host.set_text_reader(trial);
    let (_, restage, _) = layout(&ui);
    touch(&mut ui, restage, Phase::Down);
    touch(&mut ui, restage, Phase::Up);
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_none());
    assert!(ui.toast.take().unwrap().0.contains("still on trial"));

    ui.select_option(Item::BootCardMode, "HS");
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_none());
    assert!(ui.toast.as_ref().unwrap().0.contains("still on trial"));

    ui.host.set_text_reader(|path| (path == update::STATE_PATH).then(|| "kexec candidate\n".into()));
    ui.select_option(Item::BootCardMode, "HS");
    assert!(ui.upd.job.is_none() && ui.upd.card_confirmation.is_none());
    assert!(ui.toast.as_ref().unwrap().0.contains("already starting by kexec"));
}
