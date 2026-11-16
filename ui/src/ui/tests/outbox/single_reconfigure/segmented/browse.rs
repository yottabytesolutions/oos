// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{scripted::SegmentWorker, SegPhase};

fn stopped_recording() -> (Ui, crate::board::HwShared, SegmentWorker) {
    let (mut ui, shared) = segmented_setup();
    let mut worker = SegmentWorker::new();
    for _ in 0..120 {
        worker.tick(&shared);
        ui.step(0.04);
        if ui.m.run == RunState::Stop {
            break;
        }
    }
    assert_eq!((ui.m.run, ui.seg_times.len()), (RunState::Stop, 50));
    assert_eq!(lock(&shared).seg.phase, SegPhase::Done);
    (ui, shared, worker)
}

#[test]
fn segmented_stop_browse_survives_clear_and_mask_source_with_pending_request() {
    for clear in [true, false] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, mut worker) = stopped_recording();
        let times = ui.seg_times.clone();
        let old = change::acquisition::identity(&ui);
        ui.seg_show(3);
        if clear {
            ui.key(key::CLEAR);
        } else {
            ui.m.mask_src = 1;
            ui.settings_changed();
        }
        ui.step(0.04);
        let id = change::acquisition::identity(&ui);
        assert_ne!((id.gen(), id.epoch()), (old.gen(), old.epoch()));
        worker.tick(&shared);
        assert_eq!(lock(&shared).seg.phase, SegPhase::Done);
        let shown = lock(&shared).seg.shown.clone().expect("the pending browse must be answered");
        assert_eq!((shown.0, shown.1.cfg, shown.1.stamp), (3, id.gen(), id.epoch()));
        ui.step(0.04);
        assert_eq!((ui.m.run, ui.seg_pos), (RunState::Stop, Some(3)));
        assert_eq!(ui.seg_times, times);
        assert!(lock(&shared).seg.want.is_none());
        assert_eq!((event("acq_seg_start="), event("acq_configure=")), (1, 1));
        // A later browse must still read the same retained hardware recording.
        ui.seg_show(7);
        worker.tick(&shared);
        ui.step(0.04);
        assert_eq!(ui.seg_pos, Some(7));
        assert_eq!(ui.seg_times, times);
    }
}

#[test]
fn segmented_stop_hardware_reconfigure_cancels_browse_and_clears_ui_history() {
    for disable in [false, true] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, mut worker) = stopped_recording();
        ui.seg_show(3);
        assert_eq!(lock(&shared).seg.want, Some(3));
        if disable {
            ui.m.seg_hw = false;
        } else {
            ui.m.tscale += 1;
        }
        ui.settings_changed();
        ui.step(0.04);
        worker.tick(&shared);
        {
            let g = lock(&shared);
            assert_eq!(g.seg.phase, SegPhase::Idle);
            assert!(g.seg.times.is_empty() && g.seg.want.is_none() && g.seg.shown.is_none());
        }
        // A click before the UI observes Idle must also be cancelled, even with segments disabled.
        ui.seg_show(8);
        ui.step(0.04);
        assert!(ui.seg_times.is_empty(), "the discarded recording must disappear from SCPI and the table");
        assert_eq!(ui.seg_pos, None);
        assert!(lock(&shared).seg.want.is_none());
        assert_eq!((ui.m.run, event("acq_seg_start="), event("acq_configure=")), (RunState::Stop, 1, 2));
    }
}

#[test]
fn segmented_stop_browse_retries_a_pair_change_during_dma() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = stopped_recording();
    let old = change::acquisition::identity(&ui);
    ui.seg_show(3);
    ui.key(key::CLEAR);
    ui.step(0.04);
    let id = change::acquisition::identity(&ui);
    lock(&shared).acquisition = crate::board::identity::Publication::new(old, false);
    worker.change_pair_during_read(&shared, id);
    worker.tick(&shared);
    {
        let g = lock(&shared);
        assert!(g.seg.shown.is_none());
        assert_eq!(g.seg.want, Some(3));
    }
    worker.tick(&shared);
    ui.step(0.04);
    assert_eq!((ui.seg_pos, ui.seg_times.len()), (Some(3), 50));
    assert_eq!((event("acq_seg_start="), event("acq_configure=")), (1, 1));
}
