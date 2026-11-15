// SPDX-License-Identifier: GPL-2.0-or-later

//! Executable names for the reviewed task table. Rows describe old effects, never inferred class unions.

use oos_core::change::Classes as C;

pub const ROW_COUNT: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Mark,
    Layout,
    Trigger,
    Recall,
    Single,
    Stage,
    Publication,
    Clear,
    Run,
    Mask,
    Reset,
    Measurement,
    Drawing,
    Normalize,
    Acquisition,
    Math,
    Analyzer,
    MaskSource,
}
impl Row {
    pub const ALL: [Self; ROW_COUNT] = [
        Self::Mark,
        Self::Layout,
        Self::Trigger,
        Self::Recall,
        Self::Single,
        Self::Stage,
        Self::Publication,
        Self::Clear,
        Self::Run,
        Self::Mask,
        Self::Reset,
        Self::Measurement,
        Self::Drawing,
        Self::Normalize,
        Self::Acquisition,
        Self::Math,
        Self::Analyzer,
        Self::MaskSource,
    ];
    pub fn name(self) -> &'static str {
        [
            "mark",
            "layout",
            "trigger",
            "recall",
            "single",
            "stage",
            "publication",
            "clear",
            "run",
            "mask",
            "reset",
            "measurement",
            "drawing",
            "normalize",
            "acquisition",
            "math",
            "analyzer",
            "mask_source",
        ][self as usize]
    }
}

/// Reviewed difference ids retain their diagnostic bit positions. D6 has no current producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Difference {
    D1 = 0,
    D2 = 1,
    D3 = 2,
    D4 = 3,
    D5 = 4,
    D7 = 6,
    D8 = 7,
    D9 = 8,
}
impl Difference {
    pub fn mask(self) -> u16 {
        1 << self as u8
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::D1 => "D1",
            Self::D2 => "D2",
            Self::D3 => "D3",
            Self::D4 => "D4",
            Self::D5 => "D5",
            Self::D7 => "D7",
            Self::D8 => "D8",
            Self::D9 => "D9",
        }
    }
}

/// A proposed difference id is evidence to review, not an exemption from a raw disagreement counter.
/// Restrict broad legacy marks to their actual two consumers, and retain unknown cases separately.
pub fn difference(row: Row, class: C, old: bool, stopped: bool, repaired: C) -> Option<Difference> {
    match row {
        Row::Mark | Row::Recall | Row::Stage if old && (class == C::MAPPING || class == C::STATS) => {
            Some(Difference::D1)
        }
        Row::Layout | Row::Trigger if old && class == C::MAPPING => Some(Difference::D1),
        Row::Measurement if old && class == C::STATS => Some(Difference::D1),
        Row::Analyzer if old && (class == C::MATH || class == C::VIEW) => Some(Difference::D1),
        Row::Mark if !old && stopped && class == C::MAPPING => Some(Difference::D2),
        Row::Normalize if !old && repaired.contains(class) => Some(Difference::D3),
        Row::Acquisition if !old && (class == C::HW_AVERAGE || (stopped && class == C::PHOSPHOR)) => {
            Some(Difference::D7)
        }
        Row::Math if !old && class == C::MATH => Some(Difference::D8),
        // The old mask channel was applied outside the generation key. Old-channel records
        // must be dropped and mask counts reset, including for demo sources (D9).
        Row::MaskSource if !old && (class == C::ACQ || class == C::MASK) => Some(Difference::D9),
        Row::Clear if RequestClasses::clear().contains(class) => Some(Difference::D4),
        Row::Single if (C::PHOSPHOR | C::STATS | C::HW_AVERAGE).contains(class) => Some(Difference::D4),
        // RUN requests declare HW_AVERAGE for every source; the old reset only acts on hardware.
        // Demo/scripted sources have no hardware average to reset, so this extra class is inert (D4).
        Row::Run if class == C::HW_AVERAGE || class == C::HISTORY => Some(Difference::D4),
        Row::Publication if class == C::ACQ => Some(Difference::D4),
        Row::Stage if class == C::ACQ || class == C::MAPPING || class == C::STATS => Some(Difference::D5),

        _ => None,
    }
}

struct RequestClasses;
impl RequestClasses {
    fn clear() -> C {
        C::PHOSPHOR | C::STATS | C::MASK | C::ZONE | C::HW_AVERAGE | C::MATH | C::VIEW
    }
}

/// A legacy dirty clear services both geometry and persistence consumers. Select its contracts from the
/// effective fields, never from the classifier's output. Broad extra clears retain the mapping observation (D1).
pub fn dirty_contracts(old: C, geometry: bool, persistence: bool) -> C {
    let mapping = old.contains(C::MAPPING);
    let phosphor = old.contains(C::PHOSPHOR);
    if !mapping && !phosphor {
        return old;
    }
    let mut projected = old.without(C::MAPPING | C::PHOSPHOR);
    if geometry || (!persistence && mapping) {
        projected |= C::MAPPING;
    }
    if persistence || phosphor {
        projected |= C::PHOSPHOR;
    }
    projected
}

pub fn persistence_field(name: &str) -> bool {
    matches!(name, "persist" | "color_grade" | "acq" | "avg_count" | "avg_hw")
}
pub fn geometry_field(name: &str) -> bool {
    matches!(
        name,
        "tscale"
            | "tfine"
            | "delay"
            | "zoom"
            | "zscale"
            | "zdelay"
            | "dots"
            | "win_layout"
            | "ch_win"
            | "math_win"
            | "la_win"
    ) || (name.starts_with("ch") && name.contains('.') && !name.ends_with(".unit"))
}
