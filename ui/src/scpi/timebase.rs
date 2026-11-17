// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

fn model(ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    let m = &mut ui.m;
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "TIMebase") {
        let sub = nodes.get(1).copied().unwrap_or("");
        // Delayed sweep, XY, the expand point, and fine adjust need the whole screen, not just the model.
        if kw(sub, "DELay") || kw(sub, "XY") || kw(sub, "HREFerence") || kw(sub, "VERNier") {
            return None;
        }
        let last = *nodes.last()?;
        if kw(last, "SCALe") {
            if query {
                return Some(text_reply(&e(m.time_per_div() as f64)));
            }
            m.tscale = nearest(&tscales(), num(arg)?);
            m.tfine = 1.0;
            return Some(None);
        }
        if kw(last, "OFFSet") || kw(last, "POSition") {
            if query {
                return Some(text_reply(&e(m.delay as f64)));
            }
            m.delay = num(arg)?;
            return Some(None);
        }
        return None;
    }
    None
}

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    if let Some(resp) = model(ui, nodes, query, arg) {
        if !query {
            change::edited(ui);
        }
        return resp;
    }
    scpi.timebase_more(ui, &nodes[1..], query, arg)
}
