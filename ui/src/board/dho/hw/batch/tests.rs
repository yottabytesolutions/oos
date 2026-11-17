// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::switches::HeaderProbe;
use crate::{model::Model, sparse::Interp, wave::Phosphor};
use std::sync::{Arc, Mutex};

struct Bytes(Vec<u8>);
impl RawBatch for Bytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

fn record(n: u32, amplitude: i32) -> Vec<u8> {
    (0..n)
        .flat_map(|i| ((32768 + amplitude * [4, -2, 1, -3, -1, 3, -4, 2][i as usize % 8]) as u16).to_le_bytes())
        .collect()
}

fn finish(pipe: &Pipe) {
    let deadline = crate::ui::lifecycle::test_wait::Wait::new("batch workers return the completed buffer");
    let mut helper = crate::drawpipe::Helper::default();
    while pipe.take_buffer().is_none() {
        deadline.check();
        if !pipe.help(&mut helper) {
            deadline.poll();
        }
        pipe.help_done(&mut helper);
    }
}

fn stale_frame(bytes: &[u8], s: &Settings, p: &hz::Params, warm: bool) -> Frame {
    // An independent oracle for the pattern valid before frame 0: raw if nothing was learned yet.
    let mut corrected = bytes.to_vec();
    if warm {
        for (i, b) in corrected.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let code = u16::from_le_bytes([b[0], b[1]]) as i32 - 200 * [4, -2, 1, -3, -1, 3, -4, 2][i % 8];
            b.copy_from_slice(&(code as u16).to_le_bytes());
        }
    }
    super::super::decode_probed(&corrected, s, p, 0, HeaderProbe::Off)
}

fn run(frame0_drop: bool, replacement: bool, warm: bool) {
    // AC coupling: frame 1 is not checked for its edge here (frame1.rs has those tests)
    let s = Settings {
        mask: 1,
        header: false,
        compress: 1,
        trig_couple: oos_hal::dho::tpu::Coupling::Ac,
        ..Settings::default()
    };
    let mut p = hz::prep(s.mask, 1_000_000_000, 4096, 0);
    p.memsel = 4096;
    // A stale frame with a fitting but different amplitude would change the learned pattern.
    let (stale, good) = (record(p.memsel, 250), record(p.memsel, 200));
    let (mut cores, mut oracle) = (Cores::default(), Cores::default());
    if warm {
        super::super::decode_with(&good, &s, &p, 0, HeaderProbe::Off, Some(&mut cores));
        super::super::decode_with(&good, &s, &p, 0, HeaderProbe::Off, Some(&mut oracle));
    }
    let mut want = Phosphor::new(128, 64);
    let mut model = Model::default();
    for (i, ch) in model.ch.iter_mut().enumerate() {
        ch.on = i == 0;
    }
    let expected_stale = if frame0_drop || !replacement {
        stale_frame(&stale, &s, &p, warm)
    } else {
        super::super::decode_with(&stale, &s, &p, 0, HeaderProbe::Off, Some(&mut oracle))
    };
    // frame 0 is left out with `frame0_drop` (upstream 0cc8fcb6): the workers start at frame 1
    if replacement && !frame0_drop {
        crate::drawpipe::draw_frame(&mut want, &model, &expected_stale, Interp::Exact, None);
    }
    let neighbour = record(p.memsel, 220);
    if replacement {
        let frame = super::super::decode_with(&neighbour, &s, &p, 2, HeaderProbe::Off, Some(&mut oracle));
        crate::drawpipe::draw_frame(&mut want, &model, &frame, Interp::Exact, None);
    }
    let expected_newest = super::super::decode_with(&good, &s, &p, 3, HeaderProbe::Off, Some(&mut oracle));
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    pipe.publish_view(&model, 128, 64, Interp::Exact, Default::default());
    let delivered = Arc::new(Mutex::new(None));
    let done = delivered.clone();
    let frames =
        Readout::new(&pipe, (&s, &p), (good.len(), 3, frame0_drop), (3, HeaderProbe::Off, None), Default::default())
            .dispatch(
                &mut Bytes([stale, neighbour, good].concat()),
                replacement.then(|| Bytes(Vec::new())),
                Some(&mut cores),
                |frame| Frame { stamp: 17, cfg: 23, ..frame },
                move |frame| Box::new(move || *oos_hal::sync::lock(&done) = Some(frame)),
            );
    assert_eq!(cores.pattern((1, 0, p.rate)), oracle.pattern((1, 0, p.rate)), "learning includes an ineligible frame");
    let drawn = if frame0_drop { 1 } else { 2 };
    let newest = newest(&pipe, frames, (replacement, drawn), &delivered, &want);
    assert_eq!((newest.seq, newest.stamp, newest.cfg), (3, 17, 23));
    assert_eq!(newest.ch, expected_newest.ch);
}

fn newest(
    pipe: &Pipe,
    frames: Vec<Frame>,
    (replacement, drawn): (bool, usize),
    delivered: &Mutex<Option<Frame>>,
    want: &Phosphor,
) -> Frame {
    if replacement {
        assert!(frames.is_empty());
        finish(pipe);
        let mut got = Phosphor::new(128, 64);
        assert_eq!(pipe.merge(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, drawn as u64);
        assert_eq!(got.map, want.map, "frame 0 left out, or drawn with the pre-frame pattern");
        oos_hal::sync::lock(delivered).take().unwrap()
    } else {
        assert!(oos_hal::sync::lock(delivered).is_none());
        assert_eq!(frames.len(), 1);
        frames.into_iter().next().unwrap()
    }
}

#[test]
fn acquisition_batch_stale_frame_draws_without_learning() {
    for warm in [false, true] {
        for frame0_drop in [false, true] {
            run(frame0_drop, true, warm);
        }
    }
}

#[test]
fn acquisition_batch_buffer_exhaustion_learns_only_the_newest() {
    for warm in [false, true] {
        for frame0_drop in [false, true] {
            run(frame0_drop, false, warm);
        }
    }
}
