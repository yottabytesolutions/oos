// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "POWer") && nodes.len() == 2 && query {
        return power::exec(scpi, ui, cmd, nodes, arg);
    }
    if kw(n0, "SPECtrum") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return spectrum::exec(scpi, ui, arg);
    }
    if kw(n0, "MAGnetic") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return magnetic::exec(scpi, ui, arg);
    }
    if kw(n0, "INVerter") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return inverter::exec(scpi, ui, arg);
    }
    if kw(n0, "DPT") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return dpt::exec(scpi, ui, arg);
    }
    if kw(n0, "BANDwidth") && nodes.len() == 2 && kw(nodes[1], "ITEM") && query {
        return bandwidth::exec(scpi, ui, arg);
    }

    super::undefined(scpi, cmd)
}
