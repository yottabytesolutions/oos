// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::dho::{afg, seq::time::ExecuteError};

#[test]
fn stop_and_generator_proceed_during_recovery_then_calibration_and_settings_are_applied() {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let (mut source, started) = source(&out, &io);
    let request = Request::read(&lock(&out));
    source.reconfigure(&mut io, &request, &mut false).unwrap();
    io.clock.suppress_stopped_status();
    io.report_read_error(&out, "batch", ExecuteError::DmaTimeout("simulated timeout".into()));
    let changed = Settings { tdiv: want.tdiv * 2.0, ..want };
    let generator = (true, 0.001, 1.0, 0.0, afg::Wave::Sine, afg::Mod::None);
    let shared = out.clone();
    let mut backoff_service = false;
    io.clock.drive(move |clock, pause| match pause {
        Pause::Wait(delay) if delay == Duration::from_millis(20) => {
            if !lock(&shared).hold {
                clock.stop();
                lock(&shared).cal_req = Some((vec![0], true, false));
                return;
            }
            let mut g = lock(&shared);
            assert_eq!(g.final_ack, 7);
            assert_eq!(g.cal_req, Some((vec![0], true, false)));
            if g.status.dma_backoff().is_some() && !backoff_service {
                g.afg[0] = Some(generator);
                g.want = Some(changed.clone());
                drop(g);
                let source = crate::wave::Source::Hw { shared: shared.clone(), seen: 0 };
                assert!(crate::board::board().capture_state(&source).contains(" acq.dma.backoff_age_ms="));
                backoff_service = true;
                clock.mark(Event::SettingsPublished);
                clock.restore_stopped_status();
            }
        }
        Pause::Wait(HELD_POLL) => {
            let mut g = lock(&shared);
            assert_eq!(g.status.diagnostic(g.acquisition), Some(CaptureStatus::DmaStopDeadline));
            g.record_flow.shutdown = true;
        }
        Pause::Wait(_) => {}
        Pause::Park => panic!("a held source must not attempt another read"),
    });
    source.run(&mut io);
    let outcome = snapshot(&out, &io, &trace, started);
    let events = outcome.events;
    let stop = events.iter().position(|event| *event == Event::Stop).unwrap();
    let side = events.iter().position(|event| *event == Event::Generator(Some(generator))).unwrap();
    let calibration = events.iter().position(|event| matches!(event, Event::Calibration(_))).unwrap();
    let configure = events.iter().position(|event| *event == Event::SettingsApplied).unwrap();
    assert!(stop < side && side < calibration && calibration < configure);
    assert!(events[side..calibration].iter().any(|event| matches!(
        event,
        Event::Read(0x4034, flags) if flags & oos_hal::dho::scu::status::STOPPED != 0
    )));
    assert!(!events.iter().any(is_arm));
    assert!(!events.iter().any(|event| matches!(event, Event::Dma(_))));
    assert_eq!(outcome.final_ack, 7);
}

#[test]
fn segmented_timestamp_timeout_enters_recovery() {
    let _scope = Scope::new();
    let want = Settings { segments: Some(seq::Segments { n: 4, interval: 0.0 }), ..settings() };
    let out = requested(&want, Identity::default(), false);
    lock(&out).seg.running = true;
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let (mut source, _) = source(&out, &io);
    let request = Request::read(&lock(&out));
    let (want, params) = source.reconfigure(&mut io, &request, &mut false).unwrap();
    source.segments(&mut io, &want, &params, request.identity);
    io.clock.record_triggers(4);
    source.segments(&mut io, &want, &params, request.identity);
    io.c2h.timeout_next();
    source.segments(&mut io, &want, &params, request.identity);

    assert!(trace.borrow().events.contains(&Event::DmaTimeout(Duration::from_secs(1))));
    assert!(io.dma_fault.pending());
    assert!(lock(&out).status.latest().starts_with("segments: c2h DMA timeout"));
    assert_eq!(lock(&out).seg.phase, crate::board::SegPhase::Idle);
}

#[test]
fn a_non_timeout_dma_read_error_counts_without_starting_dma_recovery() {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let (mut io, _) = sim::fixture(&want, out.clone(), (Run::Live(1), 4, false), None);
    let (mut source, _) = source(&out, &io);
    io.c2h.fail_next();
    source.run(&mut io);

    let counters = drawpipe::counters().acquisition.text();
    assert!(counters.split_whitespace().any(|field| field == "acq_batch_dma_failures=1"));
    assert!(!counters.contains("acq_dma_timeouts="));
    assert_eq!(lock(&out).decoded, 4);
}
