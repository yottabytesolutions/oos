// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes, FieldDiff, Layout};
use crate::config::Config;
use crate::model::{AmpUnit, Channel, Model, NCH};

/// These projections are also the source of mapping invalidation, avoiding a second mapping field list.
#[derive(Clone, Debug, PartialEq)]
pub struct MappingKey {
    channels: [ChannelMapping; NCH],
    timebase: (usize, f32, f32, bool, usize, f32),
    display: (
        crate::model::DotsMode,
        crate::model::WinLayout,
        [usize; NCH],
        [usize; crate::math::NMATH],
        usize,
        Option<usize>,
    ),
    layout: Layout,
}

#[derive(Clone, Debug, PartialEq)]
struct ChannelMapping {
    on: bool,
    vscale: usize,
    vfine: f32,
    bias: f32,
    skew: f32,
    offset: f32,
    coupling: crate::model::Coupling,
    bw: usize,
    probe: f32,
    invert: bool,
}
fn channel(ch: &Channel) -> ChannelMapping {
    let Channel { on, vscale, vfine, unit: _, bias, skew, offset, coupling, bw, probe, invert } = *ch;
    ChannelMapping { on, vscale, vfine, bias, skew, offset, coupling, bw, probe, invert }
}
impl MappingKey {
    pub(super) fn with_playback_position(mut self, a: &Model, b: &Model, requests: &[super::Request]) -> Self {
        if super::presentation::matches(a, b, requests, false) {
            self.timebase.2 = b.delay;
        }
        if super::presentation::matches(a, b, requests, true) {
            self.timebase.5 = b.zdelay;
        }
        self
    }

    pub fn of(m: &Model, layout: Layout) -> Self {
        Self {
            channels: std::array::from_fn(|i| channel(&m.ch[i])),
            timebase: (m.tscale, m.tfine, m.delay, m.zoom, m.zscale, m.zdelay),
            display: (m.dots, m.win_layout, m.ch_win, m.math_win, m.la_win, m.z_src),
            layout,
        }
    }
}

/// Transitional superset: every dependency of the acting drawpipe::Key remains represented through M4.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerViewKey {
    mapping: MappingKey,
    units: [AmpUnit; NCH],
    trigger: (usize, Option<u8>, crate::model::TrigType, crate::model::Slope, f32, crate::model::TrigCoup, bool, bool),
    acq: crate::model::AcqMode,
}
impl WorkerViewKey {
    pub fn of(m: &Model, layout: Layout, config: &Config) -> Self {
        Self {
            mapping: MappingKey::of(m, layout),
            units: std::array::from_fn(|i| m.ch[i].unit),
            trigger: (
                m.trig_src,
                m.trig_line_in(config),
                m.trig_type,
                m.slope,
                m.trig_level,
                m.trig_coup,
                m.trig_nrej,
                m.trig_on_la_in(config),
            ),
            acq: m.acq,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DepositView {
    mapping: MappingKey,
}
impl DepositView {
    pub fn new(mapping: MappingKey) -> Self {
        Self { mapping }
    }
    pub fn mapping(&self) -> &MappingKey {
        &self.mapping
    }
}

pub(super) fn channel_fields(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(0, "ch", a.ch != b.ch, Classes::NONE));
    for i in 0..NCH {
        channel_diff(i, &a.ch[i], &b.ch[i], emit);
    }
}

const CHANNEL_NAMES: [[&str; 11]; NCH] = [
    [
        "ch1.on",
        "ch1.vscale",
        "ch1.vfine",
        "ch1.unit",
        "ch1.bias",
        "ch1.skew",
        "ch1.offset",
        "ch1.coupling",
        "ch1.bw",
        "ch1.probe",
        "ch1.invert",
    ],
    [
        "ch2.on",
        "ch2.vscale",
        "ch2.vfine",
        "ch2.unit",
        "ch2.bias",
        "ch2.skew",
        "ch2.offset",
        "ch2.coupling",
        "ch2.bw",
        "ch2.probe",
        "ch2.invert",
    ],
    [
        "ch3.on",
        "ch3.vscale",
        "ch3.vfine",
        "ch3.unit",
        "ch3.bias",
        "ch3.skew",
        "ch3.offset",
        "ch3.coupling",
        "ch3.bw",
        "ch3.probe",
        "ch3.invert",
    ],
    [
        "ch4.on",
        "ch4.vscale",
        "ch4.vfine",
        "ch4.unit",
        "ch4.bias",
        "ch4.skew",
        "ch4.offset",
        "ch4.coupling",
        "ch4.bw",
        "ch4.probe",
        "ch4.invert",
    ],
];

fn channel_diff(i: usize, a: &Channel, b: &Channel, emit: &mut impl FnMut(FieldDiff)) {
    let Channel { on, vscale, vfine, unit, bias, skew, offset, coupling, bw, probe, invert } = a;
    let signal = Classes::ACQ | Classes::STATS;
    emit(FieldDiff::new(194 + i * 11, CHANNEL_NAMES[i][0], on != &b.on, signal));
    emit(FieldDiff::new(194 + i * 11 + 1, CHANNEL_NAMES[i][1], vscale != &b.vscale, signal));
    emit(FieldDiff::new(194 + i * 11 + 2, CHANNEL_NAMES[i][2], vfine != &b.vfine, signal));
    emit(FieldDiff::new(194 + i * 11 + 3, CHANNEL_NAMES[i][3], unit != &b.unit, Classes::NONE));
    emit(FieldDiff::new(194 + i * 11 + 4, CHANNEL_NAMES[i][4], bias != &b.bias, signal));
    emit(FieldDiff::new(194 + i * 11 + 5, CHANNEL_NAMES[i][5], skew != &b.skew, signal));
    emit(FieldDiff::new(194 + i * 11 + 6, CHANNEL_NAMES[i][6], offset != &b.offset, signal));
    emit(FieldDiff::new(194 + i * 11 + 7, CHANNEL_NAMES[i][7], coupling != &b.coupling, signal));
    emit(FieldDiff::new(194 + i * 11 + 8, CHANNEL_NAMES[i][8], bw != &b.bw, signal));
    emit(FieldDiff::new(194 + i * 11 + 9, CHANNEL_NAMES[i][9], probe != &b.probe, signal));
    emit(FieldDiff::new(194 + i * 11 + 10, CHANNEL_NAMES[i][10], invert != &b.invert, signal));
}
