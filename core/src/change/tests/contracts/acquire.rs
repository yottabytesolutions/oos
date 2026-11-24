// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "run",
        |m| {
            m.run = if m.run == RunState::Run { RunState::Stop } else { RunState::Run };
        },
        C::NONE,
    ),
    (
        "status",
        |m| {
            m.status = if m.status == TrigStatus::Triggered { TrigStatus::Auto } else { TrigStatus::Triggered };
        },
        C::NONE,
    ),
    (
        "acq",
        |m| {
            m.acq = if m.acq == AcqMode::Normal { AcqMode::Peak } else { AcqMode::Normal };
        },
        C::ACQ.union(C::STATS).union(C::PHOSPHOR).union(C::HW_AVERAGE),
    ),
    (
        "cores",
        |m| {
            m.cores = !m.cores;
        },
        C::ACQ.union(C::STATS).union(C::PHOSPHOR),
    ),
    (
        "avg_count",
        |m| {
            m.avg_count += 1;
        },
        C::ACQ.union(C::STATS).union(C::PHOSPHOR).union(C::HW_AVERAGE),
    ),
    (
        "avg_hw",
        |m| {
            m.avg_hw = !m.avg_hw;
        },
        C::ACQ.union(C::STATS).union(C::PHOSPHOR).union(C::HW_AVERAGE),
    ),
    (
        "segments",
        |m| {
            m.segments += 1;
        },
        C::ACQ.union(C::STATS).union(C::HISTORY),
    ),
    (
        "seg_hw",
        |m| {
            m.seg_hw = !m.seg_hw;
        },
        C::ACQ.union(C::STATS).union(C::HISTORY),
    ),
    (
        "seg_ivl",
        |m| {
            m.seg_ivl += 1.0;
        },
        C::ACQ.union(C::STATS).union(C::HISTORY),
    ),
    (
        "mdepth",
        |m| {
            m.mdepth += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    ("auto_depth", |m| m.auto_acquisition.set_depth(AutoDepth::Screen), C::ACQ.union(C::STATS)),
    (
        "hw_stamp",
        |m| {
            m.hw_stamp += 1;
        },
        C::NONE,
    ),
    (
        "hw_avg_clear",
        |m| {
            m.hw_avg_clear += 1;
        },
        C::NONE,
    ),
    (
        "la",
        |m| {
            m.la.on = !m.la.on;
        },
        C::ACQ.union(C::STATS),
    ),
];
