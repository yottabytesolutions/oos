// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if nodes.len() == 3 && kw(n0, "DECode") && kw(nodes[1], "CPHY") && (kw(nodes[2], "RATE") || kw(nodes[2], "SRATe")) {
        return scpi.decode_cphy_rate_cmd(ui, query, arg);
    }
    if nodes.len() == 2 && kw(n0, "DECode") {
        return scpi.decode_cmd(ui, nodes[1], query, arg);
    }
    if kw(n0, "DECode") && nodes.len() == 2 && kw(nodes[1], "EVENts") && query {
        return Some(ui.events_text().as_bytes().to_vec());
    }
    super::undefined(scpi, cmd)
}
