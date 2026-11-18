// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::TrigHoldoff => Kind::Value,
        Item::TrigLevel2
        | Item::TrigT1
        | Item::TrigT2
        | Item::VidLine
        | Item::PatLevel
        | Item::NthIdle
        | Item::NthN
        | Item::Setup
        | Item::Hold => Kind::Value,
        Item::TrigLevel => Kind::Value,
        Item::SearchTable | Item::SearchSave | Item::Trig50 => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Trigger) => "Trigger",
        Item::SearchTable => "Event list",
        Item::SearchSave => "Save events",
        Item::TrigBusWhat => "Match",
        Item::TrigBusVal => "Value",
        Item::TrigSrc => "Source",
        Item::TrigSlope => "Slope",
        Item::TrigMode => "Sweep",
        Item::TrigLevel => "Level",
        Item::Trig50 => "Level 50 %",
        Item::TrigType => "Type",
        Item::SearchOn => "Mark events",
        Item::TrigLevel2 => "Level 2",
        Item::TrigSrc2 => "Source B",
        Item::TrigSlope2 => "Slope B",
        Item::TrigPol => "Polarity",
        Item::TrigWhen => "When",
        Item::TrigT1 => "Time",
        Item::TrigT2 => "Upper limit",
        Item::VidStd => "Standard",
        Item::VidSync => "Sync",
        Item::VidLine => "Line",
        Item::Pat(0) => "CH1",
        Item::Pat(1) => "CH2",
        Item::Pat(2) => "CH3",
        Item::Pat(_) => "CH4",
        Item::PatLevel => "Level",
        Item::WinModeI => "Trigger on",
        Item::NthIdle => "Idle time",
        Item::NthN => "Edge no.",
        Item::Setup => "Setup",
        Item::Hold => "Hold",
        Item::TrigHoldoff => "Holdoff",
        Item::TrigNoise => "Noise reject",
        Item::TrigAutoNoise => "Auto noise rej.",
        Item::TrigCoup => "Coupling",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    if it == Item::TrigSrc {
        if let Some(line) = ui.m.trig_line() {
            return Some(la::NAMES[line as usize].into());
        }
    }
    if let Some(v) = value_condition(ui, it) {
        return Some(v);
    }
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::TrigSrc => ui.trigger_source_label(),
        Item::TrigSlope => match m.slope {
            Slope::Rise => "Rising".into(),
            Slope::Fall => "Falling".into(),
            Slope::Either => "Either".into(),
        },
        Item::TrigMode => format!("{:?}", m.trig_mode),
        Item::TrigLevel => si(m.trig_level as f64, m.unit(m.trig_src)),
        Item::TrigType => m.trig_type.name().into(),
        Item::SearchOn => onoff(ui.search_on),
        Item::TrigLevel2 => si(m.trig_level2 as f64, m.unit(m.trig_src)),
        Item::TrigSrc2 => Ui::trigger_wave_label(m.trigger_wave2()),
        Item::TrigSlope2 => match m.trig_slope2 {
            Slope::Rise => "Rising".into(),
            Slope::Fall => "Falling".into(),
            Slope::Either => "Either".into(),
        },
        Item::TrigPol => {
            if m.trig_pol == Pol::Pos {
                "Positive".into()
            } else {
                "Negative".into()
            }
        }
        Item::TrigWhen => match m.trig_when {
            WhenKind::Off => "Off".into(),
            WhenKind::Gt => ">".into(),
            WhenKind::Lt => "<".into(),
            WhenKind::In => "Inside".into(),
            WhenKind::Out => "Outside".into(),
        },
        Item::TrigT1 => si_short(m.trig_t1 as f64, "s"),
        Item::TrigT2 => si_short(m.trig_t2 as f64, "s"),
        Item::TrigHoldoff => {
            if m.holdoff <= 0.0 {
                "Off".into()
            } else {
                si_short(m.holdoff as f64, "s")
            }
        }
        Item::TrigNoise => onoff(m.trig_nrej),
        Item::TrigAutoNoise => onoff(m.auto_nrej),
        Item::TrigCoup => m.trig_coup.name().into(),
        Item::TrigBusWhat => m.trig_bus.name().into(),
        Item::TrigBusVal => format!("0x{:X}", m.trig_bus_val),
        _ => return None,
    })
}

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

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    if it == Item::TrigSrc && ui.m.trig_la_ok() {
        return Some(ch_names().into_iter().chain(la::NAMES.iter().map(|s| s.to_string())).collect());
    }
    let v: &[&str] = match it {
        Item::SearchOn => &["Off", "On"],
        Item::TrigSrc | Item::TrigSrc2 => {
            return Some(ui.trigger_sources().into_iter().map(Ui::trigger_wave_label).collect())
        }
        Item::TrigSlope => &["Rising", "Falling", "Either"],
        Item::TrigMode => &["Auto", "Normal", "Single"],
        Item::TrigCoup => return Some(TRIG_COUPS.iter().map(|c| c.name().to_string()).collect()),
        Item::TrigSlope2 => &["Rising", "Falling", "Either"],
        Item::TrigPol => &["Positive", "Negative"],
        Item::TrigWhen => {
            if ui.m.trig_type == TrigType::Runt {
                &["Off", ">", "<", "Inside", "Outside"]
            } else {
                &[">", "<", "Inside", "Outside"]
            }
        }
        Item::VidStd => &["PAL", "NTSC"],
        Item::VidSync => &["Lines", "Line n", "Odd", "Even", "Fields"],
        Item::Pat(_) => {
            if ui.m.trig_type == TrigType::Duration {
                &["H", "L", "X"]
            } else {
                &["H", "L", "X", "Rise", "Fall"]
            }
        }
        Item::WinModeI => &["Enter", "Exit", "Inside"],
        Item::TrigBusWhat => &["Frame", "ID/Addr", "Data", "Error"],
        Item::TrigNoise | Item::TrigAutoNoise => &["Off", "On"],
        Item::TrigType => return Some(TRIG_TYPES.iter().map(|t| t.name().to_string()).collect()),
        Item::TrigBusVal => return Some(KEYPAD.iter().map(|k| k.to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Trigger => ui.trigger_items(),
        // the Horizontal Menu key opens Acquire; time base and zoom one level down
        _ => Vec::new(),
    }
}

mod rows;
pub(in crate::ui) use rows::trigger_items;

fn value_condition(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    Some(match it {
        Item::VidStd => {
            if m.video_std == VideoStd::Pal {
                "PAL".into()
            } else {
                "NTSC".into()
            }
        }
        Item::VidSync => match m.video_sync {
            VSync::AllLines => "Lines".into(),
            VSync::Line => "Line n".into(),
            VSync::Odd => "Odd".into(),
            VSync::Even => "Even".into(),
            VSync::AllFields => "Fields".into(),
        },
        Item::VidLine => format!("{}", m.video_line),
        Item::Pat(c) => match m.pat[c] {
            Bit::H => "H".into(),
            Bit::L => "L".into(),
            Bit::X => "X".into(),
            Bit::R => "Rise".into(),
            Bit::F => "Fall".into(),
        },
        Item::PatLevel => si(m.pat_levels[m.trig_src] as f64, "V"),
        Item::WinModeI => match m.win_mode {
            WinMode::Enter => "Enter".into(),
            WinMode::Exit => "Exit".into(),
            WinMode::Inside => "Inside".into(),
        },
        Item::NthIdle => si_short(m.nth_idle as f64, "s"),
        Item::NthN => format!("{}", m.nth_n),
        Item::Setup => si_short(m.setup as f64, "s"),
        Item::Hold => si_short(m.hold as f64, "s"),
        _ => return None,
    })
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
            let all: &[WhenKind] = if m.trig_type == TrigType::Runt {
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
            let all = if m.trig_type == TrigType::Duration {
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
        let secondary = it == Item::TrigLevel2 && matches!(ui.m.trig_type, TrigType::Delay | TrigType::SetupHold);
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
            let c = if matches!(m.trig_type, TrigType::Delay | TrigType::SetupHold) { m.trig_src2 } else { m.trig_src };
            m.trig_level2 += d as f32 * m.ch[c].volts_per_div() / 25.0;
            if m.trig_type.two_level() {
                m.trig_level2 = m.trig_level2.min(m.trig_level);
            }
        }
        _ => {}
    }
}
