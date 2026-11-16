// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Density, Model, StripDock, TraceBus, WaveId, NCH, NREF};

#[derive(Clone, Debug, PartialEq)]
pub enum ReferencesP0 {
    DeskewCh([f64; NCH]),
    DeskewRef([f64; NREF]),
    CurBSource(Option<WaveId>),
    TrigRef(Option<usize>),
    TrigRef2(Option<usize>),
    Density(Density),
    RefWin([usize; NREF]),
    Cat(Vec<(u16, WaveId, WaveId, WaveId, WaveId)>),
    SplitPane(usize),
    StripDock(StripDock),
    DecCphyRate(f64),
    Bus(TraceBus),
}

impl ReferencesP0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            244 => Self::DeskewCh(m.deskew_ch),
            245 => Self::DeskewRef(m.deskew_ref),
            246 => Self::CurBSource(m.cur_b_source),
            247 => Self::TrigRef(m.trig_ref),
            248 => Self::TrigRef2(m.trig_ref2),
            249 => Self::Density(m.density),
            250 => Self::RefWin(m.ref_win),
            251 => Self::Cat(m.cat.clone()),
            252 => Self::SplitPane(m.split_pane),
            253 => Self::StripDock(m.strip_dock),
            254 => Self::DecCphyRate(m.dec_cphy_rate),
            255 => Self::Bus(m.bus.clone()),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::DeskewCh(_) => 244,
            Self::DeskewRef(_) => 245,
            Self::CurBSource(_) => 246,
            Self::TrigRef(_) => 247,
            Self::TrigRef2(_) => 248,
            Self::Density(_) => 249,
            Self::RefWin(_) => 250,
            Self::Cat(_) => 251,
            Self::SplitPane(_) => 252,
            Self::StripDock(_) => 253,
            Self::DecCphyRate(_) => 254,
            Self::Bus(_) => 255,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::DeskewCh(value) => m.deskew_ch.clone_from(value),
            Self::DeskewRef(value) => m.deskew_ref.clone_from(value),
            Self::CurBSource(value) => m.cur_b_source.clone_from(value),
            Self::TrigRef(value) => m.trig_ref.clone_from(value),
            Self::TrigRef2(value) => m.trig_ref2.clone_from(value),
            Self::Density(value) => m.density.clone_from(value),
            Self::RefWin(value) => m.ref_win.clone_from(value),
            Self::Cat(value) => m.cat.clone_from(value),
            Self::SplitPane(value) => m.split_pane.clone_from(value),
            Self::StripDock(value) => m.strip_dock.clone_from(value),
            Self::DecCphyRate(value) => m.dec_cphy_rate.clone_from(value),
            Self::Bus(value) => m.bus.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::DeskewCh(value) => value.clone_from(&m.deskew_ch),
            Self::DeskewRef(value) => value.clone_from(&m.deskew_ref),
            Self::CurBSource(value) => value.clone_from(&m.cur_b_source),
            Self::TrigRef(value) => value.clone_from(&m.trig_ref),
            Self::TrigRef2(value) => value.clone_from(&m.trig_ref2),
            Self::Density(value) => value.clone_from(&m.density),
            Self::RefWin(value) => value.clone_from(&m.ref_win),
            Self::Cat(value) => value.clone_from(&m.cat),
            Self::SplitPane(value) => value.clone_from(&m.split_pane),
            Self::StripDock(value) => value.clone_from(&m.strip_dock),
            Self::DecCphyRate(value) => value.clone_from(&m.dec_cphy_rate),
            Self::Bus(value) => value.clone_from(&m.bus),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Cat(value) => value.capacity() * std::mem::size_of::<(u16, WaveId, WaveId, WaveId, WaveId)>(),
            Self::Bus(value) => value.label.capacity(),
            _ => 0,
        }
    }
}
