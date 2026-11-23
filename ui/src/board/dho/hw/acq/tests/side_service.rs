// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{CounterWant, DvmWant};
use oos_hal::dho::{afg, board, counter, dvm};

#[test]
fn held_source_services_calibration_generator_counter_and_side_units() {
    let (outcome, xadc) = twice(service_while_held);
    let trace = &outcome.events;
    let stop = trace.iter().position(|event| *event == Event::Stop).unwrap();
    let held = &trace[stop + 1..];
    let config = counter::Config { ch: 0, mode: counter::Mode::Frequency, digits: 3 };
    let counter_writes: Vec<_> = counter::start(&config).into_iter().map(|(r, v)| Event::Write(r, v)).collect();
    let calibrate = held.iter().position(|event| *event == Event::Calibration((vec![0], true, false))).unwrap();
    let generator = held.iter().position(|event| *event == Event::Generator(Some(generator_want()))).unwrap();
    assert!(calibrate < generator);
    assert!(held[generator + 1..].windows(counter_writes.len()).any(|writes| writes == counter_writes));
    assert!(held.contains(&Event::Write(counter::CTRL, 0)));
    let dvm: Vec<_> = dvm::start(0, dvm::EXP).into_iter().map(|(r, v)| Event::Write(r, v)).collect();
    assert!(held[generator + 1..].windows(dvm.len()).any(|writes| writes == dvm));
    assert!(held.contains(&Event::Read(board::XADC_REG, XADC)));
    assert_eq!(xadc, Some(board::xadc(XADC)));
    assert!(!held.iter().any(|event| matches!(event, Event::Dma(_))));
    assert_eq!(outcome.final_ack, 7);
}

const XADC: u32 = (1365 << 16) | 2424;

fn generator_want() -> crate::board::Gen {
    (true, 0.001, 1.0, 0.0, afg::Wave::Sine, afg::Mod::None)
}

fn service_while_held() -> (Outcome, Option<(f32, f32)>) {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut passes = 0;
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => {
            clock.stop();
            clock.poke(board::XADC_REG, XADC);
            let mut g = lock(&shared);
            g.cal_req = Some((vec![0], true, false));
            g.afg[0] = Some(generator_want());
        }
        Pause::Wait(delay) if delay == Duration::from_millis(5) => {
            passes += 1;
            let mut g = lock(&shared);
            match passes {
                1 => {
                    g.counter_want = Some(CounterWant { ch: 0, mode: 0, digits: 3, clear: 0 });
                    g.dvm_want = Some(DvmWant { ch: 0, lane: None });
                }
                2 => {
                    g.counter_want = None;
                    g.dvm_want = None;
                }
                201 => g.record_flow.shutdown = true,
                _ => {}
            }
        }
        Pause::Wait(_) => {}
    });
    let g = lock(&out);
    assert!(g.cal_req.is_none());
    (outcome, g.xadc)
}
