// SPDX-License-Identifier: GPL-2.0-or-later
//! Existing HW RawBatch route, real early-intake method and actual Ui::step accounting.
use super::*;
fn start() -> (Route, oos_core::change::Identity, crate::sysconf::Switches) {
    start_at(1e-6)
}
fn start_at(tdiv: f32) -> (Route, oos_core::change::Identity, crate::sysconf::Switches) {
    let mut r = Route::new(BatchKind::Raw, 4, tdiv);
    r.ui.m.persist = 0;
    r.ui.m.run = RunState::Run;
    let id = change::acquisition::script_prepare_route(&mut r.ui, false);
    let sw = r.ui.switches().clone();
    crate::ui::step_draw::owner::set_render_owner_wake(
        &mut r.ui,
        Arc::new(crate::system::intake::Wake::new().unwrap()),
    );
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    (r, id, sw)
}
fn submit(
    r: &mut Route,
    id: oos_core::change::Identity,
    sw: &crate::sysconf::Switches,
    reference: Option<&mut Phosphor>,
) -> Box<dyn RawBatch> {
    let raw = submit_records(r, id, sw, 32, reference);
    wait_for_publication();
    raw
}
fn submit_records(
    r: &mut Route,
    id: oos_core::change::Identity,
    sw: &crate::sysconf::Switches,
    records: usize,
    reference: Option<&mut Phosphor>,
) -> Box<dyn RawBatch> {
    let b = r.source.next_batch(&r.ui.m, sw, records, None).unwrap();
    submit_cohort(r, id, b, 0..records, true, true, reference)
}
fn submit_cohort(
    r: &mut Route,
    id: oos_core::change::Identity,
    b: impl Batch<Bytes = ScriptBytes>,
    selected: std::ops::Range<usize>,
    codes_available: bool,
    newest_kept: bool,
    reference: Option<&mut Phosphor>,
) -> Box<dyn RawBatch> {
    if let Some(ph) = reference {
        for i in selected.clone() {
            let mut frame = b.frame(i, id.epoch());
            (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
            crate::drawpipe::draw_frame(ph, &r.ui.m, &frame, r.ui.switches().interp, None);
        }
    }
    let newest = newest_kept.then(|| {
        let mut frame = b.newest(id.epoch());
        (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
        Arc::new(frame)
    });
    submit_readout(r, id, b, selected, codes_available, newest)
}

fn submit_readout(
    r: &Route,
    id: oos_core::change::Identity,
    b: impl Batch<Bytes = ScriptBytes>,
    selected: std::ops::Range<usize>,
    codes_available: bool,
    newest: Option<Arc<crate::board::Frame>>,
) -> Box<dyn RawBatch> {
    let (fb, decode) = (b.frame_bytes(), b.decode());
    let codes = codes_available.then(|| b.codes()).flatten();
    let pipe = drawpipe::pipe().unwrap();
    pipe.feed_at(b.into_bytes().into(), fb, selected, (id, r.ui.host.picture_time(r.ui.now)), decode, codes, newest);
    let result = drawpipe::host::finish_batch(pipe, 0);
    if result.is_err() {
        drawpipe::shutdown::close(pipe, std::time::Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    }
    result.unwrap()
}

fn wait_for_publication() {
    // The DMA buffer can return before the coordinator publishes to the picture slot.
    let pipe = drawpipe::pipe().unwrap();
    drawpipe::host::barrier(pipe);
}

#[test]
fn compose_claims_a_deposit_published_after_intake_in_the_same_picture() {
    let _scope = Scope::new();
    let (mut r, id, sw) = start();
    let counters = drawpipe::counters();
    let before = counters.read().1;
    let mut reference = Phosphor::new(r.ui.ph.w, r.ui.ph.h);
    drop(submit(&mut r, id, &sw, Some(&mut reference)));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    drop(submit(&mut r, id, &sw, Some(&mut reference)));
    assert_eq!(counters.read().1, before, "early intake never calls Ui first-use accounting");
    assert_eq!(r.ui.batch_picture.frames, 0);
    r.ui.step(1.0 / 30.0);
    assert_eq!(counters.read().1 - before, 64);
    assert_eq!(r.ui.batch_picture.frames, 64);
    assert_eq!(
        r.ui.ph.maps(),
        reference.maps(),
        "every credited raw record is represented by the actual final picture"
    );
}

#[test]
fn skipped_steps_preserve_records_and_pixels_in_the_next_picture() {
    use crate::pacing::{ClockMode, Flip, FlipDivisor, FrameClock};
    use std::{sync::atomic::Ordering, time::Duration};
    let _scope = Scope::new();
    let (mut r, id, sw) = start_at(10e-6);
    let origin = r.ui.host.picture_time(r.ui.now);
    let refresh = Duration::from_millis(20);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, origin, refresh);
    let flip = |sequence| Flip { sequence, at: origin + refresh * sequence };
    let first = flip(2);
    r.ui.step(clock.advance(first.at, Some(first)).0.unwrap().as_secs_f64());
    clock.step_finished(first.at);

    let counters = drawpipe::counters();
    let mut reference = Phosphor::new(r.ui.ph.w, r.ui.ph.h);
    for _ in 0..2 {
        let input = r.source.next_batch(&r.ui.m, &sw, 27, None).unwrap();
        drawpipe::Counters::add(&counters.acquired, 27);
        counters.discard(drawpipe::Lost::Frame0, 1);
        drop(submit_cohort(&mut r, id, input, 1..27, true, true, Some(&mut reference)));
        wait_for_publication();
        crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    }
    let shown = || counters.shown.load(Ordering::Relaxed);
    let in_flight = counters.deposited.load(Ordering::Relaxed);
    assert_eq!(shown(), 0);
    assert_eq!(in_flight, 52);
    assert_eq!(counters.read().0, 2 + in_flight);

    let resumed = flip(8);
    r.ui.step(clock.advance(resumed.at, Some(resumed)).0.unwrap().as_secs_f64());
    assert_eq!(clock.skipped(), 2);
    let state = crate::pacing::with_test_clock(&clock, resumed.at, || {
        crate::scpi::Scpi::disconnected().exec_line(&mut r.ui, &Canvas::new(1, 1), ":SYSTem:STATe?")
    });
    assert!(String::from_utf8(state).unwrap().contains(" step.missed=2"));
    assert_eq!(r.ui.batch_picture.frames, 52);
    assert_pixels(&r.ui, &reference, id);
    let (read, _, dropped) = counters.read();
    assert_eq!(read, shown() + dropped);
    assert_eq!(dropped, 2, "the fixture excludes one frame-0 record from each read");
    let text = counters.records_text();
    assert!(text.contains("lost.frame0=2 ") && text.ends_with("rejected.stale_key=0 rejected.epoch=0"), "{text}");
}
#[test]
fn completed_raw_recycles_once_before_real_output_wait_and_alias_survives_reset() {
    let _scope = Scope::new();
    let (mut r, id, sw) = start();
    let pipe = drawpipe::pipe().unwrap();
    drop(submit_records(&mut r, id, &sw, 32, None));
    drop(submit_records(&mut r, id, &sw, 32, None));
    pipe.wait_owner_output();
    assert!(pipe.take_buffer().is_none(), "exactly the two original raw buffers already returned once");
    let alias = pipe.take_waveform((r.ui.ph.w, r.ui.ph.h)).unwrap();
    assert!(!alias.ch[0].is_empty());
    drawpipe::shutdown::reset(pipe);
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    assert!(!alias.ch[0].is_empty(), "separately owned decoded tap alias is not prematurely revoked");
    assert_eq!(
        drawpipe::shutdown::close(pipe, std::time::Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD),
        0
    );
}

#[test]
fn prepared_clear_preserves_new_pair_but_adjudicates_truly_stale_owned_input() {
    let _scope = Scope::new();
    let (mut r, id, sw) = start();
    let counters = drawpipe::counters();
    // This distinct stale-input control settles bootstrap only before staging OLD data.
    // The initial-clear positive above still stages both matching inputs before its first Ui::step.
    r.ui.step(1.0 / 30.0);
    drop(submit(&mut r, id, &sw, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    let before = counters.read();
    r.ui.clear_display();
    let next = change::acquisition::script_prepare_route(&mut r.ui, false);
    assert_ne!(next.epoch(), id.epoch());
    r.ui.step(1.0 / 30.0);
    assert_eq!(counters.read().1, before.1);
    assert_eq!(counters.read().2 - before.2, 32);
    assert!(r.ui.ph.maps().iter().all(|m| m.iter().all(|&x| x == 0)));
}

#[test]
fn memory_recovery_releases_stale_owners_but_stable_wake_resumes_real_grouping() {
    use crate::system::memory::{Action, Limits, Watchdog};
    let _scope = Scope::new();
    let (mut r, id, sw) = start();
    let counters = drawpipe::counters();
    drop(submit(&mut r, id, &sw, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    r.ui.step(1.0 / 30.0);
    assert_eq!(r.ui.batch_picture.frames, 32);
    let wake = r.ui.render_owner_test_wake();
    drop(submit(&mut r, id, &sw, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    let before = counters.read();
    r.ui.host.runtime_mut().memory = Watchdog::new(Limits::parse("mem.hard_mb = 2\n"));
    r.ui.host.set_text_reader(|_| Some("VmRSS: 3072 kB".into()));
    r.ui.step(1.0 / 30.0);
    assert!(r.ui.memory_stopped());
    assert_eq!(r.ui.batch_picture.frames, 0);
    assert_eq!(counters.read().1, before.1);
    assert_eq!(counters.read().2 - before.2, 32);
    assert!(r.ui.render_owner_debug().contains("context=None"));
    assert!(Arc::ptr_eq(&wake, &r.ui.render_owner_test_wake()));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    assert!(
        r.ui.render_owner_debug().contains("context=None"),
        "inactive intake cannot retain the released source view"
    );
    r.ui.host.set_text_reader(|_| Some("VmRSS: 1024 kB".into()));
    assert_eq!(
        r.ui.host.runtime_mut().memory.sample(std::time::Duration::from_secs(u64::MAX - 1), Some(1 << 20)),
        Action::None
    );
    r.ui.m.run = RunState::Run;
    r.ui.step(1.0 / 30.0);
    assert!(!r.ui.memory_stopped());
    let fresh = change::acquisition::script_prepare_route(&mut r.ui, false);
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    drop(submit(&mut r, fresh, &sw, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    drop(submit(&mut r, fresh, &sw, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    let received = counters.read().1;
    r.ui.step(1.0 / 30.0);
    assert_eq!(counters.read().1 - received, 64);
    assert_eq!(r.ui.batch_picture.frames, 64);
    assert!(Arc::ptr_eq(&wake, &r.ui.render_owner_test_wake()));
}

#[test]
fn uncapped_production_planned_and_selected_cohort_matches_full_worker_pixel_oracle() {
    for tdiv in ["0.000001", "0.00001"] {
        for codes_available in [false, true] {
            let _scope = Scope::new();
            let (mut r, id, sw) = start_at(tdiv.parse().unwrap());
            let (accepted, published) = published_timespec(&r.ui);
            assert_eq!(published, id);
            assert_eq!(accepted, r.ui.m.hw_time_per_div() as f64, "fixture must build the accepted timespec");
            assert_eq!(accepted, tdiv.parse::<f32>().unwrap() as f64);
            let before = drawpipe::counters().read().1;
            let (input, selected, newest_kept, plan) =
                crate::board::dho::scripted::evidence::planned_picture_cohort(&r.ui.m, &sw);
            let records = selected.len() as u64;
            assert!(records > 0);
            if tdiv == "0.000001" {
                assert!(records > 64, "exercise multiple real worker slices, actualplan={plan}");
            }
            println!("UNCAPPED_COHORT tdiv={tdiv} {plan} selected={selected:?} records={records} codes_callback={codes_available} newest_kept={newest_kept} synthetic_headerless_no_fitted_correction_or_hardware_timing_claim");
            let mut expected = Phosphor::new(r.ui.ph.w, r.ui.ph.h);
            drop(submit_cohort(&mut r, id, input, selected, codes_available, newest_kept, Some(&mut expected)));
            wait_for_publication();
            crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
            assert_eq!(drawpipe::counters().read().1, before);
            r.ui.step(1.0 / 30.0);
            assert_eq!(drawpipe::counters().read().1 - before, records);
            assert_eq!(r.ui.batch_picture.frames as u64, records);
            assert_pixels(&r.ui, &expected, id);
        }
    }
}

#[test]
fn owner_groups_minimum_persistence_with_or_without_zoom() {
    let _scope = Scope::new();
    let (mut r, _, _) = start();
    use crate::ui::step_draw::owner::render_owner_active as active;
    assert!(active(&r.ui));
    let saved = r.ui.m.clone();
    r.ui.m.run = RunState::Stop;
    assert!(!active(&r.ui));
    r.ui.m = saved.clone();
    r.ui.m.trig_mode = TrigMode::Single;
    assert!(!active(&r.ui));
    r.ui.m = saved.clone();
    r.ui.m.zoom = true;
    assert!(active(&r.ui));
    r.ui.m.trig_type = TrigType::Pulse;
    assert!(!active(&r.ui), "zoom with a software-refined non-edge trigger uses records");
    r.ui.m = saved.clone();
    r.ui.m.persist = 1;
    assert!(!active(&r.ui));
    r.ui.m = saved;
    crate::ui::change::intake(&mut r.ui, |ui| crate::store::set(&mut ui.m, "tdiv", "0.2").unwrap());
    assert!((r.ui.m.time_per_div() - 0.2).abs() < 1e-6);
    assert!(!active(&r.ui), "ordinary slow/roll path retains its existing route");
}

fn assert_pixels(ui: &Ui, expected: &Phosphor, id: oos_core::change::Identity) {
    for (channel, (actual, wanted)) in ui.ph.maps().iter().zip(expected.maps()).enumerate() {
        assert_eq!(actual.len(), wanted.len());
        if let Some(index) = actual.iter().zip(wanted).position(|(a, b)| a != b) {
            panic!("pixel mismatch channel={channel} index={index} actual={} expected={} size={}x{} visible_tdiv={} accepted_tdiv={} id={id:?} {}",
                actual[index], wanted[index], ui.ph.w, ui.ph.h, ui.m.time_per_div(),
                published_timespec(ui).0, ui.render_owner_debug());
        }
    }
}

#[test]
fn accepted_ten_us_reconfiguration_rejects_late_one_us_cohort_before_first_use() {
    let _scope = Scope::new();
    let (mut r, old, old_switches) = start();
    let late = r.source.next_batch(&r.ui.m, &old_switches, 32, None).unwrap();
    r.ui.step(1.0 / 30.0); // finish the initial accepted publication before the next real input transaction
    crate::ui::change::intake(&mut r.ui, |ui| crate::store::set(&mut ui.m, "tdiv", "0.00001").unwrap());
    r.ui.step(1.0 / 30.0);
    let current = change::acquisition::identity(&r.ui);
    assert_ne!(current.gen(), old.gen());
    assert_eq!(published_timespec(&r.ui).0, 10e-6f32 as f64);
    let shown = drawpipe::counters().read().1;
    drop(submit_cohort(&mut r, old, late, 0..32, false, true, None));
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    r.ui.step(1.0 / 30.0);
    assert_eq!(drawpipe::counters().read().1, shown, "old timespec cannot receive first-use credit");
    assert!(drawpipe::counters().records_text().contains("lost.settings=32"));
    let sw = r.ui.switches().clone();
    let (input, selected, newest, _) = crate::board::dho::scripted::evidence::planned_picture_cohort(&r.ui.m, &sw);
    let records = selected.len() as u64;
    drop(submit_cohort(&mut r, current, input, selected, false, newest, None));
    wait_for_publication();
    crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
    r.ui.step(1.0 / 30.0);
    assert_eq!(drawpipe::counters().read().1 - shown, records);
}

fn published_timespec(ui: &Ui) -> (f64, oos_core::change::Identity) {
    let Source::Hw { shared, .. } = &ui.src else { panic!("hardware fixture") };
    let state = oos_hal::sync::lock(shared);
    (state.want.as_ref().expect("actual acquisition publication").tdiv, state.acquisition.identity())
}

fn submit_probe(
    r: &Route,
    id: oos_core::change::Identity,
    batch: impl Batch<Bytes = ScriptBytes>,
    records: usize,
    seq: &mut u64,
) {
    *seq += records as u64;
    let mut frame = batch.newest(id.epoch());
    (frame.cfg, frame.stamp, frame.seq) = (id.gen(), id.epoch(), *seq);
    drop(submit_readout(r, id, batch, 0..records, true, Some(Arc::new(frame))));
    wait_for_publication();
}

fn composition_probe(channels: usize, tdiv: f32, records: usize) {
    use std::time::Instant;

    let _scope = Scope::new();
    let mut r = Route::new(BatchKind::Codes, channels, tdiv);
    r.ui.m.persist = 0;
    r.ui.m.run = RunState::Run;
    r.ui.m.trig_src = 0;
    let id = change::acquisition::script_prepare_route(&mut r.ui, false);
    let sw = r.ui.switches().clone();
    crate::ui::step_draw::owner::set_render_owner_wake(
        &mut r.ui,
        Arc::new(crate::system::intake::Wake::new().unwrap()),
    );
    let counters = drawpipe::counters();
    let mut step_us = Vec::new();
    let mut seq = 0;
    for sample in 0..109 {
        let input = || crate::board::dho::scripted::evidence::draw_route_batch(&r.ui.m, &sw, records);
        let (first, _, plan) = input();
        let second = (sample % 2 == 0).then(input);
        if sample == 0 {
            println!("COMPOSE_PLAN {plan}");
        }
        crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
        submit_probe(&r, id, first, records, &mut seq);
        crate::ui::step_draw::owner::intake_render_owner(&mut r.ui);
        if let Some((second, _, _)) = second {
            submit_probe(&r, id, second, records, &mut seq);
        }
        let shown = counters.read().1;
        let started = Instant::now();
        r.ui.step(1.0 / 30.0);
        let elapsed = started.elapsed();
        let fresh = if sample % 2 == 0 { 2 * records } else { records };
        assert_eq!(counters.read().1 - shown, fresh as u64);
        if sample >= 8 {
            step_us.push(elapsed.as_micros());
        }
    }
    step_us.sort_unstable();
    println!(
        "COMPOSE_HOST channels={channels} tdiv={tdiv} records_per_batch={records} n={} step_us={}/{}/{}/{}",
        step_us.len(),
        step_us[50],
        step_us[90],
        step_us[99],
        step_us[100]
    );
    println!("{}", crate::pacing::summary());
    println!("{}", counters.pipeline_text(sw.source.work_budget(), crate::pacing::step_missed()));
    println!("{}", change::timing(&r.ui).diagnostics_text());
}

#[test]
#[ignore = "host composition evidence: run separately in release with one test thread"]
fn composition_probe_four_channels_one_us() {
    composition_probe(4, 1e-6, 32);
}

#[test]
#[ignore = "host composition evidence: run separately in release with one test thread"]
fn composition_probe_one_channel_ten_us() {
    composition_probe(1, 10e-6, 26);
}
