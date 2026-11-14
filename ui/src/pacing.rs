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
    /// per batch: its fill time (ms), and from its read to drawn by the workers (ms)
    fills: VecDeque<f32>,
    readies: VecDeque<f32>,
    ready_ms: Option<f32>,
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
/// `asked` frames recorded, `fill_ms` the ring-fill time it had.
pub fn batch(wait_ms: f32, count: u32, asked: u32, fill_ms: u64) {
    let mut s = state().lock().unwrap();
    let now = Instant::now();
    let iv = s.last_batch.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32() * 1e3);
    s.last_batch = Some(now);
    push(&mut s.batches, (wait_ms, iv, count, asked));
    push(&mut s.fills, fill_ms as f32);
}

/// The deposit workers finished the batch read last (drawpipe.rs): the time since its read went into `ready_ms`.
pub fn ready() {
    let mut s = state().lock().unwrap();
    if let Some(t) = s.last_batch {
        let ms = t.elapsed().as_secs_f32() * 1e3;
        s.ready_ms = Some(s.ready_ms.map_or(ms, |x| x + 0.2 * (ms - x)));
        push(&mut s.readies, ms);
    }
}

/// Time from a batch's read to its frames being drawn (smoothed), ms.
pub fn ready_ms() -> f32 {
    state().lock().unwrap().ready_ms.unwrap_or(3.0)
}

/// Screen frame period of the frame loop (main.rs: 30 a second).
const FRAME_MS: f32 = 1000.0 / 30.0;

/// The first frame-loop step at or after `t`, from the last one; None before the first.
pub fn next_step_after(t: Instant) -> Option<Instant> {
    let l = state().lock().unwrap().last_pass?;
    let since = t.saturating_duration_since(l).as_secs_f32() * 1e3;
    let k = (since / FRAME_MS).ceil().max(1.0);
    Some(l + std::time::Duration::from_secs_f32(k * FRAME_MS / 1e3))
}

/// Ring-fill time limits: the hardware source's former fixed wait at most, a few frames' worth at least.
pub const FILL_MAX_MS: f32 = 25.0;
pub const FILL_MIN_MS: f32 = 4.0;
/// A batch is to be drawn this long before the step that shows it (the merge comes a few ms into the step).
const MARGIN_MS: f32 = 3.0;

/// The ring-fill time of the next batch (`seq::acquire_batch_idle`'s `wait_ms`). With a fixed 25 ms fill a batch came
/// every 40-55 ms at 1-10 us/div, and every 31-34 ms at short time bases, against a screen frame of 33.3 ms: pictures
/// without a new batch and pictures with two alternated (docs/fork/stutter.md). The fill is set so that the reads come
/// one screen frame apart, each the workers' drawing time (`ready_ms`) before a screen step: from the rest of a batch
/// besides its fill (arm, first trigger, read; smoothed). The drawing of one batch overlaps the next one's recording.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fill {
    rest: Option<f32>,
}

impl Fill {
    /// The fill time, ms, for a batch starting at `start`, given the next screen step after a time (`next`) and the
    /// time to draw a batch after its read (`ready`).
    pub fn plan(&self, start: Instant, next: impl Fn(Instant) -> Option<Instant>, ready: f32) -> u64 {
        let rest = self.rest.unwrap_or(5.0);
        let ms = |a: f32| std::time::Duration::from_secs_f32(a.max(0.0) / 1e3);
        // the drawing overlaps the next batch: only the reads need a screen frame between them, each one `ready` +
        // `MARGIN_MS` before a step. The first such read time the batch can make:
        let lead = ready + MARGIN_MS;
        let earliest = start + ms(rest + FILL_MIN_MS);
        let fill = match next(earliest + ms(lead)) {
            Some(step) => step.saturating_duration_since(start).as_secs_f32() * 1e3 - lead - rest,
            None => FILL_MAX_MS,
        };
        fill.clamp(FILL_MIN_MS, FILL_MAX_MS).round() as u64
    }

    /// A batch took `took` ms from its start to its read with fill time `used` ms; `full`: the ring filled before it
    /// (its rest is not known then).
    pub fn done(&mut self, took: f32, used: u64, full: bool) {
        if !full {
            let r = (took - used as f32).max(0.0);
            self.rest = Some(self.rest.map_or(r, |x| x + 0.3 * (r - x)));
        }
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
    let mut fl: Vec<f32> = s.fills.iter().copied().collect();
    let mut rd: Vec<f32> = s.readies.iter().copied().collect();
    let nb = s.batches.len().max(1);
    let p4 = |v: &mut Vec<f32>| format!("{:.1}/{:.1}/{:.1}/{:.1}", pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), pct(v, 1.0));
    format!(
        " pacing pics {} new p10/p50/p90 {:.0}/{:.0}/{:.0} empty {:.0}% gap p50/p90/p99/max {} ms pass {} ms step {} ms batch wait {} ms every {} ms frames p50 {:.0} full {:.0}% fill p50 {:.0} ms drawn {} ms",
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
        pct(&mut fl, 0.5),
        p4(&mut rd),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Break it catches: a fill time that lets a batch miss the screen step it could make, one beyond the former fixed
    // wait, one that reacts to a ring that filled early.
    #[test]
    fn fill_time_lands_the_batch_before_a_screen_step() {
        let t = Instant::now();
        let at = |ms: f32| t + Duration::from_secs_f32(ms / 1e3);
        // screen steps every 33.3 ms from t
        let next = |x: Instant| {
            let since = x.saturating_duration_since(t).as_secs_f32() * 1e3;
            Some(at((since / FRAME_MS).ceil() * FRAME_MS))
        };
        // a rest of 15 ms (read at 1 us/div), drawn 2 ms after the read: a batch starting 1 ms after a step is read
        // and drawn 3 ms before the next one
        let mut f = Fill::default();
        f.done(40.0, 25, false);
        for _ in 0..20 {
            f.done(15.0 + 13.0, 13, false);
        }
        let fill = f.plan(at(1.0), next, 2.0) as f32;
        assert!((fill - (33.3 - 1.0 - 3.0 - 2.0 - 15.0)).abs() <= 1.0, "{}", fill);
        // starting late in the frame: too late for the next step, so the one after it
        let fill = f.plan(at(30.0), next, 2.0) as f32;
        assert!((fill - (66.7 - 30.0 - 3.0 - 2.0 - 15.0)).abs() <= 1.0, "{}", fill);
        // drawing longer than the rest of the frame (15 ms at 5 us/div): the reads still one frame apart, each landing
        // a frame later; from a read 18 ms before a step, the next read 18 ms before the step after
        let fill = f.plan(at(33.3 - 18.0), next, 15.0) as f32;
        assert!((fill - (33.3 - 15.0)).abs() <= 1.0, "{}", fill);
        // a short rest (200 ns/div): the rest of the frame, at most the former 25 ms
        let mut g = Fill::default();
        g.done(7.0, 2, false);
        assert_eq!(g.plan(at(1.0), next, 2.0), 22);
        g.done(2.5, 2, false);
        g.done(2.5, 2, false);
        assert_eq!(g.plan(at(0.0), next, 0.5), 25);
        // a long rest (rare triggers, slow read): aimed at a later step, still landing just before it
        for _ in 0..40 {
            f.done(80.0, 4, false);
        }
        let fill = f.plan(at(1.0), next, 2.0) as f32;
        assert!((fill - (100.0 - 1.0 - 3.0 - 2.0 - 76.0)).abs() <= 1.0, "{}", fill);
        // a full ring says nothing about the rest; no step known yet: the most
        let before = f.plan(at(1.0), next, 2.0);
        f.done(5.0, 25, true);
        assert_eq!(f.plan(at(1.0), next, 2.0), before);
        assert_eq!(Fill::default().plan(t, |_| None, 2.0), FILL_MAX_MS as u64);
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
