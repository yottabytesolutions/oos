// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "BODE") && nodes.len() == 2 {
        if kw(nodes[1], "RUN") && !query {
            ui.bode_run(parse_bool(arg).unwrap_or(true));
            return None;
        }
        if kw(nodes[1], "DATA") && query {
            return text_reply(&ui.bode_text());
        }
        if kw(nodes[1], "MODE") {
            return mode(scpi, ui, query, arg);
        }
        if kw(nodes[1], "GENerator") {
            return generator(scpi, ui, query, arg);
        }
    }
    super::undefined(scpi, cmd)
}

fn mode(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    const MODES: [(&str, oos_dsp::bode::Mode); 3] = [
        ("LOOP", oos_dsp::bode::Mode::Loop),
        ("PSRR", oos_dsp::bode::Mode::Psrr),
        ("IMPedance", oos_dsp::bode::Mode::Z),
    ];
    if query {
        let name = match ui.m.bode_mode {
            oos_dsp::bode::Mode::Loop => "LOOP",
            oos_dsp::bode::Mode::Psrr => "PSRR",
            oos_dsp::bode::Mode::Z => "IMP",
        };
        return text_reply(name);
    }
    match MODES.iter().find(|(k, _)| kw(arg, k)) {
        Some((_, m)) => {
            ui.m.bode_mode = *m;
            change::edited(ui);
        }
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn generator(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    // "usb": the generator on the USB-A port; anything else is its LAN address (kept while on USB)
    if query {
        return text_reply(if ui.m.bode_usb { "usb" } else { ui.m.bode_addr.as_str() });
    }
    // one word, like the setup file keeps it
    let a = arg.trim().trim_matches('"');
    if a.contains(char::is_whitespace) {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    } else if a.eq_ignore_ascii_case("usb") {
        ui.m.bode_usb = true;
    } else {
        (ui.m.bode_addr, ui.m.bode_usb) = (a.to_string(), false);
    }
    None
}
