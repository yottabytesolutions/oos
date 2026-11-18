// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const MODES: [CursorMode; 5] = [CursorMode::Off, CursorMode::Time, CursorMode::Volt, CursorMode::Track, CursorMode::Xy];

#[test]
fn every_cursor_row_keeps_its_kind_label_and_options() {
    let u = Ui::for_tests(Source::Demo { t: 0.0 });
    let expected = [
        (Item::CurMode, Kind::Choice, "Mode", vec!["Off", "Time", "Volt", "Track", "XY"]),
        (Item::CurA, Kind::Value, "Cursor A", vec![]),
        (Item::CurB, Kind::Value, "Cursor B", vec![]),
        (Item::CurC, Kind::Value, "Y1", vec![]),
        (Item::CurD, Kind::Value, "Y2", vec![]),
        (Item::CurSrcB, Kind::Choice, "Cursor B source", vec!["CH1", "CH2", "CH3", "CH4"]),
    ];
    for (item, kind, label, options) in expected {
        assert_eq!(Ui::kind(item), kind, "{item:?}");
        assert_eq!(Ui::label(item), label, "{item:?}");
        assert_eq!(u.options(item), options, "{item:?}");
        if kind == Kind::Value {
            assert_eq!(active_choice::index(&u, item), None);
        }
    }
    let open = Item::Open(MenuId::Cursor);
    assert_eq!(Ui::kind(open), Kind::Action);
    assert_eq!(Ui::label(open), "Cursor");
    assert_eq!(u.value(open), "");
    assert!(u.options(open).is_empty());
}

#[test]
fn every_cursor_mode_keeps_its_value_and_ordered_visible_rows() {
    use Item::*;
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    let rows = [
        vec![CurMode],
        vec![CurMode, CurA, CurB],
        vec![CurMode, CurA, CurB],
        vec![CurMode, CurA, CurB, CurSrcB],
        vec![CurMode, CurA, CurB, CurC, CurD],
    ];
    for (index, ((mode, value), expected)) in
        MODES.into_iter().zip(["Off", "Time", "Volt", "Track", "Xy"]).zip(rows).enumerate()
    {
        u.m.cursor = mode;
        assert_eq!(u.value(CurMode), value);
        assert_eq!(active_choice::index(&u, CurMode), Some(index));
        assert_eq!(u.menu_items(MenuId::Cursor), expected);
        assert_eq!(u.items(MenuId::Cursor), expected);
    }
}

#[test]
fn cursor_positions_use_percentages_without_readouts_and_physical_values_with_them() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    (u.m.cur_a, u.m.cur_b, u.m.cur_c, u.m.cur_d) = (0.12, 0.34, 0.56, 0.78);
    u.m.cursor = CursorMode::Off;
    for (item, value) in
        [Item::CurA, Item::CurB, Item::CurC, Item::CurD].into_iter().zip(["12 %", "34 %", "56 %", "78 %"])
    {
        assert_eq!(u.value(item), value);
    }
    u.step(0.02);
    for mode in MODES.into_iter().skip(1) {
        u.m.cursor = mode;
        let readouts = u.cursor_values();
        assert!(!readouts.is_empty());
        let b = if mode == CursorMode::Track { 2 } else { 1 };
        for (item, index) in [(Item::CurA, 0), (Item::CurB, b), (Item::CurC, 2), (Item::CurD, 3)] {
            if let Some((_, value)) = readouts.get(index) {
                assert_eq!(u.value(item), *value, "{mode:?} {item:?}");
            } else {
                assert_eq!(u.value(item), "78 %");
            }
        }
    }
}

#[test]
fn cursor_source_options_and_committed_identity_include_loaded_references() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    u.refs[2] = Some(RefWave {
        src: 0,
        color: 0,
        label: "Saved trace".into(),
        t0: 0.0,
        dt: 1e-6,
        v: vec![0.0, 1.0].into(),
        vpd: 1.0,
        offset: 0.0,
        on: false,
    });
    let expected = ["CH1", "CH2", "CH3", "CH4", "R3"];
    assert_eq!(u.options(Item::CurSrcB), expected);
    assert_eq!(u.value(Item::CurSrcB), "CH1");
    for (index, source) in (0..n_ch()).map(WaveId::Ch).chain([WaveId::Ref(2)]).enumerate() {
        assert!(u.set_cursor_b_source(source));
        assert_eq!(u.value(Item::CurSrcB), expected[index]);
        assert_eq!(active_choice::index(&u, Item::CurSrcB), Some(index));
    }
    u.refs[2] = None;
    assert_eq!(u.options(Item::CurSrcB), &expected[..4]);
    assert_eq!(active_choice::index(&u, Item::CurSrcB), None);
}

#[test]
fn cursor_channel_capabilities_in_isolated_processes() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::cursor::tests::capability_fixture", "--nocapture"])
            .env("OOS_CURSOR_TEST_MODEL", model)
            .output()
            .unwrap();
        assert!(output.status.success(), "{model}: {}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_CURSOR_TEST_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let channels = if model == "DHO802" { 2 } else { 4 };
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    for size in TEXT_SIZES {
        u.m.text_size = size;
        assert_eq!(u.options(Item::CurSrcB), ["CH1", "CH2", "CH3", "CH4"][..channels]);
        for channel in 0..channels {
            u.select_option(Item::CurSrcB, &format!("CH{}", channel + 1));
            assert_eq!(u.value(Item::CurSrcB), format!("CH{}", channel + 1));
            assert_eq!(active_choice::index(&u, Item::CurSrcB), Some(channel));
        }
    }
}
