// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const ITEMS: [Item; 11] = [
    Item::HScale,
    Item::HFine,
    Item::HExpandI,
    Item::HDelay,
    Item::HZero,
    Item::AutoDeskew,
    Item::ClearDeskew,
    Item::Zoom,
    Item::ZScale,
    Item::ZDelay,
    Item::AutoRoll,
];

#[test]
fn every_horizontal_row_keeps_its_kind_label_and_options() {
    let u = Ui::for_tests(Source::Demo { t: 0.0 });
    let expected = [
        (Kind::Value, "Time/div", vec![]),
        (Kind::Choice, "Fine time base", vec!["Off", "On"]),
        (Kind::Choice, "Expand around", vec!["Center", "Left", "Right", "Trigger"]),
        (Kind::Value, "Delay", vec![]),
        (Kind::Action, "Delay to 0", vec![]),
        (Kind::Action, "Auto deskew", vec![]),
        (Kind::Action, "Clear deskew", vec![]),
        (Kind::Choice, "Zoom", vec!["Off", "On"]),
        (Kind::Value, "Zoom time/div", vec![]),
        (Kind::Value, "Zoom position", vec![]),
        (Kind::Choice, "Auto Roll", vec!["Off", "On"]),
    ];
    for (item, (kind, label, options)) in ITEMS.into_iter().zip(expected) {
        assert_eq!(Ui::kind(item), kind, "{item:?}");
        assert_eq!(Ui::label(item), label, "{item:?}");
        assert_eq!(u.options(item), options, "{item:?}");
        if kind != Kind::Choice {
            assert_eq!(active_choice::index(&u, item), None);
        }
        if kind == Kind::Action {
            assert_eq!(u.value(item), "");
        }
    }
    let open = Item::Open(MenuId::Horizontal);
    assert_eq!(Ui::kind(open), Kind::Action);
    assert_eq!(Ui::label(open), "Horizontal");
    assert_eq!(u.value(open), "");
    assert!(u.options(open).is_empty());
}

#[test]
fn horizontal_values_and_visibility_follow_current_settings() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    for on in [false, true] {
        (u.m.fine_t, u.m.zoom) = (on, on);
        u.m.auto_acquisition.set_roll(on);
        for item in [Item::HFine, Item::Zoom, Item::AutoRoll] {
            assert_eq!(u.value(item), if on { "On" } else { "Off" });
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
        let expected: Vec<_> =
            ITEMS.into_iter().filter(|it| on || !matches!(it, Item::ZScale | Item::ZDelay)).collect();
        assert_eq!(u.menu_items(MenuId::Horizontal), expected);
        assert_eq!(u.items(MenuId::Horizontal), expected);
        for scale in [0, tscales().len() / 2, tscales().len() - 1] {
            (u.m.tscale, u.m.zscale) = (scale, scale);
            u.m.tfine = if on { 1.23 } else { 1.0 };
            u.m.delay = -0.0025;
            u.m.zdelay = 0.00003;
            assert_eq!(u.value(Item::HScale), si_short(u.m.time_per_div() as f64, "s"));
            assert_eq!(u.value(Item::ZScale), si_short(tscales()[scale] as f64, "s"));
            assert_eq!(u.value(Item::HDelay), si(u.m.delay as f64, "s"));
            assert_eq!(u.value(Item::ZDelay), si(u.m.zdelay as f64, "s"));
        }
    }
    for (index, (expand, label)) in
        [(HExpand::Center, "Center"), (HExpand::Left, "Left"), (HExpand::Right, "Right"), (HExpand::Trigger, "Trigger")]
            .into_iter()
            .enumerate()
    {
        u.m.h_expand = expand;
        assert_eq!(u.value(Item::HExpandI), label);
        assert_eq!(active_choice::index(&u, Item::HExpandI), Some(index));
    }
}

#[test]
fn both_areas_render_and_navigate_without_confusing_focus_and_choice() {
    for size in TEXT_SIZES {
        for (menu, item) in [(MenuId::Horizontal, Item::HExpandI), (MenuId::Cursor, Item::CurMode)] {
            let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
            u.m.text_size = size;
            u.m.zoom = true;
            u.m.cursor = CursorMode::Xy;
            u.m.h_expand = HExpand::Trigger;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            let rows = u.dlg_layout().unwrap().rows;
            assert_eq!(rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            for row in rows {
                assert!(match Ui::kind(row.it) {
                    Kind::Text => matches!(row.ctl, Ctl::Text),
                    Kind::Value => matches!(row.ctl, Ctl::Value),
                    Kind::Action => matches!(row.ctl, Ctl::Button),
                    Kind::Choice => matches!(row.ctl, Ctl::Pick | Ctl::Seg(..)),
                });
            }
            assert!(matches!(u.ctl(item, 1), Ctl::Pick));
            let committed = active_choice::index(&u, item).unwrap();
            assert!(matches!(u.ctl(item, 1000), Ctl::Seg(_, Some(k)) if k == committed));
            u.open_picker(item);
            assert_eq!(u.focus, committed);
            u.focus = 1;
            u.m.h_expand = HExpand::Center;
            u.m.cursor = CursorMode::Off;
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
