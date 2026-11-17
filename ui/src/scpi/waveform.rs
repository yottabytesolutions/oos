// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "WAVeform") && nodes.len() == 2 {
        let n1 = nodes[1];
        if kw(n1, "SOURce") {
            return source(scpi, query, arg);
        }
        if kw(n1, "MODE") {
            return mode(scpi, query, arg);
        }
        if kw(n1, "FORMat") {
            return format(scpi, query, arg);
        }
        if kw(n1, "STARt") || kw(n1, "STOP") {
            return range(scpi, ui, query, arg, n1);
        }
        if kw(n1, "POINts") {
            return points(scpi, ui, query, arg);
        }
        if kw(n1, "PREamble") && query {
            return text_reply(&scpi.wave_preamble(ui));
        }
        if kw(n1, "DATA") && query {
            return scpi.wave_data(ui);
        }
    }
    super::undefined(scpi, cmd)
}

fn source(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&scpi.wav_source_text());
    }
    if let Some(id) = parse_wave(arg) {
        scpi.wav_src = id;
    } else if !arg.is_empty() {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    None
}

fn mode(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(["NORM", "MAX", "RAW"][scpi.wav_mode.min(2) as usize]);
    }
    let a = arg.trim();
    scpi.wav_mode = if kw(a, "RAW") {
        2
    } else if kw(a, "MAXimum") {
        1
    } else if kw(a, "NORMal") {
        0
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    None
}

fn format(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(["BYTE", "WORD", "ASC"][scpi.wav_fmt as usize]);
    }
    let format = if kw(arg.trim(), "BYTE") {
        Some(0)
    } else if kw(arg.trim(), "WORD") {
        Some(1)
    } else if kw(arg.trim(), "ASCii") {
        Some(2)
    } else {
        None
    };
    if let Some(format) = format {
        scpi.wav_fmt = format;
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    None
}

fn range(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, n1: &str) -> Option<Vec<u8>> {
    let stop = kw(n1, "STOP");
    if query {
        let n = scpi.wave_len(ui);
        let v = if stop {
            if scpi.wav_stop == 0 {
                n
            } else {
                scpi.wav_stop
            }
        } else {
            scpi.wav_start
        };
        return text_reply(&v.to_string());
    }
    let Some(p) = arg.trim().parse::<u64>().ok().filter(|&p| p >= 1) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    if stop {
        scpi.wav_stop = p;
        if p < scpi.wav_start {
            scpi.wav_start = p;
        }
    } else {
        scpi.wav_start = p;
        if scpi.wav_stop != 0 && scpi.wav_stop < p {
            scpi.wav_stop = p;
        }
    }
    None
}

fn points(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&scpi.page_count(ui).to_string());
    }
    // Rigol: the count to read, starting at STARt. stop 0 still means "the rest" until this sets it.
    let Some(n) = arg.trim().parse::<u64>().ok().filter(|&n| n >= 1) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    scpi.wav_stop = scpi.wav_start.saturating_add(n - 1);
    None
}
