// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

fn check_rows(menu: MenuId, title: &str, rows: &[(Item, Kind, &str, Vec<String>)]) {
    let u = demo();
    let open = Item::Open(menu);
    assert_eq!(Ui::kind(open), Kind::Action);
    assert_eq!(Ui::label(open), title);
    assert_eq!(u.value(open), "");
    assert!(u.options(open).is_empty());
    for (item, kind, label, options) in rows {
        assert_eq!(Ui::kind(*item), *kind, "{item:?}");
        assert_eq!(Ui::label(*item), *label, "{item:?}");
        assert_eq!(u.options(*item), *options, "{item:?}");
        if *kind == Kind::Action {
            assert_eq!(u.value(*item), "", "{item:?}");
        }
        if *kind != Kind::Choice && options.is_empty() {
            assert_eq!(active_choice::index(&u, *item), None, "{item:?}");
        }
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).into()).collect()
}

#[test]
fn acquire_rows_keep_kinds_labels_and_complete_option_domains() {
    use Item::*;
    use Kind::*;
    let u = demo();
    check_rows(
        MenuId::Acquire,
        "Acquire",
        &[
            (AcqMode, Choice, "Mode", strings(&["Normal", "Peak", "Average", "High res"])),
            (AcqAvg, Value, "Averages", AVERAGES.iter().map(ToString::to_string).collect()),
            (AcqAvgHw, Choice, "HW average", strings(&["Off", "On"])),
            (AcqDepth, Choice, "Memory", mdepths().iter().map(|d| d.0.into()).collect()),
            (
                AcqSeg,
                Choice,
                "Segments",
                u.seg_options().iter().map(|&n| if n == 0 { "Off".into() } else { n.to_string() }).collect(),
            ),
            (AcqSegHw, Choice, "HW segments", strings(&["Off", "On"])),
            (AcqSegIvl, Choice, "Seg. interval", SEG_IVLS.iter().map(|&x| seg_ivl_name(x)).collect()),
            (AcqSegTable, Action, "Segment times", vec![]),
            (AcqCores, Choice, "ADC cores", strings(&["Raw", "Matched"])),
            (AcqDrawPath, Choice, "Draw path", strings(&["Standard", "Batch"])),
            (AcqAlias, Choice, "Aliasing", vec![]),
            (Open(MenuId::Horizontal), Action, "Horizontal", vec![]),
        ],
    );
}

#[test]
fn mask_and_zone_rows_keep_order_kinds_labels_and_options() {
    use Item::*;
    use Kind::*;
    let mask = [
        (MaskOn, Choice, "Mask test", strings(&["Off", "On"])),
        (MaskSrc, Choice, "Source", ch_names()),
        (MaskInvert, Choice, "Invert", strings(&["Off", "On"])),
        (MaskX, Value, "Tolerance X", vec![]),
        (MaskY, Value, "Tolerance Y", vec![]),
        (MaskAction, Choice, "On fail", strings(&["None", "Stop"])),
        (MaskCreate, Action, "Create from CH", vec![]),
        (MaskFromRef, Action, "Create from ref", vec![]),
        (MaskReset, Action, "Reset counts", vec![]),
        (MaskClear, Action, "Clear mask", vec![]),
        (MaskSave, Action, "Save mask", vec![]),
        (MaskLoad, Action, "Load mask", vec![]),
    ];
    let zone = [
        (ZoneSel, Choice, "Zone", strings(&["Z1", "Z2"])),
        (ZoneOn, Choice, "Enable", strings(&["Off", "On"])),
        (ZoneSrc, Choice, "Source", ch_names()),
        (ZoneMode, Choice, "Condition", strings(&["Intersect", "Not intersect"])),
        (ZoneBox, Action, "Draw box", vec![]),
        (ZoneShape, Action, "Draw shape", vec![]),
        (ZoneClear, Action, "Delete", vec![]),
    ];
    for (menu, title, rows) in [(MenuId::Mask, "Mask", mask.as_slice()), (MenuId::Zone, "Zones", zone.as_slice())] {
        check_rows(menu, title, rows);
        assert_eq!(demo().menu_items(menu), rows.iter().map(|r| r.0).collect::<Vec<_>>());
    }
}

#[test]
fn acquire_mode_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for (index, mode) in [AcqMode::Normal, AcqMode::Peak, AcqMode::Average, AcqMode::HiRes].into_iter().enumerate() {
        u.m.acq = mode;
        assert_eq!(u.value(Item::AcqMode), ["Normal", "Peak", "Average", "High res"][index]);
        assert_eq!(active_choice::index(&u, Item::AcqMode), Some(index));
    }
}

#[test]
fn acquire_average_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for (index, count) in AVERAGES.into_iter().enumerate() {
        u.m.avg_count = count;
        assert_eq!(u.value(Item::AcqAvg), count.to_string());
        assert_eq!(active_choice::index(&u, Item::AcqAvg), Some(index));
    }
}

#[test]
fn acquire_depth_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for (index, (label, _)) in mdepths().iter().enumerate() {
        u.m.mdepth = index;
        assert_eq!(u.value(Item::AcqDepth), *label);
        assert_eq!(active_choice::index(&u, Item::AcqDepth), Some(index));
    }
}

#[test]
fn acquire_segment_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for (index, count) in u.seg_options().into_iter().enumerate() {
        u.m.segments = count;
        assert_eq!(u.value(Item::AcqSeg), if count == 0 { "Off".into() } else { count.to_string() });
        assert_eq!(active_choice::index(&u, Item::AcqSeg), Some(index));
    }
    u.m.segments = usize::MAX;
    assert_eq!(u.value(Item::AcqSeg), u.seg_limit().to_string());
}

#[test]
fn acquire_interval_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for (index, interval) in SEG_IVLS.into_iter().enumerate() {
        u.m.seg_ivl = interval;
        assert_eq!(u.value(Item::AcqSegIvl), seg_ivl_name(interval));
        assert_eq!(active_choice::index(&u, Item::AcqSegIvl), Some(index));
    }
}

#[test]
fn acquire_toggle_values_and_committed_indices_follow_current_settings() {
    let mut u = demo();
    for on in [false, true] {
        (u.m.avg_hw, u.m.seg_hw, u.m.cores, u.m.draw_batch) = (on, on, on, on);
        for (item, values) in [
            (Item::AcqAvgHw, ["Off", "On"]),
            (Item::AcqSegHw, ["Off", "On"]),
            (Item::AcqCores, ["Raw", "Matched"]),
            (Item::AcqDrawPath, ["Standard", "Batch"]),
        ] {
            assert_eq!(u.value(item), values[usize::from(on)]);
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
    }
}

#[test]
fn mask_and_zone_values_and_committed_indices_follow_selected_state() {
    let mut u = demo();
    for on in [false, true] {
        (u.m.mask_on, u.m.mask_inv, u.m.mask_stop) = (on, on, on);
        for (item, values) in
            [(Item::MaskOn, ["Off", "On"]), (Item::MaskInvert, ["Off", "On"]), (Item::MaskAction, ["None", "Stop"])]
        {
            assert_eq!(u.value(item), values[usize::from(on)]);
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
        for selected in 0..2 {
            u.zone_sel = selected;
            assert_eq!(u.value(Item::ZoneSel), format!("Z{}", selected + 1));
            assert_eq!(active_choice::index(&u, Item::ZoneSel), Some(selected));
            u.m.zones[selected].on = on;
            u.m.zones[selected].inside = on;
            assert_eq!(u.value(Item::ZoneOn), if on { "On" } else { "Off" });
            assert_eq!(u.value(Item::ZoneMode), if on { "Intersect" } else { "Not intersect" });
            assert_eq!(active_choice::index(&u, Item::ZoneOn), Some(usize::from(on)));
            assert_eq!(active_choice::index(&u, Item::ZoneMode), Some(usize::from(!on)));
        }
    }
    for (x, y) in [(0.0, 0.04), (1.234, 2.345), (2.0, 4.0)] {
        (u.m.mask_x, u.m.mask_y) = (x, y);
        assert_eq!(u.value(Item::MaskX), format!("{x:.2} div"));
        assert_eq!(u.value(Item::MaskY), format!("{y:.2} div"));
    }
}

#[test]
fn acquire_visibility_keeps_conditional_rows_in_order() {
    let mut segment_visibility = [false; 2];
    let mut alias_visibility = [false; 2];
    for hardware in [false, true] {
        let source =
            if hardware { Source::Hw { shared: Default::default(), seen: 0 } } else { Source::Demo { t: 0.0 } };
        let mut u = Ui::for_tests(source);
        for mode in [AcqMode::Normal, AcqMode::Peak, AcqMode::Average, AcqMode::HiRes] {
            u.m.acq = mode;
            for segments in [0, 10] {
                u.m.segments = segments;
                for hw in [false, true] {
                    u.m.seg_hw = hw;
                    for scale in [0, tscales().len() - 1] {
                        u.m.tscale = scale;
                        segment_visibility[usize::from(u.seg_hw_on())] = true;
                        alias_visibility[usize::from(u.alias_above().is_some())] = true;
                        check_acquire_visibility(&u, hardware, mode == AcqMode::Average, segments > 0);
                    }
                }
            }
        }
    }
    assert_eq!(segment_visibility, [true, true]);
    assert_eq!(alias_visibility, [true, true]);
}

fn check_acquire_visibility(u: &Ui, hardware: bool, average: bool, segments: bool) {
    use Item::*;
    let mut expected = vec![AcqMode];
    if average {
        expected.push(AcqAvg);
        if hardware {
            expected.push(AcqAvgHw);
        }
    }
    expected.extend([AcqDepth, AcqSeg]);
    if segments {
        expected.push(AcqSegHw);
    }
    if u.seg_hw_on() {
        expected.extend([AcqSegIvl, AcqSegTable]);
    }
    expected.extend([AcqCores, AcqDrawPath]);
    if let Some(frequency) = u.alias_above() {
        expected.push(AcqAlias);
        assert_eq!(u.value(AcqAlias), format!("above {}, Peak shows it", si_short(frequency, "Hz")));
    } else {
        assert_eq!(u.value(AcqAlias), "");
    }
    assert_eq!(active_choice::index(u, AcqAlias), None);
    expected.push(Open(MenuId::Horizontal));
    assert_eq!(u.menu_items(MenuId::Acquire), expected);
    assert_eq!(u.items(MenuId::Acquire), expected);
}

fn check_row_control(row: &Row) {
    assert!(match (row.it, Ui::kind(row.it)) {
        (Item::AcqAlias, _) => matches!(row.ctl, Ctl::Text),
        (Item::AcqAvg, _) => matches!(row.ctl, Ctl::Pick),
        (_, Kind::Text) => matches!(row.ctl, Ctl::Text),
        (_, Kind::Value) => matches!(row.ctl, Ctl::Value),
        (_, Kind::Action) => matches!(row.ctl, Ctl::Button),
        (_, Kind::Choice) => matches!(row.ctl, Ctl::Pick | Ctl::Seg(..)),
    });
}

#[test]
fn new_areas_use_shared_renderer_picker_back_and_close_at_every_text_size() {
    for size in TEXT_SIZES {
        for (menu, item) in
            [(MenuId::Acquire, Item::AcqMode), (MenuId::Mask, Item::MaskAction), (MenuId::Zone, Item::ZoneSel)]
        {
            let mut u = demo();
            u.m.text_size = size;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            (u.m.acq, u.m.mask_stop, u.zone_sel) = (AcqMode::Average, true, 1);
            let rows = u.dlg_layout().unwrap().rows;
            assert_eq!(rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            for row in rows {
                check_row_control(&row);
            }
            let committed = active_choice::index(&u, item).unwrap();
            assert!(matches!(u.ctl(item, 1), Ctl::Pick));
            assert!(matches!(u.ctl(item, 1000), Ctl::Seg(_, Some(k)) if k == committed));
            u.open_picker(item);
            assert_eq!(u.focus, committed);
            u.focus = 1;
            (u.m.acq, u.m.mask_stop, u.zone_sel) = (AcqMode::Normal, false, 0);
            assert_eq!(active_choice::index(&u, item), Some(0));
            assert_eq!(u.focus, 1);
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
fn channel_capabilities_in_an_isolated_process() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        for fixture in ["layout_capability_fixture", "source_capability_fixture", "off_sources_capability_fixture"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &format!("ui::menu::tests::{fixture}"), "--nocapture"])
                .env("OOS_MENU_TEST_MODEL", model)
                .output()
                .unwrap();
            assert!(output.status.success(), "{model} {fixture}: {}", String::from_utf8_lossy(&output.stdout));
        }
    }
}

fn capability_ui() -> Option<(Ui, usize)> {
    let model = std::env::var("OOS_MENU_TEST_MODEL").ok()?;
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let channels = if model == "DHO802" { 2 } else { 4 };
    Some((demo(), channels))
}

#[test]
fn layout_capability_fixture() {
    let Some((mut u, _)) = capability_ui() else { return };
    for size in TEXT_SIZES {
        u.m.text_size = size;
        for offline in [false, true] {
            u.offline = offline;
            for menu in [MenuId::Acquire, MenuId::Mask, MenuId::Zone] {
                u.open_menu(menu);
                assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            }
        }
    }
}

#[test]
fn source_capability_fixture() {
    let Some((mut u, channels)) = capability_ui() else { return };
    for size in TEXT_SIZES {
        u.m.text_size = size;
        for offline in [false, true] {
            u.offline = offline;
            for selected in 0..2 {
                u.zone_sel = selected;
                for source in 0..channels {
                    u.m.mask_src = source;
                    u.m.zones[selected].src = source;
                    for item in [Item::MaskSrc, Item::ZoneSrc] {
                        assert_eq!(u.options(item), ["CH1", "CH2", "CH3", "CH4"][..channels]);
                        assert_eq!(u.value(item), format!("CH{}", source + 1));
                        assert_eq!(active_choice::index(&u, item), Some(source));
                    }
                }
            }
        }
    }
}

#[test]
fn off_sources_capability_fixture() {
    let Some((mut u, _)) = capability_ui() else { return };
    for size in TEXT_SIZES {
        u.m.text_size = size;
        for offline in [false, true] {
            u.offline = offline;
            u.m.ch[0].on = false;
            u.m.mask_src = 0;
            assert_eq!(u.off_sources(MenuId::Mask), vec![0]);
            assert_eq!(u.items(MenuId::Mask).last(), Some(&Item::SrcOn));
            assert!(u.off_sources(MenuId::Zone).is_empty());
        }
    }
}

#[test]
fn ordered_descriptors_take_precedence_and_apply_visibility() {
    const ROWS: &[Descriptor] = &[
        Descriptor::action(Item::Shot, "Screenshot"),
        Descriptor::action(Item::About, "About OOS").when(|u| !u.offline),
        Descriptor::title(Item::Open(MenuId::Utility), "System"),
    ];
    let area = Area {
        ordered_rows: |_, _| vec![&ROWS[2], &ROWS[1], &ROWS[0]],
        rows: |_, _| panic!("ordered rows take precedence"),
        menu_items: |_, _| panic!("descriptors take precedence over legacy items"),
        ..Area::EMPTY
    };
    let mut u = demo();
    assert_eq!(area.items(&u, MenuId::Utility), [Item::About, Item::Shot]);
    u.offline = true;
    assert_eq!(area.items(&u, MenuId::Utility), [Item::Shot]);
}

#[test]
fn static_descriptors_take_precedence_and_apply_visibility() {
    const ROWS: &[Descriptor] = &[
        Descriptor::title(Item::Open(MenuId::Utility), "System"),
        Descriptor::action(Item::Shot, "Screenshot").when(|u| !u.offline),
        Descriptor::action(Item::About, "About OOS"),
    ];
    let area = Area {
        descriptors: ROWS,
        menu_items: |_, _| panic!("descriptors take precedence over legacy items"),
        ..Area::EMPTY
    };
    let mut u = demo();
    assert_eq!(area.items(&u, MenuId::Utility), [Item::Shot, Item::About]);
    u.offline = true;
    assert_eq!(area.items(&u, MenuId::Utility), [Item::About]);
    let area = Area { rows: |_, _| ROWS, descriptors: &[], ..area };
    assert_eq!(area.items(&u, MenuId::Utility), [Item::About]);
    let area = Area { menu_items: |_, _| vec![Item::Stock], ..Area::EMPTY };
    assert_eq!(area.items(&u, MenuId::Utility), [Item::Stock]);
}
