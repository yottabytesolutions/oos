// SPDX-License-Identifier: GPL-2.0-or-later

//! Field audit and shadow classification. No acting consumer reads these values yet.

mod acquire;
mod analysis0;
mod analysis1;
mod analysis2;
mod analysis3;
mod analysis4;
mod analysis5;
mod channels;
mod classes;
mod display;
mod display2;
mod interfaces;
mod logic;
mod mapping;
mod measure;
mod measure1;
mod normalize;
mod patch;
mod references;
mod requests;
mod shadow;
mod sim;
mod snapshot;
mod timebase;
mod trigger;
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

pub const FIELD_COUNT: usize = 345;
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
    channels::channels_0(a, b, &mut emit);
    timebase::timebase_0(a, b, &mut emit);
    trigger::trigger_0(a, b, &mut emit);
    trigger::trigger_1(a, b, &mut emit);
    acquire::acquire_0(a, b, &mut emit);
    display::display_0(a, b, &mut emit);
    measure::measure_0(a, b, &mut emit);
    analysis0::analysis_0(a, b, &mut emit);
    analysis1::analysis_1(a, b, &mut emit);
    analysis2::analysis_2(a, b, &mut emit);
    analysis3::analysis_3(a, b, &mut emit);
    interfaces::interfaces_0(a, b, &mut emit);
    references::references(a, b, &mut emit);
    analysis4::analysis4(a, b, &mut emit);
    measure1::measure1(a, b, &mut emit);
    analysis5::analysis5(a, b, &mut emit);
    sim::sim(a, b, &mut emit);
    display2::display2(a, b, &mut emit);
    mapping::channel_fields(a, b, &mut emit);
    logic::fields(a, b, &mut emit);
}

pub fn refresh(a: &mut Model, b: &Model) {
    channels::refresh_0(a, b);
    timebase::refresh_0(a, b);
    trigger::refresh_0(a, b);
    trigger::refresh_1(a, b);
    acquire::refresh_0(a, b);
    display::refresh_0(a, b);
    measure::refresh_0(a, b);
    analysis0::refresh_0(a, b);
    analysis1::refresh_1(a, b);
    analysis2::refresh_2(a, b);
    analysis3::refresh_3(a, b);
    interfaces::refresh_0(a, b);
    references::refresh_references(a, b);
    analysis4::refresh_analysis4(a, b);
    measure1::refresh_measure1(a, b);
    analysis5::refresh_analysis5(a, b);
    sim::refresh_sim(a, b);
    display2::refresh_display2(a, b);
}

pub fn classify(a: &Model, b: &Model, old: UiAcq, new: UiAcq) -> Classes {
    let mut c = Classes::NONE;
    fields(a, b, |f| {
        if f.changed {
            c |= f.classes;
        }
    });
    if MappingKey::of(a, old.layout()) != MappingKey::of(b, new.layout()) {
        c |= Classes::MAPPING;
    }
    if old.acquisition() != new.acquisition() {
        c |= Classes::ACQ | Classes::STATS;
    }
    c
}

#[cfg(test)]
mod tests;
