// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, arg: &str) -> Option<Vec<u8>> {
    let src = match scpi.waves(arg_rest(arg)) {
        Ok(v) => v,
        Err(s) => {
            scpi.push_err(s);
            return text_reply("9.9E37");
        }
    };
    match ui.fact_inverter(&arg_item(arg), &src) {
        Ok(crate::ui::facts::Fact::Word(w)) => text_reply(w),
        Ok(crate::ui::facts::Fact::Num(v)) => text_reply(&shown(v)),
        Err(()) => {
            scpi.push_err("-224,\"Illegal parameter value\"".into());
            text_reply("9.9E37")
        }
    }
}
