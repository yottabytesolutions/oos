// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], arg: &str) -> Option<Vec<u8>> {
    if kw(nodes[1], "HARMonics") {
        let src = match scpi.waves(arg) {
            Ok(v) => v,
            Err(s) => {
                scpi.push_err(s);
                return text_reply("9.9E37");
            }
        };
        return match ui.fact_harmonics(&src) {
            Ok(h) if h.is_empty() => text_reply("9.9E37"),
            Ok(h) => text_reply(&h.iter().map(|(n, a)| format!("{},{}", n, e(*a))).collect::<Vec<_>>().join(";")),
            Err(()) => {
                scpi.push_err("-224,\"Illegal parameter value\"".into());
                text_reply("9.9E37")
            }
        };
    }
    if kw(nodes[1], "ITEM") {
        return scpi.sourced(ui, arg, |ui, item, src| ui.fact_power(item, src));
    }

    super::undefined(scpi, cmd)
}
