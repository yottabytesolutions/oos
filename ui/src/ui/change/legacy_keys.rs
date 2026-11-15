// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Independently project the legacy want/generator/raw/route seams; no class bit supplies these observations.
#[derive(PartialEq)]
pub(super) struct Keys {
    want: crate::board::Want,
    generator: crate::board::Gen,
    raw: bool,
    route: bool,
    acquisition: (AcqMode, u32, bool),
    math: [MathOp; NMATH],
    mask_source: usize,
}
impl Keys {
    pub(super) fn of(ui: &Ui) -> Self {
        Self {
            want: board().want(&ui.m, ui.switches()),
            generator: board().gen(&ui.m.gen_read()),
            raw: ui.needs_raw(),
            route: ui.fast_ok(),
            acquisition: (ui.m.acq, ui.m.avg_count, ui.m.avg_hw),
            math: std::array::from_fn(|k| ui.m.math_ch(k).op),
            mask_source: ui.m.mask_src,
        }
    }
}

impl Keys {
    pub(super) fn observe(&self, old: &Self, signals: &mut Signals) {
        for (changed, row) in [
            (self.acquisition != old.acquisition, Row::Acquisition),
            (self.math != old.math, Row::Math),
            (self.mask_source != old.mask_source, Row::MaskSource),
        ] {
            if changed {
                signals.rows |= 1 << row as usize;
                signals.row = row;
            }
        }
    }
    pub(super) fn compress(&self) -> u32 {
        self.want.compress
    }
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
