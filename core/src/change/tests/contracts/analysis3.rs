// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "bode_gen",
        |m| {
            m.bode_gen += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_addr",
        |m| {
            m.bode_addr.push_str("fixture");
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_usb",
        |m| {
            m.bode_usb = !m.bode_usb;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sw_time",
        |m| {
            m.bode_sw_time += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sw_log",
        |m| {
            m.bode_sw_log = !m.bode_sw_log;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sync",
        |m| {
            m.bode_sync += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sync_slope",
        |m| {
            m.bode_sync_slope = if m.bode_sync_slope == Slope::Rise { Slope::Fall } else { Slope::Rise };
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sync_level",
        |m| {
            m.bode_sync_level += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_sw_in",
        |m| {
            m.bode_sw_in += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_ref",
        |m| {
            m.bode_ref += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bw_src",
        |m| {
            m.bw_src += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bw_hint",
        |m| {
            m.bw_hint += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bw_bins",
        |m| {
            m.bw_bins += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "mask_on",
        |m| {
            m.mask_on = !m.mask_on;
        },
        C::ACQ.union(C::STATS).union(C::MASK),
    ),
    (
        "zones",
        |m| {
            m.zones[0].pts.push((0.0, 1.0));
        },
        C::ACQ.union(C::STATS).union(C::ZONE),
    ),
    (
        "mask_src",
        |m| {
            m.mask_src += 1;
        },
        C::ACQ.union(C::STATS).union(C::MASK),
    ),
    (
        "mask_x",
        |m| {
            m.mask_x += 1.0;
        },
        C::MASK,
    ),
    (
        "mask_y",
        |m| {
            m.mask_y += 1.0;
        },
        C::MASK,
    ),
    (
        "mask_stop",
        |m| {
            m.mask_stop = !m.mask_stop;
        },
        C::NONE,
    ),
];
