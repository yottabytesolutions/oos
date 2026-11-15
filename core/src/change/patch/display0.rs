// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Grid, Model, WinLayout, NCH};

#[derive(Clone, Debug, PartialEq)]
pub enum Display0 {
    Persist(usize),
    Intensity(u32),
    Grid(Grid),
    GridBright(u32),
    GridFrame(bool),
    WinLayout(WinLayout),
    ChWin([usize; NCH]),
    MathWin([usize; crate::math::NMATH]),
    LaWin(usize),
    Pane(usize),
    TouchLock(bool),
    ColorGrade(bool),
}

impl Display0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            41 => Self::Persist(m.persist),
            42 => Self::Intensity(m.intensity),
            43 => Self::Grid(m.grid),
            44 => Self::GridBright(m.grid_bright),
            45 => Self::GridFrame(m.grid_frame),
            46 => Self::WinLayout(m.win_layout),
            47 => Self::ChWin(m.ch_win),
            48 => Self::MathWin(m.math_win),
            49 => Self::LaWin(m.la_win),
            88 => Self::Pane(m.pane),
            65 => Self::TouchLock(m.touch_lock),
            121 => Self::ColorGrade(m.color_grade),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Persist(_) => 41,
            Self::Intensity(_) => 42,
            Self::Grid(_) => 43,
            Self::GridBright(_) => 44,
            Self::GridFrame(_) => 45,
            Self::WinLayout(_) => 46,
            Self::ChWin(_) => 47,
            Self::MathWin(_) => 48,
            Self::LaWin(_) => 49,
            Self::TouchLock(_) => 65,
            Self::Pane(_) => 88,
            Self::ColorGrade(_) => 121,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Persist(value) => m.persist.clone_from(value),
            Self::Intensity(value) => m.intensity.clone_from(value),
            Self::Grid(value) => m.grid.clone_from(value),
            Self::GridBright(value) => m.grid_bright.clone_from(value),
            Self::GridFrame(value) => m.grid_frame.clone_from(value),
            Self::WinLayout(value) => m.win_layout.clone_from(value),
            Self::ChWin(value) => m.ch_win.clone_from(value),
            Self::MathWin(value) => m.math_win.clone_from(value),
            Self::LaWin(value) => m.la_win.clone_from(value),
            Self::Pane(value) => m.pane.clone_from(value),
            Self::TouchLock(value) => m.touch_lock.clone_from(value),
            Self::ColorGrade(value) => m.color_grade.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Persist(value) => value.clone_from(&m.persist),
            Self::Intensity(value) => value.clone_from(&m.intensity),
            Self::Grid(value) => value.clone_from(&m.grid),
            Self::GridBright(value) => value.clone_from(&m.grid_bright),
            Self::GridFrame(value) => value.clone_from(&m.grid_frame),
            Self::WinLayout(value) => value.clone_from(&m.win_layout),
            Self::ChWin(value) => value.clone_from(&m.ch_win),
            Self::MathWin(value) => value.clone_from(&m.math_win),
            Self::LaWin(value) => value.clone_from(&m.la_win),
            Self::Pane(value) => value.clone_from(&m.pane),
            Self::TouchLock(value) => value.clone_from(&m.touch_lock),
            Self::ColorGrade(value) => value.clone_from(&m.color_grade),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
