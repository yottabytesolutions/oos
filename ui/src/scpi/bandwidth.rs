// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, arg: &str) -> Option<Vec<u8>> {
    scpi.sourced(ui, arg, |ui, item, src| ui.fact_bandwidth(item, src))
}
