// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::{plan, tdiv_fs, test_support::writes, Playback, Segments};
use std::{cell::Cell, collections::VecDeque, io};

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
        v.extend(st.read_steps(&s, &p, 100).unwrap());
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

struct DisarmClock {
    start: Instant,
    elapsed: Cell<Duration>,
}

impl Clock for DisarmClock {
    fn now(&self) -> Instant {
        self.start + self.elapsed.get()
    }

    fn wait(&self, duration: Duration) {
        self.elapsed.set(self.elapsed.get() + duration);
    }
}

struct DisarmBus {
    writes: Vec<Write>,
    reads: Vec<(usize, u32)>,
    status: VecDeque<Option<u32>>,
    done: Option<u32>,
}

impl Bus for DisarmBus {
    fn wr(&mut self, reg: u32, value: u32) {
        self.writes.push((reg, value));
    }

    fn rd(&mut self, reg: u32) -> u32 {
        panic!("expected a checked read of {reg:#x}");
    }

    fn try_rd(&mut self, reg: u32) -> io::Result<u32> {
        let writes = self.writes.len();
        self.reads.push((writes, reg));
        let value = match (writes, reg) {
            (0 | scu::STOP_PULSE_WRITES, 0x4034) => self.status.pop_front().unwrap_or(Some(0)),
            (scu::STOP_PULSE_WRITES, 0x4080) => self.done,
            _ => panic!("unexpected read of {reg:#x} after {writes} writes"),
        };
        value.ok_or_else(|| io::Error::other("scripted read error"))
    }
}

fn disarm(status: &[Option<u32>], done: Option<u32>) -> (CaptureDisarm, Duration) {
    let (report, elapsed, _) = disarm_trace(status, done);
    (report, elapsed)
}

fn disarm_trace(status: &[Option<u32>], done: Option<u32>) -> (CaptureDisarm, Duration, Vec<(usize, u32)>) {
    let clock = DisarmClock { start: Instant::now(), elapsed: Cell::default() };
    let mut bus = DisarmBus { writes: Vec::new(), reads: Vec::new(), status: status.iter().copied().collect(), done };
    let mut state = State::handover(1, None);
    let mut reference = State::handover(1, None);
    let report = state.disarm_capture(&mut bus, &clock);
    assert_eq!(bus.writes, writes(&reference.disarm_steps()));
    assert_eq!(state.sh, reference.sh);
    assert!(bus.status.is_empty());
    (report, clock.elapsed.get(), bus.reads)
}

#[test]
fn disarm_reads_only_stop_status_and_done_before_the_play_info_clear() {
    use scu::status::STOPPED;
    let (report, elapsed, reads) = disarm_trace(&[Some(STOPPED), Some(STOPPED)], Some(0));
    assert_eq!(reads, [(0, 0x4034), (6, 0x4034), (6, 0x4080)]);
    assert_eq!(report.outcome(), CaptureStop::NoCompletedRecord);
    assert_eq!(report.stopped_after(), Some(Duration::ZERO));
    assert_eq!(elapsed, Duration::ZERO);
}

#[test]
fn disarm_always_writes_the_stock_sequence_even_when_status_is_unavailable() {
    use scu::status::STOPPED;

    for (status, done, error) in [
        ([Some(STOPPED), Some(u32::MAX)], Some(scu::PLAYINFO_DONE), None),
        ([Some(STOPPED), Some(STOPPED)], Some(u32::MAX), None),
        ([None, Some(STOPPED)], Some(0), Some("read 0x4034: scripted read error")),
        ([Some(STOPPED), None], Some(0), Some("read 0x4034: scripted read error")),
        ([Some(STOPPED), Some(STOPPED)], None, Some("read 0x4080: scripted read error")),
    ] {
        let (report, elapsed) = disarm(&status, done);
        assert_eq!(elapsed, Duration::ZERO);
        assert_eq!(report.outcome(), CaptureStop::Unknown);
        assert!(!report.status_available());
        assert_eq!(report.fsm(), status[1]);
        assert_eq!(report.play_info(), done);
        assert_eq!(report.error(), error);
    }
}

#[test]
fn disarm_samples_late_stopped_and_bounds_the_wait_when_stopped_stays_clear() {
    use scu::{status::STOPPED, PLAYINFO_DONE};

    let (late, elapsed) = disarm(&[Some(STOPPED), Some(0), Some(0), Some(STOPPED)], Some(0));
    assert_eq!(elapsed, Duration::from_micros(100));
    assert_eq!(late.stopped_after(), Some(elapsed));
    assert_eq!(late.outcome(), CaptureStop::NoCompletedRecord);
    let (never, elapsed) = disarm(&[Some(STOPPED)], Some(PLAYINFO_DONE));
    assert_eq!(elapsed, Duration::from_micros(500));
    assert_eq!(never.stopped_after(), None);
    assert_eq!(never.fsm(), Some(0));
    assert_eq!(never.play_info(), Some(PLAYINFO_DONE));
    assert!(never.status_available());
    assert_eq!(never.outcome(), CaptureStop::Unknown);
    let (immediate, _) = disarm(&[Some(STOPPED), Some(STOPPED)], Some(PLAYINFO_DONE));
    assert_eq!(immediate.outcome(), CaptureStop::Recorded);
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
