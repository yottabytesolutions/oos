// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const BW: [Item; 4] = [Item::BwOn, Item::BwSrc, Item::BwHint, Item::BwBins];
const MAG: [Item; 9] = [
    Item::MagOn,
    Item::MagV,
    Item::MagI,
    Item::MagN,
    Item::MagAe,
    Item::MagLe,
    Item::MagRcu,
    Item::MagPlot,
    Item::MagCur,
];
const DPT: [Item; 8] =
    [Item::DptOn, Item::DptV, Item::DptI, Item::DptG, Item::DptFourth, Item::Dpt4, Item::DptPct, Item::DptZoom];

fn demo() -> Ui {
    Ui::for_tests(Source::Demo { t: 0.0 })
}

#[test]
fn every_row_preserves_its_kind_label_and_option_domain() {
    let u = demo();
    let rows = [
        (
            BW.as_slice(),
            vec![
                (Kind::Choice, "View", vec!["Off".into(), "On".into()]),
                (Kind::Choice, "Source", ch_names()),
                (
                    Kind::Choice,
                    "Signal freq",
                    bwtest::HINTS.iter().map(|&h| if h == 0.0 { "Auto".into() } else { si_short(h, "Hz") }).collect(),
                ),
                (Kind::Choice, "Bins / period", [250, 500, 1000, 2000, 5000].map(|b| b.to_string()).to_vec()),
            ],
        ),
        (
            MAG.as_slice(),
            vec![
                (Kind::Choice, "View", vec!["Off".into(), "On".into()]),
                (Kind::Choice, "Voltage", u.wave_names()),
                (Kind::Choice, "Current", u.wave_names()),
                (Kind::Value, "Turns", vec![]),
                (Kind::Value, "Area", vec![]),
                (Kind::Value, "Length", vec![]),
                (Kind::Value, "Rcu", vec![]),
                (Kind::Choice, "Plot", vec!["I-ψ".into()]),
                (Kind::Value, "Cursor", vec![]),
            ],
        ),
        (
            DPT.as_slice(),
            vec![
                (Kind::Choice, "View", vec!["Off".into(), "On".into()]),
                (Kind::Choice, "Vds", u.wave_names()),
                (Kind::Choice, "Id", u.wave_names()),
                (Kind::Choice, "Vgs", u.wave_names()),
                (Kind::Choice, "Fourth", vec!["Off".into(), "Gate".into(), "Diode V".into(), "Diode I".into()]),
                (Kind::Choice, "Channel", u.wave_names()),
                (Kind::Value, "Gate", vec![]),
                (Kind::Choice, "Zoom", vec!["Both".into(), "Turn-on".into(), "Turn-off".into()]),
            ],
        ),
    ];
    for (items, expected) in rows {
        for (&item, (kind, label, options)) in items.iter().zip(expected) {
            assert_eq!(Ui::kind(item), kind, "{item:?}");
            assert_eq!(Ui::label(item), label, "{item:?}");
            assert_eq!(u.options(item), options, "{item:?}");
            if kind == Kind::Value {
                assert_eq!(active_choice::index(&u, item), None);
            }
        }
    }
}

#[test]
fn submenu_entries_preserve_their_titles_and_empty_values() {
    let u = demo();
    for (menu, label) in [(MenuId::BwTest, "Bandwidth"), (MenuId::Magnetic, "Magnetic"), (MenuId::Dpt, "Double pulse")]
    {
        let item = Item::Open(menu);
        assert_eq!(Ui::kind(item), Kind::Action);
        assert_eq!(Ui::label(item), label);
        assert_eq!(u.value(item), "");
        assert!(u.options(item).is_empty());
    }
}

#[test]
fn values_and_committed_choices_follow_settings_even_while_focus_is_elsewhere() {
    let mut u = demo();
    for view in [AView::Scope, AView::Step, AView::Magnetic, AView::Dpt] {
        u.m.view = view;
        for (item, selected) in [(Item::BwOn, AView::Step), (Item::MagOn, AView::Magnetic), (Item::DptOn, AView::Dpt)] {
            assert_eq!(u.value(item), if view == selected { "On" } else { "Off" });
            assert_eq!(active_choice::index(&u, item), Some(usize::from(view == selected)));
        }
    }
    for (index, hint) in bwtest::HINTS.into_iter().enumerate() {
        u.m.bw_hint = hint;
        assert_eq!(u.value(Item::BwHint), if hint == 0.0 { "Auto".into() } else { si_short(hint, "Hz") });
        assert_eq!(active_choice::index(&u, Item::BwHint), Some(index));
    }
    for (index, bins) in bwtest::BINS.into_iter().enumerate() {
        u.m.bw_bins = bins;
        assert_eq!(u.value(Item::BwBins), bins.to_string());
        assert_eq!(active_choice::index(&u, Item::BwBins), Some(index));
    }
    for (index, fourth) in
        [DptFourth::Off, DptFourth::Gate, DptFourth::DiodeV, DptFourth::DiodeI].into_iter().enumerate()
    {
        u.m.dpt_4th = fourth;
        assert_eq!(u.value(Item::DptFourth), ["Off", "Gate", "Diode V", "Diode I"][index]);
        assert_eq!(active_choice::index(&u, Item::DptFourth), Some(index));
        let expected: Vec<_> = DPT.into_iter().filter(|&it| it != Item::Dpt4 || fourth != DptFourth::Off).collect();
        assert_eq!(u.menu_items(MenuId::Dpt), expected);
    }
    for (index, zoom) in [DptZoom::Both, DptZoom::TurnOn, DptZoom::TurnOff].into_iter().enumerate() {
        u.m.dpt_zoom = zoom;
        assert_eq!(u.value(Item::DptZoom), ["Both", "Turn-on", "Turn-off"][index]);
        assert_eq!(active_choice::index(&u, Item::DptZoom), Some(index));
    }
    for pct in [1.0, 25.0, 49.0] {
        u.m.dpt_pct = pct;
        assert_eq!(u.value(Item::DptPct), format!("{pct:.0} %"));
    }
    for n in [1.0, 123.0, 10_000.0] {
        u.m.mag_n = n;
        assert_eq!(u.value(Item::MagN), format!("{n:.0}"));
    }
    u.m.mag_cur = 123;
    assert_eq!(u.value(Item::MagCur), "123");
}

#[test]
fn magnetic_geometry_and_result_visibility_keep_their_boundaries() {
    let mut u = demo();
    assert_eq!(u.menu_items(MenuId::Magnetic), MAG[..8]);
    for loops in [vec![], vec![vec![]], vec![vec![(0.0, 1.0)], vec![]]] {
        let mut result = oos_dsp::magnetic::measure(&[], &[], 1e-6, Default::default());
        result.loops = loops.clone();
        u.azr.mag = Some(result);
        let count = if loops.iter().any(|l| !l.is_empty()) { 9 } else { 8 };
        assert_eq!(u.menu_items(MenuId::Magnetic), MAG[..count]);
    }
    for area in [-1.0, 0.0, 1e-4] {
        for length in [-1.0, 0.0, 0.05] {
            (u.m.mag_ae, u.m.mag_le) = (area, length);
            assert_eq!(u.value(Item::MagAe), if area > 0.0 { si(area, "m²") } else { "off".into() });
            assert_eq!(u.value(Item::MagLe), if length > 0.0 { si(length, "m") } else { "off".into() });
            let geometry = area > 0.0 && length > 0.0;
            for plot in [MagPlot::IPsi, MagPlot::BH] {
                u.m.mag_plot = plot;
                let bh = geometry && plot == MagPlot::BH;
                assert_eq!(u.value(Item::MagPlot), if bh { "B-H" } else { "I-ψ" });
                assert_eq!(active_choice::index(&u, Item::MagPlot), Some(usize::from(bh)));
                assert_eq!(u.options(Item::MagPlot), if geometry { vec!["I-ψ", "B-H"] } else { vec!["I-ψ"] });
            }
        }
    }
    for resistance in [-1.0, 0.0, 0.123] {
        u.m.mag_rcu = resistance;
        assert_eq!(u.value(Item::MagRcu), if resistance > 0.0 { si(resistance, "Ω") } else { "0 Ω".into() });
    }
}

#[test]
fn source_choices_and_source_on_rows_preserve_channel_and_reference_identity() {
    let mut u = demo();
    u.refs[2] = Some(RefWave {
        src: 0,
        color: 0,
        label: "Saved".into(),
        t0: 0.0,
        dt: 1e-6,
        v: vec![0.0, 1.0].into(),
        vpd: 1.0,
        offset: 0.0,
        on: false,
    });
    let mut names = ch_names();
    names.push("R3".into());
    for (index, source) in u.wave_ids().into_iter().enumerate() {
        (u.m.mag_v, u.m.mag_i) = (source, source);
        (u.m.dpt_v, u.m.dpt_i, u.m.dpt_g, u.m.dpt_4ch) = (source, source, source, source);
        for item in [Item::MagV, Item::MagI, Item::DptV, Item::DptI, Item::DptG, Item::Dpt4] {
            assert_eq!(u.options(item), names);
            assert_eq!(u.value(item), names[index]);
            assert_eq!(active_choice::index(&u, item), Some(index));
        }
    }
    for enabled in 0..16 {
        for c in 0..n_ch() {
            u.m.ch[c].on = enabled & (1 << c) != 0;
        }
        u.m.bw_src = 0;
        (u.m.mag_v, u.m.mag_i) = (WaveId::Ch(0), WaveId::Ref(2));
        (u.m.dpt_v, u.m.dpt_i, u.m.dpt_g, u.m.dpt_4ch) = (WaveId::Ch(0), WaveId::Ch(1), WaveId::Ref(2), WaveId::Ch(2));
        for fourth in [DptFourth::Off, DptFourth::Gate] {
            u.m.dpt_4th = fourth;
            for menu in [MenuId::BwTest, MenuId::Magnetic, MenuId::Dpt] {
                let expected: Vec<_> = (0..if menu == MenuId::Dpt {
                    if fourth == DptFourth::Off {
                        2
                    } else {
                        3
                    }
                } else {
                    1
                })
                    .filter(|&c| enabled & (1 << c) == 0)
                    .collect();
                assert_eq!(u.off_sources(menu), expected);
                let mut items = u.menu_items(menu);
                if !expected.is_empty() {
                    items.push(Item::SrcOn);
                }
                assert_eq!(u.items(menu), items);
            }
        }
        assert_eq!(u.menu_items(MenuId::BwTest), BW);
        assert_eq!(u.value(Item::BwSrc), "CH1");
        assert_eq!(active_choice::index(&u, Item::BwSrc), Some(0));
    }
}

#[test]
fn capabilities_render_and_navigate_in_isolated_processes() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::bandwidth::tests::capability_fixture", "--nocapture"])
            .env("OOS_READ_AREAS_MODEL", model)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{model}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_READ_AREAS_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    let channels = if model == "DHO802" { 2 } else { 4 };
    let mut u = demo();
    for c in 0..channels {
        u.m.bw_src = c;
        assert_eq!(u.value(Item::BwSrc), format!("CH{}", c + 1));
        assert_eq!(active_choice::index(&u, Item::BwSrc), Some(c));
    }
    assert_eq!(u.options(Item::BwSrc), ch_names());
    assert_eq!(u.options(Item::MagV).len(), channels);
    assert_eq!(u.options(Item::DptV).len(), channels);
    for size in TEXT_SIZES {
        for (menu, item, title) in [
            (MenuId::BwTest, Item::BwHint, "bandwidth"),
            (MenuId::Magnetic, Item::MagPlot, "magnetic"),
            (MenuId::Dpt, Item::DptFourth, "dpt"),
        ] {
            check_navigation_and_picture(&model, size, menu, item, title);
        }
    }
}

fn check_navigation_and_picture(model: &str, size: TextSize, menu: MenuId, item: Item, title: &str) {
    let mut u = demo();
    u.m.text_size = size;
    (u.m.bw_hint, u.m.mag_ae, u.m.mag_le, u.m.mag_plot, u.m.dpt_4th) = (5e6, 1e-4, 0.05, MagPlot::BH, DptFourth::Gate);
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
    let committed = active_choice::index(&u, item).unwrap();
    assert!(matches!(u.ctl(item, 1), Ctl::Pick));
    assert!(matches!(u.ctl(item, 10000), Ctl::Seg(_, Some(k)) if k == committed));
    let mut picture = Canvas::new(u.scr.w * 2, u.scr.h * 2);
    capture(&u, &mut picture, 0);
    u.open_picker(item);
    assert_eq!(u.focus, committed);
    u.focus = 1;
    (u.m.bw_hint, u.m.mag_plot, u.m.dpt_4th) = (0.0, MagPlot::IPsi, DptFourth::Off);
    assert_eq!(active_choice::index(&u, item), Some(0));
    assert_eq!(u.focus, 1);
    capture(&u, &mut picture, 1);
    let back = u.dlg_layout().unwrap().back.unwrap();
    assert!(u.tap_menu(back.x + 1, back.y + 1));
    assert_eq!((u.menu, u.focus), (menu, 1));
    capture(&u, &mut picture, 2);
    let back = u.dlg_layout().unwrap().back.unwrap();
    assert!(u.tap_menu(back.x + 1, back.y + 1));
    assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
    u.open_submenu(menu);
    let close = u.dlg_layout().unwrap().close;
    assert!(u.tap_menu(close.x + 1, close.y + 1));
    assert!(!u.menu_open);
    capture(&u, &mut picture, 3);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = root.parent().unwrap().join("target/read-areas-golden");
    std::fs::create_dir_all(&output).unwrap();
    let name = format!("{model}-{size:?}-{title}.png.ref");
    let png = crate::store::png_of(&picture);
    std::fs::write(output.join(&name), &png).unwrap();
    let reference = root.join("tests/golden/read-areas").join(&name);
    let expected = std::fs::read(reference).expect("read-area golden reference must exist");
    assert!(png == expected, "{name}: menu pictures changed; inspect read-areas-golden");
}

fn capture(u: &Ui, picture: &mut Canvas, panel: usize) {
    let (w, h) = (u.scr.w as usize, u.scr.h as usize);
    let mut cv = Canvas::new(u.scr.w, u.scr.h);
    u.draw_menu(&mut cv);
    let x = panel % 2 * w;
    let y = panel / 2 * h;
    for row in 0..h {
        let start = (y + row) * picture.w as usize + x;
        picture.px[start..start + w].copy_from_slice(&cv.px[row * w..(row + 1) * w]);
    }
}
