// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "ch",
        |m| {
            m.ch[0].vscale += 1;
        },
        C::ACQ.union(C::STATS).union(C::MAPPING),
    ),
    (
        "active",
        |m| {
            m.active += 1;
        },
        C::NONE,
    ),
    (
        "ch_label",
        |m| {
            m.ch_label[0].push_str("fixture");
        },
        C::NONE,
    ),
    (
        "fine_v",
        |m| {
            m.fine_v = !m.fine_v;
        },
        C::NONE,
    ),
    (
        "v_expand_gnd",
        |m| {
            m.v_expand_gnd = !m.v_expand_gnd;
        },
        C::NONE,
    ),
];
