// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

pub(super) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}

pub(super) fn read_rows(u: &Ui, rows: &[(Item, Kind, &str, String, Vec<String>)]) {
    for (item, kind, label, value, options) in rows {
        assert_eq!(Ui::kind(*item), *kind, "{item:?}");
        assert_eq!(Ui::label(*item), *label, "{item:?}");
        assert_eq!(u.value(*item), *value, "{item:?}");
        assert_eq!(u.options(*item), *options, "{item:?}");
    }
}

#[test]
fn backlight_logger_and_record_rows_keep_their_read_contracts() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.log_ivl = 0;
    read_rows(
        &u,
        &[
            (Backlight, Value, "Backlight", "100 %".into(), vec![]),
            (Open(MenuId::Logger), Action, "Data logger", "".into(), vec![]),
            (LogOn, Choice, "Logging", "Off".into(), strings(&["Off", "On"])),
            (LogIvl, Value, "Interval", "1 s".into(), vec![]),
            (LogInfo, Choice, "File", format!("{} measurements, off", u.meas_rows().len()), vec![]),
            (Offline, Choice, "Record", "With live".into(), strings(&["With live", "Take over", "Offline"])),
        ],
    );
    for pct in [10, 25, 85, 100] {
        u.sys.backlight = pct;
        assert_eq!(u.value(Backlight), format!("{pct} %"));
        assert_eq!(active_choice::index(&u, Backlight), None);
        assert!(u.menu_items(MenuId::Display).contains(&Backlight));
    }
    for (index, interval) in super::super::logger::LOG_STEPS.iter().enumerate() {
        u.m.log_ivl = index;
        assert_eq!(u.value(LogIvl), si_short(*interval, "s"));
    }
    u.m.log_ivl = usize::MAX;
    assert_eq!(u.value(LogIvl), si_short(*super::super::logger::LOG_STEPS.last().unwrap(), "s"));
    u.log.file = Some("record.csv".into());
    u.log.rows = 7;
    assert_eq!(u.value(LogOn), "On");
    assert_eq!(u.value(LogInfo), "record.csv: 7 rows");
    assert_eq!(active_choice::index(&u, LogOn), Some(1));
    assert_eq!(u.menu_items(MenuId::Logger), [LogOn, LogIvl, LogInfo]);
    for (index, look) in
        [reflook::RefLook::Live, reflook::RefLook::TakeOver, reflook::RefLook::Waves].into_iter().enumerate()
    {
        u.look = look;
        assert_eq!(u.value(Offline), ["With live", "Take over", "Offline"][index]);
        assert_eq!(active_choice::index(&u, Offline), Some(index));
        assert!(u.menu_items(MenuId::Utility).contains(&Offline));
    }
    // This fixture represents an already open logger without starting filesystem IO.
    u.log.file = None;
}

#[test]
fn calibration_rows_cover_idle_requested_running_and_last_result() {
    use Item::*;
    use Kind::*;
    let shared: crate::board::HwShared = Default::default();
    let mut u = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    let channels = ["All", "CH1", "CH2", "CH3", "CH4"][..=n_ch()].iter().map(|s| (*s).into()).collect();
    read_rows(
        &u,
        &[
            (Open(MenuId::SelfCal), Action, "Self-cal", "".into(), vec![]),
            (CalCh, Choice, "Channels", "All".into(), channels),
            (CalAdc, Choice, "ADC cores", "Off".into(), strings(&["Off", "On"])),
            (CalZero, Choice, "AFE zero", "Off".into(), strings(&["Off", "On"])),
            (CalStart, Action, "Start", "".into(), vec![]),
            (CalCancel, Action, "Cancel", "".into(), vec![]),
            (CalStatus, Choice, "Status", "idle".into(), vec![]),
        ],
    );
    for channel in 0..=n_ch() {
        u.cal.ch = channel;
        assert_eq!(u.value(CalCh), u.options(CalCh)[channel]);
        assert_eq!(active_choice::index(&u, CalCh), Some(channel));
    }
    for on in [false, true] {
        (u.cal.adc, u.cal.zero) = (on, on);
        for item in [CalAdc, CalZero] {
            assert_eq!(u.value(item), if on { "On" } else { "Off" });
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
    }
    let idle = [CalCh, CalAdc, CalZero, CalStart, CalStatus];
    let running = [CalCh, CalAdc, CalZero, CalCancel, CalStatus];
    assert_eq!(u.menu_items(MenuId::SelfCal), idle);
    shared.lock().unwrap().cal_req = Some((vec![0], false, false));
    assert_eq!(u.menu_items(MenuId::SelfCal), running);
    assert_eq!(u.value(CalStatus), "starting ...");
    shared.lock().unwrap().cal_progress = Some((1, 4, "Checking".into()));
    assert_eq!(u.menu_items(MenuId::SelfCal), running);
    assert_eq!(u.value(CalStatus), "25 % Checking");
    shared.lock().unwrap().cal_req = None;
    assert_eq!(u.menu_items(MenuId::SelfCal), running);
    shared.lock().unwrap().cal_progress = None;
    u.cal.last = Some("Complete".into());
    assert_eq!(u.menu_items(MenuId::SelfCal), idle);
    assert_eq!(u.value(CalStatus), "Complete");
    assert_eq!(demo().value(CalStatus), "needs the hardware source");
}

#[test]
fn identify_rows_cover_every_finding_and_the_eight_row_limit() {
    use oos_dsp::identify::{Finding, SignalKind, TriggerHint};
    let mut u = demo();
    read_rows(
        &u,
        &[
            (Item::IdentifyAct, Kind::Action, "Identify", "".into(), vec![]),
            (Item::ProtoWatch, Kind::Choice, "Protocol watch", "Off".into(), strings(&["Off", "On"])),
        ],
    );
    for on in [false, true] {
        u.azr.watch = on;
        assert_eq!(u.value(Item::ProtoWatch), if on { "On" } else { "Off" });
        assert_eq!(active_choice::index(&u, Item::ProtoWatch), Some(usize::from(on)));
    }
    for count in 0..=9 {
        let mut expected = vec![Item::ProtoWatch];
        expected.extend((0..count.min(8)).map(Item::IdFind));
        assert_eq!(u.menu_items(MenuId::Identify), expected);
        for index in 0..8 {
            let item = Item::IdFind(index);
            let value = if index < count {
                format!("CH{} Finding {index}", if index == 0 { 0 } else { index % 4 + 1 })
            } else {
                String::new()
            };
            read_rows(&u, &[(item, Kind::Choice, &format!("{}", index + 1), value.clone(), vec![value])]);
            assert_eq!(active_choice::index(&u, item), None);
        }
        u.azr.findings.push(Finding {
            channels: if count == 0 { vec![] } else { vec![count % 4] },
            kind: SignalKind::Dc { level: 1.0 },
            confidence: 1.0,
            summary: format!("Finding {count}"),
            trigger: TriggerHint::Edge { source: 0, level: 1.0, rising: true, holdoff: 0.0 },
            decoder: None,
        });
    }
}

#[test]
fn seven_areas_share_renderer_picker_back_and_close_at_all_text_sizes() {
    for size in TEXT_SIZES {
        for (menu, item) in [
            (MenuId::SelfCal, Item::CalCh),
            (MenuId::Identify, Item::ProtoWatch),
            (MenuId::Inverter, Item::InvPage),
            (MenuId::Logger, Item::LogOn),
            (MenuId::Sim, Item::SimPreset),
            (MenuId::SimAdvanced, Item::SimSsc),
            (MenuId::Display, Item::Backlight),
            (MenuId::Utility, Item::Offline),
        ] {
            let mut u = demo();
            u.m.text_size = size;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            if Ui::kind(item) == Kind::Choice {
                let committed = active_choice::index(&u, item).unwrap();
                assert!(matches!(u.ctl(item, 1), Ctl::Pick));
                assert!(matches!(u.ctl(item, 10000), Ctl::Seg(_, Some(k)) if k == committed));
                u.open_picker(item);
                assert_eq!(u.focus, committed);
                u.focus = (committed + 1) % u.options(item).len();
                assert_eq!(active_choice::index(&u, item), Some(committed));
                let focus = u.focus;
                match item {
                    Item::CalCh => u.cal.ch = 1,
                    Item::ProtoWatch => u.azr.watch = !u.azr.watch,
                    Item::InvPage => u.m.inv_page = crate::model::InvPage::Mech,
                    Item::LogOn => u.log.file = Some("record.csv".into()),
                    Item::SimPreset => u.m.sim_preset = (committed + 1) % u.options(item).len(),
                    Item::SimSsc => u.m.sim_ssc = !u.m.sim_ssc,
                    Item::Offline => u.look = reflook::RefLook::Waves,
                    _ => unreachable!(),
                }
                assert_ne!(active_choice::index(&u, item), Some(committed));
                assert_eq!(u.focus, focus);
                u.log.file = None;
                let back = u.dlg_layout().unwrap().back.unwrap();
                assert!(u.tap_menu(back.x + 1, back.y + 1));
                assert_eq!((u.menu, u.focus), (menu, 1));
            }
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
fn small_area_capabilities_in_isolated_processes() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::small_tests::capability_fixture"])
            .env("OOS_SMALL_MENU_MODEL", model)
            .output()
            .unwrap();
        assert!(output.status.success(), "{model}: {}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_SMALL_MENU_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let channels = if model == "DHO802" { 2 } else { 4 };
    let mut u = demo();
    assert_eq!(u.options(Item::CalCh), ["All", "CH1", "CH2", "CH3", "CH4"][..=channels]);
    for size in TEXT_SIZES {
        u.m.text_size = size;
        for offline in [false, true] {
            u.offline = offline;
            for item in [Item::InvA, Item::InvB, Item::InvC, Item::InvAn] {
                assert_eq!(u.options(item), u.wave_names());
                for channel in 0..channels {
                    u.select_option(item, &format!("CH{}", channel + 1));
                    assert_eq!(u.value(item), format!("CH{}", channel + 1));
                    assert_eq!(active_choice::index(&u, item), Some(channel));
                }
            }
            for menu in
                [MenuId::SelfCal, MenuId::Identify, MenuId::Inverter, MenuId::Logger, MenuId::Sim, MenuId::SimAdvanced]
            {
                u.open_menu(menu);
                assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            }
        }
    }
}

pub(super) fn contexts(menu: MenuId, contexts: &[Item]) {
    let u = demo();
    for &item in contexts {
        assert!(matches!(descriptor(item).unwrap().role, Role::Context));
        assert!(!Ui::label(item).is_empty());
        assert!(!u.menu_items(menu).contains(&item));
    }
    for item in u.menu_items(menu) {
        assert!(!Ui::label(item).is_empty(), "{item:?}: every row resolves its owner's label");
        if Ui::kind(item) == Kind::Text {
            assert!(matches!(u.ctl(item, 1000), Ctl::Text));
            assert!(u.options(item).is_empty());
        }
    }
}

pub(super) fn navigation(menu: MenuId, item: Item, change: fn(&mut Ui, usize)) {
    for size in TEXT_SIZES {
        let mut u = demo();
        u.m.text_size = size;
        let expected = u.menu_items(menu);
        for offline in [false, true] {
            u.offline = offline;
            u.usb = offline;
            u.sys.remote_ssh = offline;
            u.wlan.link = crate::netport::wlan::Link::new(offline, false, Default::default(), None, vec![]);
            assert_eq!(u.menu_items(menu), expected);
        }
        u.open_menu(MenuId::Main);
        u.focus = 3;
        u.open_submenu(menu);
        u.focus = 2;
        assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), expected);
        let committed = active_choice::index(&u, item).unwrap();
        assert!(matches!(u.ctl(item, 1), Ctl::Pick));
        assert!(matches!(u.ctl(item, 10000), Ctl::Seg(_, Some(i)) if i == committed));
        u.open_picker(item);
        assert_eq!(u.focus, committed);
        let focused = (committed + 1) % u.options(item).len();
        u.focus = focused;
        assert_eq!(active_choice::index(&u, item), Some(committed));
        change(&mut u, focused);
        assert_eq!(active_choice::index(&u, item), Some(focused));
        assert_eq!(u.focus, focused);
        let back = u.dlg_layout().unwrap().back.unwrap();
        assert!(u.tap_menu(back.x + 1, back.y + 1));
        assert_eq!((u.menu, u.focus), (menu, 2));
        let back = u.dlg_layout().unwrap().back.unwrap();
        assert!(u.tap_menu(back.x + 1, back.y + 1));
        assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
        u.open_submenu(menu);
        let close = u.dlg_layout().unwrap().close;
        assert!(u.tap_menu(close.x + 1, close.y + 1));
        assert!(!u.menu_open);
    }
}

pub(super) fn capabilities(menu: MenuId, test: &str, check: fn()) {
    if let Ok(model) = std::env::var("OOS_SYSTEM_MENU_MODEL") {
        assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
        let mut u = demo();
        for size in TEXT_SIZES {
            u.m.text_size = size;
            u.open_menu(menu);
            assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.menu_items(menu));
            check();
        }
    } else {
        for model in ["DHO802", "DHO804", "DHO924S"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", test])
                .env("OOS_SYSTEM_MENU_MODEL", model)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(output.status.success(), "{model}: {stdout}\n{}", String::from_utf8_lossy(&output.stderr));
            assert!(stdout.contains("1 passed"), "{model}: capability test did not run: {stdout}");
        }
    }
}
