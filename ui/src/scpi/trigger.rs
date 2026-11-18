// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

fn model(ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let m = &mut ui.m;
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "TRIGger") && nodes.len() == 3 && ["RS232", "SPI", "IIC"].iter().any(|b| kw(nodes[1], b)) {
        return ser::trig(m, nodes[1], nodes[2], arg, query);
    }
    if kw(n0, "TRIGger") {
        let last = *nodes.last()?;
        if nodes.len() == 2 && kw(last, "STATus") {
            let s = match (m.run, m.status) {
                (RunState::Stop, _) => "STOP",
                (_, TrigStatus::Triggered) => "TD",
                (_, TrigStatus::Auto) => "AUTO",
                (_, TrigStatus::Wait) => "WAIT",
                _ => "STOP",
            };
            return Some(text_reply(s));
        }
        if nodes.len() == 2 && kw(last, "COUPling") {
            return coupling(query, arg, m);
        }
        if nodes.len() == 2 && kw(last, "NREJect") {
            if query {
                return Some(text_reply(if m.trig_nrej { "1" } else { "0" }));
            }
            let a = arg.trim().to_ascii_uppercase();
            m.trig_nrej = match a.as_str() {
                "1" | "ON" => true,
                "0" | "OFF" => false,
                _ => return None,
            };
            return Some(None);
        }
        if nodes.len() == 2 && kw(last, "HOLDoff") {
            if query {
                return Some(text_reply(&e(m.holdoff as f64)));
            }
            m.holdoff = num(arg).filter(|&h| (0.0..=10.0).contains(&h))? as f32;
            return Some(None);
        }
        if nodes.len() == 2 && kw(last, "SWEep") {
            return sweep(query, arg, ui);
        }
        if nodes.len() == 3 && kw(nodes[1], "EDGE") {
            return edge(query, arg, m, last);
        }
        return None;
    }
    None
}

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    // Reference trigger identities belong to the UI acquisition router, not
    // a silent physical-channel alias inside Model::trigger().
    if nodes.first().is_some_and(|node| kw(node, "TRIGger")) {
        let last = nodes.last().copied().unwrap_or("");
        let (leaf, source_number) = suffix(last);
        if (nodes.len() == 2 || nodes.len() == 3) && kw(leaf, "SOURce") {
            if let Some(reply) = source(scpi, ui, query, arg, source_number) {
                return reply;
            }
        }
        if nodes.len() == 2 && (kw(last, "TYPe") || kw(last, "MODE")) {
            return kind(scpi, ui, query, arg, last);
        }
        if nodes.len() == 3 && kw(nodes[1], "RUNT") {
            if let Some(reply) = runt(scpi, ui, query, arg, last) {
                return reply;
            }
        }
    }

    if !query
        && (nodes.len() == 1 && kw(nodes[0], "SINGle")
            || nodes.len() == 2
                && kw(nodes[0], "TRIGger")
                && kw(nodes[1], "SWEep")
                && arg.to_ascii_uppercase().starts_with("SING")
                && ui.m.run == RunState::Run)
    {
        ui.scpi_arm_operation();
    }
    if let Some(resp) = model(ui, nodes, query, arg) {
        if !query {
            change::edited(ui);
        }
        return resp;
    }

    super::undefined(scpi, cmd)
}

fn coupling(query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match m.trig_coup {
            TrigCoup::Dc => "DC",
            TrigCoup::Ac => "AC",
            TrigCoup::LfReject => "LFR",
            TrigCoup::HfReject => "HFR",
        }));
    }
    let a = arg.trim().to_ascii_uppercase();
    m.trig_coup = match a.as_str() {
        "DC" => TrigCoup::Dc,
        "AC" => TrigCoup::Ac,
        "LFR" | "LFREJECT" => TrigCoup::LfReject,
        "HFR" | "HFREJECT" => TrigCoup::HfReject,
        _ => return None,
    };
    Some(None)
}

fn sweep(query: bool, arg: &str, ui: &mut Ui) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match ui.m.trig_mode {
            TrigMode::Auto => "AUTO",
            TrigMode::Normal => "NORM",
            TrigMode::Single => "SING",
        }));
    }
    let a = arg.to_ascii_uppercase();
    let mode = if a.starts_with("AUTO") {
        TrigMode::Auto
    } else if a.starts_with("NORM") {
        TrigMode::Normal
    } else if a.starts_with("SING") {
        TrigMode::Single
    } else {
        return None;
    };
    ui.set_sweep(mode);
    Some(None)
}

fn edge(query: bool, arg: &str, m: &mut Model, last: &str) -> Option<Option<Vec<u8>>> {
    if kw(last, "SOURce") {
        if query {
            return Some(text_reply(&m.trig_line().map_or(format!("CHAN{}", m.trig_src + 1), |n| format!("D{}", n))));
        }
        // D0..D15 like Rigol's source names, with the LA on
        match arg
            .trim()
            .to_ascii_uppercase()
            .strip_prefix('D')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|&n| n < 16 && m.trig_la_ok())
        {
            Some(n) => m.trig_la = Some(n),
            None => (m.trig_src, m.trig_la) = (parse_ch(arg)?, None),
        }
    } else if kw(last, "LEVel") {
        if query {
            return Some(text_reply(&e(m.trig_level as f64)));
        }
        m.trig_level = num(arg)?;
    } else if kw(last, "SLOPe") {
        if query {
            return Some(text_reply(match m.slope {
                Slope::Rise => "POS",
                Slope::Fall => "NEG",
                Slope::Either => "RFAL",
            }));
        }
        let a = arg.to_ascii_uppercase();
        m.slope = if a.starts_with("POS") {
            Slope::Rise
        } else if a.starts_with("NEG") {
            Slope::Fall
        } else if a.starts_with("RFAL") {
            Slope::Either
        } else {
            return None;
        };
    } else {
        return None;
    }
    Some(None)
}

fn source(
    scpi: &mut Scpi,
    ui: &mut Ui,
    query: bool,
    arg: &str,
    source_number: Option<usize>,
) -> Option<Option<Vec<u8>>> {
    let secondary = source_number == Some(2);
    if query {
        if !secondary && ui.m.trig_ref.is_none() {
            if let Some(line) = ui.m.trig_line() {
                return Some(text_reply(&format!("D{line}")));
            }
        }
        let id = if secondary { ui.m.trigger_wave2() } else { ui.m.trigger_wave() };
        return Some(text_reply(&match id {
            WaveId::Ref(i) => format!("R{}", i + 1),
            WaveId::Ch(c) => format!("CHAN{}", c + 1),
        }));
    }
    if let Some(id) = parse_wave(arg) {
        if !ui.set_trigger_source(id, secondary) {
            scpi.push_err("-224,\"Trigger source is unavailable\"".into());
        }
        change::edited(ui);
        return Some(None);
    }
    // Leave D0..D15 to the existing LA source handler.
    if !arg.trim().to_ascii_uppercase().starts_with('D') {
        scpi.push_err("-224,\"Invalid trigger source\"".into());
        return Some(None);
    }

    None
}

fn kind(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, last: &str) -> Option<Vec<u8>> {
    if query {
        return kind_query(ui, last);
    }
    let name = arg.trim().to_ascii_uppercase();
    let kind = match name.as_str() {
        "EDGE" => Some(TrigType::Edge),
        "PULSE" | "PULS" => Some(TrigType::Pulse),
        "SLOPE" | "SLOP" => Some(TrigType::Slope),
        "VIDEO" | "VID" => Some(TrigType::Video),
        "PATTERN" | "PATT" => Some(TrigType::Pattern),
        "DURATION" | "DUR" => Some(TrigType::Duration),
        "TIMEOUT" | "TIM" => Some(TrigType::Timeout),
        "RUNT" => Some(TrigType::Runt),
        "WINDOW" | "WIND" => Some(TrigType::Window),
        "DELAY" | "DEL" => Some(TrigType::Delay),
        "SETUPHOLD" | "SETUP" | "SET" => Some(TrigType::SetupHold),
        "NTHEDGE" | "NTH" | "NEDGE" | "NEDG" => Some(TrigType::NthEdge),
        "BUS" => Some(TrigType::Bus),
        _ => None,
    };
    if let Some(kind) = kind {
        ui.trigger_type_set(kind);
        change::edited(ui);
    } else {
        scpi.push_err("-224,\"Invalid trigger type\"".into());
    }
    None
}

fn kind_query(ui: &mut Ui, last: &str) -> Option<Vec<u8>> {
    text_reply(if kw(last, "MODE") {
        match ui.m.trig_type {
            TrigType::Edge => "EDGE",
            TrigType::Pulse => "PULS",
            TrigType::Slope => "SLOP",
            TrigType::Video => "VID",
            TrigType::Pattern => "PATT",
            TrigType::Duration => "DUR",
            TrigType::Timeout => "TIM",
            TrigType::Runt => "RUNT",
            TrigType::Window => "WIND",
            TrigType::Delay => "DEL",
            TrigType::SetupHold => "SET",
            TrigType::NthEdge => "NEDG",
            TrigType::Bus => "BUS",
        }
    } else {
        ui.m.trig_type.name()
    })
}

fn runt(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, last: &str) -> Option<Option<Vec<u8>>> {
    let upper = kw(last, "UPPer") || kw(last, "ALEVel");
    let (lower, higher) = (ui.m.trig_level2, ui.m.trig_level);
    let value = if upper {
        Some(&mut ui.m.trig_level)
    } else if kw(last, "LOWer") || kw(last, "BLEVel") {
        Some(&mut ui.m.trig_level2)
    } else {
        None
    };
    if let Some(value) = value {
        if query {
            return Some(text_reply(&e(*value as f64)));
        }
        // the pair stays in order, as :MEASure:SETup:MAX|MIN: a threshold past the other is refused
        match num(arg) {
            Some(number) if (upper && number < lower) || (!upper && number > higher) => {
                let other = if upper { "below the lower" } else { "above the upper" };
                scpi.push_err(format!("-224,\"Illegal parameter value\" {other} threshold"));
            }
            Some(number) => {
                *value = number;
                change::edited(ui);
            }
            None => scpi.push_err("-224,\"Invalid runt threshold\"".into()),
        }
        return Some(None);
    }

    None
}
