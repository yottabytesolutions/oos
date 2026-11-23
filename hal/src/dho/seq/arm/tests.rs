// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::{plan, tdiv_fs, test_support::writes, Playback, Segments};

#[test]
fn batch_of_frames() {
    // 1 us/div, CH1: 12 500 points per frame, ring of max_frames frames; 100 frames in one arm and one read
    let s = Settings { tdiv: 1e-6, frames: 100, window: None, ..Settings::default() };
    let mut state = State::handover(1, None);
    let (st, p) = plan(&mut state, &s, None, None).unwrap();
    assert!(p.max_frames >= 100, "max_frames {}", p.max_frames);
    let w = writes(&st);
    // arm: 0x4018 = first + n - 1 twice (record and play)
    assert_eq!(w.iter().filter(|x| **x == (0x4018, 99)).count(), 2);
    assert!(matches!(st.last(), Some(&Step::Read { frames: 100, frame }) if frame == 2 * p.memsel as usize));
    // more than the ring is clamped, play-last stays at one frame
    let (_, p2) =
        plan(&mut State::handover(1, None), &Settings { frames: 1_000_000, ..s.clone() }, None, None).unwrap();
    assert_eq!(frames(&Settings { frames: 1_000_000, ..s.clone() }, &p2), p2.max_frames);
    assert_eq!(frames(&Settings { playback: Playback::Last, ..s }, &p2), 1);
}

// Break it catches: an arm ahead of the decode that writes anything else, or in another order, than the batch loop
// without it (stop sequence, record, play and their shadows must stay exactly as verified on the unit). Two batches
// of 100 frames; the pre-armed loop issues arm 2 right after read 1 and only waits before read 2.
#[test]
fn prearm_writes_the_same_sequence_as_the_plain_batch_loop() {
    let s = Settings { tdiv: 1e-6, frames: 100, window: None, ..Settings::default() };
    let p = State::handover(1, None).configure(&s, None, None).unwrap().1;
    let batch = |st: &mut State, pre: bool, armed: bool| -> Vec<Step> {
        let mut v = if armed { wait_steps(&p) } else { st.arm_steps(&s, &p) };
        v.extend(st.read_steps(&s, &p, 100));
        if pre {
            v.extend(st.arm_only_steps(&s, &p));
        }
        v
    };
    let mut a = State::handover(1, None);
    a.configure(&s, None, None).unwrap();
    let plain: Vec<Step> = (0..3).flat_map(|_| batch(&mut a, false, false)).collect();
    let mut b = State::handover(1, None);
    b.configure(&s, None, None).unwrap();
    let mut pre = batch(&mut b, true, false);
    pre.extend(batch(&mut b, true, true));
    pre.extend(batch(&mut b, false, true));
    assert_eq!(writes(&pre), writes(&plain));
    assert_eq!(writes(&pre).len(), 3 * (29 + 10 + 7 + 17 + 12));
    // the waits and reads are the same steps too, only the arm moved before the decode
    let polls = |v: &[Step]| v.iter().filter(|x| matches!(x, Step::Poll { .. } | Step::Read { .. })).count();
    assert_eq!(polls(&pre), polls(&plain));
    assert_eq!((a.sh.s4004, a.sh.s4008, a.sh.s400c, a.sh.s4080), (b.sh.s4004, b.sh.s4008, b.sh.s400c, b.sh.s4080));
    // arm_steps is still arm + wait
    let mut c = State::handover(1, None);
    let mut d = State::handover(1, None);
    let mut split = c.arm_only_steps(&s, &p);
    split.extend(wait_steps(&p));
    assert_eq!(format!("{:?}", d.arm_steps(&s, &p)), format!("{:?}", split));
}

// Break it catches: a segmented record that is not stamp clear + the batch arm with N and the interval, a count
// beyond the whole-memory limit, segments that leak into the live batch path.
#[test]
fn record_steps_clear_the_stamp_and_arm_n_frames() {
    let seg = |n: u32, interval: f64| Settings {
        tdiv: 1e-6,
        depth: 1000,
        window: None,
        segments: Some(Segments { n, interval }),
        ..Settings::default()
    };
    let mut st = State::handover(1, None);
    let p = st.configure(&seg(100, 1e-3), None, None).unwrap().1;
    let w = writes(&st.record_steps(&seg(100, 1e-3), &p));
    assert_eq!(&w[..2], &[(0x40b8, 4), (0x40b8, 0)]);
    assert_eq!(w.len(), 2 + 29);
    assert!(w.contains(&(0x4068, 156_250)) && w.contains(&(0x4018, 99)));
    // the stock limit for 1,000 points: 61,440 frames
    assert_eq!(record_frames(&seg(1_000_000, 0.0), &p), 61_440);
    assert_eq!(record_frames(&seg(0, 0.0), &p), 1);
    assert_eq!(record_frames(&Settings::default(), &p), 0);
    // the live plan does not change with segments set
    let live = |s: &Settings| writes(&plan(&mut State::handover(1, None), s, None, None).unwrap().0);
    assert_eq!(live(&seg(100, 1e-3)), live(&Settings { segments: None, ..seg(100, 1e-3) }));
}

// Break it catches: a disarm that is not the plain stop sequence every arm starts with.
#[test]
fn disarm_is_the_stop_sequence_of_an_arm() {
    let s = Settings::default();
    let p = hz::prep(1, tdiv_fs(s.tdiv), 0, 0);
    let mut a = State::handover(1, None);
    let d = writes(&a.disarm_steps());
    assert_eq!(d.len(), 17);
    let arm = writes(&State::handover(1, None).arm_only_steps(&s, &p));
    assert_eq!(&arm[..17], &d[..]);
    assert!(matches!(a.disarm_steps()[1], Step::Poll { reg: 0x4034, ms: 10, .. }));
}

#[test]
fn slot_zero_arms_a_live_record_larger_than_the_segmented_ring() {
    let settings = Settings::default();
    let params = hz::prep(1, 100_000_000_000_000, 125_000_000, 0);
    assert_eq!(hz::max_record_frames(params.seg), 0);
    let mut state = State::handover(1, None);
    let steps = state.arm_only_steps(&settings, &params);
    assert!(steps.iter().any(|step| matches!(step, Step::Wr((0x4018, 0)))));
}
