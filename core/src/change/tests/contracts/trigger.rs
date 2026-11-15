// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "trig_src",
        |m| {
            m.trig_src += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_la",
        |m| {
            m.trig_la = if m.trig_la.is_some() { None } else { Some(1) };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "pat_la",
        |m| {
            m.pat_la[0] = Bit::H;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_level",
        |m| {
            m.trig_level += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "slope",
        |m| {
            m.slope = if m.slope == Slope::Rise { Slope::Fall } else { Slope::Rise };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_mode",
        |m| {
            m.trig_mode = if m.trig_mode == TrigMode::Auto { TrigMode::Normal } else { TrigMode::Auto };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "holdoff",
        |m| {
            m.holdoff += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_type",
        |m| {
            m.trig_type = if m.trig_type == TrigType::Edge { TrigType::Pulse } else { TrigType::Edge };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_level2",
        |m| {
            m.trig_level2 += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_nrej",
        |m| {
            m.trig_nrej = !m.trig_nrej;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "auto_nrej",
        |m| {
            m.auto_nrej = !m.auto_nrej;
        },
        C::NONE,
    ),
    (
        "trig_coup",
        |m| {
            m.trig_coup = if m.trig_coup == TrigCoup::Dc { TrigCoup::Ac } else { TrigCoup::Dc };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_src2",
        |m| {
            m.trig_src2 += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_slope2",
        |m| {
            m.trig_slope2 = if m.trig_slope2 == Slope::Rise { Slope::Fall } else { Slope::Rise };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_pol",
        |m| {
            m.trig_pol = if m.trig_pol == Pol::Pos { Pol::Neg } else { Pol::Pos };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_when",
        |m| {
            m.trig_when = if m.trig_when == WhenKind::Off { WhenKind::Gt } else { WhenKind::Off };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_t1",
        |m| {
            m.trig_t1 += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_t2",
        |m| {
            m.trig_t2 += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "video_std",
        |m| {
            m.video_std = if m.video_std == VideoStd::Pal { VideoStd::Ntsc } else { VideoStd::Pal };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "video_sync",
        |m| {
            m.video_sync = if m.video_sync == VSync::AllLines { VSync::Line } else { VSync::AllLines };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "video_line",
        |m| {
            m.video_line += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "pat",
        |m| {
            m.pat[0] = Bit::L;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "pat_levels",
        |m| {
            m.pat_levels[0] += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "win_mode",
        |m| {
            m.win_mode = if m.win_mode == WinMode::Enter { WinMode::Exit } else { WinMode::Enter };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "nth_idle",
        |m| {
            m.nth_idle += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "nth_n",
        |m| {
            m.nth_n += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "setup",
        |m| {
            m.setup += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "hold",
        |m| {
            m.hold += 1.0;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_bus",
        |m| {
            m.trig_bus = if m.trig_bus == BusWhat::Frame { BusWhat::Id } else { BusWhat::Frame };
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "trig_bus_val",
        |m| {
            m.trig_bus_val += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "ser",
        |m| {
            m.ser.bytes += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "auto_fast",
        |m| {
            m.auto_fast = !m.auto_fast;
        },
        C::ACQ.union(C::STATS),
    ),
];
