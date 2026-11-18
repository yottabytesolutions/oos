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

#[test]
fn clamped_offset_previews_last() {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.step(0.04);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    change::reset(&mut ui);
    let value = -ui.m.ch[0].offset_max() + ui.m.ch[0].volts_per_div() * 0.5;
    ui.last[0].as_mut().unwrap().ch[0].fill(value);
    let last = ui.last[0].as_ref().unwrap().ch.clone();
    let picture = ui.ph.maps().clone();
    ui.m.ch[0].offset = 1e6;
    ui.step(0.04);
    assert_eq!(ui.m.ch[0].offset, ui.m.ch[0].offset_max());
    assert_eq!(ui.last[0].as_ref().unwrap().ch, last);
    assert_ne!(ui.ph.maps(), &picture);
    assert!(ui.ph.maps()[0].iter().any(|&v| v != 0));
    assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::MAPPING));
    assert_eq!(change::counters(&ui).4, 0);
}

mod picture;

#[test]
fn stopped_mapping_redraws_last_without_blank() {
    for batch in [false, true] {
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
        ui.m.meas.clear();
        ui.m.meas2.clear();
        ui.step(0.04);
        ui.m.run = RunState::Stop;
        ui.m.draw_batch = batch;
        ui.src = Source::Hw { shared: Default::default(), seen: 0 };
        ui.step(0.04);
        let rec = ui.last[0].as_ref().unwrap().ch.clone();
        for edit in 0..6 {
            let picture = ui.ph.maps().clone();
            match edit {
                0 => ui.m.ch[0].vscale += 1,
                1 => ui.m.ch[0].offset += ui.m.ch[0].volts_per_div() * 0.5,
                2 => ui.set_zoom(true),
                3 => ui.m.zscale = ui.m.zscale.saturating_sub(1),
                4 => ui.scr.w += 80,
                _ => ui.m.delay += ui.m.time_per_div(),
            }
            ui.step(0.04);
            assert_eq!(ui.last[0].as_ref().unwrap().ch, rec, "no new record: {edit}");
            assert!(ui.ph.maps()[0].iter().any(|&v| v != 0), "no blank main trace: {edit}");
            if ui.m.zoom {
                assert!(ui.phz.maps()[0].iter().any(|&v| v != 0), "no blank zoom trace: {edit}");
            }
            if edit != 3 {
                assert_ne!(ui.ph.maps(), &picture, "effective mapping: {edit}");
            }
        }
    }
}

mod sessions;
