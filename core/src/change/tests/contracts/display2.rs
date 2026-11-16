// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "draw_batch",
        |m| {
            m.draw_batch = !m.draw_batch;
        },
        C::PHOSPHOR,
    ),
    (
        "bar_compact",
        |m| {
            m.bar_compact = !m.bar_compact;
        },
        C::NONE,
    ),
    (
        "hdmi_view",
        |m| {
            m.hdmi_view += 1;
        },
        C::NONE,
    ),
];
