// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "sim_preset",
        |m| {
            m.sim_preset += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_slot",
        |m| {
            m.sim_slot += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_rate",
        |m| {
            m.sim_rate += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_prbs",
        |m| {
            m.sim_prbs += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_vhigh",
        |m| {
            m.sim_vhigh += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_vlow",
        |m| {
            m.sim_vlow += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_rise",
        |m| {
            m.sim_rise += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_noise",
        |m| {
            m.sim_noise += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_bw",
        |m| {
            m.sim_bw += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_deemph",
        |m| {
            m.sim_deemph += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_ssc",
        |m| {
            m.sim_ssc = !m.sim_ssc;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_ssc_center",
        |m| {
            m.sim_ssc_center = !m.sim_ssc_center;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_ssc_deviation",
        |m| {
            m.sim_ssc_deviation += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_ssc_hz",
        |m| {
            m.sim_ssc_hz += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_rj",
        |m| {
            m.sim_rj += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_pj",
        |m| {
            m.sim_pj += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_pjhz",
        |m| {
            m.sim_pjhz += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_dcd_pp",
        |m| {
            m.sim_dcd_pp += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_corr_rj",
        |m| {
            m.sim_corr_rj += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "sim_corr_hz",
        |m| {
            m.sim_corr_hz += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
];
