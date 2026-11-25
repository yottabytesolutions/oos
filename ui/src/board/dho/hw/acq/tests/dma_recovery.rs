// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::dho::{afg, seq::time::ExecuteError};

#[test]
fn stop_and_side_service_proceed_during_recovery_then_latest_settings_are_applied() {
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
    let mut recovery_waits = 0;
    io.clock.drive(move |clock, pause| match pause {
        Pause::Wait(delay) if delay == Duration::from_millis(20) => {
            recovery_waits += 1;
            if recovery_waits == 1 {
                clock.stop();
                lock(&shared).afg[0] = Some(generator);
                lock(&shared).want = Some(changed.clone());
                clock.mark(Event::SettingsPublished);
            } else if recovery_waits == 2 {
                assert_eq!(lock(&shared).final_ack, 7);
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
    let configure = events.iter().position(|event| *event == Event::SettingsApplied).unwrap();
    assert!(stop < side && side < configure);
    assert!(!events.iter().any(is_arm));
    assert_eq!(outcome.final_ack, 7);
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
