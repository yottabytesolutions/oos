// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::dho::tpu::{Coupling, Slope};

fn settings() -> Settings {
    Settings { level: 0.0, ..Settings::default() }
}

fn frame(edge: usize) -> Frame {
    let mut f = Frame { pre: 500, ..Frame::default() };
    f.ch[0] = (0..1000).map(|i| if i >= edge { 1.0 } else { -1.0 }).collect();
    f
}

// Issue #5 on the unit (2026-11-16): frame 1 now and then has its rising edge about 90 samples before the trigger
// point, or none. Break it catches: such a frame kept, a good frame or one of another trigger type left out.
#[test]
fn frame1_without_its_edge_is_found() {
    let s = settings();
    // the hardware's edge, a few samples after pre
    assert!(!off_trigger(&frame(506), &s));
    // the unit's frame 1: the edge 90 samples early, or none at all (a level the whole record)
    assert!(off_trigger(&frame(410), &s));
    assert!(off_trigger(&frame(2000), &s));
    // falling slope on a rising edge: off; either: on
    assert!(off_trigger(&frame(506), &Settings { slope: Slope::Falling, ..s.clone() }));
    assert!(!off_trigger(&frame(506), &Settings { slope: Slope::Either, ..s.clone() }));
    // noise around the level, under a fifth of a division: no edge, the frame is off
    let mut flat = frame(2000);
    flat.ch[0] = (0..1000).map(|i| if i % 2 == 0 { 0.05 } else { -0.05 }).collect();
    assert!(off_trigger(&flat, &s));
    // what cannot be checked stays: another trigger type, AC coupling, min/max pairs, an LA line as source
    let timeout = Settings { trigger: oos_hal::dho::seq::Trig::Timeout { time: 1e-6 }, ..s.clone() };
    assert!(!off_trigger(&frame(410), &timeout));
    assert!(!off_trigger(&frame(410), &Settings { trig_couple: Coupling::Ac, ..s.clone() }));
    assert!(!off_trigger(&Frame { envelope: true, ..frame(410) }, &s));
    assert!(!off_trigger(&frame(410), &Settings { trig_src: 9, ..s.clone() }));
}

#[test]
fn the_record_path_checks_only_the_early_slots_selected_by_policy() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let s = settings();
    let off = frame(410);
    let on = frame(506);
    let rejects = |slot, policy, frame| off_trigger_if_checked((slot, 2), policy, (frame, None), &s);
    assert!(!rejects(0, FrameZero::Drop, &off));
    assert!(!rejects(0, FrameZero::Steady, &on));
    assert!(rejects(0, FrameZero::Steady, &off));
    assert!(rejects(1, FrameZero::Drop, &off));
    assert!(rejects(1, FrameZero::Steady, &off));
    assert!(!rejects(1, FrameZero::Keep, &off));
    assert!(crate::drawpipe::counters().records_text().contains("lost.frame1=2"));
    assert_eq!(crate::drawpipe::counters().read().2, 0);
}

// Break it catches: the raw newest frame put in place of the record without the batch's identity and sequence.
#[test]
fn the_raw_newest_frame_takes_the_place_of_the_last() {
    let mut all = vec![Frame { seq: 4, ..frame(506) }, Frame { seq: 5, ..frame(506) }];
    raw_newest(&mut all, Some(Frame { seq: 99, pre: 7, ..Frame::default() }), (11, 12));
    let last = &all[1];
    assert_eq!((last.seq, last.stamp, last.cfg, last.pre), (5, 11, 12, 7));
    let mut none: Vec<Frame> = Vec::new();
    raw_newest(&mut none, Some(Frame::default()), (1, 1));
    assert!(none.is_empty());
}

#[test]
fn dma_trigger_gate_matches_decoding_with_headers_windows_and_interleaving() {
    for (mask, source) in [(1, 1), (6, 3), (13, 4)] {
        for header in [false, true] {
            let s = Settings { mask, header, trig_src: source, window: Some((8000, -5000)), ..settings() };
            let p = hz::Params { memsel: 12_000, pre: 6500, ..hz::prep(mask, 1_000_000_000, 4096, 0) };
            let map = oos_hal::dho::seq::code_map(mask, s.ch.map(|c| (c.scale, c.offset)));
            let lane = map.word[(source - 1) as usize].unwrap();
            for (edge, off) in [(5006, false), (4910, true), (8000, true)] {
                let mut bytes = vec![0; if header { 16 } else { 0 }];
                for i in 0..8000 {
                    for j in 0..map.ratio {
                        let word: u16 = if j == lane && i < edge { 22000 } else { 44000 };
                        bytes.extend(word.to_le_bytes());
                    }
                }
                let decoded = super::super::hw::decode(&bytes, &s, &p, 0);
                assert_eq!(off_trigger(&decoded, &s), off);
                assert_eq!(bytes_off_trigger(&bytes, &s, &p), off);
            }
        }
    }
}
