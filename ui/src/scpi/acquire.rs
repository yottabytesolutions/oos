// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

fn model(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let m = &mut ui.m;
    let src = &ui.src;
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "ACQuire") && nodes.len() == 3 && kw(nodes[1], "ULTRa") && kw(nodes[2], "MAXFrame") {
        return ultra(scpi, query, arg, m);
    }
    if kw(n0, "ACQuire") && nodes.len() == 2 && !query && kw(nodes[1], "MDEPth") {
        return depth(arg, m);
    }
    if kw(n0, "ACQuire") && nodes.len() == 2 && kw(nodes[1], "CORes") {
        return cores(query, arg, m);
    }
    // segmented recording (OOS): :ACQuire:SEGMents <n>|OFF, :SEGMents:HARDware ON|OFF, :SEGMents:INTerval <s>
    if kw(n0, "ACQuire") && (2..=3).contains(&nodes.len()) && kw(nodes[1], "SEGMents") {
        return segments(nodes, query, arg, m);
    }
    // OOS: :ACQuire:AVERages:HARDware ON|OFF
    if kw(n0, "ACQuire") && nodes.len() == 3 && kw(nodes[1], "AVERages") && kw(nodes[2], "HARDware") {
        if query {
            return Some(text_reply(if m.avg_hw { "1" } else { "0" }));
        }
        m.avg_hw = parse_bool(arg)?;
        return Some(None);
    }
    if kw(n0, "ACQuire") && nodes.len() == 2 {
        return settings(scpi, nodes, query, arg, m, src);
    }

    None
}

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if let Some(resp) = model(scpi, ui, nodes, query, arg) {
        if !query {
            change::edited(ui);
        }
        return resp;
    }
    if kw(n0, "ACQuire") && nodes.len() == 3 && kw(nodes[1], "SEGMents") && kw(nodes[2], "TIMes") && query {
        return text_reply(&ui.seg_times_text());
    }
    super::undefined(scpi, cmd)
}

fn ultra(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&m.segments.to_string()));
    }
    let Some(n) = arg.trim().parse::<usize>().ok() else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    let steps = crate::ui::SEGMENTS;
    m.segments = *steps.iter().min_by_key(|s| s.abs_diff(n)).unwrap_or(&0);
    Some(None)
}

fn depth(arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    // AUTO or one of the depths of the Acquire menu (`mdepths`), e.g. 1000000 or 1M
    let a = arg.trim().to_ascii_uppercase();
    let want = if a.starts_with("AUTO") {
        0
    } else {
        let (v, mul) = match a.chars().last() {
            Some('K') => (&a[..a.len() - 1], 1e3),
            Some('M') => (&a[..a.len() - 1], 1e6),
            _ => (&a[..], 1.0),
        };
        (v.parse::<f64>().ok()? * mul).round() as u64
    };
    m.mdepth = mdepths().iter().position(|d| d.1 == want)?;
    Some(None)
}

fn cores(query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    // the ADC cores matched in software (on) or the words as they come (Acquire > ADC cores)
    if query {
        return Some(text_reply(if m.cores { "MATC" } else { "RAW" }));
    }
    let a = arg.trim().to_ascii_uppercase();
    m.cores = if a.starts_with("MATC") || a == "ON" || a == "1" {
        true
    } else if a.starts_with("RAW") || a == "OFF" || a == "0" {
        false
    } else {
        return None;
    };
    Some(None)
}

fn segments(nodes: &[&str], query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    match (nodes.get(2).copied(), query) {
        (None, true) => return Some(text_reply(&m.segments.to_string())),
        (None, false) => {
            m.segments = if parse_bool(arg) == Some(false) {
                0
            } else {
                (arg.trim().parse::<usize>().ok()?).min(crate::ui::SEG_HW_MAX)
            }
        }
        (Some(x), true) if kw(x, "HARDware") => return Some(text_reply(if m.seg_hw { "1" } else { "0" })),
        (Some(x), false) if kw(x, "HARDware") => m.seg_hw = parse_bool(arg)?,
        (Some(x), true) if kw(x, "INTerval") => return Some(text_reply(&e(m.seg_ivl as f64))),
        (Some(x), false) if kw(x, "INTerval") => m.seg_ivl = num(arg)?.clamp(0.0, crate::ui::SEG_IVL_MAX),
        _ => return None,
    }
    Some(None)
}

fn settings(
    scpi: &mut Scpi,
    nodes: &[&str],
    query: bool,
    arg: &str,
    m: &mut Model,
    src: &crate::wave::Source,
) -> Option<Option<Vec<u8>>> {
    let n1 = nodes[1];
    if kw(n1, "TYPE") {
        return mode(scpi, query, arg, m);
    }
    if kw(n1, "AVERages") {
        return averages(scpi, query, arg, m);
    }
    if query {
        // what the top bar shows: the live hardware record, not the model's stock-system choice for Auto
        let (sr, depth) = src.rate_depth(m);
        if kw(n1, "SRATe") {
            return Some(text_reply(&e(sr)));
        }
        if kw(n1, "MDEPth") {
            return Some(text_reply(&e(depth as f64)));
        }
    }
    None
}

fn mode(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match m.acq {
            AcqMode::Normal => "NORM",
            AcqMode::Peak => "PEAK",
            AcqMode::Average => "AVER",
            AcqMode::HiRes => "HRES",
        }));
    }
    let a = arg.trim();
    m.acq = if kw(a, "NORMal") {
        AcqMode::Normal
    } else if kw(a, "PEAK") {
        AcqMode::Peak
    } else if kw(a, "AVERages") {
        AcqMode::Average
    } else if kw(a, "HRESolution") || a.eq_ignore_ascii_case("HRES") {
        AcqMode::HiRes
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    Some(None)
}

fn averages(scpi: &mut Scpi, query: bool, arg: &str, m: &mut Model) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(&m.avg_count.to_string()));
    }
    // The Acquire menu's counts. A value that is not one of them snaps down to the next lower power of two.
    const COUNTS: [u32; 10] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
    let Some(n) = arg.trim().parse::<u32>().ok().filter(|n| *n >= 2) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return Some(None);
    };
    m.avg_count = COUNTS.iter().rev().find(|&&c| c <= n).copied().unwrap_or(2);
    Some(None)
}
