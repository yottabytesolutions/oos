// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "MEASure") && nodes.len() == 2 && query {
        if kw(nodes[1], "ITEM") {
            return scpi.meas_item(ui, arg);
        }
        if kw(nodes[1], "STRip") {
            return text_reply(&meas_strip(ui));
        }
        // every measurement on the screen: the strip, and the eye figures when that page is up
        if kw(nodes[1], "READout") {
            return text_reply(&meas_readout(ui));
        }
    }
    if kw(n0, "EYE") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return scpi.eye_item(ui, arg);
    }
    if kw(n0, "MEASure") && nodes.len() == 3 && kw(nodes[1], "BURSt") && kw(nodes[2], "IDLE") {
        return burst(scpi, ui, query, arg);
    }
    if kw(n0, "MEASure") && nodes.len() == 3 && kw(nodes[1], "STATistic") {
        return scpi.meas_stat_cmd(ui, nodes[2], query, arg);
    }
    if kw(n0, "MEASure") && nodes.len() == 3 && kw(nodes[1], "SETup") {
        if let Some(reply) = setup(scpi, ui, nodes, query, arg) {
            return reply;
        }
    }

    if kw(n0, "MEASure") && nodes.len() == 2 {
        if let Some(reply) = settings(scpi, ui, nodes, query, arg) {
            return reply;
        }
    }

    super::undefined(scpi, cmd)
}

fn burst(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&e(ui.m.meas_idle));
    }
    match arg.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0) {
        Some(v) => {
            ui.m.meas_idle = v;
            ui.measure_last();
            change::edited(ui);
        }
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn setup(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let which = nodes[2];
    if kw(which, "MAX") || kw(which, "MIN") {
        return Some(scpi.meas_threshold(ui, kw(which, "MAX"), query, arg));
    }
    if kw(which, "PSA") || kw(which, "DSA") || kw(which, "PSB") || kw(which, "DSB") {
        let second = kw(which, "PSB") || kw(which, "DSB");
        if query {
            return Some(text_reply(&wave_tok(if second { ui.meas_source_b() } else { ui.meas_source() })));
        }
        let Some(id) = scpi.named_source(arg) else { return Some(None) };
        if second {
            ui.set_meas_source_b(id);
        } else {
            ui.set_meas_source(id);
        }
        change::edited(ui);
        return Some(None);
    }

    None
}

fn settings(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let n1 = nodes[1];
    if kw(n1, "GATing") {
        return gating(scpi, ui, query, arg);
    }
    if kw(n1, "SOURce") {
        return source(scpi, ui, query, arg);
    }
    if kw(n1, "CLEar") && !query {
        actions::meas_clear(scpi, ui, arg);
        return Some(None);
    }
    if kw(n1, "ITEM") && !query {
        return Some(scpi.meas_place(ui, arg));
    }
    if kw(n1, "REFS") {
        return refs(scpi, ui, query, arg);
    }
    if kw(n1, "INDicator") {
        return Some(scpi.meas_indicator(ui, query, arg));
    }
    if kw(n1, "OUTSide") || kw(n1, "NPERiods") || kw(n1, "EDGes") {
        return Some(scpi.meas_gate(ui, n1, query, arg));
    }
    if kw(n1, "TSETup") || kw(n1, "THOLd") {
        return setup_hold(scpi, ui, query, arg, n1);
    }
    if kw(n1, "ITEM") && query {
        return Some(scpi.meas_item(ui, arg));
    }

    None
}

fn gating(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if ui.m.meas_full_record { "NONE" } else { "SCREEN" }));
    }
    let full_record = if kw(arg.trim(), "SCReen") {
        false
    } else if kw(arg.trim(), "NONE") {
        true
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    ui.set_measurement_full_record(full_record);
    Some(None)
}

fn source(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&wave_tok(ui.meas_source())));
    }
    let words: Vec<_> = arg.split(',').map(str::trim).collect();
    if words.is_empty() || words.len() > 2 || words.iter().any(|word| word.is_empty()) {
        scpi.push_err("-224,\"Illegal parameter value\"".into());
        return Some(None);
    }
    let Some(a) = scpi.named_source(words[0]) else { return Some(None) };
    let b = if words.len() == 2 {
        let Some(b) = scpi.named_source(words[1]) else { return Some(None) };
        Some(b)
    } else {
        None
    };
    ui.set_meas_source(a);
    if let Some(b) = b {
        ui.set_meas_source_b(b);
    }
    change::edited(ui);
    Some(None)
}

fn refs(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        let ten = ui.m.meas_lpct < 0.15 && ui.m.meas_hpct > 0.85;
        return Some(text_reply(if ten { "10/90" } else { "20/80" }));
    }
    let a = arg.trim().replace(' ', "");
    let (lo, hi) = if a == "10/90" || a == "1090" || a == "10" {
        (0.1, 0.9)
    } else if a == "20/80" || a == "2080" || a == "20" {
        (0.2, 0.8)
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    ui.m.meas_lpct = lo;
    ui.m.meas_hpct = hi;
    ui.measure_last();
    change::edited(ui);
    Some(None)
}

fn setup_hold(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, n1: &str) -> Option<Option<Vec<u8>>> {
    let setup = kw(n1, "TSETup");
    if query {
        return Some(text_reply(&e(if setup { ui.m.meas_tsetup } else { ui.m.meas_thold })));
    }
    match arg.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0) {
        Some(v) => {
            if setup {
                ui.m.meas_tsetup = v;
            } else {
                ui.m.meas_thold = v;
            }
            change::edited(ui);
        }
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    Some(None)
}
