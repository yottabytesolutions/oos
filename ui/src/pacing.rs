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
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// per step with the fast path: its queued-frame drawing and merge (ms)
    fast: VecDeque<[f32; 2]>,
    last_pass: Option<Instant>,
    last_batch: Option<Instant>,
    slow_logged: u64,
    /// the settings change being followed (`changed`), and the last results (`Settle`)
    settle: Option<Settle>,
    settled: VecDeque<Settle>,
    /// per picture (`composition`): frames drawn by a deposit kernel, frames drawn through the record path, and the
    /// age (ms) of the newest batch the pipe merged into it
    comps: VecDeque<(u32, u32, Option<f32>)>,
}

/// From a change of the acquisition settings to steady pictures again: times (ms after the change) of the hardware
/// configuration, the first picture with new frames and the picture that began `STEADY` pictures with new frames in a
/// row; pictures without new frames in between.
#[derive(Clone, Copy, Debug)]
struct Settle {
    at: Instant,
    configured: Option<f32>,
    first: Option<f32>,
    steady: Option<f32>,
    run_from: Option<f32>,
    run: u32,
    empty: u32,
}

/// Pictures with new frames in a row that count as steady again.
const STEADY: u32 = 10;
/// A change not steady after this long is reported as such.
const SETTLE_MAX_MS: f32 = 5000.0;

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
    let now = Instant::now();
    push(&mut s.pics, (now, frames as u32));
    if let Some(mut c) = s.settle {
        let ms = now.saturating_duration_since(c.at).as_secs_f32() * 1e3;
        if frames > 0 {
            c.first.get_or_insert(ms);
            c.run_from.get_or_insert(ms);
            c.run += 1;
            if c.run >= STEADY {
                c.steady = c.run_from;
            }
        } else {
            c.empty += 1;
            (c.run, c.run_from) = (0, None);
        }
        if c.steady.is_some() || ms > SETTLE_MAX_MS {
            eprintln!("oos-ui pacing: settings change: configured {:?} ms, first new frames {:?} ms, steady {:?} ms, {} empty pictures", c.configured.map(|x| x.round()), c.first.map(|x| x.round()), c.steady.map(|x| x.round()), c.empty);
            push(&mut s.settled, c);
            s.settle = None;
        } else {
            s.settle = Some(c);
        }
    }
}

/// Counters for the status line: new screen mappings of the deposit workers (`Pipe::set_view`).
pub static VIEWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// UI-side times of a step's fast block (ms): drawing the queued frames, merging the workers' maps.
pub fn fast_block(draw_ms: f32, merge_ms: f32) {
    let mut s = state().lock().unwrap();
    push(&mut s.fast, [draw_ms, merge_ms]);
}

/// Settings generations (docs/architecture/drawing.md, step 0): `WANTED` counts the acquisition settings (`Want`) the
/// UI asked for (`changed`), `APPLIED` the one the acquisition thread acquires under. A frame carries `APPLIED` as it
/// was when it was acquired (`Frame::cfg`); shown while `WANTED` is newer, it was acquired before a change of `Want`
/// (trigger level and slope included): `STALE`. Not covered: changes outside `Want` (`Shared::needs_raw`, `slow`,
/// `mask_want`, `zone_want`).
pub static WANTED: AtomicU64 = AtomicU64::new(0);
pub static APPLIED: AtomicU64 = AtomicU64::new(0);
/// Frames of older settings drawn after a change (above).
pub static STALE: AtomicU64 = AtomicU64::new(0);
/// Batches whose frames reached the picture in more than one merge (drawpipe.rs).
pub static PARTIAL: AtomicU64 = AtomicU64::new(0);

/// A frame acquired under settings generation `cfg` (0: not from the hardware) drawn now: counted when the UI has asked
/// for newer settings since.
pub fn drawn(cfg: u64, frames: u64) {
    if is_stale(cfg) {
        STALE.fetch_add(frames, Ordering::Relaxed);
    }
}

/// `n` stale frames reached the picture through the pipe's maps (counted by `is_stale` when they were drawn).
pub fn drawn_stale(n: u64) {
    STALE.fetch_add(n, Ordering::Relaxed);
}

/// A frame of generation `cfg` would be stale if shown now (`drawn`); the pipe counts it once its maps are merged.
pub fn is_stale(cfg: u64) -> bool {
    stale(cfg, WANTED.load(Ordering::Relaxed))
}

/// A frame of generation `cfg` is stale when the UI has asked for a newer one (`wanted`); 0 is not from the hardware.
fn stale(cfg: u64, wanted: u64) -> bool {
    cfg != 0 && cfg < wanted
}

/// What a picture is made of: `deposit` frames through a deposit kernel (one span per column), `record` frames through
/// the record path (`draw_ex`, about 3.7 times the weight in lines), `age_ms` of the newest pipe batch in it.
pub fn composition(deposit: usize, record: usize, age_ms: Option<f32>) {
    let mut s = state().lock().unwrap();
    push(&mut s.comps, (deposit as u32, record as u32, age_ms));
}

/// The UI asked for other acquisition settings (wave.rs): follow how long until the pictures are steady again.
pub fn changed() {
    WANTED.fetch_add(1, Ordering::Relaxed);
    let mut s = state().lock().unwrap();
    s.settle = Some(Settle {
        at: Instant::now(),
        configured: None,
        first: None,
        steady: None,
        run_from: None,
        run: 0,
        empty: 0,
    });
}

/// The hardware took the settings (hw.rs).
pub fn configured() {
    let mut s = state().lock().unwrap();
    if let Some(c) = s.settle.as_mut().filter(|c| c.configured.is_none()) {
        c.configured = Some(c.at.elapsed().as_secs_f32() * 1e3);
    }
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
        eprintln!(
            "oos-ui pacing: pass interval {:.1} ms (step {:.1} render {:.1} present {:.1})",
            p[0], p[1], p[2], p[3]
        );
    }
    push(&mut s.passes, p);
}

/// A batch of the hardware done: `wait_ms` from the arm (or the wait of a batch armed ahead) to the read, `count` of
/// `asked` frames recorded, `fill_ms` the ring-fill time it had, `read_ms` from the end of the fill to the read done.
pub fn batch(wait_ms: f32, count: u32, asked: u32, fill_ms: u64, read_ms: f32) {
    let mut s = state().lock().unwrap();
    let now = Instant::now();
    let iv = s.last_batch.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32() * 1e3);
    s.last_batch = Some(now);
    push(&mut s.batches, (wait_ms, iv, count, asked));
    push(&mut s.fills, fill_ms as f32);
    push(&mut s.reads, read_ms);
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
    /// the read time the last plan aimed at
    target: Option<Instant>,
}

impl Default for Fill {
    fn default() -> Fill {
        Fill { rest: None, dev: 0.0, target: None }
    }
}

impl Fill {
    /// The fill time, ms, for a batch starting at `start`, given the next screen step after a time (`next`) and the
    /// time to draw a batch after its read (`ready`).
    pub fn plan(&mut self, start: Instant, next: impl Fn(Instant) -> Option<Instant>, ready: f32) -> u64 {
        let rest = self.rest.unwrap_or(5.0);
        let ms = |a: f32| std::time::Duration::from_secs_f32(a.max(0.0) / 1e3);
        // the drawing overlaps the next batch: only the reads need a screen frame between them, each one `ready` +
        // `MARGIN_MS` before a step. The first such read time the batch can make, the slot after the last one's. (Every
        // second slot was tried where a batch does not fit one frame: the longer fill brought more frames and a longer
        // read, and the reads fell 20-30 ms behind their slots; docs/fork/stutter.md.)
        let lead = ready + MARGIN_MS;
        // planned with the rest's usual spread on top: a read a little early still shows in the same picture, a late
        // one a frame later
        let rest = rest + 2.0 * self.dev;
        // no room for a batch in a frame: as fast as it goes (waiting for a slot would read more frames, longer)
        if rest + FILL_MIN_MS > FRAME_MS {
            self.target = None;
            return FILL_MIN_MS as u64;
        }
        let mut lower = start + ms(rest + FILL_MIN_MS);
        if let Some(t) = self.target {
            lower = lower.max(t + ms(0.5 * FRAME_MS));
        }
        let Some(step) = next(lower + ms(lead)) else { return FILL_MAX_MS as u64 };
        let read = step - ms(lead);
        self.target = Some(read);
        let fill = read.saturating_duration_since(start).as_secs_f32() * 1e3 - rest;
        // a batch that starts early (the read before was early) fills longer to keep its slot
        fill.clamp(FILL_MIN_MS, FILL_MAX_MS.max(FRAME_MS)).round() as u64
    }

    /// A batch spent `rest` ms besides its fill: before it (arm, wait of a batch armed ahead, first trigger) and in the
    /// read.
    pub fn done(&mut self, r: f32) {
        let r = r.max(0.0);
        let rest = self.rest.map_or(r, |x| x + 0.3 * (r - x));
        self.dev += 0.3 * ((r - rest).abs() - self.dev);
        self.rest = Some(rest);
    }
}

/// The fill a capped batch keeps room for (the ring then fills in it, and the planner keeps the reads on their slots).
const CAP_FILL_MS: f32 = 6.0;

/// The most frames a batch reads (`Settings::frames`), where its read would otherwise leave no room for a batch in a
/// screen frame. At 5 us/div with the live compression (no prearm) a batch spent 12-20 ms arming and 21-29 ms reading
/// ~25 frames at ~0.8 ms each: it did not fit 33 ms, and about one picture in ten had nothing new
/// (docs/fork/stutter.md). The cap comes from the read time per frame and the time before the fill (both smoothed);
/// a full ring ends the fill early.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameCap {
    per_frame: Option<f32>,
    pre: Option<f32>,
    /// mean deviation of the rest (time before the fill and read)
    dev: f32,
    cap: Option<u32>,
}

impl FrameCap {
    /// `frames` (what the batch would read) capped.
    pub fn apply(&self, frames: u32) -> u32 {
        self.cap.map_or(frames, |c| frames.min(c))
    }

    /// A batch read `count` frames in `read_ms` after `pre_ms` from its start to the start of its fill; `asked` is
    /// what it would read without the cap. `keep` (prearm: a new cap re-arms): only a change of more than a quarter.
    pub fn done(&mut self, count: u32, read_ms: f32, pre_ms: f32, asked: u32, keep: bool) {
        if count == 0 || read_ms <= 0.0 {
            return;
        }
        let ema = |x: Option<f32>, v: f32| Some(x.map_or(v, |x| x + 0.2 * (v - x)));
        if let (Some(per), Some(pre)) = (self.per_frame, self.pre) {
            let r = pre_ms.max(0.0) + read_ms;
            self.dev += 0.2 * ((r - pre - per * count as f32).abs() - self.dev);
        }
        self.per_frame = ema(self.per_frame, read_ms / count as f32);
        self.pre = ema(self.pre, pre_ms.max(0.0));
        let (per, pre) = (self.per_frame.unwrap_or(1.0), self.pre.unwrap_or(0.0));
        // the read that leaves the batch a frame: the time before the fill, its spread, a fill of `CAP_FILL_MS` and the
        // margin taken off
        let budget = FRAME_MS - MARGIN_MS - CAP_FILL_MS - pre - 2.0 * self.dev;
        let want = ((budget / per.max(0.01)).floor().max(1.0) as u32).min(asked.max(1));
        let new = (want < asked).then_some(want);
        let far = match (self.cap, new) {
            (Some(a), Some(b)) => (a as f32 - b as f32).abs() > 0.25 * a as f32,
            (a, b) => a != b,
        };
        if !keep || far {
            self.cap = new;
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
    let mut fd: Vec<f32> = s.fast.iter().map(|x| x[0]).collect();
    let mut fm: Vec<f32> = s.fast.iter().map(|x| x[1]).collect();
    let settles: Vec<String> = s
        .settled
        .iter()
        .rev()
        .take(4)
        .map(|c| {
            format!(
                "{}/{}/{}/{}",
                c.configured.map_or(-1.0, |x| x.round()),
                c.first.map_or(-1.0, |x| x.round()),
                c.steady.map_or(-1.0, |x| x.round()),
                c.empty
            )
        })
        .collect();
    let nb = s.batches.len().max(1);
    let mut cd: Vec<f32> = s.comps.iter().map(|c| c.0 as f32).collect();
    let mut cr: Vec<f32> = s.comps.iter().map(|c| c.1 as f32).collect();
    // the weight of a picture in deposit frames: a record-path frame weighs about 3.7 (docs/architecture/drawing.md)
    let mut cw: Vec<f32> = s.comps.iter().map(|c| c.0 as f32 + 3.7 * c.1 as f32).collect();
    let mut age: Vec<f32> = s.comps.iter().filter_map(|c| c.2).collect();
    let p3 = |v: &mut Vec<f32>| format!("{:.0}/{:.0}/{:.0}", pct(v, 0.1), pct(v, 0.5), pct(v, 0.9));
    let comp = format!(
        " picture deposit p10/p50/p90 {} record {} weight {} age p50/p90/p99/max {} ms stale {} partial {} wanted {} applied {}",
        p3(&mut cd),
        p3(&mut cr),
        p3(&mut cw),
        format!("{:.1}/{:.1}/{:.1}/{:.1}", pct(&mut age, 0.5), pct(&mut age, 0.9), pct(&mut age, 0.99), pct(&mut age, 1.0)),
        STALE.load(Ordering::Relaxed),
        PARTIAL.load(Ordering::Relaxed),
        WANTED.load(Ordering::Relaxed),
        APPLIED.load(Ordering::Relaxed),
    );
    let p4 = |v: &mut Vec<f32>| format!("{:.1}/{:.1}/{:.1}/{:.1}", pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), pct(v, 1.0));
    format!(
        " pacing pics {} new p10/p50/p90 {:.0}/{:.0}/{:.0} empty {:.0}% gap p50/p90/p99/max {} ms pass {} ms step {} ms batch wait {} ms every {} ms frames p50 {:.0} full {:.0}% fill p50 {:.0} ms until fill end {} ms read {} ms drawn {} ms ui fast {} merge {} ms views {}{} settle configured/first/steady ms/empty {}",
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
        p4(&mut pre),
        p4(&mut re),
        p4(&mut rd),
        p4(&mut fd),
        p4(&mut fm),
        VIEWS.load(std::sync::atomic::Ordering::Relaxed),
        comp,
        if settles.is_empty() { "-".to_string() } else { settles.join(" ") },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Break it catches (step 0, docs/architecture/drawing.md): a frame of the current settings counted as stale, one
    // of older settings not counted, a frame not from the hardware (generation 0) counted, or the picture composition
    // missing from the status line.
    #[test]
    fn stale_frames_and_composition() {
        assert!(!stale(0, 5) && !stale(5, 5) && !stale(6, 5) && stale(4, 5));
        let w0 = WANTED.load(Ordering::Relaxed);
        changed();
        changed();
        assert!(WANTED.load(Ordering::Relaxed) >= w0 + 2, "a settings change raises the wanted generation");
        let s0 = STALE.load(Ordering::Relaxed);
        drawn(1, 3);
        assert!(STALE.load(Ordering::Relaxed) >= s0 + 3);
        composition(20, 1, Some(12.5));
        let line = summary();
        assert!(
            line.contains(" picture deposit p10/p50/p90 ") && line.contains(" stale ") && line.contains(" partial "),
            "{}",
            line
        );
        assert!(line.find(" picture deposit").unwrap() < line.find(" settle ").unwrap(), "settle stays last");
    }

    // Break it catches: a cap where the batch fits a frame anyway, no cap where its read takes the frame (5 us/div), a
    // cap that changes with every batch while a prearmed batch would have to be re-armed for it.
    #[test]
    fn frame_cap_where_the_read_takes_the_frame() {
        let mut c = FrameCap::default();
        // 1 us/div: 58 frames in 10 ms, 5 ms before the fill: fits, no cap
        for _ in 0..10 {
            c.done(58, 10.0, 5.0, 100, false);
        }
        assert_eq!(c.apply(100), 100);
        // 5 us/div: 25 frames in 20 ms (0.8 ms each), 12 ms before the fill: 33.3 - 3 - 6 - 12 = 12.3 ms of read
        let mut c = FrameCap::default();
        for _ in 0..20 {
            c.done(25, 20.0, 12.0, 32, false);
        }
        assert_eq!(c.apply(32), 15);
        // with prearm a small change keeps the cap, a large one takes the new one
        for _ in 0..20 {
            c.done(15, 13.5, 12.0, 32, true);
        }
        assert_eq!(c.apply(32), 15, "13 is within a quarter of 15");
        for _ in 0..20 {
            c.done(15, 24.0, 12.0, 32, true);
        }
        // 1.6 ms a frame: 12.3 / 1.6 = 7, one less while the spread of the change has not settled
        assert!((6..=7).contains(&c.apply(32)), "{}", c.apply(32));
    }

    /// Batches against screen steps every `FRAME_MS`: each batch takes `rest(k)` ms besides its fill, the next one
    /// starts 0.5 ms after a read. Returns the read times (ms).
    fn run(rest: impl Fn(usize) -> f32, ready: f32, n: usize) -> Vec<f32> {
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
            f.done(r);
            reads.push(read);
            now = read + 0.5;
        }
        reads
    }

    // Break it catches: reads that drift against the screen steps (pictures without a new batch next to pictures with
    // two), reads that land after the step minus the drawing time, a batch that does not fit a frame (its reads then
    // come as fast as they can, not a frame late on purpose).
    #[test]
    fn reads_come_evenly_before_the_screen_steps() {
        let lead = 5.0 + MARGIN_MS;
        for (name, rest) in [("200 ns/div", 7.0f32), ("1 us/div", 15.0), ("50 us/div", 18.0)] {
            let reads = run(|i| rest + if i % 3 == 0 { 0.8 } else { -0.4 }, 5.0, 60);
            for w in reads[20..].windows(2) {
                let gap = w[1] - w[0];
                assert!((gap - FRAME_MS).abs() < 2.5, "{}: gap {} ({:?})", name, gap, &reads[20..30]);
            }
            for r in &reads[20..] {
                // lead before a step, within the jitter of the rest and the whole-ms fill
                let ph = (r + lead).rem_euclid(FRAME_MS);
                assert!(
                    ph < 2.5 || ph > FRAME_MS - 2.5,
                    "{}: read {} is {} ms off the lead before a step",
                    name,
                    r,
                    ph
                );
            }
        }
        // 32 ms besides the fill (5 us/div): the least fill, a read every 36.5 ms
        let reads = run(|_| 32.0, 5.0, 40);
        assert!(
            reads[20..].windows(2).all(|w| (w[1] - w[0] - (32.0 + FILL_MIN_MS + 0.5)).abs() < 0.6),
            "{:?}",
            &reads[20..26]
        );
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
        let pics: VecDeque<(Instant, u32)> =
            [(0, 4), (33, 2), (66, 0), (99, 5)].iter().map(|&(ms, n)| (t + Duration::from_millis(ms), n)).collect();
        let g = gaps(&pics);
        assert_eq!(g.len(), 2);
        assert!((g[0] - 33.0).abs() < 0.5 && (g[1] - 66.0).abs() < 0.5, "{:?}", g);
    }
}
