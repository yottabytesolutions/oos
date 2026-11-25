// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::{self, test_support::writes, Playback, Settings, State, Step};

#[test]
fn seeded_playback_asks_for_exactly_the_generated_bytes() {
    let mut state = 0x54ce_17a1_u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for i in 0..4096 {
        let mask = next() % 15 + 1;
        let playback = if i % 8 == 0 { Playback::Last } else { Playback::Raw };
        let compress = if i % 3 == 0 { 1 } else { next() % 30_000 };
        let mut s = Settings {
            mask,
            tdiv: [1e-9, 5e-9, 1e-7, 2e-4, 5e-2][next() as usize % 5],
            depth: [0, 1000, 100_000, 1_000_000, 10_000_000][next() as usize % 5],
            frames: next() % 127 + 1,
            compress,
            header: next() % 2 == 0,
            playback,
            window: None,
            la: (i % 11 == 0).then(Default::default),
            ..Settings::default()
        };
        let p = seq::record_geometry(&s).unwrap();
        if p.memsel < 8 {
            s.compress = 1;
        }
        if i % 4 != 0 {
            let points = 8 * (1 + next() % (p.memsel / 8).max(1));
            s.window = Some((points, -i64::from(p.pre)));
        }
        let steps = State::handover(mask, None)
            .read_steps_at(&s, &p, next() % 8, seq::frames(&s, &p))
            .unwrap_or_else(|error| panic!("case {i}: {error}"));
        let requested = match steps.last().unwrap() {
            Step::Read { frames, frame } => seq::read_len(*frames, *frame),
            Step::ReadPadded { frames, stride, .. } => frames * stride,
            _ => panic!("case {i}: missing read"),
        };
        assert_eq!(played_bytes(&writes(&steps), (p.memsel, s.la.is_some())), Some(requested as u64), "case {i}");
    }
}

#[test]
fn drains_the_minimum_transfer_for_tiny_raw_windows() {
    use crate::dho::{bus::DmaBuf, seg::sim};
    for (mask, points) in [(1, 5), (1, 8), (3, 7), (3, 8), (15, 8)] {
        let s = Settings { mask, window: Some((points, 0)), ..Settings::default() };
        let p = seq::record_geometry(&s).unwrap();
        let mut state = State::handover(mask, None);
        let (_, mut bus, mut c2h) = sim::engine(sim::Engine::new(0, 800_000, 1, p.ratio));
        let steps = state.read_steps(&s, &p, 2).unwrap();
        assert_eq!(played_bytes(&writes(&steps), (p.memsel, false)), Some(512));
        let mut dma = DmaBuf::new();
        seq::execute_bus(&steps, &mut bus, &mut c2h, &mut dma, |_| {}).unwrap();
        let frame = 2 * (points * p.ratio) as usize;
        assert_eq!(dma.data().len(), 2 * frame);
        assert_eq!(&dma.data()[frame..frame + 2], &sim::Engine::word(1, 0).to_le_bytes());
        assert_eq!(std::io::Read::read(&mut c2h, &mut [0; 8]).unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    }
}

#[test]
fn modeled_compression_cannot_finish_the_historical_overlong_play() {
    let writes = [(0x4008, 1), (0x1100, 1_000_000 + 0x248), (0x11f0, 3912), (0x1170, 0xc000_0100)];
    assert_eq!(played_bytes(&writes, (1_000_000, false)), None);
}

#[test]
fn rejects_la_stream_with_a_stale_analog_transfer_length() {
    use crate::dho::{la, scu};
    let mut writes = vec![(0x11f0, 64)];
    let mut shadows = scu::Shadows::HANDOVER;
    writes.extend(la::readout(&mut shadows, 1000, 0, 2, 3));
    assert_eq!(played_bytes(&writes, (1000, true)), None);
}
