// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker
use super::*;

pub(in crate::ui) fn trigger_items(ui: &Ui) -> Vec<Item> {
    use crate::model::TrigType as TT;
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
    use crate::model::TrigType as TT;
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
    if m.trig_type == crate::model::TrigType::Duration {
        condition_items(m, v);
    }
    v.extend([TrigSrc, PatLevel]);
    if m.la_on() {
        v.push(Open(MenuId::LaPat));
    }
}
