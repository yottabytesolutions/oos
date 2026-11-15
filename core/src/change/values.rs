// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Identity, MappingKey};
use crate::model::Model;

/// All plot geometry comes from the layout already processed by intake.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    main: (usize, usize),
    zoom: (usize, usize),
    panel: (usize, usize),
    hdmi: Option<(usize, usize)>,
    pane: usize,
    menu: usize,
}
impl Layout {
    pub fn new(
        main: (usize, usize),
        zoom: (usize, usize),
        panel: (usize, usize),
        hdmi: Option<(usize, usize)>,
        pane: usize,
        menu: usize,
    ) -> Self {
        Self { main, zoom, panel, hdmi, pane, menu }
    }
    pub fn main(self) -> (usize, usize) {
        self.main
    }
}

/// UI needs that the flat settings cannot express. Re-derived each step, never mutated by consumers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiAcq {
    raw: bool,
    route: bool,
    mask_reference: bool,
    autoset: bool,
    bode: bool,
    layout: Layout,
}
impl UiAcq {
    pub fn of(raw: bool, route: bool, mask_reference: bool, autoset: bool, bode: bool, layout: Layout) -> Self {
        Self { raw, route, mask_reference, autoset, bode, layout }
    }
    pub fn layout(self) -> Layout {
        self.layout
    }
    pub fn route(self) -> bool {
        self.route
    }
    pub fn raw(self) -> bool {
        self.raw
    }
    pub(super) fn acquisition(self) -> (bool, bool, bool, bool, bool) {
        (self.raw, self.route, self.mask_reference, self.autoset, self.bode)
    }
}

/// Board-neutral shadow intent borrowing the normalized scratch; no board publication uses it in M2.2.
pub struct AcqIntent<'a> {
    settings: &'a Model,
    needs: UiAcq,
    identity: Identity,
}
impl<'a> AcqIntent<'a> {
    pub fn new(settings: &'a Model, needs: UiAcq, identity: Identity) -> Self {
        Self { settings, needs, identity }
    }
    pub fn settings(&self) -> &Model {
        self.settings
    }
    pub fn needs(&self) -> UiAcq {
        self.needs
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    pub fn mapping(&self) -> MappingKey {
        MappingKey::of(self.settings, self.needs.layout)
    }
}
