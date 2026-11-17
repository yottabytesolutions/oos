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
    if kw(n0, "DISPlay") && !(nodes.len() == 2 && kw(nodes[1], "DATA")) && !(nodes.len() >= 2 && kw(nodes[1], "VIEW")) {
        return scpi.display_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "DISPlay") && nodes.len() == 2 && kw(nodes[1], "DATA") && query {
        return data(scpi, cv, arg);
    }
    // Display > Views (ui/views.rs): :DISPlay:VIEW <n> shows view n, :DISPlay:VIEW:COUNt <1..4> how many there are
    if kw(n0, "DISPlay") && nodes.len() >= 2 && kw(nodes[1], "VIEW") {
        return views(scpi, ui, nodes, query, arg);
    }
    // OOS extensions: analyser view, Identify, self-calibration, Bode
    if kw(n0, "VIEW") && nodes.len() == 1 {
        return view(scpi, ui, query, arg);
    }

    super::undefined(scpi, cmd)
}

fn data(scpi: &mut Scpi, cv: &Canvas, arg: &str) -> Option<Vec<u8>> {
    let kind = arg.split_whitespace().next().unwrap_or("").trim();
    let up = kind.to_ascii_uppercase();
    if up.starts_with("PNG") {
        return Some(block(crate::store::png_of(cv)));
    }
    if up.is_empty() || up.starts_with("BMP") {
        return Some(block(bmp(cv)));
    }
    scpi.push_err(format!("-224,\"Illegal parameter value\" {}", kind));
    None
}

fn views(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let count = nodes.len() == 3 && kw(nodes[2], "COUNt");
    // Display > Monitor view (ui/monitor.rs): the view an HDMI monitor in Layout shows, 0 the panel's
    if nodes.len() == 3 && kw(nodes[2], "MONitor") {
        if query {
            return text_reply(&ui.m.hdmi_view.to_string());
        }
        match arg.trim().parse::<usize>().ok() {
            Some(n) if n <= ui.m.views && (n == 0 || ui.m.views >= 2) => ui.m.hdmi_view = n,
            _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return None;
    }
    if nodes.len() > 3 || (nodes.len() == 3 && !count) {
        scpi.push_err("-113,\"Undefined header\"".into());
        return None;
    }
    if query {
        return text_reply(&(if count { ui.m.views } else { ui.m.view_cur + 1 }).to_string());
    }
    let n = arg.trim().parse::<usize>().ok();
    match n {
        Some(n) if count && (1..=crate::ui::VIEWS_MAX).contains(&n) => ui.views_set(n),
        Some(n) if !count && (1..=ui.m.views).contains(&n) => ui.view_go(n - 1),
        _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn view(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    let views = crate::ui::scpi_views();
    if query {
        return text_reply(&format!("{:?}", ui.m.view).to_ascii_uppercase());
    }
    match views.iter().find(|(k, _)| kw(arg, k)) {
        Some((_, v)) => ui.set_analysis_view(*v),
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}
