// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;

mod peak;

fn hardware() -> (Ui, crate::board::HwShared) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.zoom = true; // Exercise the full record and measurement route.
    ui.step(0.04);
    (ui, shared)
}

fn frame(ui: &Ui, seq: u64) -> crate::board::Frame {
    let id = ui.host.runtime().change.last.as_ref().unwrap().identity();
    let ch: [Vec<f32>; NCH] = std::array::from_fn(|c| if c == 0 { vec![1.0; 64] } else { Vec::new() });
    // the acquisition thread fills the six full-frame readings of every frame (PR #8, `BasicStats`)
    let basic = std::array::from_fn(|c| crate::board::BasicStats::of(&ch[c]));
    crate::board::Frame {
        seq,
        cfg: id.gen(),
        stamp: id.epoch(),
        dt: 1e-6,
        pre: 32,
        ch,
        basic: basic.into(),
        ..Default::default()
    }
}

#[test]
fn old_pair_records_are_not_measured() {
    for epoch_only in [false, true] {
        let (mut ui, shared) = hardware();
        let old = frame(&ui, 1);
        if epoch_only {
            ui.clear_display();
        } else {
            ui.m.tscale += 1;
        }
        ui.step(0.04);
        oos_hal::sync::lock(&shared).queue.push_back(old.into());
        let records = ui.records;
        ui.step(0.04);
        assert_eq!(ui.records, records, "old pair reached record/measurement path: epoch_only={epoch_only}");
        assert!(ui.last[0].is_none());
        assert_eq!(ui.stats[0].vavg, 0.0);
        let valid = frame(&ui, 2);
        oos_hal::sync::lock(&shared).queue.push_back(valid.into());
        ui.step(0.04);
        assert_eq!(ui.records, records + 1, "valid current-pair record was dropped");
        assert!(ui.stats[0].vavg > 0.0);
    }
}

#[test]
fn autoset_accepts_only_resolved_epoch() {
    stage_tokens(oos_core::change::Requester::Autoset);
}
#[test]
fn bode_accepts_only_resolved_epoch() {
    stage_tokens(oos_core::change::Requester::Bode);
}

fn stage_tokens(requester: oos_core::change::Requester) {
    let (mut ui, _) = hardware();
    let mut old = crate::wave::hw_record(&ui.m, std::sync::Arc::new(frame(&ui, 1)));
    for _ in 0..2 {
        match requester {
            oos_core::change::Requester::Autoset => change::autoset_begin(&mut ui),
            oos_core::change::Requester::Bode => {
                change::bode_begin(&mut ui);
                change::bode_epoch(&mut ui);
            }
        }
        assert!(!change::acquisition::stage_record(&ui, requester, &old), "unresolved accepts nothing");
        ui.step(0.04);
        assert!(!change::acquisition::stage_record(&ui, requester, &old), "previous run/stage pair");
        let current = frame(&ui, 2);
        let record = crate::wave::hw_record(&ui.m, std::sync::Arc::new(current.clone()));
        assert!(change::acquisition::stage_record(&ui, requester, &record));
        old = record;
        let newer = crate::wave::hw_record(
            &ui.m,
            std::sync::Arc::new(crate::board::Frame { stamp: current.stamp + 1, ..current }),
        );
        assert!(
            !change::acquisition::stage_record(&ui, requester, &newer),
            "a different newer epoch is not this token"
        );
    }
}

#[test]
fn stage_settings_and_token_publish_together_on_the_next_step() {
    use oos_core::change::Requester;
    let (mut ui, shared) = hardware();
    let scale = ui.m.tscale;
    change::begin_step(&mut ui, 0.0).unwrap();
    ui.m.tscale += 1;
    change::stage(&mut ui, Requester::Bode);
    assert_eq!(ui.m.tscale, scale, "late stage settings wait for their token");
    assert!(!change::acquisition::stage_record(
        &ui,
        Requester::Bode,
        &crate::wave::hw_record(&ui.m, std::sync::Arc::new(frame(&ui, 1)))
    ));
    change::finish_step(&mut ui);
    crate::cmd::run(&mut ui, "knob hscale +1");
    let expected = ui.m.tscale;
    ui.step(0.04);
    assert_eq!(ui.m.tscale, expected, "external input follows the internal stage commands");
    let g = oos_hal::sync::lock(&shared);
    assert_eq!(g.want, Some(board().want(&ui.m, ui.switches())));
    assert_eq!(g.acquisition.identity(), change::acquisition::identity(&ui));
    assert_eq!(g.acquisition.route(), ui.fast_ok());
}

#[test]
fn delayed_measurements_require_gen_epoch_and_stats_gen() {
    for boundary in 0..3 {
        let (mut ui, _) = hardware();
        let id = change::acquisition::identity(&ui);
        let (done, rx) = std::sync::mpsc::channel();
        let (tx, _jobs) = std::sync::mpsc::channel();
        let (_analysis, analysis_rx) = std::sync::mpsc::channel();
        ui.meas_bg = Some(MeasWorker {
            tx,
            rx: rx.into(),
            analysis_rx: analysis_rx.into(),
            analysis_busy: false,
            busy: true,
            accept: true,
            tap: None,
            join: std::thread::spawn(|| {}),
        });
        match boundary {
            0 => ui.m.tscale += 1,
            1 => change::request(&mut ui, oos_core::change::Request::Clear, change::oracle::Row::Clear),
            _ => change::stats_reset(&mut ui),
        }
        ui.step(0.04);
        done.send((id, vec![(0, Stats { vavg: 123.0, ..Default::default() }, Default::default())])).unwrap();
        ui.meas_poll();
        assert_ne!(ui.stats[0].vavg, 123.0, "late result from boundary {boundary}");
        let current = change::acquisition::identity(&ui);
        ui.meas_bg.as_mut().unwrap().accept = true;
        done.send((current, vec![(0, Stats { vavg: 7.0, ..Default::default() }, Default::default())])).unwrap();
        ui.meas_poll();
        assert_eq!(ui.stats[0].vavg, 7.0, "current results remain valid");
    }
}

#[test]
fn rejected_resolution_keeps_the_previous_publication() {
    let (mut ui, shared) = hardware();
    let before = oos_hal::sync::lock(&shared).want.clone();
    let id = oos_hal::sync::lock(&shared).acquisition.identity();
    ui.m.tscale += 1;
    assert_eq!(change::duplicate_requests(&mut ui), 2);
    let requested_scale = ui.m.tscale;
    ui.step(0.04);
    assert_eq!(
        ui.m.tscale, requested_scale,
        "a rejected preparation cannot recapture external edits as internal commands"
    );
    let g = oos_hal::sync::lock(&shared);
    assert_eq!(g.want, before, "failed resolution cannot publish new settings under the previous pair");
    assert_eq!(g.acquisition.identity(), id);
    assert_eq!(ui.records, 0);
    drop(g);
    assert_eq!(change::pending_count(&ui), 0, "failed requests must be quarantined");
    ui.step(0.04);
    assert_ne!(oos_hal::sync::lock(&shared).acquisition.identity(), id, "next step recovers");
    assert!(change::text(&ui).contains("DuplicateToken=1"));
}

#[test]
fn oversized_model_keeps_stepping_and_recovers_without_replaying_requests() {
    let (mut ui, shared) = hardware();
    let id = change::acquisition::identity(&ui);
    ui.m.bode_addr = "x".repeat(70 * 1024);
    ui.clear_display();
    let before = ui.now;
    for _ in 0..3 {
        ui.step(0.04);
    }
    assert!(ui.now > before + 0.1);
    assert_eq!(oos_hal::sync::lock(&shared).acquisition.identity(), id);
    assert_eq!(change::pending_count(&ui), 0);
    assert!(change::text(&ui).contains("SnapshotTooLarge="));
    ui.m.bode_addr = String::new();
    ui.m.tscale += 1;
    ui.step(0.04);
    let next = oos_hal::sync::lock(&shared).acquisition.identity();
    assert!(next.gen() > id.gen());
    assert_eq!(next.epoch(), id.epoch(), "quarantined CLEAR must not replay");
}

#[test]
fn synchronous_measurement_rejection_does_not_feed_statistics() {
    let (mut ui, _) = hardware();
    let old = crate::wave::hw_record(&ui.m, std::sync::Arc::new(frame(&ui, 1)));
    ui.clear_display();
    ui.step(0.04);
    ui.meas_new = false;
    assert!(!ui.measure_rec(&old, usize::MAX));
    assert!(!ui.meas_new);
}

#[test]
fn foreign_clear_or_generation_cannot_stall_a_stepped_bode_stage() {
    for clear in [false, true] {
        let (mut ui, shared) = hardware();
        ui.bode.running = true;
        ui.bode.freqs = vec![1000.0];
        ui.m.bode_in = WaveId::Ch(0);
        ui.m.bode_out = WaveId::Ch(1);
        change::bode_begin(&mut ui);
        change::bode_epoch(&mut ui);
        ui.step(0.04);
        let old = change::acquisition::identity(&ui);
        if clear {
            ui.clear_display();
        } else {
            ui.m.tscale += 1;
        }
        ui.step(0.04);
        assert!(change::proposals::resolved(&ui, oos_core::change::Requester::Bode).is_none());
        ui.step(0.04);
        let id = change::acquisition::identity(&ui);
        assert_ne!(id, old);
        assert_eq!(change::proposals::resolved(&ui, oos_core::change::Requester::Bode), Some(id));
        let started = ui.now;
        for seq in 1..=3 {
            let mut f = frame(&ui, seq);
            f.ch[0] = (0..1000).map(|k| (std::f32::consts::TAU * k as f32 / 1000.0).sin()).collect();
            f.ch[1] = f.ch[0].iter().map(|v| v * 0.5).collect();
            oos_hal::sync::lock(&shared).queue.push_back(f.into());
            ui.step(0.04);
        }
        assert_eq!(ui.bode.points.len(), 1, "restarted stage finishes after its two settling records");
        assert!(!ui.bode.running);
        assert!(ui.now - started < 0.13);
        assert!(change::text(&ui).contains("stage_restart_bode=1"));
    }
}

#[test]
fn shadow_initialization_failure_keeps_services_stepping_and_can_recover() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.m.bode_addr = "x".repeat(70 * 1024);
    change::reset(&mut ui);
    ui.was_running = true;
    let before = ui.now;
    ui.step(0.04);
    assert!(ui.now > before);
    assert!(ui.last[0].is_some(), "acquisition loop still runs");
    assert_eq!(change::pending_count(&ui), 0);
    ui.m.bode_addr = String::new();
    ui.step(0.04);
    assert!(ui.host.runtime().change.last.is_some());
}

#[test]
fn publication_and_queue_pop_report_stale_frames_at_their_own_sites() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    use std::sync::atomic::Ordering::Relaxed;
    let (mut ui, shared) = hardware();
    let loss = &crate::drawpipe::counters().losses;
    let before = (loss.ui_publish.load(Relaxed), loss.queue_pop.load(Relaxed), loss.overflow.load(Relaxed));
    let old = frame(&ui, 1);
    oos_hal::sync::lock(&shared).queue.push_back(old.clone().into());
    ui.clear_display();
    ui.step(0.04);
    assert_eq!(loss.ui_publish.load(Relaxed) - before.0, 1);
    oos_hal::sync::lock(&shared).queue.push_back(old.clone().into());
    ui.src.drain_newest(crate::board::QUEUE_MAX);
    oos_hal::sync::lock(&shared).queue.push_back(old.into());
    assert!(ui.src.acquire(&ui.m, 0.04, &mut ui.rng, ui.host.switches()).is_none());
    assert_eq!(loss.queue_pop.load(Relaxed) - before.1, 2);
    assert_eq!(loss.overflow.load(Relaxed), before.2);
    let text = crate::cmd::stat_text(&ui);
    for name in
        ["stale_worker_push=", "stale_ui_publish=", "stale_queue_pop=", "stale_pipe=", "queue_overflow=", "fold_drops="]
    {
        assert!(text.contains(name), "state query missing {name}");
    }
}

#[test]
fn state_query_reports_acquisition_counters_beside_stale_counts() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, _) = hardware();
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    let reply = remote.exec_line(&mut ui, &crate::gfx::Canvas::new(1, 1), ":SYSTem:STATe?");
    let text = String::from_utf8(reply).unwrap();
    let fields = " acq_seg_start=0 acq_seg_abort=0 acq_roll_stop=0 acq_roll_start=0 acq_batch_prearm=0 acq_batch_disarm=0 acq_configure=0 acq_single_arm=0";
    assert!(text.contains(&format!("{}{}", crate::drawpipe::counters().losses.text(), fields)));
    assert_eq!(text.matches("acq_single_arm=").count(), 1);
}

#[test]
fn mask_source_publication_discards_queued_old_generation_without_dropping_current_records() {
    use std::sync::atomic::Ordering::Relaxed;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = hardware();
    ui.m.ch[1].on = true;
    ui.step(0.04);
    let old = frame(&ui, 1);
    let id = change::acquisition::identity(&ui);
    let loss = &crate::drawpipe::counters().losses;
    oos_hal::sync::lock(&shared).queue.extend(std::iter::repeat_n(std::sync::Arc::new(old.clone()), 96));
    ui.m.mask_src = 1;
    let records = ui.records;
    ui.step(0.04);
    let next = change::acquisition::identity(&ui);
    assert_eq!(next.gen(), id.gen() + 1);
    assert_eq!(next.epoch(), id.epoch());
    assert_eq!(loss.ui_publish.load(Relaxed), 96);
    assert_eq!(loss.queue_pop.load(Relaxed), 0);
    assert_eq!(loss.overflow.load(Relaxed), 0);
    assert_eq!(ui.records, records, "old mask-source captures cannot feed readers");
    for seq in 2..=4 {
        oos_hal::sync::lock(&shared).queue.push_back(frame(&ui, seq).into());
        ui.step(0.04);
        assert_eq!(ui.records, records + seq - 1, "current-pair work must remain eligible");
        assert_eq!(loss.ui_publish.load(Relaxed), 96, "unchanged source must not discard records");
    }
}
