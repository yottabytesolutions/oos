// SPDX-License-Identifier: GPL-2.0-or-later

//! Display pacing (fork, docs/fork/stutter.md): how evenly new waveforms reach the screen. The averages of
//! `:SYSTem:STATe?` (fps, step, wfm/s) hide a picture that now and then shows nothing new. Kept here over the last
//! `KEEP` screen frames and acquisition batches:
//! - per picture: the frames that went into it (`picture`), the gaps between pictures with new frames;
//! - per frame-loop pass: its interval and step, render and present times (`pass`), passes over `SLOW_MS` logged;
//! - per batch of the hardware: the wait for its frames, the time from one batch to the next, frames per batch
//!   (`batch`).
//!
//! `summary` gives percentiles for the status line.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

/// Entries kept of each kind: 10 s of screen frames at 30 per second.
const KEEP: usize = 300;
/// A frame-loop pass longer than this is logged with its parts.
const SLOW_MS: f32 = 60.0;

#[derive(Default)]
struct State {
    /// per picture: time of its step, frames in it
    pics: VecDeque<(Instant, u32)>,
    /// per frame-loop pass: interval since the last one, step, render, present (ms)
    passes: VecDeque<[f32; 4]>,
    /// per batch: wait (arm to read done, ms), interval since the last batch (ms), frames recorded, frames asked
    batches: VecDeque<(f32, f32, u32, u32)>,
    last_pass: Option<Instant>,
    last_batch: Option<Instant>,
    slow_logged: u64,
}

fn state() -> &'static Mutex<State> {
    static S: std::sync::OnceLock<Mutex<State>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

fn push<T>(q: &mut VecDeque<T>, x: T) {
    if q.len() >= KEEP {
        q.pop_front();
    }
    q.push_back(x);
}

/// A screen picture took `frames` new frames (records and fast-path frames, `Ui::step`).
pub fn picture(frames: usize) {
    let mut s = state().lock().unwrap();
    push(&mut s.pics, (Instant::now(), frames as u32));
}

/// A frame-loop pass from `t0` (step start) with its step, render and present ends.
pub fn pass(t0: Instant, t1: Instant, t2: Instant, t3: Instant) {
    let ms = |a: Instant, b: Instant| b.saturating_duration_since(a).as_secs_f32() * 1e3;
    let mut s = state().lock().unwrap();
    let iv = s.last_pass.map_or(0.0, |l| ms(l, t0));
    s.last_pass = Some(t0);
    let p = [iv, ms(t0, t1), ms(t1, t2), ms(t2, t3)];
    if iv > SLOW_MS && s.slow_logged < 2000 {
        s.slow_logged += 1;
        eprintln!("oos-ui pacing: pass interval {:.1} ms (step {:.1} render {:.1} present {:.1})", p[0], p[1], p[2], p[3]);
    }
    push(&mut s.passes, p);
}

/// A batch of the hardware done: `wait_ms` from the arm (or the wait of a batch armed ahead) to the read, `count` of
/// `asked` frames recorded.
pub fn batch(wait_ms: f32, count: u32, asked: u32) {
    let mut s = state().lock().unwrap();
    let now = Instant::now();
    let iv = s.last_batch.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32() * 1e3);
    s.last_batch = Some(now);
    push(&mut s.batches, (wait_ms, iv, count, asked));
}

/// Batch cycle the ring-fill time aims at: under one screen frame (33.3 ms), so every picture gets a new batch.
pub const CYCLE_MS: f32 = 30.0;
/// Ring-fill time limits: the hardware source's former fixed wait at most, a few frames' worth at least.
pub const FILL_MAX_MS: f32 = 25.0;
pub const FILL_MIN_MS: f32 = 4.0;

/// The ring-fill time of the next batch (`seq::acquire_batch_idle`'s `wait_ms`). A batch took its fill time plus a
/// rest (arm, first trigger, read, hand-over); with a fixed 25 ms fill the cycle grew to 40-55 ms at 1-10 us/div, longer
/// than a screen frame, and every few pictures had no new batch (docs/fork/stutter.md). The fill is set so that fill +
/// rest stays near `CYCLE_MS`, from the rest of the batches before (smoothed).
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    ms: f32,
    rest: Option<f32>,
    last: Option<Instant>,
}

impl Default for Fill {
    fn default() -> Fill {
        Fill { ms: FILL_MAX_MS, rest: None, last: None }
    }
}

impl Fill {
    /// The fill time for the next batch, ms.
    pub fn ms(&self) -> u64 {
        self.ms.round() as u64
    }

    /// A batch done at `now` with fill time `used` ms (what `ms` gave), `full`: the ring filled before it.
    pub fn done(&mut self, now: Instant, used: u64, full: bool) {
        if let Some(l) = self.last {
            let cycle = now.saturating_duration_since(l).as_secs_f32() * 1e3;
            // a full ring ended the fill early: its rest is not known, keep the last one
            if !full {
                let r = (cycle - used as f32).max(0.0);
                let rest = self.rest.map_or(r, |x| x + 0.3 * (r - x));
                self.rest = Some(rest);
                self.ms = (CYCLE_MS - rest).clamp(FILL_MIN_MS, FILL_MAX_MS);
            }
        }
        self.last = Some(now);
    }

    /// A batch that did not complete (new settings, an error): the next cycle starts anew.
    pub fn restart(&mut self) {
        self.last = None;
    }
}

/// The value at fraction `q` (0..1) of the sorted `v` (0 when empty).
fn pct(v: &mut [f32], q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q).round() as usize]
}

/// Gaps (ms) between consecutive pictures that had new frames.
fn gaps(pics: &VecDeque<(Instant, u32)>) -> Vec<f32> {
    let with: Vec<Instant> = pics.iter().filter(|p| p.1 > 0).map(|p| p.0).collect();
    with.windows(2).map(|w| w[1].saturating_duration_since(w[0]).as_secs_f32() * 1e3).collect()
}

/// One line for the status: ` pacing pics N new p10/p50/p90 a/b/c empty E% gap p50/p90/p99/max ... pass ... batch ...`.
pub fn summary() -> String {
    let s = state().lock().unwrap();
    let mut fr: Vec<f32> = s.pics.iter().map(|p| p.1 as f32).collect();
    let empty = s.pics.iter().filter(|p| p.1 == 0).count();
    let n = s.pics.len().max(1);
    let mut g = gaps(&s.pics);
    let mut iv: Vec<f32> = s.passes.iter().map(|p| p[0]).skip(1).collect();
    let mut st: Vec<f32> = s.passes.iter().map(|p| p[1]).collect();
    let mut bw: Vec<f32> = s.batches.iter().map(|b| b.0).collect();
    let mut bi: Vec<f32> = s.batches.iter().map(|b| b.1).skip(1).collect();
    let mut bc: Vec<f32> = s.batches.iter().map(|b| b.2 as f32).collect();
    let full = s.batches.iter().filter(|b| b.2 >= b.3).count();
    let nb = s.batches.len().max(1);
    let p4 = |v: &mut Vec<f32>| format!("{:.1}/{:.1}/{:.1}/{:.1}", pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), pct(v, 1.0));
    format!(
        " pacing pics {} new p10/p50/p90 {:.0}/{:.0}/{:.0} empty {:.0}% gap p50/p90/p99/max {} ms pass {} ms step {} ms batch wait {} ms every {} ms frames p50 {:.0} full {:.0}%",
        s.pics.len(),
        pct(&mut fr, 0.1),
        pct(&mut fr, 0.5),
        pct(&mut fr, 0.9),
        100.0 * empty as f32 / n as f32,
        p4(&mut g),
        p4(&mut iv),
        p4(&mut st),
        p4(&mut bw),
        p4(&mut bi),
        pct(&mut bc, 0.5),
        100.0 * full as f32 / nb as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Break it catches: a fill time that keeps the batch cycle above a screen frame, one that grows past the former
    // fixed wait, one that reacts to a ring that filled early.
    #[test]
    fn fill_time_keeps_the_batch_cycle_under_a_screen_frame() {
        let t = Instant::now();
        let at = |ms: f32| t + Duration::from_secs_f32(ms / 1e3);
        // a rest of 15 ms (1 us/div): the fill comes down to 15 ms
        let mut f = Fill::default();
        let (mut now, mut used) = (0.0f32, f.ms());
        f.done(at(now), used, false);
        for _ in 0..30 {
            now += used as f32 + 15.0;
            f.done(at(now), used, false);
            used = f.ms();
        }
        assert!((14..=16).contains(&f.ms()), "{}", f.ms());
        // a rest of 5 ms (200 ns/div): back up to the most, 25 ms
        for _ in 0..30 {
            now += used as f32 + 5.0;
            f.done(at(now), used, false);
            used = f.ms();
        }
        assert_eq!(f.ms(), 25);
        // a rest longer than the cycle (rare triggers): the least
        for _ in 0..30 {
            now += used as f32 + 80.0;
            f.done(at(now), used, false);
            used = f.ms();
        }
        assert_eq!(f.ms(), FILL_MIN_MS as u64);
        // a full ring says nothing about the rest
        let before = f.ms();
        f.done(at(now + 1.0), used, true);
        assert_eq!(f.ms(), before);
    }

    #[test]
    fn percentiles_and_gaps() {
        let mut v = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!((pct(&mut v, 0.0), pct(&mut v, 0.5), pct(&mut v, 1.0)), (1.0, 3.0, 5.0));
        assert_eq!(pct(&mut [], 0.5), 0.0);
        // pictures at 0, 33, 66, 99 ms; the one at 66 had nothing new: gaps 33 and 66
        let t = Instant::now();
        let pics: VecDeque<(Instant, u32)> = [(0, 4), (33, 2), (66, 0), (99, 5)].iter().map(|&(ms, n)| (t + Duration::from_millis(ms), n)).collect();
        let g = gaps(&pics);
        assert_eq!(g.len(), 2);
        assert!((g[0] - 33.0).abs() < 0.5 && (g[1] - 66.0).abs() < 0.5, "{:?}", g);
    }
}
