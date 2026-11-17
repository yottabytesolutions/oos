// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "CALibrate") && nodes.len() == 2 {
        if kw(nodes[1], "STATus") && query {
            return text_reply(&ui.cal_status_text());
        }
        // :CALibrate:STARt 0..4[,ZERO]   (0 = all channels; the inputs must be disconnected)
        if kw(nodes[1], "STARt") && !query {
            let mut it = arg.split(',');
            let ch = it.next().unwrap_or("0").trim();
            ui.selfcal_remote(ch, it.next().is_some_and(|z| kw(z.trim(), "ZERO")));
            return None;
        }
        if kw(nodes[1], "CANCel") && !query {
            ui.selfcal_remote("cancel", false);
            return None;
        }
    }
    super::undefined(scpi, cmd)
}
