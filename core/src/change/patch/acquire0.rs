// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{AcqMode, Model, RunState, TrigStatus};

#[derive(Clone, Debug, PartialEq)]
pub enum Acquire0 {
    Run(RunState),
    Status(TrigStatus),
    Acq(AcqMode),
    AvgCount(u32),
    AvgHw(bool),
    Segments(usize),
    SegHw(bool),
    SegIvl(f32),
    Mdepth(usize),
    HwStamp(u64),
    HwAvgClear(u64),
    La(crate::la::LaSet),
}

impl Acquire0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            32 => Self::Run(m.run),
            33 => Self::Status(m.status),
            34 => Self::Acq(m.acq),
            35 => Self::AvgCount(m.avg_count),
            36 => Self::AvgHw(m.avg_hw),
            37 => Self::Segments(m.segments),
            38 => Self::SegHw(m.seg_hw),
            39 => Self::SegIvl(m.seg_ivl),
            40 => Self::Mdepth(m.mdepth),
            162 => Self::HwStamp(m.hw_stamp),
            163 => Self::HwAvgClear(m.hw_avg_clear),
            193 => Self::La(m.la.clone()),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Run(_) => 32,
            Self::Status(_) => 33,
            Self::Acq(_) => 34,
            Self::AvgCount(_) => 35,
            Self::AvgHw(_) => 36,
            Self::Segments(_) => 37,
            Self::SegHw(_) => 38,
            Self::SegIvl(_) => 39,
            Self::Mdepth(_) => 40,
            Self::HwStamp(_) => 162,
            Self::HwAvgClear(_) => 163,
            Self::La(_) => 193,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Run(value) => m.run.clone_from(value),
            Self::Status(value) => m.status.clone_from(value),
            Self::Acq(value) => m.acq.clone_from(value),
            Self::AvgCount(value) => m.avg_count.clone_from(value),
            Self::AvgHw(value) => m.avg_hw.clone_from(value),
            Self::Segments(value) => m.segments.clone_from(value),
            Self::SegHw(value) => m.seg_hw.clone_from(value),
            Self::SegIvl(value) => m.seg_ivl.clone_from(value),
            Self::Mdepth(value) => m.mdepth.clone_from(value),
            Self::HwStamp(value) => m.hw_stamp.clone_from(value),
            Self::HwAvgClear(value) => m.hw_avg_clear.clone_from(value),
            Self::La(value) => m.la.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Run(value) => value.clone_from(&m.run),
            Self::Status(value) => value.clone_from(&m.status),
            Self::Acq(value) => value.clone_from(&m.acq),
            Self::AvgCount(value) => value.clone_from(&m.avg_count),
            Self::AvgHw(value) => value.clone_from(&m.avg_hw),
            Self::Segments(value) => value.clone_from(&m.segments),
            Self::SegHw(value) => value.clone_from(&m.seg_hw),
            Self::SegIvl(value) => value.clone_from(&m.seg_ivl),
            Self::Mdepth(value) => value.clone_from(&m.mdepth),
            Self::HwStamp(value) => value.clone_from(&m.hw_stamp),
            Self::HwAvgClear(value) => value.clone_from(&m.hw_avg_clear),
            Self::La(value) => value.clone_from(&m.la),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
