// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "meas",
        |m| {
            m.meas.push((Meas::Vavg, 0));
        },
        C::STATS,
    ),
    (
        "meas2",
        |m| {
            m.meas2.push((0, 0, 1));
        },
        C::STATS,
    ),
    (
        "dvm_on",
        |m| {
            m.dvm_on = !m.dvm_on;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dvm_src",
        |m| {
            m.dvm_src += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dvm_mode",
        |m| {
            m.dvm_mode += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "cnt_on",
        |m| {
            m.cnt_on = !m.cnt_on;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "cnt_src",
        |m| {
            m.cnt_src += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "cnt_mode",
        |m| {
            m.cnt_mode += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "cnt_digits",
        |m| {
            m.cnt_digits += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "cursor",
        |m| {
            m.cursor = if m.cursor == CursorMode::Off { CursorMode::Time } else { CursorMode::Off };
        },
        C::NONE,
    ),
    (
        "cur_a",
        |m| {
            m.cur_a += 1.0;
        },
        C::NONE,
    ),
    (
        "cur_b",
        |m| {
            m.cur_b += 1.0;
        },
        C::NONE,
    ),
    (
        "cur_c",
        |m| {
            m.cur_c += 1.0;
        },
        C::NONE,
    ),
    (
        "cur_d",
        |m| {
            m.cur_d += 1.0;
        },
        C::NONE,
    ),
    (
        "meas_labels",
        |m| {
            m.meas_labels = !m.meas_labels;
        },
        C::NONE,
    ),
    (
        "trend_sel",
        |m| {
            m.trend_sel[0] += 1;
        },
        C::VIEW,
    ),
    (
        "log_ivl",
        |m| {
            m.log_ivl += 1;
        },
        C::NONE,
    ),
];
