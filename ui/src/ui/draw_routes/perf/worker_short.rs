// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded whole-worker host comparison, same production DHO words/decoder/plan and instrumentation.
use super::*;

fn worker(build: &mut Build) -> u128 {
    let ui = &mut build.route.ui;
    let id = change::acquisition::script_prepare_route(ui, false);
    let sw = ui.switches().clone();
    build.input.first = ui.wfm_total + 1;
    let batch = &mut build.input;
    let mut newest = batch.newest(id.epoch());
    (newest.cfg, newest.stamp) = (id.gen(), id.epoch());
    let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
    let at = ui.host.picture_time(ui.now);
    let bytes = batch.into_bytes();
    let pipe = drawpipe::start_pipe(&sw);
    let started = Instant::now();
    pipe.feed_at(bytes, fb, 0..32, (id, at), decode, codes, Some(Arc::new(newest)));
    let result = drawpipe::host::finish_batch(pipe, 1);
    let ns = started.elapsed().as_nanos();
    if result.is_err() {
        drawpipe::shutdown::close(pipe, Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    }
    build.input.bytes = Some(result.unwrap());
    crate::ui::step_draw::owner::intake_render_owner(ui);
    ui.step(1.0 / 30.0);
    assert_eq!(ui.batch_picture.frames, 32);
    ns
}

#[test]
#[ignore = "bounded native ARM comparison only, OOS_WORKER_COMPARE=1"]
fn worker_route_short_extrema_compare() {
    assert_eq!(std::env::var("OOS_WORKER_COMPARE").as_deref(), Ok("1"));
    let _scope = Scope::new();
    let mut build = Build::new(false, 4, 1e-6, 32);
    build.route.ui.m.persist = 0;
    build.route.ui.m.run = RunState::Run;
    build.route.ui.m.dots = DotsMode::Auto;
    build.route.ui.m.trig_src = 1;
    build.route.ui.m.trig_mode = TrigMode::Normal;
    build.route.ui.m.trig_level = 4.93;
    crate::ui::step_draw::owner::set_render_owner_wake(
        &mut build.route.ui,
        Arc::new(crate::system::intake::Wake::new().unwrap()),
    );
    let input = &mut build.input;
    let f = input.frame(0, 0);
    let mut metadata = Frame::default();
    let layout = input.codes().unwrap()(0, &input.bytes.as_ref().unwrap().bytes()[..input.fb], &mut metadata).unwrap();
    assert_eq!((layout.stride, metadata.ch.iter().map(Vec::len).sum::<usize>()), (4, 0));
    assert_eq!(f.ch.each_ref().map(Vec::len), [3520; 4]);
    let mut expected = Phosphor::new(build.route.ui.ph.w, build.route.ui.ph.h);
    for i in 0..32 {
        let frame = input.frame(i, 0);
        drawpipe::draw_frame(&mut expected, &build.route.ui.m, &frame, crate::sparse::Interp::Exact, None);
    }
    worker(&mut build);
    assert_eq!(build.route.ui.ph.maps(), expected.maps(), "actual credited32 match scalar pixel oracle");
    for _ in 0..8 {
        worker(&mut build);
    }
    let mut ns = (0..64).map(|_| worker(&mut build)).collect::<Vec<_>>();
    let mean = ns.iter().sum::<u128>() / ns.len() as u128;
    ns.sort_unstable();
    println!("WORKER_COMPARE n={} mean_ns={} p50_ns={} p90_ns={} geometry={}x{} samples=3520 stride=4 kept=32 timing=feed_to_raw_return ui_outside_timing no_backing_pressure=true platform=Apple_ARM_not_target raw={ns:?}",
        ns.len(), mean, ns[31], ns[57], build.route.ui.ph.w, build.route.ui.ph.h);
}
