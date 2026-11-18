// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "SYSTem") && nodes.len() == 2 {
        if let Some(reply) = settings(scpi, ui, nodes, query, arg) {
            return reply;
        }
    }

    // fork: the TPU UART receiver's words for the current settings (docs/fork/hwuart.md)
    if kw(n0, "SYSTem")
        && nodes.len() == 3
        && kw(nodes[1], "DEBug")
        && (kw(nodes[2], "UART") || kw(nodes[2], "SERial"))
        && query
    {
        return text_reply(&board().uart_debug(&ui.m, ui.switches()));
    }
    // fork: the zone unit's raw status and counter next to the software filter (docs/fork/hwzone.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "ZONE") && query {
        return text_reply(&ui.zone_debug_text());
    }
    // fork: the comparator mapping of the mask test and zone trigger, and the per-column view (docs/fork/hwmask.md)
    if kw(n0, "SYSTem") && nodes.len() == 4 && kw(nodes[1], "DEBug") && kw(nodes[2], "MASK") && kw(nodes[3], "MAP") {
        return mask_map(scpi, ui, query, arg);
    }
    if kw(n0, "SYSTem")
        && nodes.len() == 4
        && kw(nodes[1], "DEBug")
        && kw(nodes[2], "MASK")
        && kw(nodes[3], "COLumns")
        && query
    {
        let mut it = arg.split(',').map(|x| x.trim().parse::<usize>());
        let (a, b) = match (it.next(), it.next()) {
            (Some(Ok(a)), Some(Ok(b))) => (a.min(b), a.max(b)),
            (Some(Ok(a)), None) => (a, a),
            _ => (0, 9),
        };
        return text_reply(&ui.mask_columns_text(a, b));
    }
    // fork: the mask unit's raw counters and slot next to the software test (docs/fork/hwmask.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "MASK") && query {
        return text_reply(&ui.mask_debug_text());
    }
    // fork: the DVM block's raw registers next to the software DVM (docs/fork/dvm.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "DVM") && query {
        return text_reply(&ui.dvm_debug_text());
    }
    // fork, unit debug: force the DVM block's lane with one channel (each lane = one pair of ADC cores)
    if kw(n0, "SYSTem") && nodes.len() == 4 && kw(nodes[1], "DEBug") && kw(nodes[2], "DVM") && kw(nodes[3], "LANE") {
        return dvm_lane(scpi, ui, query, arg);
    }
    if kw(n0, "SYSTem") && nodes.len() == 2 && kw(nodes[1], "ERRor") && query {
        return text_reply(&if scpi.err.is_empty() { "0,\"No error\"".to_string() } else { scpi.err.remove(0) });
    }

    super::undefined(scpi, cmd)
}

fn settings(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if kw(nodes[1], "SET") && !query {
        return set(scpi, ui, arg);
    }
    if kw(nodes[1], "GET") && query {
        let text = crate::store::setup_text(&ui.m);
        let v = text
            .lines()
            .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == arg.trim()).map(|x| x.1.trim().to_string()));
        return Some(text_reply(&v.unwrap_or_default()));
    }
    if kw(nodes[1], "STATe") && query {
        let mut text = crate::cmd::stat_text(ui).trim().to_string();
        if let Some(raw) = &scpi.transport.endpoint.io {
            text.push_str(&raw.ports().enqueue().text());
        }
        return Some(text_reply(&text));
    }
    // Acquire > Draw path: FRAMes (Standard) or BATCh (default); the `pipe` part of
    // :SYSTem:STATe? says which runs
    if kw(nodes[1], "PIPeline") {
        return pipeline(scpi, ui, query, arg);
    }
    // System > Graphics: CPU or GPU (whether the GPU really draws: the `gpu` part of :SYSTem:STATe?)
    if kw(nodes[1], "GRAPhics") {
        return graphics(scpi, ui, query, arg);
    }
    // rear AUX OUT, stock names TOUT and PFAil; OFF is OOS's (constant low)
    if kw(nodes[1], "AOUTput") {
        return aux_output(scpi, ui, query, arg);
    }
    if kw(nodes[1], "LOG") && !query {
        match parse_bool(arg) {
            Some(on) => ui.log_set(on),
            None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return Some(None);
    }

    None
}

fn mask_map(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    match board().mask_map(if query { None } else { Some(arg.trim()) }) {
        Ok(s) if query => return text_reply(&s),
        Ok(_) => change::edited(ui),
        Err(e) => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", e)),
    }
    None
}

fn set(scpi: &mut Scpi, ui: &mut Ui, arg: &str) -> Option<Option<Vec<u8>>> {
    let r = arg.split_once('=').ok_or("no '='".to_string()).and_then(|(k, v)| {
        if k.trim() == "trig_mode" {
            ui.set_sweep(crate::store::parse_sweep(v.trim())?);
        } else {
            crate::store::set(&mut ui.m, k.trim(), v.trim())?;
        }
        Ok(())
    });
    match r {
        Ok(()) => change::edited(ui),
        Err(e) => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", e)),
    }
    // a setup file may hold keys of newer versions, here a typo must not pass for a setting; the keys that
    // are only written when they differ from their default do not show
    let k = arg.split('=').next().unwrap_or("").trim();
    let optional = ["vfine", "unit", "bias", "skew", "label", "tfine"].iter().any(|s| k.ends_with(s))
        || k.starts_with("math_expr")
        || k == "cat";
    if !optional && !crate::store::setup_text(&ui.m).lines().any(|l| l.split('=').next().unwrap_or("").trim() == k) {
        scpi.push_err(format!("-224,\"Illegal parameter value\" unknown key {}", k));
    }
    Some(None)
}

fn pipeline(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if ui.m.draw_batch { "BATC" } else { "FRAM" }));
    }
    let a = arg.trim().to_ascii_uppercase();
    if a.starts_with("BATC") {
        ui.m.draw_batch = true;
    } else if a.starts_with("FRAM") || a.starts_with("STAN") {
        ui.m.draw_batch = false;
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    Some(None)
}

fn graphics(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if ui.sys.gpu { "GPU" } else { "CPU" }));
    }
    match arg.trim().to_ascii_uppercase().as_str() {
        "GPU" => ui.graphics_set(true),
        "CPU" => ui.graphics_set(false),
        _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    Some(None)
}

fn aux_output(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match ui.m.aux_out {
            AuxOut::TrigOut => "TOUT",
            AuxOut::Off => "OFF",
            AuxOut::PassFail => "PFA",
        }));
    }
    let want = match arg.trim().to_ascii_uppercase().as_str() {
        "TOUT" => Some(AuxOut::TrigOut),
        "OFF" => Some(AuxOut::Off),
        "PFA" | "PFAIL" => Some(AuxOut::PassFail),
        _ => None,
    };
    match want.filter(|a| a.offered(board().mask_hw(ui.switches()) || board().zone_hw(ui.switches()))) {
        Some(a) => {
            ui.m.aux_out = a;
            change::edited(ui);
        }
        None => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    Some(None)
}

fn dvm_lane(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&ui.dvm_lane_text());
    }
    match arg.trim().to_ascii_uppercase().as_str() {
        "AUTO" => ui.dvm_lane_set(None),
        a => match a.parse::<u32>() {
            Ok(l) if l < 4 => ui.dvm_lane_set(Some(l)),
            _ => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", arg)),
        },
    }
    None
}
