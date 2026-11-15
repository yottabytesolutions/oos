// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Bit, Model, Pol, Slope, VSync, VideoStd, WhenKind, WinMode, NCH};

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger1 {
    TrigSrc2(usize),
    TrigSlope2(Slope),
    TrigPol(Pol),
    TrigWhen(WhenKind),
    TrigT1(f32),
    TrigT2(f32),
    VideoStd(VideoStd),
    VideoSync(VSync),
    VideoLine(u32),
    Pat([Bit; NCH]),
    PatLevels([f32; NCH]),
    WinMode(WinMode),
}

impl Trigger1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            16 => Self::TrigSrc2(m.trig_src2),
            17 => Self::TrigSlope2(m.trig_slope2),
            18 => Self::TrigPol(m.trig_pol),
            19 => Self::TrigWhen(m.trig_when),
            20 => Self::TrigT1(m.trig_t1),
            21 => Self::TrigT2(m.trig_t2),
            22 => Self::VideoStd(m.video_std),
            23 => Self::VideoSync(m.video_sync),
            24 => Self::VideoLine(m.video_line),
            25 => Self::Pat(m.pat),
            26 => Self::PatLevels(m.pat_levels),
            27 => Self::WinMode(m.win_mode),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::TrigSrc2(_) => 16,
            Self::TrigSlope2(_) => 17,
            Self::TrigPol(_) => 18,
            Self::TrigWhen(_) => 19,
            Self::TrigT1(_) => 20,
            Self::TrigT2(_) => 21,
            Self::VideoStd(_) => 22,
            Self::VideoSync(_) => 23,
            Self::VideoLine(_) => 24,
            Self::Pat(_) => 25,
            Self::PatLevels(_) => 26,
            Self::WinMode(_) => 27,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::TrigSrc2(value) => m.trig_src2.clone_from(value),
            Self::TrigSlope2(value) => m.trig_slope2.clone_from(value),
            Self::TrigPol(value) => m.trig_pol.clone_from(value),
            Self::TrigWhen(value) => m.trig_when.clone_from(value),
            Self::TrigT1(value) => m.trig_t1.clone_from(value),
            Self::TrigT2(value) => m.trig_t2.clone_from(value),
            Self::VideoStd(value) => m.video_std.clone_from(value),
            Self::VideoSync(value) => m.video_sync.clone_from(value),
            Self::VideoLine(value) => m.video_line.clone_from(value),
            Self::Pat(value) => m.pat.clone_from(value),
            Self::PatLevels(value) => m.pat_levels.clone_from(value),
            Self::WinMode(value) => m.win_mode.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::TrigSrc2(value) => value.clone_from(&m.trig_src2),
            Self::TrigSlope2(value) => value.clone_from(&m.trig_slope2),
            Self::TrigPol(value) => value.clone_from(&m.trig_pol),
            Self::TrigWhen(value) => value.clone_from(&m.trig_when),
            Self::TrigT1(value) => value.clone_from(&m.trig_t1),
            Self::TrigT2(value) => value.clone_from(&m.trig_t2),
            Self::VideoStd(value) => value.clone_from(&m.video_std),
            Self::VideoSync(value) => value.clone_from(&m.video_sync),
            Self::VideoLine(value) => value.clone_from(&m.video_line),
            Self::Pat(value) => value.clone_from(&m.pat),
            Self::PatLevels(value) => value.clone_from(&m.pat_levels),
            Self::WinMode(value) => value.clone_from(&m.win_mode),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
