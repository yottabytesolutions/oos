// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

mod bus;
mod channel;
mod decode;
mod logic;
mod reference;
mod serial;
mod time;
mod trigger;

fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}

fn check(u: &Ui, item: Item, kind: Kind, label: &str, value: &str, options: Vec<String>) {
    assert_eq!(Ui::kind(item), kind, "{item:?}");
    assert_eq!(Ui::label(item), label, "{item:?}");
    assert_eq!(u.value(item), value, "{item:?}");
    assert_eq!(u.options(item), options, "{item:?}");
}

fn wave() -> RefWave {
    RefWave {
        src: 1,
        color: 2,
        label: String::new(),
        t0: 0.0,
        dt: 1e-6,
        v: vec![0.0, 1.0].into(),
        vpd: 2.0,
        offset: -0.25,
        on: true,
    }
}

#[test]
fn all_five_areas_keep_picker_identity_back_and_close_at_each_text_size() {
    for size in TEXT_SIZES {
        for (menu, item) in [
            (MenuId::Bus, Item::BusRw),
            (MenuId::Decode, Item::DecOn),
            (MenuId::La, Item::LaShow),
            (MenuId::Ref, Item::RefSrc),
            (MenuId::Trigger, Item::UartPol),
        ] {
            let mut u = demo();
            u.m.text_size = size;
            u.m.dec_proto = Proto::UART;
            u.m.trig_type = TrigType::Bus;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            let committed = active_choice::index(&u, item).unwrap();
            assert!(matches!(u.ctl(item, 1), Ctl::Pick));
            assert!(matches!(u.ctl(item, 1000), Ctl::Seg(_, Some(k)) if k == committed));
            u.open_picker(item);
            assert_eq!(u.focus, committed);
            u.focus = 1;
            u.m.bus.addr_rw = true;
            u.m.dec_on = true;
            u.m.la.show = 0xff;
            u.ref_src = 1;
            u.m.ser.uart_inv = true;
            assert_eq!(u.focus, 1);
            assert_eq!(active_choice::index(&u, item), Some(if item == Item::LaShow { 0 } else { 1 }));
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (menu, 1));
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
            u.open_submenu(menu);
            let close = u.dlg_layout().unwrap().close;
            assert!(u.tap_menu(close.x + 1, close.y + 1));
            assert!(!u.menu_open);
        }
    }
}

#[test]
fn capability_variants_in_isolated_processes() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::read_tests::capability_fixture", "--nocapture"])
            .env("OOS_READ_TEST_MODEL", model)
            .output()
            .unwrap();
        assert!(output.status.success(), "{model}: {}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_READ_TEST_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let mut u = demo();
    let la = model == "DHO924S";
    let channels = if model == "DHO802" { 2 } else { 4 };
    u.m.la.on = true;
    u.m.dec_la_in = true;
    u.m.dec_proto = Proto::UART;
    for size in TEXT_SIZES {
        u.m.text_size = size;
        let rows = u.menu_items(MenuId::Decode);
        assert_eq!(rows.contains(&Item::DecIn), la);
        assert_eq!(rows.contains(&Item::DecThr), !la);
        let sources: Vec<_> = (0..if la { 16 } else { channels })
            .map(|n| if la { format!("D{n}") } else { format!("CH{}", n + 1) })
            .collect();
        assert_eq!(u.options(Item::DecSrc), sources);
        assert_eq!(u.options(Item::RefSrc).len(), channels);
        assert_eq!(u.options(Item::BusClk).len(), channels + NREF);
        let main: Vec<_> = u.main_groups().into_iter().flat_map(|(_, rows)| rows).collect();
        assert_eq!(main.contains(&Item::Open(MenuId::La)), la);
        u.m.trig_type = TrigType::Edge;
        u.m.trig_la = Some(15);
        assert_eq!(u.value(Item::TrigSrc), if la { "D15" } else { "CH1" });
        assert_eq!(u.options(Item::TrigSrc).len(), channels + if la { 16 } else { 0 });
        for kind in TRIG_TYPES {
            u.m.trig_type = kind;
            assert_eq!(u.menu_items(MenuId::Trigger), trigger::expected_items(&u));
        }
        for c in 0..channels {
            assert_eq!(u.menu_items(MenuId::Channel(c)).len(), 10);
            assert_eq!(u.menu_items(MenuId::ChMore(c)).len(), 6);
        }
    }
}

#[test]
fn channel_trigger_and_time_keep_picker_identity_and_navigation() {
    for size in TEXT_SIZES {
        for (menu, item) in [
            (MenuId::Channel(1), Item::ChProbe(1)),
            (MenuId::ChMore(1), Item::ChUnit(1)),
            (MenuId::Trigger, Item::TrigMode),
            (MenuId::Time, Item::NtpOn),
        ] {
            let mut u = demo();
            u.m.text_size = size;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            let rows = u.dlg_layout().unwrap().rows;
            assert_eq!(rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            for row in rows {
                assert_control_kind(row);
            }
            let committed = active_choice::index(&u, item).unwrap();
            assert!(matches!(u.ctl(item, 1), Ctl::Pick));
            assert!(matches!(u.ctl(item, 1000), Ctl::Seg(_, Some(k)) if k == committed));
            u.open_picker(item);
            assert_eq!(u.focus, committed);
            u.focus = 1;
            u.m.ch[1].probe = 100.0;
            u.m.ch[1].unit = AmpUnit::ALL[1];
            u.m.trig_mode = TrigMode::Single;
            u.sys.ntp = true;
            assert_eq!(u.focus, 1);
            assert_eq!(
                active_choice::index(&u, item),
                Some(if matches!(item, Item::ChProbe(_) | Item::TrigMode) { 2 } else { 1 })
            );
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (menu, 1));
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
            u.open_submenu(menu);
            let close = u.dlg_layout().unwrap().close;
            assert!(u.tap_menu(close.x + 1, close.y + 1));
            assert!(!u.menu_open);
        }
    }
}

fn assert_control_kind(row: Row) {
    assert!(match Ui::kind(row.it) {
        Kind::Text => matches!(row.ctl, Ctl::Text),
        Kind::Value => matches!(row.ctl, Ctl::Value),
        Kind::Action => matches!(row.ctl, Ctl::Button),
        Kind::Choice => matches!(row.ctl, Ctl::Pick | Ctl::Seg(..)),
    });
}
