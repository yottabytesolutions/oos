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
