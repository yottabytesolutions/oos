// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn active_choice_identify_tap_keeps_the_menu_and_settings() {
    use oos_dsp::identify::{Finding, SignalKind, TriggerHint};
    let mut u = demo();
    u.azr.findings.push(Finding {
        channels: vec![0],
        kind: SignalKind::Dc { level: 1.0 },
        confidence: 1.0,
        summary: "A finding with a long description that needs the smaller font fallback".into(),
        trigger: TriggerHint::Edge { source: 0, level: 1.0, rising: true, holdoff: 0.0 },
        decoder: None,
    });
    u.m.trig_src = 3;
    u.open_menu(MenuId::Identify);
    let r = row(&u, Item::IdFind(0));
    assert!(u.tap_menu(r.ctrl.x + r.ctrl.w / 2, r.ctrl.y + r.ctrl.h / 2));
    assert!(u.menu_open);
    assert_eq!(u.menu, MenuId::Identify);
    assert_eq!(u.m.trig_src, 3);
    assert!(matches!(r.ctl, Ctl::Seg(ref opts, None) if opts == &[u.value(Item::IdFind(0))]));
    filled(&u, r.ctrl, false);
}

#[test]
fn active_choice_option_order_round_trips_through_selection() {
    let mut cases = 0;
    for menu in menus() {
        let fixture = demo();
        for it in fixture.items(menu) {
            if index(&fixture, it).is_none() {
                continue;
            }
            for (k, option) in fixture.options(it).iter().enumerate() {
                let mut u = demo();
                u.select_option(it, option);
                let selected = index(&u, it).unwrap();
                assert_eq!(u.options(it)[selected], committed_label(&u, it), "{menu:?} {it:?} option {k}");
                cases += 1;
            }
        }
    }
    assert!(cases >= 619, "only {cases} option cases checked");
    println!("active choice option round trips: {cases}");
}

fn committed_label(u: &Ui, it: Item) -> String {
    // These value readers decorate or spell their labels differently; the raw state identifies the option.
    match it {
        Item::CurMode if u.m.cursor == CursorMode::Xy => "XY".into(),
        Item::RemName => u.value(it).trim_end_matches(".local").into(),
        Item::TimeZone if u.sys.tz == crate::sysconf::DHCP => "DHCP".into(),
        _ => u.value(it),
    }
}

#[test]
fn active_choice_pairs_sources_keep_channel_colour_and_contrasting_text() {
    let mut u = demo();
    u.meas_src = WaveId::Ch(0);
    u.meas_src_b = WaveId::Ch(1);
    u.open_menu(MenuId::MeasPair);
    let d = u.dlg_layout().unwrap();
    let cv = rendered(&u);
    for ((_, r), id) in u.pair_header_sources(&d).into_iter().zip([u.meas_src, u.meas_src_b]) {
        assert_filled(&cv, r, true);
        assert!(
            (r.y + 5..r.bottom() - 5)
                .any(|y| (r.x + 2..r.x + 6).any(|x| cv.px[(y * cv.w + x) as usize] == u.wave_color(id))),
            "source {id:?} must retain its channel colour"
        );
    }
}
