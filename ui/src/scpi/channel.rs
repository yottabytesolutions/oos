// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

fn model(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let m = &mut ui.m;
    let n0 = nodes.first().copied().unwrap_or("");
    let (k0, sfx) = suffix(n0);
    if kw(k0, "CHANnel") && (nodes.len() == 2 || nodes.len() == 3) {
        let c = sfx?.checked_sub(1).filter(|&c| c < NCH)?;
        let n1 = nodes[1];
        if nodes.len() == 3 && kw(nodes[1], "LABel") && kw(nodes[2], "CONTent") {
            return label_content(query, arg, m, c);
        }
        if nodes.len() == 3 && kw(nodes[1], "LABel") && kw(nodes[2], "SHOW") {
            return label_show(scpi, query, arg, m, c);
        }
        if nodes.len() != 2 {
            return None;
        }
        if kw(n1, "VERNier") {
            return vernier(scpi, query, arg, m);
        }
        if kw(n1, "EXPand") {
            return expand(scpi, query, arg, m);
        }
        let expand_gnd = m.v_expand_gnd;
        let slow_deskew = m.time_per_div() > 10e-6;
        let fine = m.fine_v;
        let ch = &mut m.ch[c];
        if kw(n1, "DISPlay") {
            if query {
                return Some(text_reply(if ch.on { "1" } else { "0" }));
            }
            ch.on = parse_bool(arg)?;
        } else if kw(n1, "SCALe") {
            return scale(query, arg, ch, fine, expand_gnd);
        } else if kw(n1, "OFFSet") || kw(n1, "POSition") {
            if query {
                return Some(text_reply(&e(ch.offset as f64)));
            }
            ch.offset = num(arg)?;
        } else if kw(n1, "UNITs") {
            return units(scpi, query, arg, ch);
        } else if kw(n1, "COUPling") {
            return coupling(query, arg, ch);
        } else if kw(n1, "BWLimit") {
            return bandwidth(query, arg, ch);
        } else if kw(n1, "PROBe") {
            return probe(query, arg, ch);
        } else if kw(n1, "INVert") {
            return invert(query, arg, ch);
        } else if kw(n1, "BIAS") {
            return bias(scpi, query, arg, ch);
        } else if kw(n1, "TCALibrate") {
            return deskew(scpi, query, arg, ch, slow_deskew);
        } else {
            return None;
        }
        return Some(None);
    }
    None
}

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    if let Some(resp) = model(scpi, ui, nodes, query, arg) {
        if !query {
            change::edited(ui);
        }
        return resp;
    }

    super::undefined(scpi, cmd)
}

fn label_content(query: bool, arg: &str, m: &mut Model, c: usize) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&m.ch_label[c]));
    }
    let text = arg.trim().trim_matches('"').chars().take(10).collect::<String>();
    m.ch_label[c] = text;
    Some(None)
}

fn label_show(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model, c: usize) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if m.ch_label[c].is_empty() { "0" } else { "1" }));
    }
    match parse_bool(arg) {
        Some(false) => {
            m.ch_label[c].clear();
            Some(None)
        }
        Some(true) => {
            if m.ch_label[c].is_empty() {
                m.ch_label[c] = format!("CH{}", c + 1);
            }
            Some(None)
        }
        None => {
            scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
            Some(None)
        }
    }
}

fn vernier(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    // One fine-adjust switch for every channel: the screen has a single control.
    if query {
        return Some(text_reply(if m.fine_v { "1" } else { "0" }));
    }
    match parse_bool(arg) {
        Some(on) => {
            // off keeps a fine scale until the next coarse step, as the Channel menu's switch does
            m.fine_v = on;
            Some(None)
        }
        None => {
            scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
            Some(None)
        }
    }
}

fn expand(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    // One switch for every channel: scale around ground, or around the screen center.
    // The manual spells this GND|CENTer on math. This screen keeps it on the channel.
    if query {
        return Some(text_reply(if m.v_expand_gnd { "GND" } else { "CENT" }));
    }
    if kw(arg, "GND") {
        m.v_expand_gnd = true;
    } else if kw(arg, "CENTer") {
        m.v_expand_gnd = false;
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    Some(None)
}

fn scale(query: bool, arg: &str, ch: &mut Channel, fine: bool, expand_gnd: bool) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&e(ch.volts_per_div() as f64)));
    }
    let old = ch.volts_per_div();
    let want = num(arg).filter(|&v| v > 0.0)? / ch.probe;
    (ch.vscale, ch.vfine) = if fine { fine_scale(want) } else { (nearest(&VSCALES, want), 1.0) };
    if expand_gnd && old > 0.0 {
        ch.offset *= ch.volts_per_div() / old;
    }

    Some(None)
}

fn units(scpi: &mut Scpi, query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match ch.unit {
            AmpUnit::V => "VOLT",
            AmpUnit::A => "AMP",
            AmpUnit::W => "WATT",
            AmpUnit::U => "UNKN",
        }));
    }
    let a = arg.trim();
    ch.unit = if kw(a, "VOLTage") {
        AmpUnit::V
    } else if kw(a, "AMPere") {
        AmpUnit::A
    } else if kw(a, "WATT") {
        AmpUnit::W
    } else if kw(a, "UNKNown") {
        AmpUnit::U
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };

    Some(None)
}

fn coupling(query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(ch.coupling.name()));
    }
    ch.coupling = match arg.to_ascii_uppercase().as_str() {
        "DC" => Coupling::Dc,
        "AC" => Coupling::Ac,
        "GND" => Coupling::Gnd,
        "DC50" if board().couplings().contains(&Coupling::Dc50) => Coupling::Dc50,
        _ => return None,
    };

    Some(None)
}

fn bandwidth(query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(
            match ch.bw {
                BW_20M => "20M".to_string(),
                BW_FULL => "OFF".to_string(),
                0 => "AUTO".to_string(),
                i => BWS[i].0.replace(' ', "").to_ascii_uppercase(),
            }
            .as_str(),
        ));
    }
    // OFF (full), 20M, AUTO, GND or a filter like 100M / 100MHZ / 1GHZ
    let a = arg.trim().to_ascii_uppercase().replace(' ', "");
    ch.bw = match a.as_str() {
        "OFF" | "FULL" => BW_FULL,
        "AUTO" => 0,
        "GND" => 3,
        "20M" | "20MHZ" => BW_20M,
        _ => BWS.iter().position(|b| {
            b.0.replace(' ', "").to_ascii_uppercase() == a
                || b.0.replace(' ', "").to_ascii_uppercase() == format!("{}HZ", a)
        })?,
    };

    Some(None)
}

fn bias(scpi: &mut Scpi, query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    // The screen's zero correction, subtracted from the channel reading.
    // The DHO programmer manual has no header for it.
    if query {
        return Some(text_reply(&e(ch.bias as f64)));
    }
    let Some(v) = num(arg) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    ch.bias = v;

    Some(None)
}

fn deskew(scpi: &mut Scpi, query: bool, arg: &str, ch: &mut Channel, slow_deskew: bool) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&e(ch.skew as f64)));
    }
    // The manual refuses a deskew change on a slow time base.
    if slow_deskew {
        scpi.push_err("-224,\"Illegal parameter value\" timebase".into());
        return Some(None);
    }
    let Some(v) = num(arg).filter(|v| v.abs() <= SKEW_MAX) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    ch.skew = v;

    Some(None)
}

fn probe(query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&e(ch.probe as f64)));
    }
    ch.probe = num(arg).filter(|&p| p > 0.0)?;
    Some(None)
}

fn invert(query: bool, arg: &str, ch: &mut Channel) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if ch.invert { "1" } else { "0" }));
    }
    ch.invert = parse_bool(arg)?;
    Some(None)
}
