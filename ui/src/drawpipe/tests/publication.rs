// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn accepting_a_new_zoomed_view_does_not_allocate_with_shared_locked() {
    use crate::{board::HwShared, ui::Ui, wave::Source};
    use oos_hal::sync::lock;

    let _scope = test_scope::Scope::new();
    let shared = HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    // A full first step also warms the pacing mutex on hosts with lazy pthread locks.
    ui.step(0.04);
    let pipe = pipe().unwrap();
    let before = lock(&pipe.view).gen;
    ui.m.zoom = true;
    ui.m.trig_level += 0.125;
    crate::ui::change::settle(&mut ui);
    let view = lock(&pipe.view);
    assert_eq!(view.gen, before + 1);
    assert!(view.view.as_ref().unwrap().zoom.is_some());
    drop(view);
    assert_eq!(lock(&shared).record_flow.acquisition_commit_allocations, 0);
}

#[test]
fn mapping_publication_preserves_a_projection_held_by_a_reader() {
    use oos_hal::sync::lock;

    let pipe = Pipe::start(0, 0, fresh());
    let identity = oos_core::change::Identity::default();
    let mut model = model(1e-6, false, false);
    let offset = model.ch[0].offset;
    let mut publish = |offset| {
        model.ch[0].offset = offset;
        let mut prepared = pipe.prepare_picture_view(&model, (970, 448), Interp::Exact, identity, None);
        pipe.stage_picture_view(&mut prepared, true, identity);
        let state = lock(&pipe.view);
        let view = state.view.as_ref().unwrap();
        assert_eq!(view.m.ch[0].offset, offset);
        std::sync::Arc::as_ptr(view)
    };
    publish(offset);
    let original = lock(&pipe.view).view.as_ref().unwrap().clone();
    let reusable = publish(offset + 0.125);
    let while_held = publish(offset + 0.25);
    assert_ne!(while_held, std::sync::Arc::as_ptr(&original), "a reader still owns the retired view");
    assert_eq!(std::sync::Arc::as_ptr(lock(&pipe.view).spare.as_ref().unwrap()), reusable);
    assert_eq!(publish(offset + 0.5), reusable, "the publisher reuses the unshared retired view");
    assert_eq!(original.m.ch[0].offset, offset);
}

#[test]
fn accepted_publication_refreshes_trigger_only_and_disables_for_single() {
    let _scope = test_scope::Scope::new();
    let mut ui = crate::ui::Ui::new(
        crate::wave::Source::Hw { shared: Default::default(), seen: 0 },
        crate::ui::host::Host::headless(),
    );
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.step(0.04);
    let pipe = pipe().unwrap();
    assert!(pipe.accepting());
    let mapping = oos_core::change::MappingKey::of(&ui.m, Default::default());
    let before = oos_hal::sync::lock(&pipe.view).gen;
    ui.m.trig_level += 0.125;
    crate::ui::change::edited(&mut ui);
    ui.step(0.04);
    assert_eq!(mapping, oos_core::change::MappingKey::of(&ui.m, Default::default()));
    {
        let view = oos_hal::sync::lock(&pipe.view);
        assert_eq!(view.view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
        assert_eq!(view.gen, before + 1, "one accepted worker refresh");
    }
    ui.step(0.04);
    assert_eq!(oos_hal::sync::lock(&pipe.view).gen, before + 1, "no late per-step refresh");
    ui.m.trig_mode = crate::model::TrigMode::Single;
    ui.step(0.04);
    assert!(!pipe.accepting());
    ui.m.trig_level += 0.125;
    ui.m.trig_mode = crate::model::TrigMode::Auto;
    ui.step(0.04);
    assert!(pipe.accepting());
    assert_eq!(oos_hal::sync::lock(&pipe.view).view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
    ui.src = crate::wave::Source::Demo { t: 0.0 };
    ui.step(0.04);
    assert!(!pipe.accepting(), "leaving the hardware source disables its retained view");
}

#[test]
fn trigger_level_change_refreshes_batch_worker_view() {
    let current = refreshed_batch(true);
    let mapping_only = refreshed_batch(false);
    assert_ne!(current, mapping_only, "negative control: mapping-only refresh must leave the old trigger view");
}

fn refreshed_batch(refresh: bool) -> Vec<u16> {
    use oos_core::change::{Layout, MappingKey};
    let (w, h) = (970, 448);
    let mut m = model(1e-6, false, false);
    let old_mapping = MappingKey::of(&m, Layout::default());
    let (bytes, fb, s, p) = batch(13, 12_512, false);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 13, 0, dec(&s, &p));
    settle(&pipe);
    let mut warm = Phosphor::new(w, h);
    pipe.accept(&mut warm, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    m.trig_level = 0.6;
    assert_eq!(MappingKey::of(&m, Layout::default()), old_mapping);
    if refresh && std::env::var_os("OOS_MAPPING_ONLY_PROBE").is_none() {
        pipe.set_view(Some(&m), w, h, Interp::Exact);
    }
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 13, 0, dec(&s, &p));
    settle(&pipe);
    let mut got = Phosphor::new(w, h);
    let result = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    let (want, trig, _, _, _) = reference(&bytes, fb, &s, &p, &m, w, h);
    if refresh {
        assert_eq!((result.frames, result.triggered), (13, trig));
        assert!(got.maps() == want.maps(), "workers must match the new-level record path");
    } else {
        assert_ne!(got.maps(), want.maps(), "negative control must actually distinguish the levels");
    }
    got.maps()[0].clone()
}

// Break it catches: the newest frame of a batch reaching the UI before the batch's other frames (a picture with
// the newest frame alone, the rest a picture later: docs/fork/stutter.md), or not at all.
#[test]
fn a_batch_is_delivered_with_its_frames() {
    let (w, h) = (970, 448);
    let m = model(1e-6, false, false);
    let (bytes, fb, s, p) = batch(30, 12_512, false);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.publish_view(&m, w, h, Interp::Exact, Default::default());
    let newest = Arc::new(Frame { seq: 30, ..crate::board::dho::hw::decode(&bytes[29 * fb..], &s, &p, 0) });
    pipe.feed_with(Box::new(Bytes(bytes)), fb, 0..30, Default::default(), dec(&s, &p), None, Some(newest.clone()));
    settle(&pipe);
    let mut ph = Phosphor::new(w, h);
    let n = pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(n.frames, 30);
    assert!(Arc::ptr_eq(&pipe.take_waveform((w, h)).unwrap(), &newest));
    assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
}

// Break it catches: frames of older settings drawn, maps of an older mapping merged, frames nobody drew not
// counted, batches taken while the fast path does not apply.
#[test]
fn stale_work_is_dropped_and_counted() {
    let (w, h) = (970, 448);
    let mut m = model(1e-6, false, false);
    let (bytes, fb, s, p) = batch(10, 12_512, false);
    let c = fresh();
    let pipe = Pipe::start(2, 0, c);
    let mut ph = Phosphor::new(w, h);
    let merge = |ph: &mut Phosphor| pipe.accept(ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    assert!(!pipe.accepting());
    // settings stamp older than the view's: dropped
    m.hw_stamp = 3;
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 2, dec(&s, &p));
    settle(&pipe);
    assert_eq!(merge(&mut ph).frames, 0);
    assert_eq!(c.read(), (0, 0, 10));
    // drawn, then the mapping changes before the merge: dropped at the merge
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
    settle(&pipe);
    m.ch[0].offset = 0.5;
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    assert_eq!(merge(&mut ph).frames, 0);
    assert!(ph.maps().iter().all(|x| x.iter().all(|&y| y == 0)));
    assert_eq!(c.read(), (0, 0, 20));
    // a change the drawing does not depend on keeps the maps
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
    settle(&pipe);
    m.status = TrigStatus::Wait;
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    assert_eq!(merge(&mut ph).frames, 10);
    // many batches at once: all drawn, nothing more dropped
    let (big, fb2, s2, p2) = batch(200, 12_512, false);
    for _ in 0..6 {
        pipe.feed(Box::new(Bytes(big.clone())), fb2, 200, 3, dec(&s2, &p2));
    }
    settle(&pipe);
    assert_eq!(collect(&pipe, &mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 1200);
    assert_eq!(c.read().2, 20);
    // a batch dropped before anybody claimed a frame counts all of them
    let pipe0 = Pipe::start(0, 0, c);
    pipe0.set_view(Some(&m), w, h, Interp::Exact);
    pipe0.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
    assert_eq!(c.read().2, 30);
    // no view: the acquisition thread queues the frames itself
    pipe.set_view(None, 0, 0, Interp::Exact);
    assert!(!pipe.accepting());
}
