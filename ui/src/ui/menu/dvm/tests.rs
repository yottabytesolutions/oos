// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const ITEMS: [Item; 8] = [
    Item::DvmOn,
    Item::DvmSrc,
    Item::DvmMode,
    Item::CntOn,
    Item::CntSrc,
    Item::CntMode,
    Item::CntDigits,
    Item::CntClear,
];

fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

#[test]
fn every_dvm_row_keeps_its_kind_label_and_options() {
    let u = demo();
    let expected = [
        (Kind::Choice, "DVM", vec!["Off", "On"]),
        (Kind::Choice, "DVM source", vec!["CH1", "CH2", "CH3", "CH4"]),
        (Kind::Choice, "DVM mode", vec!["DC", "AC rms", "AC+DC rms"]),
        (Kind::Choice, "Counter", vec!["Off", "On"]),
        (Kind::Choice, "Count source", vec!["CH1", "CH2", "CH3", "CH4"]),
        (Kind::Choice, "Count mode", vec!["Frequency", "Period", "Totalize"]),
        (Kind::Value, "Digits", vec![]),
        (Kind::Action, "Clear counter", vec![]),
    ];
    assert_eq!(u.menu_items(MenuId::Dvm), ITEMS);
    assert_eq!(Ui::label(Item::Open(MenuId::Dvm)), "DVM / counter");
    assert_eq!(Ui::kind(Item::Open(MenuId::Dvm)), Kind::Action);
    assert_eq!(u.value(Item::Open(MenuId::Dvm)), "");
    assert!(u.options(Item::Open(MenuId::Dvm)).is_empty());
    for (item, (kind, label, options)) in ITEMS.into_iter().zip(expected) {
        assert_eq!(Ui::kind(item), kind, "{item:?}");
        assert_eq!(Ui::label(item), label, "{item:?}");
        assert_eq!(u.options(item), options, "{item:?}");
    }
}

#[test]
fn every_dvm_value_and_committed_option_follow_current_state() {
    let mut u = demo();
    for on in [false, true] {
        (u.m.dvm_on, u.m.cnt_on) = (on, on);
        for item in [Item::DvmOn, Item::CntOn] {
            assert_eq!(u.value(item), if on { "On" } else { "Off" });
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
    }
    for (mode, (dvm, counter)) in
        [("DC", "Frequency"), ("AC rms", "Period"), ("AC+DC rms", "Totalize"), ("AC+DC rms", "Totalize")]
            .into_iter()
            .enumerate()
    {
        (u.m.dvm_mode, u.m.cnt_mode) = (mode, mode);
        assert_eq!(u.value(Item::DvmMode), dvm);
        assert_eq!(u.value(Item::CntMode), counter);
        assert_eq!(active_choice::index(&u, Item::DvmMode), Some(mode.min(2)));
        assert_eq!(active_choice::index(&u, Item::CntMode), Some(mode.min(2)));
    }
    for digits in 3..=7 {
        u.m.cnt_digits = digits;
        assert_eq!(u.value(Item::CntDigits), digits.to_string());
        assert_eq!(active_choice::index(&u, Item::CntDigits), None);
    }
    assert_eq!(u.value(Item::CntClear), "");
    assert_eq!(active_choice::index(&u, Item::CntClear), None);
}

#[test]
fn live_and_offline_sources_keep_their_domains_and_identity() {
    let mut u = demo();
    for source in 0..n_ch() {
        (u.m.dvm_src, u.m.cnt_src) = (source, source);
        for item in [Item::DvmSrc, Item::CntSrc] {
            assert_eq!(u.value(item), format!("CH{}", source + 1));
            assert_eq!(active_choice::index(&u, item), Some(source));
        }
    }
    u.offline = true;
    let options: Vec<_> = (1..=NREF).map(|n| format!("R{n}")).collect();
    for source in 0..=NREF {
        u.offline_ref = source;
        for item in [Item::DvmSrc, Item::CntSrc] {
            assert_eq!(u.options(item), options);
            assert_eq!(u.value(item), format!("R{}", source.min(NREF - 1) + 1));
            assert_eq!(active_choice::index(&u, item), Some(source.min(NREF - 1)));
        }
    }
}

#[test]
fn row_order_and_source_on_visibility_cover_both_meters() {
    let mut u = demo();
    for offline in [false, true] {
        u.offline = offline;
        for dvm in [false, true] {
            u.m.dvm_on = dvm;
            for counter in [false, true] {
                u.m.cnt_on = counter;
                for same_source in [false, true] {
                    (u.m.dvm_src, u.m.cnt_src) = (0, usize::from(!same_source));
                    for enabled in 0..4 {
                        for c in 0..2 {
                            u.m.ch[c].on = enabled & (1 << c) != 0;
                        }
                        let mut missing = Vec::new();
                        if !offline && dvm && enabled & 1 == 0 {
                            missing.push(0);
                        }
                        let c = u.m.cnt_src;
                        if !offline && counter && enabled & (1 << c) == 0 && !missing.contains(&c) {
                            missing.push(c);
                        }
                        let mut expected = ITEMS.to_vec();
                        if !missing.is_empty() {
                            expected.push(Item::SrcOn);
                        }
                        assert_eq!(u.items(MenuId::Dvm), expected);
                        assert_eq!(u.off_sources(MenuId::Dvm), missing);
                    }
                }
            }
        }
    }
}

#[test]
fn renderer_and_picker_keep_focus_separate_from_committed_choices() {
    for size in TEXT_SIZES {
        let mut u = demo();
        u.m.text_size = size;
        u.open_menu(MenuId::Main);
        u.focus = 3;
        u.open_submenu(MenuId::Dvm);
        u.focus = 5;
        u.m.dvm_mode = 2;
        let rows = u.dlg_layout().unwrap().rows;
        assert_eq!(rows.iter().map(|r| r.it).collect::<Vec<_>>(), ITEMS);
        assert!(rows.iter().any(|r| matches!(r.ctl, Ctl::Value) && r.it == Item::CntDigits));
        assert!(rows.iter().any(|r| matches!(r.ctl, Ctl::Button) && r.it == Item::CntClear));
        assert!(matches!(u.ctl(Item::DvmMode, 1), Ctl::Pick));
        assert!(matches!(u.ctl(Item::DvmMode, 1000), Ctl::Seg(_, Some(2))));
        u.open_picker(Item::DvmMode);
        assert_eq!(u.focus, 2);
        u.focus = 1;
        u.m.dvm_mode = 0;
        assert_eq!(active_choice::index(&u, Item::DvmMode), Some(0));
        assert_eq!(u.focus, 1);
        let back = u.dlg_layout().unwrap().back.unwrap();
        assert!(u.tap_menu(back.x + 1, back.y + 1));
        assert_eq!((u.menu, u.focus), (MenuId::Dvm, 5));
        let back = u.dlg_layout().unwrap().back.unwrap();
        assert!(u.tap_menu(back.x + 1, back.y + 1));
        assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
        u.open_submenu(MenuId::Dvm);
        let close = u.dlg_layout().unwrap().close;
        assert!(u.tap_menu(close.x + 1, close.y + 1));
        assert!(!u.menu_open);
    }
}

#[test]
fn channel_capabilities_in_an_isolated_process() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::dvm::tests::capability_fixture", "--nocapture"])
            .env("OOS_DVM_TEST_MODEL", model)
            .output()
            .unwrap();
        assert!(output.status.success(), "{model}: {}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_DVM_TEST_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let channels = if model == "DHO802" { 2 } else { 4 };
    let options = ["CH1", "CH2", "CH3", "CH4"][..channels].to_vec();
    let mut u = demo();
    for size in TEXT_SIZES {
        u.m.text_size = size;
        u.open_menu(MenuId::Dvm);
        assert_eq!(u.menu_items(MenuId::Dvm), ITEMS);
        for item in [Item::DvmSrc, Item::CntSrc] {
            assert_eq!(u.options(item), options);
            for (channel, option) in options.iter().enumerate() {
                u.select_option(item, option);
                assert_eq!(u.value(item), *option);
                assert_eq!(active_choice::index(&u, item), Some(channel));
            }
        }
        u.offline = true;
        assert_eq!(u.options(Item::DvmSrc).len(), 10);
        assert_eq!(u.options(Item::CntSrc).len(), 10);
        u.offline = false;
    }
}
