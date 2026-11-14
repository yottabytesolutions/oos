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
    reads: VecDeque<f32>,
    readies: VecDeque<f32>,
    ready_ms: Option<f32>,
    /// screen frames between two reads (`Fill`)
    frames_per_read: u32,
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
/// `asked` frames recorded, `fill_ms` the ring-fill time it had, `read_ms` from the end of the fill to the read done.
pub fn batch(wait_ms: f32, count: u32, asked: u32, fill_ms: u64, read_ms: f32, frames_per_read: u32) {
    let mut s = state().lock().unwrap();
    let now = Instant::now();
    let iv = s.last_batch.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32() * 1e3);
    s.last_batch = Some(now);
    push(&mut s.batches, (wait_ms, iv, count, asked));
    push(&mut s.fills, fill_ms as f32);
    push(&mut s.reads, read_ms);
    s.frames_per_read = frames_per_read;
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

/// Ring-fill time limits: the hardware source's former fixed wait before the first screen step is known (and at most
/// k screen frames after), a few frames' worth at least.
pub const FILL_MAX_MS: f32 = 25.0;
pub const FILL_MIN_MS: f32 = 4.0;
/// A batch is to be drawn this long before the step that shows it (the merge comes a few ms into the step).
const MARGIN_MS: f32 = 3.0;

/// The ring-fill time of the next batch (`seq::acquire_batch_idle`'s `wait_ms`). With a fixed 25 ms fill a batch came
/// every 40-55 ms at 1-10 us/div, and every 31-34 ms at short time bases, against a screen frame of 33.3 ms: pictures
/// without a new batch and pictures with two alternated (docs/fork/stutter.md). The fill is set so that the reads come
/// one screen frame apart, each the workers' drawing time (`ready_ms`) before a screen step: from the rest of a batch
/// besides its fill (arm, first trigger, read; smoothed). The drawing of one batch overlaps the next one's recording.
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    /// the rest of a batch besides its fill (smoothed) and its mean deviation
    rest: Option<f32>,
    dev: f32,
    /// screen frames between two reads: 1, or more while the rest leaves no room for a batch in one frame
    k: u32,
    /// the read time the last plan aimed at
    target: Option<Instant>,
}

impl Default for Fill {
    fn default() -> Fill {
        Fill { rest: None, dev: 0.0, k: 1, target: None }
    }
}

impl Fill {
    /// The fill time, ms, for a batch starting at `start`, given the next screen step after a time (`next`) and the
    /// time to draw a batch after its read (`ready`).
    pub fn plan(&mut self, start: Instant, next: impl Fn(Instant) -> Option<Instant>, ready: f32) -> u64 {
        let rest = self.rest.unwrap_or(5.0);
        let ms = |a: f32| std::time::Duration::from_secs_f32(a.max(0.0) / 1e3);
        // reads one screen frame apart when a batch fits (with its usual spread), else every k-th frame, evenly: an
        // uneven 33/66 ms mix is the stutter. Down again only with room to spare (no flapping at the edge).
        let need = rest + 2.0 * self.dev + FILL_MIN_MS;
        if need > self.k as f32 * FRAME_MS {
            self.k = ((need / FRAME_MS).ceil() as u32).clamp(1, 4);
        } else if self.k > 1 && need < (self.k - 1) as f32 * FRAME_MS - 3.0 {
            self.k -= 1;
        }
        // the drawing overlaps the next batch: only the reads need the frames between them, each one `ready` +
        // `MARGIN_MS` before a step. The first such read time the batch can make, k frames after the last one:
        let lead = ready + MARGIN_MS;
        // planned with the rest's usual spread on top: a read a little early still shows in the same picture, a late
        // one a frame later
        let rest = rest + 2.0 * self.dev;
        let mut lower = start + ms(rest + FILL_MIN_MS);
        if let Some(t) = self.target {
            lower = lower.max(t + ms((self.k as f32 - 0.5) * FRAME_MS));
        }
        let Some(step) = next(lower + ms(lead)) else { return FILL_MAX_MS as u64 };
        let read = step - ms(lead);
        self.target = Some(read);
        let fill = read.saturating_duration_since(start).as_secs_f32() * 1e3 - rest;
        // a batch that starts early (the read before was early) fills longer to keep its slot
        fill.clamp(FILL_MIN_MS, FILL_MAX_MS.max(self.k as f32 * FRAME_MS)).round() as u64
    }

    /// Screen frames between two reads (`plan`).
    pub fn frames_per_read(&self) -> u32 {
        self.k
    }

    /// A batch took `took` ms from its start to its read with fill time `used` ms; `full`: the ring filled before it
    /// (its rest is not known then).
    pub fn done(&mut self, took: f32, used: u64, full: bool) {
        if !full {
            let r = (took - used as f32).max(0.0);
            let rest = self.rest.map_or(r, |x| x + 0.3 * (r - x));
            self.dev += 0.3 * ((r - rest).abs() - self.dev);
            self.rest = Some(rest);
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
    let mut re: Vec<f32> = s.reads.iter().copied().collect();
    // before the fill ended: arm or the wait of a batch armed ahead, first trigger, fill
    let mut pre: Vec<f32> = s.batches.iter().zip(s.reads.iter()).map(|(b, r)| b.0 - r).collect();
    let nb = s.batches.len().max(1);
    let p4 = |v: &mut Vec<f32>| format!("{:.1}/{:.1}/{:.1}/{:.1}", pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), pct(v, 1.0));
    format!(
        " pacing pics {} new p10/p50/p90 {:.0}/{:.0}/{:.0} empty {:.0}% gap p50/p90/p99/max {} ms pass {} ms step {} ms batch wait {} ms every {} ms frames p50 {:.0} full {:.0}% fill p50 {:.0} ms every {} frame(s) until fill end {} ms read {} ms drawn {} ms",
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
        s.frames_per_read,
        p4(&mut pre),
        p4(&mut re),
        p4(&mut rd),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Batches against screen steps every `FRAME_MS`: each batch takes `rest(k)` ms besides its fill, the next one
    /// starts 0.5 ms after a read. Returns the read times (ms) and the k of the planner at the end.
    fn run(rest: impl Fn(usize) -> f32, ready: f32, n: usize) -> (Vec<f32>, u32) {
        let t = Instant::now();
        let at = |ms: f32| t + Duration::from_secs_f32(ms / 1e3);
        let next = |x: Instant| {
            let since = x.saturating_duration_since(t).as_secs_f32() * 1e3;
            Some(at((since / FRAME_MS).ceil() * FRAME_MS))
        };
        let mut f = Fill::default();
        let (mut now, mut reads) = (2.0f32, Vec::new());
        for i in 0..n {
            let fill = f.plan(at(now), next, ready);
            let r = rest(i);
            let read = now + r + fill as f32;
            f.done(r + fill as f32, fill, false);
            reads.push(read);
            now = read + 0.5;
        }
        (reads, f.frames_per_read())
    }

    // Break it catches: reads that drift against the screen steps (pictures without a new batch next to pictures with
    // two), an uneven mix of one- and two-frame gaps where the batch does not fit one frame, reads that land after
    // the step minus the drawing time.
    #[test]
    fn reads_come_evenly_before_the_screen_steps() {
        let lead = 5.0 + MARGIN_MS;
        for (name, rest, k_want) in [("200 ns/div", 7.0f32, 1u32), ("1 us/div", 15.0, 1), ("5 us/div", 32.0, 2), ("slow", 75.0, 3)] {
            let (reads, k) = run(|i| rest + if i % 3 == 0 { 0.8 } else { -0.4 }, 5.0, 60);
            assert_eq!(k, k_want, "{}", name);
            for w in reads[20..].windows(2) {
                let gap = w[1] - w[0];
                assert!((gap - k as f32 * FRAME_MS).abs() < 2.5, "{}: gap {} ({:?})", name, gap, &reads[20..30]);
            }
            for r in &reads[20..] {
                // lead before a step, within the jitter of the rest and the whole-ms fill
                let ph = (r + lead).rem_euclid(FRAME_MS);
                assert!(ph < 2.5 || ph > FRAME_MS - 2.5, "{}: read {} is {} ms off the lead before a step", name, r, ph);
            }
        }
        // no step known yet: the former fixed wait
        assert_eq!(Fill::default().plan(Instant::now(), |_| None, 2.0), FILL_MAX_MS as u64);
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
