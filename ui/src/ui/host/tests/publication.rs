// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::ui::{change, *};
use oos_hal::sync::lock;

fn hardware() -> (Ui, crate::board::HwShared) {
    let mut host = Host::fixture();
    host.switches.board.mask = true;
    host.switches.board.zone = true;
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host);
    ui.step(0.04);
    (ui, shared)
}

#[test]
fn create_and_enable_mask_publish_limits_and_pair_together() {
    let (mut ui, shared) = hardware();
    for _ in 0..20 {
        ui.m.mask_on = false;
        ui.step(0.04);
        let before = lock(&shared).acquisition.identity();
        let frame = crate::board::Frame {
            seq: 1,
            cfg: before.gen(),
            stamp: before.epoch(),
            dt: 1e-6,
            ch: std::array::from_fn(|c| if c == 0 { vec![1.0; 64] } else { Vec::new() }),
            ..Default::default()
        };
        ui.last[0] = Some(crate::wave::hw_record(&ui.m, frame));
        ui.create_mask();
        ui.mask_hw_poll();
        assert!(lock(&shared).mask_want.is_none(), "poll cannot publish new mask under old pair");
        let id = change::acquisition::script_prepare(&mut ui);
        let g = lock(&shared);
        assert_ne!(id, before);
        assert_eq!(g.acquisition.identity(), id);
        let mask = g.mask_want.as_ref().expect("new-pair acquisition must have its mask");
        assert_eq!(mask.clear, ui.mask_hw_clear);
        assert_eq!(mask.ch, ui.m.mask_src);
        assert_eq!((&mask.lo, &mask.hi), ui.mask_cols.as_ref().map(|(lo, hi)| (lo, hi)).unwrap());
        drop(g);
        ui.step(0.04);
        // Enabling an existing mask follows the same atomic handoff as creating one.
        ui.m.mask_on = false;
        ui.step(0.04);
        let before = lock(&shared).acquisition.identity();
        ui.m.mask_on = true;
        ui.mask_hw_poll();
        assert!(lock(&shared).mask_want.is_none());
        let id = change::acquisition::script_prepare(&mut ui);
        let g = lock(&shared);
        assert_ne!(id, before);
        assert_eq!(g.acquisition.identity(), id);
        assert_eq!(g.mask_want, ui.mask_hw_want());
        assert!(g.mask_want.is_some());
        drop(g);
        ui.step(0.04);
    }
}

#[test]
fn zone_poll_reads_only_and_publication_owns_its_want() {
    let (mut ui, shared) = hardware();
    ui.m.zones[0] = crate::zone::Zone { on: true, src: 0, inside: true, pts: vec![(0.0, 0.0), (1e-6, 1.0)] };
    let before = lock(&shared).acquisition.identity();
    ui.zone_hw_poll();
    assert!(lock(&shared).zone_want.is_none());
    let id = change::acquisition::script_prepare(&mut ui);
    let g = lock(&shared);
    assert_ne!(id, before);
    assert_eq!(g.acquisition.identity(), id);
    assert_eq!(g.zone_want, ui.zone_hw_want());
    assert!(g.zone_want.is_some());
}

#[test]
fn live_and_scripted_prepare_see_the_same_lifecycle_poll_state() {
    for scripted in [false, true] {
        let (mut ui, shared) = hardware();
        ui.m.mask_on = true;
        ui.mask_ref = Some(vec![1.0; 64]);
        ui.build_mask();
        assert!(ui.mask_hw_active());
        lock(&shared).unit_blocked = Some("test stall".into());
        if scripted {
            change::acquisition::script_prepare(&mut ui);
        } else {
            change::begin_step(&mut ui, 0.04).unwrap();
        }
        assert!(ui.unit_blocked.is_some());
        assert!(!ui.fast_ok());
        let g = lock(&shared);
        assert!(!g.acquisition.route());
        assert!(g.mask_want.is_none());
    }
}
