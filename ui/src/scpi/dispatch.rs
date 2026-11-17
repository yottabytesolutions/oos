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
    let (family, _) = suffix(n0);
    if kw(family, "CHANnel") {
        return channel::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "TIMebase") {
        return timebase::exec(scpi, ui, nodes, query, arg);
    }
    if kw(n0, "TRIGger") {
        return trigger::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "ACQuire") {
        return acquire::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "MEASure") || kw(n0, "EYE") {
        return measure::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "POWer")
        || kw(n0, "SPECtrum")
        || kw(n0, "MAGnetic")
        || kw(n0, "INVerter")
        || kw(n0, "DPT")
        || kw(n0, "BANDwidth")
    {
        return analysis::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "WAVeform") {
        return waveform::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "DISPlay") || kw(n0, "VIEW") {
        return display::exec(scpi, ui, cv, cmd, nodes, query, arg);
    }
    if kw(n0, "CALibrate") {
        return calibrate::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if kw(n0, "BODE") {
        return bode::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if suffix(n0).1.is_none_or(|n| (1..=gen_count().max(1)).contains(&n)) && kw(family, "SOURce") && nodes.len() >= 2 {
        return source::exec(scpi, ui, cv, cmd, nodes, query, arg);
    }
    if kw(n0, "SYSTem") {
        return system::exec(scpi, ui, cmd, nodes, query, arg);
    }
    remaining(scpi, ui, cmd, nodes, query, arg)
}

fn remaining(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "DECode") {
        return decode::exec(scpi, ui, cmd, nodes, query, arg);
    }
    if nodes.len() >= 2 && kw(n0, "LA") {
        return scpi.la_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "REFerence") {
        return scpi.ref_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "SIM") {
        return scpi.sim_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "CURSor") {
        return scpi.cursor_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "MASK") {
        return scpi.mask_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "HISTogram") {
        return scpi.hist_cmd(ui, &nodes[1..], query, arg);
    }
    if let Some(k) = math_index(n0) {
        return scpi.math_cmd(ui, k, &nodes[1..], query, arg);
    }
    if let Some(k) = bus_index(n0) {
        return scpi.bus_cmd(ui, k, &nodes[1..], query, arg);
    }
    if kw(n0, "DVM") {
        return scpi.dvm_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "COUNter") {
        return scpi.counter_cmd(ui, &nodes[1..], query, arg);
    }
    undefined(scpi, cmd)
}
