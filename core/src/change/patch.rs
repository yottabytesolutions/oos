// SPDX-License-Identifier: GPL-2.0-or-later

//! Bounded, typed flat-model proposals for the transitional internal outbox.
use crate::model::Model;
mod acquire0;
mod analysis0;
mod analysis1;
mod analysis2;
mod analysis3;
mod analysis4;
mod analysis5;
mod analysis6;
mod channels0;
mod display0;
mod display1;
mod interfaces0;
mod interfaces1;
mod measure0;
mod measure1;
mod timebase0;
mod trigger0;
mod trigger1;
mod trigger2;

#[derive(Clone, Debug, PartialEq)]
pub enum Patch {
    Channels0(channels0::Channels0),
    Timebase0(timebase0::Timebase0),
    Trigger0(trigger0::Trigger0),
    Trigger1(trigger1::Trigger1),
    Trigger2(trigger2::Trigger2),
    Acquire0(acquire0::Acquire0),
    Display0(display0::Display0),
    Display1(display1::Display1),
    Measure0(measure0::Measure0),
    Measure1(measure1::Measure1),
    Analysis0(analysis0::Analysis0),
    Analysis1(analysis1::Analysis1),
    Analysis2(analysis2::Analysis2),
    Analysis3(analysis3::Analysis3),
    Analysis4(analysis4::Analysis4),
    Analysis5(analysis5::Analysis5),
    Analysis6(analysis6::Analysis6),
    Interfaces0(interfaces0::Interfaces0),
    Interfaces1(interfaces1::Interfaces1),
}

impl Patch {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        match index {
            0 | 1 | 59 | 67 | 70 => channels0::Channels0::capture(index, m).map(Self::Channels0),
            2 | 3 | 68 | 69 | 71 | 117 | 119 | 120 | 118 => {
                timebase0::Timebase0::capture(index, m).map(Self::Timebase0)
            }
            4..=15 => trigger0::Trigger0::capture(index, m).map(Self::Trigger0),
            16..=27 => trigger1::Trigger1::capture(index, m).map(Self::Trigger1),
            28 | 29 | 30 | 31 | 105 | 106 | 107 | 164 => trigger2::Trigger2::capture(index, m).map(Self::Trigger2),
            32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 162 | 163 | 193 => {
                acquire0::Acquire0::capture(index, m).map(Self::Acquire0)
            }
            41 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49 | 88 | 65 | 121 => {
                display0::Display0::capture(index, m).map(Self::Display0)
            }
            122 | 123 | 124 | 125 | 126 | 127 | 128 | 129 | 192 => {
                display1::Display1::capture(index, m).map(Self::Display1)
            }
            50 | 51 | 52 | 53 | 54 | 55 | 56 | 57 | 58 | 60 | 61 | 62 => {
                measure0::Measure0::capture(index, m).map(Self::Measure0)
            }
            63 | 64 | 89 | 90 | 79 => measure1::Measure1::capture(index, m).map(Self::Measure1),
            72 | 73 | 74 | 75 | 76 | 77 | 78 | 80 | 81 | 82 | 83 | 84 => {
                analysis0::Analysis0::capture(index, m).map(Self::Analysis0)
            }
            85 | 86 | 87 | 91 | 92 | 93 | 94 | 95 | 96 | 97 | 98 | 99 => {
                analysis1::Analysis1::capture(index, m).map(Self::Analysis1)
            }
            100 | 101 | 102 | 103 | 104 | 108 | 109 | 110 | 111 | 112 | 113 | 114 => {
                analysis2::Analysis2::capture(index, m).map(Self::Analysis2)
            }
            115 | 116 | 130 | 131 | 132 | 133 | 134 | 135 | 136 | 137 | 138 | 139 => {
                analysis3::Analysis3::capture(index, m).map(Self::Analysis3)
            }
            140..=151 => analysis4::Analysis4::capture(index, m).map(Self::Analysis4),
            152 | 153 | 154 | 155 | 156 | 157 | 158 | 159 | 160 | 161 | 165 | 166 => {
                analysis5::Analysis5::capture(index, m).map(Self::Analysis5)
            }
            167 | 185 | 186 | 187 | 188 | 189 | 190 => analysis6::Analysis6::capture(index, m).map(Self::Analysis6),
            66 | 168 | 169 | 170 | 171 | 172 | 173 | 174 | 175 | 176 | 177 | 178 => {
                interfaces0::Interfaces0::capture(index, m).map(Self::Interfaces0)
            }
            179 | 180 | 181 | 182 | 183 | 184 | 191 => {
                interfaces1::Interfaces1::capture(index, m).map(Self::Interfaces1)
            }
            _ => None,
        }
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Channels0(p) => p.index(),
            Self::Timebase0(p) => p.index(),
            Self::Trigger0(p) => p.index(),
            Self::Trigger1(p) => p.index(),
            Self::Trigger2(p) => p.index(),
            Self::Acquire0(p) => p.index(),
            Self::Display0(p) => p.index(),
            Self::Display1(p) => p.index(),
            Self::Measure0(p) => p.index(),
            Self::Measure1(p) => p.index(),
            Self::Analysis0(p) => p.index(),
            Self::Analysis1(p) => p.index(),
            Self::Analysis2(p) => p.index(),
            Self::Analysis3(p) => p.index(),
            Self::Analysis4(p) => p.index(),
            Self::Analysis5(p) => p.index(),
            Self::Analysis6(p) => p.index(),
            Self::Interfaces0(p) => p.index(),
            Self::Interfaces1(p) => p.index(),
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Channels0(p) => p.apply(m),
            Self::Timebase0(p) => p.apply(m),
            Self::Trigger0(p) => p.apply(m),
            Self::Trigger1(p) => p.apply(m),
            Self::Trigger2(p) => p.apply(m),
            Self::Acquire0(p) => p.apply(m),
            Self::Display0(p) => p.apply(m),
            Self::Display1(p) => p.apply(m),
            Self::Measure0(p) => p.apply(m),
            Self::Measure1(p) => p.apply(m),
            Self::Analysis0(p) => p.apply(m),
            Self::Analysis1(p) => p.apply(m),
            Self::Analysis2(p) => p.apply(m),
            Self::Analysis3(p) => p.apply(m),
            Self::Analysis4(p) => p.apply(m),
            Self::Analysis5(p) => p.apply(m),
            Self::Analysis6(p) => p.apply(m),
            Self::Interfaces0(p) => p.apply(m),
            Self::Interfaces1(p) => p.apply(m),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Channels0(p) => p.reuse(m),
            Self::Timebase0(p) => p.reuse(m),
            Self::Trigger0(p) => p.reuse(m),
            Self::Trigger1(p) => p.reuse(m),
            Self::Trigger2(p) => p.reuse(m),
            Self::Acquire0(p) => p.reuse(m),
            Self::Display0(p) => p.reuse(m),
            Self::Display1(p) => p.reuse(m),
            Self::Measure0(p) => p.reuse(m),
            Self::Measure1(p) => p.reuse(m),
            Self::Analysis0(p) => p.reuse(m),
            Self::Analysis1(p) => p.reuse(m),
            Self::Analysis2(p) => p.reuse(m),
            Self::Analysis3(p) => p.reuse(m),
            Self::Analysis4(p) => p.reuse(m),
            Self::Analysis5(p) => p.reuse(m),
            Self::Analysis6(p) => p.reuse(m),
            Self::Interfaces0(p) => p.reuse(m),
            Self::Interfaces1(p) => p.reuse(m),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Channels0(p) => p.bytes(),
            Self::Timebase0(p) => p.bytes(),
            Self::Trigger0(p) => p.bytes(),
            Self::Trigger1(p) => p.bytes(),
            Self::Trigger2(p) => p.bytes(),
            Self::Acquire0(p) => p.bytes(),
            Self::Display0(p) => p.bytes(),
            Self::Display1(p) => p.bytes(),
            Self::Measure0(p) => p.bytes(),
            Self::Measure1(p) => p.bytes(),
            Self::Analysis0(p) => p.bytes(),
            Self::Analysis1(p) => p.bytes(),
            Self::Analysis2(p) => p.bytes(),
            Self::Analysis3(p) => p.bytes(),
            Self::Analysis4(p) => p.bytes(),
            Self::Analysis5(p) => p.bytes(),
            Self::Analysis6(p) => p.bytes(),
            Self::Interfaces0(p) => p.bytes(),
            Self::Interfaces1(p) => p.bytes(),
        }
    }
}
