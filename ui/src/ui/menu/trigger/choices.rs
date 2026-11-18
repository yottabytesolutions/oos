// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const SLOPES: &[(Slope, &str)] =
    &[(Slope::Rise, "Rising"), (Slope::Fall, "Falling"), (Slope::Either, "Either")];
pub(super) const POLARITIES: &[(Pol, &str)] = &[(Pol::Pos, "Positive"), (Pol::Neg, "Negative")];
pub(super) const CONDITIONS: &[(WhenKind, &str)] = &[
    (WhenKind::Off, "Off"),
    (WhenKind::Gt, ">"),
    (WhenKind::Lt, "<"),
    (WhenKind::In, "Inside"),
    (WhenKind::Out, "Outside"),
];
pub(super) const VIDEO_STANDARDS: &[(VideoStd, &str)] = &[(VideoStd::Pal, "PAL"), (VideoStd::Ntsc, "NTSC")];
pub(super) const VIDEO_SYNC: &[(VSync, &str)] = &[
    (VSync::AllLines, "Lines"),
    (VSync::Line, "Line n"),
    (VSync::Odd, "Odd"),
    (VSync::Even, "Even"),
    (VSync::AllFields, "Fields"),
];
pub(super) const WINDOW_MODES: &[(WinMode, &str)] =
    &[(WinMode::Enter, "Enter"), (WinMode::Exit, "Exit"), (WinMode::Inside, "Inside")];
pub(super) const PATTERN_BITS: &[(Bit, &str)] =
    &[(Bit::H, "H"), (Bit::L, "L"), (Bit::X, "X"), (Bit::R, "Rise"), (Bit::F, "Fall")];

pub(super) fn conditions(ui: &Ui) -> &'static [(WhenKind, &'static str)] {
    &CONDITIONS[usize::from(ui.m.trig_type != TT::Runt)..]
}

pub(super) fn pattern_bits(ui: &Ui) -> &'static [(Bit, &'static str)] {
    &PATTERN_BITS[..if ui.m.trig_type == TT::Duration { 3 } else { 5 }]
}
