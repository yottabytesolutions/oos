// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;
use crate::model::TrigType as TT;

pub(super) fn expected_items(ui: &Ui) -> Vec<Item> {
    use Item::*;
    let m = &ui.m;
    let mut v = vec![Item::TrigType];
    match m.trig_type {
        // an LA line has no level of its own (LA page: pod threshold)
        TT::Edge if m.trig_line().is_some() => v.extend([TrigSrc, TrigSlope]),
        TT::Edge => v.extend([TrigSrc, TrigSlope, TrigLevel, Trig50, TrigCoup]),
        // the protocol and its sources come from the Decode menu
        TT::Bus => {
            v.extend([DecProto, TrigBusWhat]);
            if matches!(m.trig_bus, BusWhat::Id | BusWhat::Data) {
                v.push(TrigBusVal);
            }
            v.extend(ui.serial_items());
        }
        TT::Pulse => {
            v.extend([TrigSrc, TrigPol]);
            condition_items(m, &mut v);
            v.push(TrigLevel);
        }
        TT::Slope => {
            v.extend([TrigSrc, TrigSlope]);
            condition_items(m, &mut v);
            v.extend([TrigLevel, TrigLevel2]);
        }
        TT::Video => {
            v.extend([TrigSrc, VidStd, VidSync]);
            if m.video_sync == VSync::Line {
                v.push(VidLine);
            }
            v.push(TrigLevel);
        }
        TT::Pattern | TT::Duration => pattern_items(m, &mut v),
        TT::Timeout => v.extend([TrigSrc, TrigSlope, TrigT1, TrigLevel]),
        TT::Runt => {
            v.extend([TrigSrc, TrigPol]);
            condition_items(m, &mut v);
            v.extend([TrigLevel, TrigLevel2]);
        }
        TT::Window => {
            v.extend([TrigSrc, WinModeI]);
            if m.win_mode == WinMode::Inside {
                condition_items(m, &mut v);
            }
            v.extend([TrigLevel, TrigLevel2]);
        }
        TT::Delay => {
            v.extend([TrigSrc, TrigSlope, TrigSrc2, TrigSlope2]);
            condition_items(m, &mut v);
            v.extend([TrigLevel, TrigLevel2]);
        }
        TT::SetupHold => v.extend([TrigSrc, TrigSrc2, TrigSlope2, Setup, Hold, TrigLevel, TrigLevel2]),
        TT::NthEdge => v.extend([TrigSrc, TrigSlope, NthIdle, NthN, TrigLevel]),
    }
    common_items(ui, &mut v);
    v
}

fn common_items(ui: &Ui, v: &mut Vec<Item>) {
    use Item::*;
    let m = &ui.m;
    // the analog comparators: not for a protocol or an LA line
    if m.trig_type != TT::Bus && m.trig_line().is_none() {
        v.push(TrigNoise);
    }
    v.push(TrigAutoNoise);
    v.push(TrigMode);
    v.push(SearchOn);
    if ui.search_on {
        v.extend([SearchTable, SearchSave]);
    }
    v.push(Open(MenuId::Zone));
    v.push(TrigHoldoff);
}

fn condition_items(m: &Model, v: &mut Vec<Item>) {
    v.push(Item::TrigWhen);
    if m.trig_when != WhenKind::Off {
        v.push(Item::TrigT1);
    }
    if matches!(m.trig_when, WhenKind::In | WhenKind::Out) {
        v.push(Item::TrigT2);
    }
}

fn pattern_items(m: &Model, v: &mut Vec<Item>) {
    use Item::*;
    v.extend([Pat(0), Pat(1), Pat(2), Pat(3)]);
    if m.trig_type == TT::Duration {
        condition_items(m, v);
    }
    v.extend([TrigSrc, PatLevel]);
    if m.la_on() {
        v.push(Open(MenuId::LaPat));
    }
}

#[test]
fn trigger_visibility_and_order_cover_every_recipe() {
    let mut u = demo();
    for kind in TRIG_TYPES {
        u.m.trig_type = kind;
        for when in [WhenKind::Off, WhenKind::Gt, WhenKind::Lt, WhenKind::In, WhenKind::Out] {
            u.m.trig_when = when;
            for window in [WinMode::Enter, WinMode::Exit, WinMode::Inside] {
                u.m.win_mode = window;
                for sync in [VSync::AllLines, VSync::Line, VSync::Odd, VSync::Even, VSync::AllFields] {
                    u.m.video_sync = sync;
                    for search in [false, true] {
                        u.search_on = search;
                        assert_eq!(
                            u.menu_items(MenuId::Trigger),
                            expected_items(&u),
                            "{kind:?} {when:?} {window:?} {sync:?}"
                        );
                    }
                }
            }
        }
    }
    u.m.trig_type = TT::Bus;
    for &protocol in PROTOS.iter() {
        u.m.dec_proto = protocol;
        for condition in BUS_WHATS {
            u.m.trig_bus = condition;
            assert_eq!(u.menu_items(MenuId::Trigger), expected_items(&u));
        }
    }
}

#[test]
fn every_trigger_row_keeps_its_read_contract() {
    let mut u = demo();
    u.m.trig_when = WhenKind::Gt;
    u.m.video_sync = VSync::AllLines;
    u.m.trig_level = 1.25;
    u.m.trig_level2 = -0.25;
    u.m.trig_t1 = 2e-6;
    u.m.trig_t2 = 5e-6;
    u.m.trig_bus_val = 0xA5;
    u.m.video_line = 42;
    u.m.nth_n = 7;
    check_trigger_source_rows(&u);
    check_trigger_detail_rows(&u);
}

fn check_trigger_source_rows(u: &Ui) {
    use Item::*;
    use Kind::*;
    let sources = u.trigger_sources().into_iter().map(Ui::trigger_wave_label).collect::<Vec<_>>();
    super::super::small_tests::read_rows(
        u,
        &[
            (Open(MenuId::Trigger), Action, "Trigger", "".into(), vec![]),
            (
                TrigType,
                Choice,
                "Type",
                u.m.trig_type.name().into(),
                TRIG_TYPES.iter().map(|t| t.name().into()).collect(),
            ),
            (TrigSrc, Choice, "Source", u.trigger_source_label(), sources.clone()),
            (TrigSrc2, Choice, "Source B", Ui::trigger_wave_label(u.m.trigger_wave2()), sources),
            (TrigSlope, Choice, "Slope", "Rising".into(), strings(&["Rising", "Falling", "Either"])),
            (TrigSlope2, Choice, "Slope B", "Rising".into(), strings(&["Rising", "Falling", "Either"])),
            (TrigMode, Choice, "Sweep", format!("{:?}", u.m.trig_mode), strings(&["Auto", "Normal", "Single"])),
            (TrigLevel, Value, "Level", si(u.m.trig_level as f64, u.m.unit(u.m.trig_src)), vec![]),
            (TrigLevel2, Value, "Level 2", si(u.m.trig_level2 as f64, u.m.unit(u.m.trig_src)), vec![]),
            (Trig50, Action, "Level 50 %", "".into(), vec![]),
            (TrigPol, Choice, "Polarity", "Positive".into(), strings(&["Positive", "Negative"])),
            (TrigWhen, Choice, "When", ">".into(), strings(&[">", "<", "Inside", "Outside"])),
            (TrigT1, Value, "Time", si_short(u.m.trig_t1 as f64, "s"), vec![]),
            (TrigT2, Value, "Upper limit", si_short(u.m.trig_t2 as f64, "s"), vec![]),
        ],
    );
}

fn check_trigger_detail_rows(u: &Ui) {
    use Item::*;
    use Kind::*;
    super::super::small_tests::read_rows(
        u,
        &[
            (VidStd, Choice, "Standard", "PAL".into(), strings(&["PAL", "NTSC"])),
            (VidSync, Choice, "Sync", "Lines".into(), strings(&["Lines", "Line n", "Odd", "Even", "Fields"])),
            (VidLine, Value, "Line", "42".into(), vec![]),
            (PatLevel, Value, "Level", si(u.m.pat_levels[u.m.trig_src] as f64, "V"), vec![]),
            (WinModeI, Choice, "Trigger on", "Enter".into(), strings(&["Enter", "Exit", "Inside"])),
            (NthIdle, Value, "Idle time", si_short(u.m.nth_idle as f64, "s"), vec![]),
            (NthN, Value, "Edge no.", "7".into(), vec![]),
            (Setup, Value, "Setup", si_short(u.m.setup as f64, "s"), vec![]),
            (Hold, Value, "Hold", si_short(u.m.hold as f64, "s"), vec![]),
            (TrigHoldoff, Value, "Holdoff", "Off".into(), vec![]),
            (TrigNoise, Choice, "Noise reject", onoff(u.m.trig_nrej), strings(&["Off", "On"])),
            (TrigAutoNoise, Choice, "Auto noise rej.", onoff(u.m.auto_nrej), strings(&["Off", "On"])),
            (
                TrigCoup,
                Choice,
                "Coupling",
                u.m.trig_coup.name().into(),
                TRIG_COUPS.iter().map(|c| c.name().into()).collect(),
            ),
            (TrigBusWhat, Choice, "Match", u.m.trig_bus.name().into(), strings(&["Frame", "ID/Addr", "Data", "Error"])),
            (TrigBusVal, Choice, "Value", "0xA5".into(), KEYPAD.iter().map(|k| k.to_string()).collect()),
            (SearchOn, Choice, "Mark events", "Off".into(), strings(&["Off", "On"])),
            (SearchTable, Action, "Event list", "".into(), vec![]),
            (SearchSave, Action, "Save events", "".into(), vec![]),
        ],
    );
}

#[test]
fn trigger_pattern_and_slope_choices_keep_labels_and_identity() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    for (c, label) in ["CH1", "CH2", "CH3", "CH4"].into_iter().enumerate() {
        for (bit, value) in [(Bit::H, "H"), (Bit::L, "L"), (Bit::X, "X"), (Bit::R, "Rise"), (Bit::F, "Fall")] {
            u.m.pat[c] = bit;
            check(&u, Pat(c), Choice, label, value, strings(&["H", "L", "X", "Rise", "Fall"]));
            assert_eq!(u.options(Pat(c))[active_choice::index(&u, Pat(c)).unwrap()], value);
        }
        u.m.trig_type = TT::Duration;
        assert_eq!(u.options(Pat(c)), strings(&["H", "L", "X"]));
        u.m.trig_type = TT::Pattern;
    }
    for (slope, value) in [(Slope::Rise, "Rising"), (Slope::Fall, "Falling"), (Slope::Either, "Either")] {
        u.m.slope = slope;
        u.m.trig_slope2 = slope;
        assert_eq!(u.value(TrigSlope), value);
        assert_eq!(u.value(TrigSlope2), value);
        for item in [TrigSlope, TrigSlope2] {
            assert_eq!(u.options(item)[active_choice::index(&u, item).unwrap()], value);
        }
    }
}

#[test]
fn trigger_condition_and_video_choices_keep_labels_and_identity() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    for (when, value) in [
        (WhenKind::Off, "Off"),
        (WhenKind::Gt, ">"),
        (WhenKind::Lt, "<"),
        (WhenKind::In, "Inside"),
        (WhenKind::Out, "Outside"),
    ] {
        u.m.trig_when = when;
        u.m.trig_type = TT::Runt;
        check(&u, TrigWhen, Choice, "When", value, strings(&["Off", ">", "<", "Inside", "Outside"]));
        assert_eq!(u.options(TrigWhen)[active_choice::index(&u, TrigWhen).unwrap()], value);
    }
    for (sync, value) in [
        (VSync::AllLines, "Lines"),
        (VSync::Line, "Line n"),
        (VSync::Odd, "Odd"),
        (VSync::Even, "Even"),
        (VSync::AllFields, "Fields"),
    ] {
        u.m.video_sync = sync;
        assert_eq!(u.value(VidSync), value);
        assert_eq!(u.options(VidSync)[active_choice::index(&u, VidSync).unwrap()], value);
    }
    u.m.video_std = VideoStd::Ntsc;
    assert_eq!(u.value(VidStd), "NTSC");
    u.m.trig_pol = Pol::Neg;
    assert_eq!(u.value(TrigPol), "Negative");
    for (mode, value) in [(WinMode::Enter, "Enter"), (WinMode::Exit, "Exit"), (WinMode::Inside, "Inside")] {
        u.m.win_mode = mode;
        assert_eq!(u.value(WinModeI), value);
        assert_eq!(u.options(WinModeI)[active_choice::index(&u, WinModeI).unwrap()], value);
    }
    for holdoff in [-1.0, 0.0, 2e-6] {
        u.m.holdoff = holdoff;
        assert_eq!(u.value(TrigHoldoff), if holdoff <= 0.0 { "Off".into() } else { si_short(holdoff as f64, "s") });
    }
}
