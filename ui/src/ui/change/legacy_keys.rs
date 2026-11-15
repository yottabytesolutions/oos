// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Independently project the legacy want/generator/raw/route seams; no class bit supplies these observations.
#[derive(PartialEq)]
pub(super) struct Keys {
    want: crate::board::Want,
    generator: crate::board::Gen,
    raw: bool,
    route: bool,
}
impl Keys {
    pub(super) fn of(ui: &Ui) -> Self {
        Self {
            want: board().want(&ui.m, ui.switches()),
            generator: board().gen(&ui.m.gen_read()),
            raw: ui.needs_raw(),
            route: ui.fast_ok(),
        }
    }
}

impl Keys {
    pub(super) fn changed(&self, other: &Self, transition: bool) -> bool {
        let mut a = self.want.clone();
        let mut b = other.want.clone();
        if transition {
            // Sweep/run companions belong to the request boundary, not an ordinary settings invalidation.
            a.auto = false;
            b.auto = false;
            a.single = false;
            b.single = false;
        }
        a != b || self.generator != other.generator || self.raw != other.raw || self.route != other.route
    }
}
