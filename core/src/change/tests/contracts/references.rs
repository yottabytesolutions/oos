// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "deskew_ch",
        |m| {
            m.deskew_ch[0] = 1e-9;
        },
        C::STATS.union(C::PHOSPHOR),
    ),
    (
        "deskew_ref",
        |m| {
            m.deskew_ref[0] = 1e-9;
        },
        C::STATS,
    ),
    (
        "cur_b_source",
        |m| {
            m.cur_b_source = Some(WaveId::Ref(1));
        },
        C::NONE,
    ),
    (
        "trig_ref",
        |m| {
            m.trig_ref = Some(1);
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_ref2",
        |m| {
            m.trig_ref2 = Some(1);
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "density",
        |m| {
            m.density = Density::Full;
        },
        C::VIEW,
    ),
    (
        "ref_win",
        |m| {
            m.ref_win[0] = 1;
        },
        C::VIEW,
    ),
    (
        "cat",
        |m| {
            m.cat.push((15, WaveId::Ch(0), WaveId::Ch(1), WaveId::Ch(2), WaveId::Ch(3)));
        },
        C::STATS,
    ),
    (
        "split_pane",
        |m| {
            m.split_pane += 1;
        },
        C::VIEW,
    ),
    (
        "strip_dock",
        |m| {
            m.strip_dock = StripDock::Left;
        },
        C::VIEW,
    ),
    (
        "dec_cphy_rate",
        |m| {
            m.dec_cphy_rate += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "bus",
        |m| {
            m.bus.on = !m.bus.on;
        },
        C::NONE,
    ),
    (
        "mask_inv",
        |m| {
            m.mask_inv = !m.mask_inv;
        },
        C::STATS.union(C::MASK),
    ),
];
