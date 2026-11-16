// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "eye_from_record",
        |m| {
            m.eye_from_record = !m.eye_from_record;
        },
        C::VIEW,
    ),
    (
        "eye_place",
        |m| {
            m.eye_place = EyePlace::Side;
        },
        C::VIEW,
    ),
    (
        "eye_ber",
        |m| {
            m.eye_ber += 1.0;
        },
        C::VIEW,
    ),
    (
        "eye_pn_lo",
        |m| {
            m.eye_pn_lo += 1.0;
        },
        C::VIEW,
    ),
    (
        "eye_pn_hi",
        |m| {
            m.eye_pn_hi += 1.0;
        },
        C::VIEW,
    ),
    (
        "eye_npj",
        |m| {
            m.eye_npj = !m.eye_npj;
        },
        C::VIEW,
    ),
    (
        "eye_b",
        |m| {
            m.eye_b = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "eye_rail",
        |m| {
            m.eye_rail = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "eye_notch",
        |m| {
            m.eye_notch += 1.0;
        },
        C::VIEW,
    ),
    (
        "eye_hp",
        |m| {
            m.eye_hp = !m.eye_hp;
        },
        C::VIEW,
    ),
    (
        "pw_fline",
        |m| {
            m.pw_fline += 1.0;
        },
        C::VIEW,
    ),
    (
        "pw_fsw",
        |m| {
            m.pw_fsw += 1.0;
        },
        C::VIEW,
    ),
    (
        "pw_vin",
        |m| {
            m.pw_vin = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "pw_clk",
        |m| {
            m.pw_clk = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "pw_ref",
        |m| {
            m.pw_ref = Some(1.5);
        },
        C::VIEW,
    ),
    (
        "bode_mode",
        |m| {
            m.bode_mode = oos_dsp::bode::Mode::Psrr;
        },
        C::STATS.union(C::VIEW),
    ),
    (
        "spec_acpr_w",
        |m| {
            m.spec_acpr_w += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "spec_acpr_s",
        |m| {
            m.spec_acpr_s += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
];
