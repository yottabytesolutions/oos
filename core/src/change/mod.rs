// SPDX-License-Identifier: GPL-2.0-or-later

//! Field audit and shadow classification. No acting consumer reads these values yet.

mod classes;
mod field_table;
mod flat;
mod logic;
mod mapping;
mod normalize;
mod patch;
mod presentation;
mod requests;
mod shadow;
mod snapshot;
mod storage;
mod values;

use crate::model::Model;
pub use classes::Classes;
pub use mapping::{DepositView, MappingKey, WorkerViewKey};
pub use normalize::normalize;
pub use patch::Patch;
use requests::Generations;
pub use requests::{ChangeSet, Command, Error, Identity, Outbox, Request, Requester, Token};
pub use shadow::Shadow;
pub use snapshot::snapshot_bytes;
pub use values::{AcqIntent, Layout, UiAcq};

pub const FIELD_COUNT: usize = 344;
/// Indexes of the derived member fields (channel and LA members, `mapping`, `logic`); every other index is a whole
/// `Model` field.
pub(crate) const MEMBER_FIELDS: std::ops::Range<usize> = 194..243;

#[derive(Clone, Copy, Debug)]
pub struct FieldDiff {
    index: usize,
    name: &'static str,
    changed: bool,
    classes: Classes,
}
impl FieldDiff {
    fn new(index: usize, name: &'static str, changed: bool, classes: Classes) -> Self {
        Self { index, name, changed, classes }
    }
    pub fn index(self) -> usize {
        self.index
    }
    pub fn name(self) -> &'static str {
        self.name
    }
    pub fn changed(self) -> bool {
        self.changed
    }
    pub fn classes(self) -> Classes {
        self.classes
    }
}

pub fn fields(a: &Model, b: &Model, mut emit: impl FnMut(FieldDiff)) {
    flat::fields(a, b, &mut emit);
    mapping::channel_fields(a, b, &mut emit);
    logic::fields(a, b, &mut emit);
}

pub fn refresh(a: &mut Model, b: &Model) {
    flat::refresh(a, b);
}

pub fn classify(a: &Model, b: &Model, old: UiAcq, new: UiAcq) -> Classes {
    classify_boundary(a, b, old, new, &[])
}

fn classify_boundary(a: &Model, b: &Model, old: UiAcq, new: UiAcq, requests: &[Request]) -> Classes {
    let mut c = Classes::NONE;
    let sweep_companion =
        a.trig_mode == crate::model::TrigMode::Single && requests.contains(&Request::ResumeSweep(b.trig_mode));
    presentation::fields(a, b, requests, |f| {
        if f.changed && !(sweep_companion && f.name == "trig_mode") {
            c |= f.classes;
        }
    });
    let previous = MappingKey::of(a, old.layout()).with_playback_position(a, b, requests);
    if previous != MappingKey::of(b, new.layout()) {
        c |= Classes::MAPPING;
    }
    if old.acquisition() != new.acquisition() {
        c |= Classes::ACQ | Classes::STATS;
    }
    c
}

#[cfg(test)]
mod tests;
