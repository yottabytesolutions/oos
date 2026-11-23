// SPDX-License-Identifier: GPL-2.0-or-later

use super::{capture, run, settings, sim, twice, AcqLoop, Event, FixedPacing, Frame, Io, Run};
use crate::{
    board::PictureEligibility,
    drawpipe::Pipe,
    model::{self, DotsMode, Model},
    wave::Phosphor,
};
use oos_core::change::{Request, Shadow, UiAcq};

pub(super) const SIZE: (usize, usize) = (128, 128);

#[derive(Clone, Copy, Debug)]
pub(super) enum StopAt {
    During,
    Between,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Picture {
    pub(super) frames: u64,
    rows: Vec<(usize, u32)>,
    pub(super) tap: Option<Waveform>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Waveform {
    pub(super) frame: Frame,
    pub(super) envelope: bool,
    pub(super) points: usize,
    pub(super) eligibility: PictureEligibility,
}

impl Picture {
    pub(super) fn compose(pipe: &Pipe) -> Self {
        let (w, h) = SIZE;
        let mut ph = Phosphor::new(w, h);
        let frames = pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames;
        let rows = (0..h).map(|r| (r, ph.count(0, r * w + w / 2))).filter(|&(_, hits)| hits != 0).collect();
        let tap = pipe.take_waveform(SIZE).map(|tap| Waveform {
            frame: Frame::read(&tap),
            envelope: tap.envelope,
            points: tap.ch[0].len(),
            eligibility: tap.eligibility(),
        });
        assert!(pipe.take_waveform(SIZE).is_none());
        Self { frames, rows, tap }
    }
}

pub(super) fn model(tdiv: f64) -> Model {
    let tscale = model::tscales().iter().position(|&t| (f64::from(t) - tdiv).abs() < tdiv * 1e-4).unwrap();
    let mut model = Model { tscale, dots: DotsMode::Off, ..Default::default() };
    let vscale = model::VSCALES.iter().position(|&scale| scale == 1.0).unwrap();
    for (i, ch) in model.ch.iter_mut().enumerate() {
        (ch.on, ch.vscale, ch.offset) = (i == 0, vscale, 0.0);
    }
    model
}

pub(super) fn record_code(index: u64) -> u16 {
    // Independent record IDs occupy distinct, unclipped horizontal rows.
    12_288 + u16::try_from(index).unwrap() * 1_536
}

pub(super) fn script(source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>, stop: StopAt, records: u32) {
    capture(source, io);
    capture(source, io); // The prior submission deliberately stays in intake until the source parks.
    io.clock.compose();
    match stop {
        StopAt::During => {
            io.clock.stop_next_fill(records);
            capture(source, io);
        }
        StopAt::Between => io.clock.stop(),
    }
    capture(source, io);
    io.clock.compose();
    let model = model(settings().tdiv);
    let config = model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let run = shadow.observe(&model, UiAcq::default(), &[Request::Run], &config).unwrap().identity();
    io.clock.resume(run);
    capture(source, io);
    io.clock.compose();
    io.clock.compose();
}

fn assert_rows(picture: &Picture, records: &[u16]) {
    const HITS_PER_LINE: u32 = 24;
    let mut expected: Vec<_> = records
        .iter()
        .map(|&code| {
            let volts = (f32::from(code) - 32768.0) / 7500.0;
            let height = SIZE.1 as f32;
            ((height / 2.0 - volts * (height / model::VDIV)).round() as usize, HITS_PER_LINE)
        })
        .collect();
    expected.sort_unstable();
    assert_eq!(picture.frames, records.len() as u64);
    assert_eq!(picture.rows, expected);
}

#[test]
fn auto_paints_each_admitted_record_once_under_backpressure_and_across_stop_run() {
    for count in [1, 2, 4, 6] {
        check_pictures(count, StopAt::Between);
        check_pictures(count, StopAt::During);
    }
}

fn check_pictures(count: u32, stop: StopAt) {
    let result = twice(|| run(Run::Pictures(stop), (count, count), settings(), (false, true), true));
    let n = u64::from(count);
    let batches = if matches!(stop, StopAt::During) { [0, n, 3 * n] } else { [0, n, 2 * n] };
    let mut expected = batches.map(|start| (start..start + n).map(record_code).collect::<Vec<_>>()).into_iter();
    let mut words = Vec::new();
    let mut pictures = Vec::new();
    for event in &result.events {
        match event {
            Event::Intake(batch) => {
                assert!(words.is_empty());
                words.extend_from_slice(batch.first_words());
                assert_eq!(words, expected.next().unwrap());
            }
            Event::Picture(picture) => {
                assert_rows(picture, &words);
                assert_eq!(picture.tap.as_ref().map(|tap| tap.frame.code()), words.last().copied());
                if let Some(tap) = &picture.tap {
                    assert!(!tap.envelope);
                    assert_eq!(tap.eligibility, PictureEligibility::AlreadyDeposited);
                }
                pictures.push(picture.frames);
                words.clear();
            }
            _ => {}
        }
    }
    assert!(expected.next().is_none() && words.is_empty(), "count={count}, STOP {stop:?}");
    assert_eq!(pictures, [n, n, 0, n, 0], "count={count}, STOP {stop:?}");
    assert_source_order(&result.events, stop);
    assert_eq!(result.final_ack, 7);
    assert!(result.queue.is_empty());
    let lost = result.counters.losses.iter().map(|(_, count)| count).sum::<u64>();
    let stopped = if matches!(stop, StopAt::During) { n } else { 0 };
    assert_eq!(result.counters.read, 3 * n + stopped);
    assert_eq!(result.counters.loss("lost.settings"), stopped);
    assert_eq!(result.counters.dropped, stopped);
    assert_eq!(lost, stopped);
    assert_eq!(result.counters.deposited, pictures.iter().sum::<u64>());
    assert_eq!(result.counters.read, result.counters.deposited + lost);
}

fn assert_source_order(events: &[Event], stop: StopAt) {
    let handoffs: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, Event::Dma(_) | Event::Park(_) | Event::Intake(_) | Event::Stop | Event::Run))
        .collect();
    let [Event::Dma(_), Event::Dma(_), Event::Park(_), Event::Intake(_), Event::Intake(_), Event::Stop, rest @ .., Event::Run, Event::Dma(_), Event::Intake(_)] =
        handoffs.as_slice()
    else {
        panic!("expected a blocked handoff before STOP and one after RUN: {handoffs:?}");
    };
    match stop {
        StopAt::Between => assert!(rest.is_empty()),
        StopAt::During => assert!(matches!(rest, [Event::Dma(_)])),
    }
}
