// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn every_system_row_keeps_its_metadata_and_menu_order() {
    use Item::*;
    use Kind::*;
    let u = demo();
    read_rows(
        &u,
        &[
            (Open(MenuId::Utility), Action, "System", "".into(), vec![]),
            (About, Action, "About OOS", "".into(), vec![]),
            (SysInfo, Action, "System info", "".into(), vec![]),
            (Shot, Action, "Screenshot", "".into(), vec![]),
            (Stock, Action, "Back to stock", "".into(), vec![]),
            (Update, Action, "Update", "".into(), vec![]),
            (UpdateRollback, Action, "Roll back update", "".into(), vec![]),
            (BootMode, Choice, "Boot mode", "One-shot".into(), strings(&["One-shot", "Persist"])),
            (BootCardMode, Choice, "Boot card mode", "HS".into(), strings(&["SDR104", "HS"])),
            (NegotiatedCardMode, Text, "Negotiated", "not reported".into(), vec![]),
        ],
    );
    assert_eq!(
        u.menu_items(MenuId::Utility),
        [
            Offline,
            QuickAct,
            AuxOut,
            BootMode,
            BootCardMode,
            NegotiatedCardMode,
            Update,
            Overclock,
            Graphics,
            Shot,
            Open(MenuId::SelfCal),
            Open(MenuId::Network),
            Open(MenuId::Features),
            Open(MenuId::Time),
            SysInfo,
            About,
            UpdateRollback,
            Stock
        ]
    );
    assert_eq!(Ui::label(Offline), "Record");
    assert_eq!(Ui::label(Graphics), "Graphics");
    assert_eq!(Ui::label(Open(MenuId::SelfCal)), "Self-cal");
    assert_eq!(Ui::label(Open(MenuId::Features)), "Features");
    assert_eq!(Ui::label(Open(MenuId::Time)), "Date and time");
}

#[test]
fn system_choices_read_current_settings_and_board_options() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    let quicks = strings(&[
        "Screenshot",
        "Save PNG",
        "Save setup",
        "Save CSV",
        "Autoset",
        "Single",
        "Run/Stop",
        "Clear",
        "Next view",
    ]);
    for (index, quick) in QUICKS.into_iter().enumerate() {
        u.m.quick = quick;
        read_rows(&u, &[(QuickAct, Choice, "Quick key", quicks[index].clone(), quicks.clone())]);
        assert_eq!(active_choice::index(&u, QuickAct), Some(index));
    }
    let hardware = board().mask_hw(u.switches()) || board().zone_hw(u.switches());
    let aux = if hardware { strings(&["Trigger out", "Off", "Pass/fail"]) } else { strings(&["Trigger out", "Off"]) };
    for (index, value) in [crate::model::AuxOut::TrigOut, crate::model::AuxOut::Off, crate::model::AuxOut::PassFail]
        .into_iter()
        .enumerate()
    {
        u.m.aux_out = value;
        read_rows(&u, &[(AuxOut, Choice, "AUX out", ["Trigger out", "Off", "Pass/fail"][index].into(), aux.clone())]);
        assert_eq!(active_choice::index(&u, AuxOut), (index < aux.len()).then_some(index));
    }
    let clocks: Vec<_> = board().clock_steps().iter().map(|&hz| overclock::label(hz)).collect();
    for (index, &hz) in board().clock_steps().iter().enumerate() {
        u.clock_next = hz;
        read_rows(&u, &[(Overclock, Choice, "Overclock", clocks[index].clone(), clocks.clone())]);
        assert_eq!(active_choice::index(&u, Overclock), Some(index));
    }
    assert_eq!(active_choice::index(&u, BootMode), Some(0));
}

#[test]
fn contexts_stay_out_of_visible_rows() {
    super::super::small_tests::contexts(MenuId::Utility, &[Item::Open(MenuId::Utility)]);
}

#[test]
fn navigation_and_visibility_at_every_text_size() {
    super::super::small_tests::navigation(MenuId::Utility, Item::QuickAct, |u, focused| u.m.quick = QUICKS[focused]);
}

#[test]
fn capabilities_in_isolated_processes() {
    super::super::small_tests::capabilities(
        MenuId::Utility,
        "ui::menu::system::tests::capabilities_in_isolated_processes",
        system_choices_read_current_settings_and_board_options,
    );
}
