// SPDX-License-Identifier: GPL-2.0-or-later

//! Executable names for the reviewed task table. Rows describe old effects, never inferred class unions.

use oos_core::change::Classes as C;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Mark,
    Offset,
    Platform,
    Zoom,
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
}
impl Row {
    pub const ALL: [Self; 16] = [
        Self::Mark,
        Self::Offset,
        Self::Platform,
        Self::Zoom,
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
    ];
    pub fn name(self) -> &'static str {
        [
            "mark",
            "offset",
            "platform",
            "zoom",
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
        ][self as usize]
    }
}

/// A proposed difference id is evidence to review, not an exemption from a raw disagreement counter.
/// Restrict broad legacy marks to their actual two consumers, and retain unknown cases separately.
pub fn difference(row: Row, class: C, old: bool, stopped: bool) -> Option<&'static str> {
    match row {
        Row::Mark | Row::Platform | Row::Recall | Row::Stage if old && (class == C::MAPPING || class == C::STATS) => {
            Some("D1")
        }
        Row::Layout | Row::Trigger if old && class == C::MAPPING => Some("D1"),
        Row::Measurement if old && class == C::STATS => Some("D1"),
        Row::Mark if !old && stopped && class == C::MAPPING => Some("D2"),
        Row::Offset if class == C::ACQ || class == C::MAPPING || class == C::STATS => Some("D3"),
        Row::Zoom if class == C::MAPPING => Some("D3"),
        Row::Clear if RequestClasses::clear().contains(class) => Some("D4"),
        Row::Single if (C::PHOSPHOR | C::STATS | C::HW_AVERAGE).contains(class) => Some("D4"),
        // RUN requests declare HW_AVERAGE for every source; the old reset only acts on hardware.
        // Demo/scripted sources have no hardware average to reset, so this extra class is inert (D4).
        Row::Run if class == C::HW_AVERAGE || class == C::HISTORY => Some("D4"),
        Row::Publication if class == C::ACQ => Some("D4"),
        Row::Stage if class == C::ACQ || class == C::MAPPING || class == C::STATS => Some("D5"),

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
