// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// The settings of one built-in generator. The selected one is in the `afg_*` fields of the model, the others in
/// `Model::gen_bank` (`Model::gen_select`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenCh {
    pub on: bool,
    pub arb: u32,
    pub arb_src: usize,
    pub freq: f64,
    pub ampl: f64,
    pub offset: f64,
    pub wave: usize,
    pub duty: f64,
    pub symmetry: f64,
    pub modulation: usize,
    pub mod_rate: f64,
    pub mod_depth: f64,
    pub fm_dev: f64,
    pub pm_dev: f64,
    pub mod_shape: usize,
}

impl GenCh {
    pub const STANDARD: GenCh = GenCh {
        on: false,
        arb: 0,
        arb_src: 0,
        freq: 1000.0,
        ampl: 1.0,
        offset: 0.0,
        wave: 0,
        duty: 50.0,
        symmetry: 50.0,
        modulation: 0,
        mod_rate: 100.0,
        mod_depth: 100.0,
        fm_dev: 100.0,
        pm_dev: 90.0,
        mod_shape: 0,
    };
}
