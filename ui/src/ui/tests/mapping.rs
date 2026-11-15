// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use oos_core::change::{Classes as C, Layout, MappingKey};

#[test]
fn relayout_changes_mapping_key() {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    change::reset(&mut ui);
    ui.scr.w += 80;
    ui.step(0.04);
    assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), C::MAPPING);
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
    let m = Model::default();
    let base = Layout::new((970, 448), (970, 200), (1024, 600), None, 0, 0);
    for layout in [
        Layout::new((800, 448), (970, 200), (1024, 600), None, 0, 0),
        Layout::new((970, 448), (800, 200), (1024, 600), None, 0, 0),
        Layout::new((970, 448), (970, 200), (1920, 1080), None, 0, 0),
        Layout::new((970, 448), (970, 200), (1024, 600), Some((1920, 1080)), 0, 0),
        Layout::new((970, 448), (970, 200), (1024, 600), None, 100, 0),
        Layout::new((970, 448), (970, 200), (1024, 600), None, 0, 100),
    ] {
        assert_ne!(MappingKey::of(&m, base), MappingKey::of(&m, layout));
    }
}
