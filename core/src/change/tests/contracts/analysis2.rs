// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "spec_span",
        |m| {
            m.spec_span += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_rbw",
        |m| {
            m.spec_rbw += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_win",
        |m| {
            m.spec_win = if m.spec_win == crate::math::Window::Rect {
                crate::math::Window::Hann
            } else {
                crate::math::Window::Rect
            };
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_sort",
        |m| {
            m.spec_sort += 1;
        },
        C::NONE,
    ),
    (
        "spec_harm",
        |m| {
            m.spec_harm += 1;
        },
        C::NONE,
    ),
    (
        "spec_trace",
        |m| {
            m.spec_trace += 1;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_ref",
        |m| {
            m.spec_ref += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_wf",
        |m| {
            m.spec_wf = !m.spec_wf;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_dist",
        |m| {
            m.spec_dist = !m.spec_dist;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "eye_src",
        |m| {
            m.eye_src = WaveId::Ch(1);
        },
        C::VIEW,
    ),
    (
        "eye_rate",
        |m| {
            m.eye_rate += 1.0;
        },
        C::VIEW,
    ),
    (
        "pw_v",
        |m| {
            m.pw_v = WaveId::Ch(1);
        },
        C::VIEW,
    ),
    (
        "pw_i",
        |m| {
            m.pw_i = WaveId::Ch(2);
        },
        C::VIEW,
    ),
    (
        "pw_mode",
        |m| {
            m.pw_mode = if m.pw_mode == PowerMode::Quality { PowerMode::Harmonics } else { PowerMode::Quality };
        },
        C::VIEW,
    ),
    (
        "bode_start",
        |m| {
            m.bode_start += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_stop",
        |m| {
            m.bode_stop += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_points",
        |m| {
            m.bode_points += 1;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_amp",
        |m| {
            m.bode_amp += 1.0;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_in",
        |m| {
            m.bode_in = WaveId::Ch(1);
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "bode_out",
        |m| {
            m.bode_out = WaveId::Ch(2);
        },
        C::STATS.union(C::VIEW),
    ),
];
