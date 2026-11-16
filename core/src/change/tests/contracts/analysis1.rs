// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "dec_fdrate",
        |m| {
            m.dec_fdrate += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_arate",
        |m| {
            m.dec_arate += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_tick",
        |m| {
            m.dec_tick += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_nibbles",
        |m| {
            m.dec_nibbles += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_uspeed",
        |m| {
            m.dec_uspeed += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_src4",
        |m| {
            m.dec_src4 = if m.dec_src4.is_some() { None } else { Some(1) };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_la_in",
        |m| {
            m.dec_la_in = !m.dec_la_in;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_la",
        |m| {
            m.dec_la[0] = Some(1);
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_diff",
        |m| {
            m.dec_diff = if m.dec_diff.is_some() { None } else { Some(1) };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_thr",
        |m| {
            m.dec_thr += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_ascii",
        |m| {
            m.dec_ascii = !m.dec_ascii;
        },
        C::NONE,
    ),
    (
        "dec_proto",
        |m| {
            m.dec_proto = if m.dec_proto == Proto::Uart { Proto::I2c } else { Proto::Uart };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_src2",
        |m| {
            m.dec_src2 += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_cs",
        |m| {
            m.dec_cs = if m.dec_cs.is_some() { None } else { Some(1) };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_mode",
        |m| {
            m.dec_mode += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_bits",
        |m| {
            m.dec_bits += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_parity",
        |m| {
            m.dec_parity = if m.dec_parity == oos_dsp::decode::Parity::Even {
                oos_dsp::decode::Parity::None
            } else {
                oos_dsp::decode::Parity::Even
            };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_opt",
        |m| {
            m.dec_opt[0] += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "spec_src",
        |m| {
            m.spec_src = WaveId::Ch(1);
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_center",
        |m| {
            m.spec_center += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
];
