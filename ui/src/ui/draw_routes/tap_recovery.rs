// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use std::time::Duration;

const STEP: f64 = 1.0 / 30.0;

#[test]
fn watchdog_step_frees_the_window_and_restart_retains_only_new_deposits() {
    use crate::system::memory::{Action, Limits, Watchdog};

    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.step(1);
    let pipe = drawpipe::pipe().unwrap();
    step_draw::reset_window(&mut route.ui);
    assert!(pipe.take_recycled_deposit().is_some(), "normal clearing keeps a reusable deposit");
    route.step(1);
    assert_eq!(route.ui.batch_picture.frames, 1);
    let records = route.ui.records;
    let bytes = HEAP.own_live();
    let backing = route.ui.ph.w * route.ui.ph.h * size_of::<u16>();
    route.ui.host.runtime_mut().memory = Watchdog::new(Limits::parse("mem.hard_mb = 2\n"));
    route.ui.host.set_text_reader(|_| Some("VmRSS: 3072 kB".into()));

    route.ui.step(STEP);

    assert!(route.ui.memory_stopped() && route.ui.history.is_empty());
    assert_eq!((route.ui.records, route.ui.batch_picture.frames), (records, 0));
    assert!(bytes - HEAP.own_live() >= backing as isize, "the step freed the difference plane backing");
    assert!(pipe.take_recycled_deposit().is_none(), "the live pipe did not inherit the window");
    route.ui.host.set_text_reader(|_| Some("VmRSS: 1024 kB".into()));
    let recovered = Duration::from_secs(u64::MAX - 1);
    assert_eq!(route.ui.host.runtime_mut().memory.sample(recovered, Some(1 << 20)), Action::None);
    route.ui.step(STEP);
    assert!(route.ui.memory_stopped() && pipe.take_recycled_deposit().is_none());
    assert_eq!((route.ui.records, route.ui.batch_picture.frames), (records, 0));

    route.ui.m.run = RunState::Run;
    route.ui.step(STEP);
    assert!(!route.ui.memory_stopped());
    route.step(1);
    assert_eq!((route.ui.records, route.ui.batch_picture.frames, route.ui.history.len()), (records + 1, 1, 1));
    assert!(route.ui.ph.maps()[0].iter().any(|&v| v > 0));
}

fn submit(route: &mut Route, seq: u64, samples: Vec<f32>) -> Arc<Frame> {
    submit_charged(route, seq, samples, |_| {}).0
}

fn submit_charged(
    route: &mut Route,
    seq: u64,
    samples: Vec<f32>,
    configure: impl FnOnce(&mut Frame),
) -> (Arc<Frame>, std::sync::Weak<drawpipe::TapReservation>) {
    let id = change::acquisition::script_prepare(&mut route.ui);
    let sw = route.ui.switches().clone();
    let pipe = drawpipe::start_pipe(&sw);
    let batch = route.source.next_batch(&route.ui.m, &sw, 1, None).unwrap();
    let mut frame = batch.newest(id.epoch());
    (frame.cfg, frame.seq) = (id.gen(), seq);
    frame.pre = samples.len() / 2;
    frame.ch[0] = samples;
    configure(&mut frame);
    let frame = Arc::new(frame);
    let charge = pipe.reserve_waveform(&frame, id).unwrap();
    let receipt = Arc::downgrade(&charge);
    let (fb, decode) = (batch.frame_bytes(), batch.decode());
    let at = route.ui.host.picture_time(route.ui.now);
    let decode = Arc::new(move |_: usize, data: &[u8], frame: &mut Frame| decode(data, frame));
    assert!(pipe
        .feed_readout(
            (batch.into_bytes().into(), fb, 0..1),
            (id, at),
            decode,
            None,
            (Some(frame.clone()), Some(charge))
        )
        .is_none());
    drawpipe::host::finish_batch(pipe, seq).unwrap();
    (frame, receipt)
}

fn fallback(ui: &Ui, template: &Frame, seq: u64, value: f32) -> Arc<Frame> {
    let Source::Hw { shared, .. } = &ui.src else { unreachable!() };
    let mut frame = template.clone();
    frame.seq = seq;
    frame.ch[0].fill(value);
    let frame = Arc::new(frame);
    let mut shared = lock(shared);
    shared.frame = frame.clone();
    shared.queue.push_back(frame.clone());
    frame
}

fn last_seq(ui: &Ui) -> u64 {
    ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq
}

fn assert_mean_stats(ui: &Ui, n: u64, sum: f64, squares: f64, min: f64, max: f64) {
    let stat = ui.mstat[0];
    assert_eq!(stat.n, n);
    for (got, want) in [(stat.sum, sum), (stat.sumsq, squares), (stat.min, min), (stat.max, max)] {
        assert!((got - want).abs() < 1e-5, "{got} versus {want}");
    }
}

#[cfg(feature = "private-picture-counts")]
fn assert_observer_redraw_accounting(before: &str, after: &str) {
    fn metric(text: &str, key: &str) -> u64 {
        let prefix = format!("{key}=");
        let values: Vec<_> = text.split_whitespace().filter_map(|v| v.strip_prefix(&prefix)).collect();
        assert_eq!(values.len(), 1, "one exact observer field {key}");
        values[0].parse().expect("observer integer field")
    }
    fn stable(text: &str) -> Vec<&str> {
        text.split_whitespace()
            .filter(|v| !v.starts_with("ns=") && !v.starts_with("eligible=") && !v.starts_with("empty="))
            .collect()
    }
    assert!(before.contains("fresh.v1") && after.contains("fresh.v1"));
    let _elapsed = (metric(before, "ns"), metric(after, "ns"));
    assert_eq!(metric(after, "fresh"), metric(before, "fresh"));
    assert_eq!(metric(after, "first_records"), metric(before, "first_records"));
    assert_eq!(metric(after, "eligible") - metric(before, "eligible"), 2);
    assert_eq!(metric(after, "empty") - metric(before, "empty"), 2);
    assert_eq!(stable(after), stable(before), "all ordinary counts, losses and latency bins remain unchanged");
}

#[test]
fn accepted_tap_redraws_once_after_remap() {
    for queued in [true, false] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
        route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
        let tap = submit(&mut route, 2, vec![0.75; 12_512]);
        if queued {
            fallback(&route.ui, &tap, 1, 0.25);
        }
        route.ui.step(STEP);
        let pipe = drawpipe::pipe().unwrap();
        assert_eq!(pipe.tap_count(), usize::from(queued));
        assert_eq!(last_seq(&route.ui), if queued { 1 } else { 2 });
        assert_eq!(route.ui.wfm_total, 1 + u64::from(queued));
        assert_eq!(route.ui.history.len(), 1);
        let counters = drawpipe::counters();
        let accounting = (counters.read(), counters.records_text());
        let edges = route.ui.fast_edges;

        route.ui.m.dots = DotsMode::On;
        route.ui.step(STEP);

        let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
        drawpipe::draw_frame(&mut expected, &route.ui.m, &tap, crate::sparse::Interp::Exact, None);
        assert!(expected.maps()[0].iter().any(|&v| v > 0));
        assert!(route.ui.ph.maps() == expected.maps(), "one copy of the remapped waveform, queued={queued}");
        assert_eq!(route.ui.batch_picture.frames, 1);
        assert_eq!(last_seq(&route.ui), 2);
        let history: Vec<_> = route.ui.history.iter().map(|r| r.ch[0][r.ch[0].len() / 2]).collect();
        assert_eq!(history, if queued { vec![0.25, 0.75] } else { vec![0.75] });
        assert_eq!(route.ui.records, 1 + u64::from(queued));
        route.ui.step(STEP);
        assert!(route.ui.ph.maps() == expected.maps());
        assert_eq!(route.ui.wfm_total, 1 + u64::from(queued));
        assert_eq!(route.ui.fast_edges, edges);
        #[cfg(not(feature = "private-picture-counts"))]
        assert_eq!((counters.read(), counters.records_text()), accounting);
        #[cfg(feature = "private-picture-counts")]
        {
            assert_eq!(counters.read(), accounting.0);
            assert_observer_redraw_accounting(&accounting.1, &counters.records_text());
        }
        if queued {
            assert_mean_stats(&route.ui, 2, 1.0, 0.625, 0.25, 0.75);
        } else {
            assert_mean_stats(&route.ui, 1, 0.75, 0.5625, 0.75, 0.75);
        }
    }
}

#[test]
fn pending_tap_remaps_before_its_deposit_is_accepted() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    submit(&mut route, 1, vec![0.25; 12_512]);
    let tap = submit(&mut route, 2, vec![0.75; 12_512]);
    route.ui.step(STEP);
    assert_eq!((last_seq(&route.ui), route.ui.wfm_total, route.ui.history.len()), (2, 1, 2));
    let pipe = drawpipe::pipe().unwrap();
    drawpipe::host::barrier(pipe);

    route.ui.m.dots = DotsMode::On;
    route.ui.step(STEP);

    let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    drawpipe::draw_frame(&mut expected, &route.ui.m, &tap, crate::sparse::Interp::Exact, None);
    assert!(expected.maps()[0].iter().any(|&v| v > 0));
    assert!(route.ui.ph.maps() == expected.maps());
    assert!(drawpipe::counters().records_text().contains("rejected.stale_key=1 "));
    route.ui.step(STEP);
    assert!(route.ui.ph.maps() == expected.maps());
    assert_eq!((last_seq(&route.ui), route.ui.wfm_total, route.ui.history.len()), (2, 2, 2));
    assert_mean_stats(&route.ui, 2, 1.0, 0.625, 0.25, 0.75);
}

#[test]
fn reader_merge_counts_taps_and_keeps_the_newer_fallback_preview() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    let initial = route.ui.ph.upload_rows(0, None).0;
    let mut stamps = [initial.clone(), initial];
    let mut maps = [route.ui.ph.maps()[0].clone(), route.ui.ph.maps()[0].clone()];
    let mut publish = |ph: &Phosphor, consumer: usize| {
        assert!(!ph.is_wide(), "this reader fixture exercises narrow row revisions");
        let (stamp, rows) = ph.upload_rows(0, Some(&stamps[consumer]));
        maps[consumer].resize(ph.maps()[0].len(), 0);
        let pixels = rows.start * ph.w..rows.end * ph.w;
        maps[consumer][pixels.clone()].copy_from_slice(&ph.maps()[0][pixels]);
        stamps[consumer] = stamp;
        assert!(maps[consumer] == ph.maps()[0], "consumer {consumer} differs from the picture");
        rows
    };
    let first = submit(&mut route, 1, vec![0.25; 12_512]);
    let second = submit(&mut route, 2, vec![0.5; 12_512]);
    let pipe = drawpipe::pipe().unwrap();
    route.ui.step(STEP);
    assert_eq!(last_seq(&route.ui), second.seq);
    assert_eq!(route.ui.wfm_total, 1, "tap 1 is deposited while tap 2's deposit is still private");
    assert_mean_stats(&route.ui, 2, 0.75, 0.3125, 0.25, 0.5);
    assert!(!publish(&route.ui.ph, 0).is_empty());
    let third = fallback(&route.ui, &first, 3, 0.75);
    route.ui.m.dots = DotsMode::On;
    route.ui.step(STEP);
    assert_eq!(last_seq(&route.ui), third.seq);
    assert_mean_stats(&route.ui, 3, 1.5, 0.875, 0.25, 0.75);
    let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    drawpipe::draw_frame(&mut expected, &route.ui.m, &third, crate::sparse::Interp::Exact, None);
    assert_eq!(route.ui.ph.maps(), expected.maps(), "remapping does not replay a superseded tap");
    for consumer in 0..2 {
        assert!(!publish(&route.ui.ph, consumer).is_empty(), "remapping reaches skipped consumers");
        assert!(publish(&route.ui.ph, consumer).is_empty(), "unchanged revisions need no upload");
    }
    assert!(matches!(route.ui.src, Source::Hw { seen: 3, .. }));
    let history: Vec<_> = route.ui.history.iter().map(|r| r.ch[0][r.ch[0].len() / 2]).collect();
    assert_eq!(history, [0.25, 0.5, 0.75]);
    drawpipe::host::barrier(pipe);
}

#[test]
fn drawable_fallback_after_a_pending_tap_does_not_replace_its_statistics() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    let first = submit(&mut route, 1, vec![0.25; 12_512]);
    let second = submit(&mut route, 2, vec![0.5; 12_512]);
    submit(&mut route, 4, vec![1.0; 12_512]);
    fallback(&route.ui, &first, 3, 0.75);
    route.ui.step(STEP);
    assert_eq!(last_seq(&route.ui), 3);
    assert_mean_stats(&route.ui, 3, 1.5, 0.875, 0.25, 0.75);
    assert!((route.ui.ana_get(0, Meas::Vavg) - 0.75).abs() < 1e-5);
    let pipe = drawpipe::pipe().unwrap();
    drawpipe::host::barrier(pipe);
    route.ui.step(STEP);
    assert!(pipe.take_preview(&second, (route.ui.ph.w - 1, route.ui.ph.h)).is_none());
    assert_eq!(last_seq(&route.ui), 4);
    assert_mean_stats(&route.ui, 4, 2.5, 1.875, 0.25, 1.0);
    route.ui.step(STEP);
    assert_mean_stats(&route.ui, 4, 2.5, 1.875, 0.25, 1.0);
}

#[test]
fn single_tap_folds_now_and_expensive_measurement_completes_once() {
    for background in [false, true] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, 1, 50e-6);
        route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0)), (Meas::Freq, WaveId::Ch(0))];
        route.ui.m.meas_full_record = true;
        if background {
            route.ui.measure_in_background();
        }
        let samples = (0..300_000).map(|i| if i % 100 < 50 { 0.0 } else { 0.5 }).collect();
        let newest = submit(&mut route, 1, samples);
        route.ui.step(STEP);
        assert_mean_stats(&route.ui, 1, 0.25, 0.0625, 0.25, 0.25);
        assert_eq!(last_seq(&route.ui), 1);
        let expected = 1.0 / (100.0 * newest.dt);
        let wait = lifecycle::test_wait::Wait::new("tap measurement statistics");
        while route.ui.mstat.get(1).is_none_or(|stat| stat.n == 0) {
            wait.ui(&route.ui);
            route.ui.step(STEP);
        }
        assert!((route.ui.meas(0, Meas::Freq) - expected).abs() < expected * 1e-6);
        assert!((route.ui.mstat[1].sum - expected).abs() < expected * 1e-6);
        route.ui.wait_background();
        route.ui.step(STEP);
        assert_eq!(route.ui.mstat[1].n, 1);
        assert_mean_stats(&route.ui, 1, 0.25, 0.0625, 0.25, 0.25);
    }
}

#[test]
fn tap_statistics_keep_both_gates_without_scanning_on_the_reader() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    route.ui.m.meas_full_record = true;
    route.ui.measure_in_background();
    let mut samples = vec![0.75; 200_000];
    samples[80_000..120_000].fill(0.25);
    let configure = |f: &mut Frame| {
        f.fixed_ti = Some(f.pre as f64);
        f.dt = 1e-9;
    };
    submit_charged(&mut route, 1, samples.clone(), configure);
    submit_charged(&mut route, 2, samples, configure);

    let before = crate::framestats::SAMPLE_SCANS.with(std::cell::Cell::get);
    route.ui.step(STEP);
    assert_eq!(crate::framestats::SAMPLE_SCANS.with(std::cell::Cell::get), before);
    assert_eq!(last_seq(&route.ui), 2);
    assert_mean_stats(&route.ui, 2, 0.9, 0.485, 0.25, 0.65);
}

#[test]
fn stale_tap_attempt_releases_its_charge_without_counting_a_second_loss() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    let (old, receipt) = submit_charged(&mut route, 1, vec![0.25; 12_512], |_| {});
    assert_eq!(receipt.strong_count(), 1, "the pending reader owns the charge after DMA return");
    change::begin_step(&mut route.ui, 0.0).unwrap();
    change::finish_step(&mut route.ui);
    route.ui.clear_display();
    let current = submit(&mut route, 2, vec![0.5; 12_512]);
    assert_ne!((old.cfg, old.stamp), (current.cfg, current.stamp));
    route.ui.step(STEP);
    assert_eq!(receipt.strong_count(), 0);
    assert_eq!(last_seq(&route.ui), 2);
    assert_eq!(route.ui.history.len(), 1);
    assert_eq!(drawpipe::counters().read().2, 1, "only the old deposit counts as a picture loss");
    assert_mean_stats(&route.ui, 1, 0.5, 0.25, 0.5, 0.5);
}

#[test]
fn old_tap_worker_completion_cannot_count_the_new_fallback_twice() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    route.ui.measure_in_background();
    let tap = submit(&mut route, 1, vec![0.25; 12_512]);
    route.ui.step(STEP);
    assert_mean_stats(&route.ui, 1, 0.25, 0.0625, 0.25, 0.25);
    route.ui.wait_background();
    fallback(&route.ui, &tap, 2, 0.75);
    route.ui.step(STEP);
    assert_eq!(last_seq(&route.ui), 2);
    route.ui.wait_background();
    route.ui.step(STEP);
    assert_mean_stats(&route.ui, 2, 1.0, 0.625, 0.25, 0.75);
}

#[test]
fn held_deep_measurement_uses_the_raw_taps_statistics_once() {
    for background in [false, true] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, 1, 1e-6);
        route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
        if background {
            route.ui.measure_in_background();
        }
        let Source::Hw { shared, .. } = &route.ui.src else { unreachable!() };
        lock(shared).deep_ok = true;
        let depth = 10_000_000;
        let raw = submit_charged(&mut route, 1, vec![0.25; 12_512], |f| f.acq.1 = depth).0;
        submit_charged(&mut route, 3, vec![0.75; 12_512], |f| {
            f.acq.1 = depth;
            f.envelope = true;
        });
        let paired = Frame { envelope: true, ..(*raw).clone() };
        fallback(&route.ui, &paired, 2, 0.75);
        route.ui.step(STEP);
        assert_eq!(last_seq(&route.ui), 2);
        assert_mean_stats(&route.ui, 1, 0.25, 0.0625, 0.25, 0.25);
        route.ui.wait_background();
        route.ui.step(STEP);
        assert_eq!(last_seq(&route.ui), 3);
        assert!((route.ui.meas(0, Meas::Vavg) - 0.25).abs() < 1e-5);
        assert_mean_stats(&route.ui, 1, 0.25, 0.0625, 0.25, 0.25);
    }
}
