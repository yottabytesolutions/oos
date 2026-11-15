// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{BusWhat, Model, SerialTrig};

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger2 {
    NthIdle(f32),
    NthN(u32),
    Setup(f32),
    Hold(f32),
    TrigBus(BusWhat),
    TrigBusVal(u64),
    Ser(SerialTrig),
    AutoFast(bool),
}

impl Trigger2 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            28 => Self::NthIdle(m.nth_idle),
            29 => Self::NthN(m.nth_n),
            30 => Self::Setup(m.setup),
            31 => Self::Hold(m.hold),
            105 => Self::TrigBus(m.trig_bus),
            106 => Self::TrigBusVal(m.trig_bus_val),
            107 => Self::Ser(m.ser),
            164 => Self::AutoFast(m.auto_fast),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::NthIdle(_) => 28,
            Self::NthN(_) => 29,
            Self::Setup(_) => 30,
            Self::Hold(_) => 31,
            Self::TrigBus(_) => 105,
            Self::TrigBusVal(_) => 106,
            Self::Ser(_) => 107,
            Self::AutoFast(_) => 164,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::NthIdle(value) => m.nth_idle.clone_from(value),
            Self::NthN(value) => m.nth_n.clone_from(value),
            Self::Setup(value) => m.setup.clone_from(value),
            Self::Hold(value) => m.hold.clone_from(value),
            Self::TrigBus(value) => m.trig_bus.clone_from(value),
            Self::TrigBusVal(value) => m.trig_bus_val.clone_from(value),
            Self::Ser(value) => m.ser.clone_from(value),
            Self::AutoFast(value) => m.auto_fast.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::NthIdle(value) => value.clone_from(&m.nth_idle),
            Self::NthN(value) => value.clone_from(&m.nth_n),
            Self::Setup(value) => value.clone_from(&m.setup),
            Self::Hold(value) => value.clone_from(&m.hold),
            Self::TrigBus(value) => value.clone_from(&m.trig_bus),
            Self::TrigBusVal(value) => value.clone_from(&m.trig_bus_val),
            Self::Ser(value) => value.clone_from(&m.ser),
            Self::AutoFast(value) => value.clone_from(&m.auto_fast),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
