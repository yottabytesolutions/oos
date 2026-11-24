// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "tscale",
        |m| {
            m.tscale += 1;
        },
        C::ACQ.union(C::STATS).union(C::MAPPING),
    ),
    (
        "delay",
        |m| {
            m.delay += 1.0;
        },
        C::ACQ.union(C::STATS).union(C::MAPPING),
    ),
    (
        "fine_t",
        |m| {
            m.fine_t = !m.fine_t;
        },
        C::NONE,
    ),
    (
        "tfine",
        |m| {
            m.tfine += 1.0;
        },
        C::ACQ.union(C::STATS).union(C::MAPPING),
    ),
    (
        "h_expand",
        |m| {
            m.h_expand = if m.h_expand == HExpand::Center { HExpand::Left } else { HExpand::Center };
        },
        C::NONE,
    ),
    (
        "zoom",
        |m| {
            m.zoom = !m.zoom;
        },
        C::NONE.union(C::MAPPING),
    ),
    (
        "zscale",
        |m| {
            m.zscale += 1;
        },
        C::NONE.union(C::MAPPING),
    ),
    (
        "zdelay",
        |m| {
            m.zdelay += 1.0;
        },
        C::NONE.union(C::MAPPING),
    ),
    (
        "auto_roll",
        |m| {
            m.auto_acquisition.set_roll(!m.auto_acquisition.roll());
        },
        C::ACQ.union(C::STATS),
    ),
];
