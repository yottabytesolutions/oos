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
    vertical_zoom: [u32; 2],
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
        Self { main, zoom, panel, hdmi, pane, menu, vertical_zoom: [0; 2] }
    }
    pub fn with_vertical_zoom(mut self, factor: f32, center: f32) -> Self {
        self.vertical_zoom = [factor.to_bits(), center.to_bits()];
        self
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
    read_format: u32,
    autoset: bool,
    bode: bool,
    layout: Layout,
}
impl UiAcq {
    pub fn of(raw: bool, route: bool, autoset: bool, bode: bool, layout: Layout) -> Self {
        Self { raw, route, read_format: 0, autoset, bode, layout }
    }
    /// Effective raw/pair read format supplied by the acquisition adapter, not a settings hint.
    pub fn with_read_format(mut self, format: u32) -> Self {
        self.read_format = format;
        self
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
    pub(super) fn acquisition(self) -> (bool, bool, u32, bool, bool) {
        // Mask publication alone does not change acquisition; its effective worker route does.
        (self.raw, self.route, self.read_format, self.autoset, self.bode)
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
