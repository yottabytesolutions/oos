// SPDX-License-Identifier: GPL-2.0-or-later
//! Production-planned decoded raw input: actual Ui::step owns first-use credit.
use super::*;
#[test]
fn default_two_channel_bound_is_composed_and_credited_once_by_real_ui() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, true, 2, 1e-6);
    for (key, value) in [
        ("trig_mode", "Normal"),
        ("trig_type", "Edge"),
        ("trig_src", "1"),
        ("trig_level", "4.93"),
        ("acq", "Normal"),
        ("mdepth", "0"),
        ("cores", "true"),
        ("persist", "0"),
        ("dots", "Auto"),
        ("ch1.vdiv", "0.01"),
        ("ch1.offset", "-1.5"),
        ("ch2.vdiv", "0.2"),
        ("ch2.vfine", "1.125"),
        ("ch2.offset", "-4.9"),
    ] {
        crate::store::set(&mut route.ui.m, key, value).unwrap();
    }
    route.ui.m.run = RunState::Run;
    route.ui.relayout();
    let id = change::acquisition::script_prepare_route(&mut route.ui, false);
    let switches = change::acquisition::draw_switches(&route.ui, &route.ui.m);
    let original = crate::board::dho::hw::settings(&route.ui.m, false, &switches);
    let params = oos_hal::dho::hz::prep(original.mask, oos_hal::dho::seq::tdiv_fs(original.tdiv), original.depth, 0);
    assert_eq!((original.mask, params.rate, params.memsel), (3, 625_000_000, 7032));
    // Preserve the pixel and credit fixture's explicit batch size.
    let frames = 64;
    let (input, count, _) = scripted::draw_route_batch(&route.ui.m, &switches, frames);
    assert_eq!((count, input.frame_bytes()), (64, 28128));
    let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    for index in 0..count {
        crate::drawpipe::draw_frame(&mut expected, &route.ui.m, &input.frame(index, id.epoch()), switches.interp, None);
    }
    let (fb, decode) = (input.frame_bytes(), input.decode());
    let raw_bytes = input.into_bytes();
    let input = Input { bytes: Some(Box::new(raw_bytes)), decode, codes: None, fb, frames, first: 1 };
    let mut build = Build { route, input, frames };
    let counters = drawpipe::counters();
    // Same completed synthetic-read event as the established quiescent_live_routes fixture.
    // Deposit and shown credit below come only from the worker and actual Ui::step.
    crate::drawpipe::Counters::add(&counters.acquired, frames as u64);
    assert!(counters.records_text().contains("read=64 deposited=0 shown=0"));
    build.run();
    assert_eq!(build.route.ui.batch_picture.frames, 64);
    assert!(counters.records_text().contains("read=64 deposited=64 shown=64"));
    assert_eq!(
        build.route.ui.ph.maps(),
        expected.maps(),
        "every first-use record has exact pixels at actual composition"
    );
    let newest = build.route.ui.last[0].as_ref().unwrap().raw.as_ref().unwrap();
    assert_eq!(newest.frame.seq, 64);
    build.route.ui.m.run = RunState::Stop;
    build.route.ui.step(1.0 / 30.0);
    assert!(counters.records_text().contains("read=64 deposited=64 shown=64"), "STOP redraw cannot recredit first use");
}
