// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

mod review;

fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

fn row(u: &Ui, it: Item) -> Row {
    u.dlg_layout().unwrap().rows.into_iter().find(|r| r.it == it).unwrap()
}

fn rendered(u: &Ui) -> Canvas {
    let mut cv = Canvas::new(u.scr.w, u.scr.h);
    u.draw_menu(&mut cv);
    cv
}

fn filled(u: &Ui, r: Rect, active: bool) {
    let cv = rendered(u);
    assert_filled(&cv, r, active);
}

fn assert_filled(cv: &Canvas, r: Rect, active: bool) {
    assert_eq!(cv.px[((r.y + 7) * cv.w + r.x + 7) as usize], if active { ACCENT } else { BTN });
    if active {
        assert!(
            (r.y + 5..r.bottom() - 5)
                .any(|y| { (r.x + 5..r.right() - 5).any(|x| cv.px[(y * cv.w + x) as usize] == DARK_TEXT) }),
            "selected text must contrast with the fill"
        );
    }
}

fn check_choice(menu: MenuId, it: Item, selected: usize) {
    let mut u = demo();
    u.open_menu(menu);
    assert_eq!(index(&u, it), Some(selected), "{it:?}");
    // Every kind can collapse when its option labels no longer fit.
    let mut cv = Canvas::new(u.scr.w, u.scr.h);
    let mid = Rect::new(10, 10, 240, 40);
    value(&u, &mut cv, it, mid, true);
    assert_filled(&cv, mid, true);
    for elsewhere in [false, true] {
        let d = u.dlg_layout().unwrap();
        let k = d.rows.iter().position(|r| r.it == it).unwrap();
        u.focus = if elsewhere { (k + 1) % d.rows.len() } else { k };
        let row = row(&u, it);
        let r = match row.ctl {
            Ctl::Seg(ref opts, sel) => {
                assert_eq!(sel, Some(selected));
                let sw = (row.ctrl.w - (opts.len() as i32 - 1) * 4) / opts.len() as i32;
                Rect::new(row.ctrl.x + selected as i32 * (sw + 4), row.ctrl.y, sw, row.ctrl.h)
            }
            Ctl::Pick => Ui::value_parts(row.ctrl).1,
            _ => panic!("expected a choice: {it:?}"),
        };
        filled(&u, r, true);
    }
    u.open_picker(it);
    assert_eq!(u.focus, selected);
    for focus in [selected, (selected + 1) % u.options(it).len()] {
        u.focus = focus;
        for k in 0..u.options(it).len() {
            filled(&u, row(&u, Item::PickOpt(k)).ctrl, k == selected);
        }
    }
    u.close_picker();
    assert_eq!(index(&u, it), Some(selected));
}

#[test]
fn active_choice_toggle_survives_focus_and_picker() {
    check_choice(MenuId::Channel(0), Item::ChOn(0), 1);
}

#[test]
fn active_choice_segmented_mode_survives_focus_and_picker() {
    check_choice(MenuId::Trigger, Item::TrigMode, 0);
}

#[test]
fn active_choice_source_survives_focus_and_picker() {
    check_choice(MenuId::Trigger, Item::TrigSrc, 0);
}

#[test]
fn active_choice_collapsed_picker_survives_focus_and_return() {
    check_choice(MenuId::Trigger, Item::TrigType, 0);
}

#[test]
fn active_choice_analysis_and_arrangement_survive_focus_and_picker() {
    check_choice(MenuId::Spectrum, Item::SpecTrace, 0);
    check_choice(MenuId::Windows, Item::WinLayout, 0);
    check_choice(MenuId::Display, Item::BottomBar, 0);
}

#[test]
fn active_choice_remote_uses_committed_identity_with_a_display_suffix() {
    let mut u = demo();
    u.sys.net_name = "example-scope-with-a-long-name".into();
    u.sys.remote_mdns = true;
    u.open_menu(MenuId::Remote);
    assert_ne!(u.value(Item::RemName), u.options(Item::RemName)[1]);
    filled(&u, Ui::value_parts(row(&u, Item::RemName).ctrl).1, true);
    u.open_picker(Item::RemName);
    assert_eq!(u.focus, 1);
    u.focus = 0;
    filled(&u, row(&u, Item::PickOpt(1)).ctrl, true);
    filled(&u, row(&u, Item::PickOpt(0)).ctrl, false);
}

#[test]
fn active_choice_colliding_truncated_labels_mark_only_the_committed_identity() {
    let mut u = demo();
    // Reuse an existing long preset without embedding a real hostname in the fixture.
    let preset = time::SERVERS.iter().position(|s| s.len() > 18).unwrap();
    u.sys.ntp_server = format!("{}-other.example.invalid", &time::SERVERS[preset][..17]);
    u.open_menu(MenuId::Time);
    let opts = u.options(Item::NtpServer);
    let custom = time::SERVERS.len();
    assert_eq!(opts[preset], opts[custom], "both labels must really be truncated identically");
    assert_eq!(index(&u, Item::NtpServer), Some(custom));
    filled(&u, Ui::value_parts(row(&u, Item::NtpServer).ctrl).1, true);
    u.open_picker(Item::NtpServer);
    assert_eq!(u.focus, custom);
    for focus in [custom, preset] {
        u.focus = focus;
        filled(&u, row(&u, Item::PickOpt(custom)).ctrl, true);
        filled(&u, row(&u, Item::PickOpt(preset)).ctrl, false);
    }
}

#[test]
fn active_choice_focus_outline_is_separate_from_the_selection_fill() {
    let mut u = demo();
    u.open_menu(MenuId::Trigger);
    u.open_picker(Item::TrigType);
    for focus in [0, 1] {
        u.focus = focus;
        let c = row(&u, Item::PickOpt(focus)).ctrl;
        let cv = rendered(&u);
        assert_eq!(cv.px[((c.y - 3) * cv.w + c.x + c.w / 2) as usize], ACCENT);
        filled(&u, row(&u, Item::PickOpt(0)).ctrl, true);
        filled(&u, row(&u, Item::PickOpt(1)).ctrl, false);
    }
}

#[test]
fn active_choice_inventory_has_an_identity_for_each_committed_selector() {
    let mut u = demo();
    for menu in menus() {
        for it in u.items(menu) {
            if u.options(it).is_empty()
                || matches!(
                    it,
                    Item::MeasCopyTo
                        | Item::WlanNet
                        | Item::WlanForgetNet
                        | Item::SshRemoveKey
                        | Item::RefImport
                        | Item::TrigBusVal
                        | Item::SerMask
                        | Item::SerMax
                        | Item::AfgFreqEntry
                        | Item::IdFind(_)
                )
            {
                continue;
            }
            if matches!(it, Item::SimShow) && u.refs[u.m.sim_slot].is_none() {
                continue;
            }
            // no address list (everyone) has no row in Allowed's picker; with a list it is the first, below
            if matches!(it, Item::RemAllow) && u.sys.net_allow().is_empty() {
                continue;
            }
            let selected = index(&u, it).unwrap_or_else(|| panic!("missing identity: {menu:?} {it:?}"));
            assert!(selected < u.options(it).len(), "invalid option identity: {it:?}");
        }
    }
    u.m.bar_compact = true;
    assert_eq!(index(&u, Item::BottomBar), Some(1));
    u.m.dec_la_in = true;
    u.m.la.on = true;
    u.m.dec_la = [Some(7), Some(2), None, Some(12)];
    assert_eq!(index(&u, Item::DecSrc), Some(7));
    assert_eq!(index(&u, Item::DecCs), Some(0));
    assert_eq!(index(&u, Item::DecSrc4), Some(13));
    // Allowed (upstream c24899c4): the list in force is the first row of its picker
    u.sys.set_net_allow(Some("192.0.2.0/24"));
    assert_eq!(index(&u, Item::RemAllow), Some(0));
    assert_eq!(u.options(Item::RemAllow).first().map(String::as_str), Some("192.0.2.0/24"));
}

#[test]
fn active_choice_all_analyser_selectors_use_the_displayed_view() {
    let mut u = demo();
    let choices = [
        (Item::SpecOn, AView::Spectrum),
        (Item::EyeOn, AView::Eye),
        (Item::PowOn, AView::Power),
        (Item::BodeOn, AView::Bode),
        (Item::BwOn, AView::Step),
        (Item::MagOn, AView::Magnetic),
        (Item::InvOn, AView::Inverter),
        (Item::DptOn, AView::Dpt),
    ];
    for (it, view) in choices {
        u.m.view = view;
        assert_eq!(index(&u, it), Some(1));
        u.xy = true;
        assert_eq!(index(&u, it), Some(0), "XY suppresses {view:?}");
        u.xy = false;
    }
}

#[test]
fn active_choice_numeric_buffers_and_momentary_actions_are_unselected() {
    let mut u = demo();
    for it in [
        Item::TrigLevel,
        Item::TrigBusVal,
        Item::SerMask,
        Item::AfgFreq,
        Item::AfgFreqEntry,
        Item::MeasCopyTo,
        Item::WlanNet,
        Item::WlanForgetNet,
        Item::SshRemoveKey,
        Item::RefImport,
        Item::ClearDisp,
    ] {
        assert_eq!(index(&u, it), None, "{it:?}");
    }
    u.open_menu(MenuId::Trigger);
    filled(&u, Ui::value_parts(row(&u, Item::TrigLevel).ctrl).1, false);
    u.open_picker(Item::TrigBusVal);
    u.keypad = "0".into();
    filled(&u, row(&u, Item::PickOpt(0)).ctrl, false);
}

#[test]
fn active_choice_current_option_updates_while_picker_focus_stays_elsewhere() {
    let mut u = demo();
    u.open_menu(MenuId::Trigger);
    u.open_picker(Item::TrigType);
    u.focus = 3;
    u.m.trig_type = TrigType::Pulse;
    filled(&u, row(&u, Item::PickOpt(0)).ctrl, false);
    filled(&u, row(&u, Item::PickOpt(1)).ctrl, true);
    assert_eq!(u.focus, 3);
}

fn menus() -> Vec<MenuId> {
    use MenuId::*;
    vec![
        Main,
        Channel(0),
        ChMore(0),
        Logger,
        Horizontal,
        Trigger,
        Acquire,
        Measure,
        MeasAdd,
        MeasHold,
        MeasPair,
        MeasSelected,
        Dvm,
        Cursor,
        Display,
        Windows,
        Utility,
        Math,
        Ref,
        Decode,
        Bus,
        Storage,
        Mask,
        Zone,
        Spectrum,
        Eye,
        Power,
        SelfCal,
        Bode,
        Afg,
        BwTest,
        La,
        LaLines,
        LaPat,
        Network,
        Remote,
        Ssh,
        Features,
        Decoders(0),
        Time,
        Magnetic,
        Inverter,
        Dpt,
        Sim,
        SimAdvanced,
        ViewMore,
        Identify,
        Pick,
    ]
}

#[test]
fn active_choice_reference_toggle_keeps_its_on_cue_after_focus_moves() {
    let mut u = demo();
    u.refs[0] = Some(RefWave {
        src: 0,
        color: 0,
        label: String::new(),
        t0: 0.0,
        dt: 1e-6,
        v: vec![0.0, 1.0].into(),
        vpd: 1.0,
        offset: 0.0,
        on: true,
    });
    u.open_menu(MenuId::Ref);
    for focus in [0, 1] {
        u.focus = focus;
        filled(&u, row(&u, Item::RefShow(0)).ctrl, true);
        filled(&u, Ui::value_parts(row(&u, Item::RefColor).ctrl).1, true);
    }
    u.refs[0].as_mut().unwrap().on = false;
    filled(&u, row(&u, Item::RefShow(0)).ctrl, false);
    u.refs[0] = None;
    filled(&u, row(&u, Item::RefShow(0)).ctrl, false);
}

#[test]
fn active_choice_stepped_slots_keep_the_committed_cue() {
    let mut u = demo();
    for (menu, it) in [(MenuId::Ref, Item::RefSel), (MenuId::Storage, Item::StoreSlot)] {
        u.open_menu(menu);
        for focus in [0, 1] {
            u.focus = focus;
            filled(&u, Ui::value_parts(row(&u, it).ctrl).1, true);
        }
    }
}

#[test]
fn active_choice_unoffered_reference_never_marks_an_analogue_trigger_source() {
    let mut u = demo();
    u.m.la.on = true;
    assert!(u.m.trig_la_ok());
    u.m.trig_ref = Some(0);
    assert_eq!(index(&u, Item::TrigSrc), None);
    u.m.trig_ref = None;
    u.m.trig_la = Some(7);
    assert_eq!(index(&u, Item::TrigSrc), Some(n_ch() + 7));
}
