// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;
use crate::model::TrigType as TT;

use choices::*;

pub(super) const AREA: Area = Area {
    descriptors: ROWS,
    label: |it| LABELS.iter().find(|r| r.0 == it).map(|r| r.1),
    ordered_rows: rows::ordered_rows,
    adjust,
    activate,
    ..Area::EMPTY
};

const LABELS: &[(Item, &str)] = &[(Item::Open(MenuId::Trigger), "Trigger")];

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::choice(
            TrigType,
            "Type",
            |u| u.m.trig_type.name().into(),
            |_| TRIG_TYPES.iter().map(|t| t.name().into()).collect(),
        ),
        Descriptor::choice(
            TrigSrc,
            "Source",
            |u| u.m.trig_line().map_or_else(|| u.trigger_source_label(), |line| la::NAMES[line as usize].into()),
            |u| {
                if u.m.trig_la_ok() {
                    ch_names().into_iter().chain(la::NAMES.iter().map(|s| s.to_string())).collect()
                } else {
                    sources(u)
                }
            },
        ),
        Descriptor::choice(TrigSlope, "Slope", |u| choice_label(u.m.slope, SLOPES), |_| choice_options(SLOPES)),
        Descriptor::choice(
            TrigMode,
            "Sweep",
            |u| format!("{:?}", u.m.trig_mode),
            |_| ["Auto", "Normal", "Single"].into_iter().map(String::from).collect(),
        ),
        Descriptor::value(TrigLevel, "Level", |u| si(u.m.trig_level as f64, u.m.unit(u.m.trig_src)))
            .when(|u| u.m.trig_type != TT::Edge || analog(u)),
        Descriptor::action(Trig50, "Level 50 %").when(analog),
        Descriptor::choice(SearchOn, "Mark events", |u| onoff(u.search_on), toggles),
        Descriptor::action(SearchTable, "Event list").when(|u| u.search_on),
        Descriptor::action(SearchSave, "Save events").when(|u| u.search_on),
        Descriptor::value(TrigLevel2, "Level 2", |u| si(u.m.trig_level2 as f64, u.m.unit(u.m.trig_src))),
        Descriptor::choice(TrigSrc2, "Source B", |u| Ui::trigger_wave_label(u.m.trigger_wave2()), sources),
        Descriptor::choice(
            TrigSlope2,
            "Slope B",
            |u| choice_label(u.m.trig_slope2, SLOPES),
            |_| choice_options(SLOPES),
        ),
        Descriptor::choice(
            TrigPol,
            "Polarity",
            |u| choice_label(u.m.trig_pol, POLARITIES),
            |_| choice_options(POLARITIES),
        ),
        Descriptor::choice(
            TrigWhen,
            "When",
            |u| choice_label(u.m.trig_when, CONDITIONS),
            |u| choice_options(conditions(u)),
        )
        .when(condition),
        Descriptor::value(TrigT1, "Time", |u| si_short(u.m.trig_t1 as f64, "s"))
            .when(|u| u.m.trig_type == TT::Timeout || (condition(u) && u.m.trig_when != WhenKind::Off)),
        Descriptor::value(TrigT2, "Upper limit", |u| si_short(u.m.trig_t2 as f64, "s"))
            .when(|u| condition(u) && (matches!(u.m.trig_when, WhenKind::In | WhenKind::Out))),
        Descriptor::choice(
            VidStd,
            "Standard",
            |u| choice_label(u.m.video_std, VIDEO_STANDARDS),
            |_| choice_options(VIDEO_STANDARDS),
        ),
        Descriptor::choice(
            VidSync,
            "Sync",
            |u| choice_label(u.m.video_sync, VIDEO_SYNC),
            |_| choice_options(VIDEO_SYNC),
        ),
        Descriptor::value(VidLine, "Line", |u| u.m.video_line.to_string()).when(|u| u.m.video_sync == VSync::Line),
        pattern::<0>(),
        pattern::<1>(),
        pattern::<2>(),
        pattern::<3>(),
        Descriptor::value(PatLevel, "Level", |u| si(u.m.pat_levels[u.m.trig_src] as f64, "V")),
        Descriptor::choice(
            WinModeI,
            "Trigger on",
            |u| choice_label(u.m.win_mode, WINDOW_MODES),
            |_| choice_options(WINDOW_MODES),
        ),
        Descriptor::value(NthIdle, "Idle time", |u| si_short(u.m.nth_idle as f64, "s")),
        Descriptor::value(NthN, "Edge no.", |u| u.m.nth_n.to_string()),
        Descriptor::value(Setup, "Setup", |u| si_short(u.m.setup as f64, "s")),
        Descriptor::value(Hold, "Hold", |u| si_short(u.m.hold as f64, "s")),
        Descriptor::value(TrigHoldoff, "Holdoff", |u| {
            if u.m.holdoff <= 0.0 {
                "Off".into()
            } else {
                si_short(u.m.holdoff as f64, "s")
            }
        }),
        Descriptor::choice(TrigNoise, "Noise reject", |u| onoff(u.m.trig_nrej), toggles)
            .when(|u| u.m.trig_type != TT::Bus && analog(u)),
        Descriptor::choice(TrigAutoNoise, "Auto noise rej.", |u| onoff(u.m.auto_nrej), toggles),
        Descriptor::choice(
            TrigCoup,
            "Coupling",
            |u| u.m.trig_coup.name().into(),
            |_| TRIG_COUPS.iter().map(|c| c.name().into()).collect(),
        )
        .when(analog),
        Descriptor::choice(
            TrigBusWhat,
            "Match",
            |u| u.m.trig_bus.name().into(),
            |_| ["Frame", "ID/Addr", "Data", "Error"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            TrigBusVal,
            "Value",
            |u| format!("0x{:X}", u.m.trig_bus_val),
            |_| KEYPAD.iter().map(|k| k.to_string()).collect(),
        )
        .when(|u| matches!(u.m.trig_bus, BusWhat::Id | BusWhat::Data)),
        Descriptor::reference(Open(MenuId::Zone)),
        Descriptor::reference(Open(MenuId::LaPat)).when(|u| u.m.la_on()),
    ]
};

fn analog(ui: &Ui) -> bool {
    ui.m.trig_line().is_none()
}

fn condition(ui: &Ui) -> bool {
    ui.m.trig_type != TT::Window || ui.m.win_mode == WinMode::Inside
}

fn sources(ui: &Ui) -> Vec<String> {
    ui.trigger_sources().into_iter().map(Ui::trigger_wave_label).collect()
}

const fn pattern<const C: usize>() -> Descriptor {
    Descriptor::choice(
        Item::Pat(C),
        ["CH1", "CH2", "CH3", "CH4"][C],
        |u| choice_label(u.m.pat[C], PATTERN_BITS),
        |u| choice_options(pattern_bits(u)),
    )
}

mod choices;

mod rows;

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if ui.la_adjust(it, d) {
        return true;
    }
    if adjust_timing(ui, it, d) {
        return false;
    }
    if adjust_condition(ui, it, d) {
        return false;
    }
    if adjust_source(ui, it, d) {
        return true;
    }
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::TrigLevel | Item::TrigLevel2 => adjust_channel_level(ui, it, d),
        Item::TrigSlope => {
            let all = [Slope::Rise, Slope::Fall, Slope::Either];
            let i = all.iter().position(|&x| x == m.slope).expect("all slopes are listed");
            m.slope = all[cyc(i, 3, d)];
        }
        Item::TrigMode => {
            let all = [TrigMode::Auto, TrigMode::Normal, TrigMode::Single];
            let i = all.iter().position(|&x| x == m.trig_mode).expect("all trigger modes are listed");
            let mode = all[cyc(i, 3, d)];
            ui.set_sweep(mode);
            if mode == TrigMode::Single {
                ui.m.run = RunState::Run;
            }
        }
        Item::TrigType => {
            let i = TRIG_TYPES.iter().position(|&x| x == m.trig_type).unwrap_or(0);
            let t = TRIG_TYPES[cyc(i, TRIG_TYPES.len(), d)];
            ui.set_trig_type(t);
        }
        Item::SearchOn => {
            ui.search_on = !ui.search_on;
            return true;
        }
        Item::TrigNoise => m.trig_nrej = !m.trig_nrej,
        Item::TrigAutoNoise => m.auto_nrej = !m.auto_nrej,
        Item::TrigCoup => {
            let i = TRIG_COUPS.iter().position(|&x| x == m.trig_coup).unwrap_or(0);
            m.trig_coup = TRIG_COUPS[cyc(i, TRIG_COUPS.len(), d)];
        }
        Item::TrigBusWhat => {
            let i = BUS_WHATS.iter().position(|&x| x == ui.m.trig_bus).unwrap_or(0);
            ui.m.trig_bus = BUS_WHATS[cyc(i, BUS_WHATS.len(), d)];
        }
        Item::TrigBusVal => {
            ui.m.trig_bus_val = (ui.m.trig_bus_val as i64 + d as i64).clamp(0, BUS_VAL_MAX as i64) as u64
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::Trig50 => ui.level_50(),
        Item::SearchTable => ui.table_toggle(tables::TableKind::Search),
        Item::SearchSave => ui.events_export(tables::TableKind::Search),
        _ => {}
    }
    true
}

fn adjust_condition(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::TrigSlope2 => {
            let all = [Slope::Rise, Slope::Fall, Slope::Either];
            let i = all.iter().position(|&x| x == m.trig_slope2).expect("all slopes are listed");
            m.trig_slope2 = all[cyc(i, 3, d)];
        }
        Item::TrigPol => m.trig_pol = if m.trig_pol == Pol::Pos { Pol::Neg } else { Pol::Pos },
        Item::TrigWhen => {
            let all: &[WhenKind] = if m.trig_type == TT::Runt {
                &[WhenKind::Off, WhenKind::Gt, WhenKind::Lt, WhenKind::In, WhenKind::Out]
            } else {
                &[WhenKind::Gt, WhenKind::Lt, WhenKind::In, WhenKind::Out]
            };
            let i = all.iter().position(|&x| x == m.trig_when).unwrap_or(0);
            m.trig_when = all[cyc(i, all.len(), d)];
        }
        Item::VidStd => m.video_std = if m.video_std == VideoStd::Pal { VideoStd::Ntsc } else { VideoStd::Pal },
        Item::VidSync => {
            let all = [VSync::AllLines, VSync::Line, VSync::Odd, VSync::Even, VSync::AllFields];
            let i = all.iter().position(|&x| x == m.video_sync).expect("all video sync modes are listed");
            m.video_sync = all[cyc(i, all.len(), d)];
        }
        Item::VidLine => m.video_line = (m.video_line as i32 + d).clamp(1, m.video_std.lines() as i32 / 2 - 10) as u32,
        Item::Pat(c) => {
            let all = if m.trig_type == TT::Duration {
                &[Bit::H, Bit::L, Bit::X][..]
            } else {
                &[Bit::H, Bit::L, Bit::X, Bit::R, Bit::F][..]
            };
            let i = all.iter().position(|&x| x == m.pat[c]).unwrap_or(0);
            m.pat[c] = all[cyc(i, all.len(), d)];
        }
        Item::PatLevel => {
            let c = m.trig_src;
            m.pat_levels[c] += d as f32 * m.ch[c].volts_per_div() / 25.0;
        }
        Item::WinModeI => {
            let all = [WinMode::Enter, WinMode::Exit, WinMode::Inside];
            let i = all.iter().position(|&x| x == m.win_mode).expect("all window modes are listed");
            m.win_mode = all[cyc(i, 3, d)];
        }
        _ => return false,
    }
    true
}

fn adjust_timing(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    match it {
        Item::TrigT1 => m.trig_t1 = ladder_step(m.trig_t1, d),
        Item::TrigT2 => m.trig_t2 = ladder_step(m.trig_t2, d),
        Item::NthIdle => m.nth_idle = ladder_step(m.nth_idle, d),
        Item::Setup => m.setup = ladder_step(m.setup, d),
        Item::Hold => m.hold = ladder_step(m.hold, d),
        Item::NthN => m.nth_n = (m.nth_n as i32 + d).clamp(1, 65535) as u32,
        Item::TrigHoldoff => m.holdoff = holdoff_step(m.holdoff, d),
        _ => return false,
    }
    true
}

fn adjust_source(ui: &mut Ui, it: Item, d: i32) -> bool {
    if matches!(it, Item::TrigSrc | Item::TrigSrc2) {
        let secondary = it == Item::TrigSrc2;
        let sources = ui.trigger_sources();
        let selected = if secondary { ui.m.trigger_wave2() } else { ui.m.trigger_wave() };
        if !sources.is_empty() {
            let pos = sources.iter().position(|&id| id == selected).unwrap_or(0);
            let id = sources[(pos as i32 + d).rem_euclid(sources.len() as i32) as usize];
            ui.set_trigger_source(id, secondary);
        }
        return true;
    }
    if matches!(it, Item::TrigLevel | Item::TrigLevel2) && ui.m.trig_ref.is_some() {
        let secondary = it == Item::TrigLevel2 && matches!(ui.m.trig_type, TT::Delay | TT::SetupHold);
        let step = d as f32 * ui.trigger_vpd(secondary) / 25.0;
        if it == Item::TrigLevel {
            ui.m.trig_level += step;
        } else {
            ui.m.trig_level2 += step;
        }
        ui.replay.reset_search();
        return true;
    }
    false
}

fn adjust_channel_level(ui: &mut Ui, it: Item, d: i32) {
    let m = &mut ui.m;
    match it {
        Item::TrigLevel => {
            m.trig_level += d as f32 * m.ch[m.trig_src].volts_per_div() / 25.0;
            // the upper level of a two-level type stops at the lower one, as Bode's start stops at its stop
            if m.trig_type.two_level() {
                m.trig_level = m.trig_level.max(m.trig_level2);
            }
        }
        Item::TrigLevel2 => {
            let c = if matches!(m.trig_type, TT::Delay | TT::SetupHold) { m.trig_src2 } else { m.trig_src };
            m.trig_level2 += d as f32 * m.ch[c].volts_per_div() / 25.0;
            if m.trig_type.two_level() {
                m.trig_level2 = m.trig_level2.min(m.trig_level);
            }
        }
        _ => {}
    }
}
