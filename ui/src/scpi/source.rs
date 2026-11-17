// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(
    scpi: &mut Scpi,
    ui: &mut Ui,
    cv: &Canvas,
    cmd: &str,
    nodes: &[&str],
    query: bool,
    arg: &str,
) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if suffix(n0).1.is_none_or(|n| (1..=gen_count().max(1)).contains(&n))
        && kw(suffix(n0).0, "SOURce")
        && nodes.len() >= 2
    {
        if !oos_hal::model::platform().caps.has_gen() {
            scpi.push_err(format!(
                "-241,\"Hardware missing\" no generator on the {}",
                oos_hal::model::platform().caps.name
            ));
            return None;
        }
        // :SOURce<n> talks to generator n, whichever one the page shows
        let g = suffix(n0).1.unwrap_or(1) - 1;
        if g != ui.m.gen_sel {
            let shown = ui.m.gen_sel;
            ui.m.gen_select(g);
            let r = scpi.exec_cmd(ui, cv, cmd);
            ui.m.gen_select(shown);
            return r;
        }
        let has = |k: &str| nodes[1..].iter().any(|n| kw(suffix(n).0, k));
        let m = &ui.m;
        let x = arg.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        if has("OUTPut") {
            if query {
                return text_reply(if m.afg_on { "1" } else { "0" });
            }
            match parse_bool(arg) {
                Some(on) => ui.afg_set(Some(on), None, None, None),
                None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
            }
            return None;
        }
        // modulation (Rigol DG style): :MOD[:STATe] ON|OFF, :MOD:TYPe AM|FM|PM, :MOD:AM:DEPTh <%>, :MOD:FM:DEViation <Hz>,
        // :MOD:PM:DEViation <deg>, :MOD:<type>:INTernal:FREQuency <Hz>, :MOD:<type>:INTernal:FUNCtion <shape>
        if has("MOD") {
            return modulation(scpi, ui, query, arg, has, x);
        }
        // :FUNCtion:SQUare:DUTY / :FUNCtion:PULSe:DUTY and :FUNCtion:RAMP:SYMMetry in %
        // arbitrary wave: :ARB:DATA <v>,<v>,... (one period, any scale), :ARB:CAPTure CHAN<n> (the screen of a channel)
        if has("ARB") {
            return arbitrary(scpi, ui, query, arg, has);
        }
        if has("FUNCtion") && (has("DUTY") || has("SYMMetry")) {
            return shape(scpi, ui, query, arg, has, x);
        }
        if has("FUNCtion") {
            return function(scpi, ui, query, arg);
        }
        return value(scpi, ui, cmd, query, arg, has, x);
    }
    super::undefined(scpi, cmd)
}

fn modulation(
    scpi: &mut Scpi,
    ui: &mut Ui,
    query: bool,
    arg: &str,
    has: impl Fn(&str) -> bool,
    x: Option<f64>,
) -> Option<Vec<u8>> {
    let m = &ui.m;
    let a = arg.trim();
    if has("INTernal") && has("FUNCtion") {
        return mod_shape(scpi, ui, query, arg, a);
    }
    let (cur, key): (f64, u8) = if has("INTernal") && has("FREQuency") {
        (m.afg_mod_rate, 0)
    } else if has("DEPTh") {
        (m.afg_mod_depth, 1)
    } else if has("DEViation") && has("PM") {
        (m.afg_pm_dev, 3)
    } else if has("DEViation") {
        (m.afg_fm_dev, 2)
    } else if has("TYPe") {
        if query {
            return text_reply(["NONE", "AM", "FM", "PM"][m.afg_mod.min(3)]);
        }
        match ["AM", "FM", "PM"].iter().position(|k| kw(a, k)) {
            Some(i) => ui.m.afg_mod = i + 1,
            None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        ui.afg_set(None, None, None, None);
        return None;
    } else {
        // :MOD[:STATe]
        if query {
            return text_reply(if m.afg_mod > 0 { "1" } else { "0" });
        }
        match parse_bool(arg) {
            Some(on) => ui.m.afg_mod = if on { ui.m.afg_mod.max(1) } else { 0 },
            None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return None;
    };
    if query {
        return text_reply(&e(cur));
    }
    match x {
        Some(v) => {
            let mm = &mut ui.m;
            match key {
                0 => mm.afg_mod_rate = v.clamp(0.002, 1e6),
                1 => mm.afg_mod_depth = v.clamp(0.0, 120.0),
                2 => mm.afg_fm_dev = v.max(0.01),
                _ => mm.afg_pm_dev = v.clamp(0.0, 360.0),
            }
            ui.afg_set(None, None, None, None);
        }
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn arbitrary(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, has: impl Fn(&str) -> bool) -> Option<Vec<u8>> {
    if has("DATA") && !query {
        let v: Vec<f32> = arg.split(',').filter_map(num).collect();
        if !ui.afg_arb_load(&v) {
            scpi.push_err(format!("-224,\"Illegal parameter value\" {} samples", v.len()));
        }
    } else if has("CAPTure") && !query {
        match parse_ch(arg).ok_or("no channel".to_string()).and_then(|c| ui.afg_arb_capture(c)) {
            Ok(_) => {}
            Err(e) => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", e)),
        }
    } else {
        scpi.push_err("-113,\"Undefined header\" :SOURce:ARB".into());
    }
    None
}

fn shape(
    scpi: &mut Scpi,
    ui: &mut Ui,
    query: bool,
    arg: &str,
    has: impl Fn(&str) -> bool,
    x: Option<f64>,
) -> Option<Vec<u8>> {
    let m = &ui.m;
    let duty = has("DUTY");
    if query {
        return text_reply(&e(if duty { m.afg_duty } else { m.afg_symmetry }));
    }
    match x {
        Some(v) if duty => ui.m.afg_duty = v.clamp(5.0, 95.0),
        Some(v) => ui.m.afg_symmetry = v.clamp(0.0, 100.0),
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn function(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    let m = &ui.m;
    const SHORT: [&str; 7] = ["SIN", "SQU", "RAMP", "PULS", "NOIS", "DC", "ARB"];
    const LONG: [&str; 7] = ["SINusoid", "SQUare", "RAMP", "PULSe", "NOISe", "DC", "ARB"];
    if query {
        return text_reply(SHORT[m.afg_wave.min(6)]);
    }
    match LONG.iter().position(|l| kw(arg.trim(), l)) {
        Some(i) => {
            ui.m.afg_wave = i;
            ui.afg_set(None, None, None, None);
        }
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn mod_shape(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, a: &str) -> Option<Vec<u8>> {
    const SHAPES: [&str; 6] = ["SINusoid", "SQUare", "TRIangle", "RAMP", "NRAMp", "NOISe"];
    let m = &ui.m;
    if query {
        return text_reply(
            &SHAPES[m.afg_mod_shape.min(5)].chars().filter(|c| c.is_ascii_uppercase()).collect::<String>(),
        );
    }
    match SHAPES.iter().position(|s| kw(a, s)) {
        Some(i) => ui.m.afg_mod_shape = i,
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn value(
    scpi: &mut Scpi,
    ui: &mut Ui,
    cmd: &str,
    query: bool,
    arg: &str,
    has: impl Fn(&str) -> bool,
    x: Option<f64>,
) -> Option<Vec<u8>> {
    let m = &ui.m;
    let (want, cur): (u8, f64) = if has("FREQuency") {
        (0, m.afg_freq)
    } else if has("VOLTage") && has("OFFSet") {
        (2, m.afg_offset)
    } else if has("VOLTage") {
        (1, m.afg_ampl)
    } else {
        (9, 0.0)
    };
    if want < 9 {
        if query {
            return text_reply(&e(cur));
        }
        match x {
            Some(v) => ui.afg_set(None, (want == 0).then_some(v), (want == 1).then_some(v), (want == 2).then_some(v)),
            None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return None;
    }
    super::undefined(scpi, cmd)
}
