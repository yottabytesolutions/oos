// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "aux_out",
        |m| {
            m.aux_out = if m.aux_out == AuxOut::TrigOut { AuxOut::Off } else { AuxOut::TrigOut };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "gen_sel",
        |m| {
            m.gen_sel += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "gen_bank",
        |m| {
            m.gen_bank[0].freq += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_on",
        |m| {
            m.afg_on = !m.afg_on;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_arb",
        |m| {
            m.afg_arb += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_arb_src",
        |m| {
            m.afg_arb_src += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_freq",
        |m| {
            m.afg_freq += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_ampl",
        |m| {
            m.afg_ampl += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_offset",
        |m| {
            m.afg_offset += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_wave",
        |m| {
            m.afg_wave += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_duty",
        |m| {
            m.afg_duty += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_symmetry",
        |m| {
            m.afg_symmetry += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_mod",
        |m| {
            m.afg_mod += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_mod_rate",
        |m| {
            m.afg_mod_rate += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_mod_depth",
        |m| {
            m.afg_mod_depth += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_fm_dev",
        |m| {
            m.afg_fm_dev += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_pm_dev",
        |m| {
            m.afg_pm_dev += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "afg_mod_shape",
        |m| {
            m.afg_mod_shape += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "quick",
        |m| {
            m.quick = if m.quick == Quick::Screenshot { Quick::SavePng } else { Quick::Screenshot };
        },
        C::NONE,
    ),
];
