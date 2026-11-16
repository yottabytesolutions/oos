// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "meas_idle",
        |m| {
            m.meas_idle += 1.0;
        },
        C::STATS,
    ),
    (
        "meas_out",
        |m| {
            m.meas_out = oos_dsp::measure::Outside::Above;
        },
        C::STATS,
    ),
    (
        "meas_n",
        |m| {
            m.meas_n += 1;
        },
        C::STATS,
    ),
    (
        "meas_data",
        |m| {
            m.meas_data = !m.meas_data;
        },
        C::STATS,
    ),
    (
        "meas_lpct",
        |m| {
            m.meas_lpct += 1.0;
        },
        C::STATS,
    ),
    (
        "meas_hpct",
        |m| {
            m.meas_hpct += 1.0;
        },
        C::STATS,
    ),
    (
        "meas_full_record",
        |m| {
            m.meas_full_record = !m.meas_full_record;
        },
        C::STATS,
    ),
    (
        "meas_tsetup",
        |m| {
            m.meas_tsetup += 1.0;
        },
        C::STATS,
    ),
    (
        "meas_thold",
        |m| {
            m.meas_thold += 1.0;
        },
        C::STATS,
    ),
];
