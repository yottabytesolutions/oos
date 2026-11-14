// SPDX-License-Identifier: GPL-2.0-or-later

//! Deposit pipeline (fork, docs/fork/perf-plan.md item 1): every acquired frame is used. In `batch` mode the
//! acquisition thread decodes only the newest frame of a batch for the UI queue (the record for measurements, layers
//! and the record path) and hands the read-out buffer itself (no copy) with the other frames to the deposit workers,
//! one per little core. The workers claim the frames a few at a time; while the FPGA's ring fills with the next batch
//! the acquisition thread claims single frames too (`help`, from `seq::acquire_batch_idle`). Each frame is
//! decoded into kept vectors and drawn with the fast path (`draw_frame`, the same function as `Ui::fast_draw`) into
//! the claimer's own maps, with the screen mapping the UI published (`View`); the trigger statistics and the cheap
//! measurements (framestats.rs) are counted with it. Once per screen refresh the UI adds the maps to its phosphor
//! (`merge`); it never iterates the frames.
//!
//! The picture is the one of the `frames` mode (every frame through the queue) for the same frames: the workers run
//! `draw_frame` on the frames the UI would have fast-drawn, and the newest frame of each batch takes the UI's path.
//! A batch whose workers are still busy, frames of older settings and maps of an older mapping are dropped and
//! counted (`Counters`). Where the fast path does not apply (no view), the acquisition thread queues every frame as in
//! `frames` mode. Mode: `acq.pipeline = batch` in system.conf or OOS_PIPELINE=batch; `frames` (default) is the old
//! path, kept for comparison until the unit has verified the new one.

use crate::board::Frame;
use crate::framecost::{self as cost, Part};
use crate::model::*;
use crate::wave::Phosphor;
use std::any::Any;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

/// Trigger statistics of the fast path (`Ui::fast_edges`, `Ui::fast_off`).
pub struct Stats<'a> {
    pub edges: &'a mut [u64; 4],
    pub off: &'a mut (f64, f64),
}

/// One frame of the fast path into `ph`; false if its trigger edge was not found near the hardware trigger point.
/// Sparse frames (fewer than 2 samples per column) take the record path (`sparse::draw`, else `board::to_record`).
pub fn draw_frame(ph: &mut Phosphor, m: &Model, f: &Frame, stats: Option<Stats>) -> bool {
    let (w, h) = (ph.w, ph.h);
    let tdiv = m.time_per_div() as f64;
    let x_per_sample = f.dt / tdiv * w as f64 / HDIV as f64;
    // min/max pairs of the live compression (compress.rs) take the dense path with a pair deposit
    let pairs = crate::compress::live(f) && x_per_sample <= 0.5;
    // the raw screen window of a deep record (`deepmeas`, once a second) takes the record path of its record's pair
    // frames: drawn densely it was 1.44 times their brightness (lines) and the grade swung (docs/fork/flicker.md)
    if !pairs && (x_per_sample > 0.5 || f.envelope || f.roll || f.scan || crate::deepmeas::deep_raw_frame(f)) {
        if let Some(t) = crate::sparse::draw(ph, m, f, 24) {
            return t;
        }
        let fr = std::sync::Arc::new(f.clone());
        let window = tdiv * HDIV as f64;
        let rec = crate::board::to_record(&fr, m, 3600, window, m.delay as f64 - window / 2.0);
        for c in 0..NCH {
            if m.ch[c].on && !rec.ch[c].is_empty() {
                ph.draw_ex(c, m, (m.time_per_div(), m.delay), rec.t0, rec.dt, &rec.ch[c], 24, None, rec.pairs);
            }
        }
        return rec.triggered;
    }
    let src = &f.ch[m.trig_src];
    let present: [bool; NCH] = std::array::from_fn(|c| !f.ch[c].is_empty());
    let refine = |hyst: f32, rising: bool, either: bool, half: usize, col: usize| {
        let t = std::time::Instant::now();
        let level = crate::board::sw_trig_level(m, f);
        cost::add(Part::Level, t);
        refine_near(src, 0, src.len(), f.pre, near_only(m, f), level, hyst, rising, either, half, col).unwrap_or_else(|| refine_wide(std::time::Instant::now(), src, f.pre, level, hyst, rising, either, half, col))
    };
    let deposit = |c: usize, map: &mut [u16], xm: &crate::fastdraw::Map| {
        if pairs {
            crate::compress::deposit_pairs(map, w, h, &f.ch[c], xm, 24, m.dots_at(x_per_sample));
        } else if m.dots_at(x_per_sample) {
            crate::fastdraw::deposit_dots(map, w, h, &f.ch[c], xm, 24);
        } else {
            crate::fastdraw::deposit(map, w, h, &f.ch[c], xm, 24);
        }
    };
    dense(ph, m, f, x_per_sample, pairs, present, stats, refine, deposit)
}

/// The dense fast path of `draw_frame` for the frame `f` (its samples reached only through `refine`, the trigger
/// refinement on the source channel, and `deposit`, one channel into its map): trigger statistics, mapping, channels.
#[allow(clippy::too_many_arguments)]
fn dense(ph: &mut Phosphor, m: &Model, f: &Frame, x_per_sample: f64, pairs: bool, present: [bool; NCH], stats: Option<Stats>, refine: impl FnOnce(f32, bool, bool, usize, usize) -> (Option<f64>, Option<oos_dsp::edges::Edge>), mut deposit: impl FnMut(usize, &mut [u16], &crate::fastdraw::Map)) -> bool {
    let (w, h) = (ph.w, ph.h);
    let tdiv = m.time_per_div() as f64;
    // an LA line as the source: the hardware trigger point stands, the analog samples cannot refine it
    let on_la = m.trig_on_la();
    // the edge of the set direction nearest the hardware trigger point, up to half a screen away, on the signal smoothed
    // to one screen column: at decimated rates the hardware point lands near the level on either edge (trigger.md 8)
    let hyst = m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div();
    let (rising, either) = (m.slope != Slope::Fall, m.slope == Slope::Either);
    let half = (tdiv * HDIV as f64 / 2.0 / f.dt) as usize;
    let col = if pairs { crate::compress::smooth_len(x_per_sample) } else { (1.0 / x_per_sample).floor().clamp(1.0, 256.0) as usize };
    let edge = if on_la {
        None
    } else {
        let (e, any) = refine(hyst, rising, either, half, col);
        if let Some(s) = stats {
            let k = match (e, any) {
                (None, _) => 3,
                (Some(t), _) if (t - f.pre as f64).abs() <= (col + 16) as f64 => 0,
                (Some(_), Some(n)) if !either && n.rising != rising => 1,
                (Some(t), _) => {
                    let d = (t - f.pre as f64).abs();
                    *s.off = (s.off.0 + d, s.off.1.max(d));
                    2
                }
            };
            s.edges[k] += 1;
        }
        e
    };
    let ti = edge.unwrap_or(f.pre as f64);
    let x0 = w as f64 / 2.0 - m.delay as f64 / tdiv * w as f64 / HDIV as f64;
    let ppd_y = h as f32 / VDIV;
    for c in 0..NCH {
        let ch = &m.ch[c];
        if !ch.on || !present[c] {
            continue;
        }
        let vpd = ch.volts_per_div();
        let sign = -1.0;
        let xm = crate::fastdraw::Map { ti, x_per_sample, x0, y_per_volt: sign * ppd_y / vpd, y0: h as f32 / 2.0 - ch.offset / vpd * ppd_y };
        deposit(c, &mut ph.map[c], &xm);
    }
    // a filtered trigger path (coupling): the hardware point stands where the samples show no crossing
    edge.is_some() || on_la || m.trig_coupling() != TrigCoup::Dc
}

/// Smallest read-out the workers draw from its words (`draw_frame_codes`): 64 kB. Below it the f32 copy stays in the
/// caches and decoding first was as fast or faster on the host (12.5 k points, 1 channel: 1.08x the time from the words).
pub const CODES_MIN_BYTES: usize = 64 << 10;

/// Most words per group the workers draw from the words: each channel's deposit reads every group, so with four
/// channels the read-out is read four times; on the host that was 0.95x the decode path, too little to rely on.
pub const CODES_MAX_STRIDE: usize = 2;

/// Where a frame's read-out holds each channel (`fastdraw::Codes`), as the board gives it (`CodesInto`).
#[derive(Clone, Copy)]
pub struct CodeLayout {
    pub stride: usize,
    pub word: [Option<usize>; NCH],
    pub affine: [(f32, f32); NCH],
    pub volt: fn(u16, (f32, f32)) -> f32,
    /// volts of samples a..b of one channel (bytes, stride, word, a, b, affine, out): the board's decode loop
    pub volts: fn(&[u8], usize, usize, usize, usize, (f32, f32), &mut Vec<f32>),
}

impl CodeLayout {
    /// Channel `c` of the read-out `bytes` (empty when not in it).
    pub fn channel<'a>(&self, bytes: &'a [u8], c: usize) -> crate::fastdraw::Codes<'a> {
        let words = bytes.len() / 2;
        let (word, n) = match self.word[c] {
            Some(k) if k < words => (k, (words - k).div_ceil(self.stride)),
            _ => (0, 0),
        };
        // SAFETY: u16 has no invalid bit patterns; align_to only hands out the aligned middle part
        let (pre, words, _) = unsafe { bytes.align_to::<u16>() };
        let words = (pre.is_empty() && cfg!(target_endian = "little")).then_some(words);
        crate::fastdraw::Codes { bytes, words, stride: self.stride, word, n, affine: self.affine[c], volt: self.volt }
    }
}

/// The board's view of a frame without decoding it: sets every field of the frame but the channel vectors (left
/// empty) as the decode would, and says where the channels are; None where only the decode will do (compressed or
/// with a header).
pub type CodesInto = Arc<dyn Fn(&[u8], &mut Frame) -> Option<CodeLayout> + Send + Sync>;

/// The trigger refinement of frame `f` searches the near window only (`trignear::edge_near`), never wide
/// (docs/fork/ac-trig.md, docs/fork/trig-gate.md). AC or LF-reject coupling: the hardware comparators see a
/// high-passed signal the samples do not hold, so a farther crossing is not the one they fired on. An edge trigger
/// on a frame whose `pre` is exact: the edge is at `pre` (or in its pair), within the near window when the column
/// average blurs it; with noise reject and with a holdoff too (measured at the full rate, trig-gate.md). Kept wide:
/// other trigger types (they fire at the end of a condition, possibly far from the edge).
// not inlined: inlined into the codes path it cost the two-channel deposit 6-7 us a frame on the host (perf guard
// ratio 0.76 -> 0.90), the same code at run time either way
#[inline(never)]
fn near_only(m: &Model, f: &Frame) -> bool {
    let filtered = !matches!(m.trig_coupling(), TrigCoup::Dc | TrigCoup::HfReject);
    let exact = f.pre_exact && m.trig_type == TrigType::Edge;
    filtered || exact
}

/// The near part of the dense path's trigger refinement on the source `v` (samples `base..` of `len`, covering
/// `trignear::near_window`): with `near_only` it decides there alone (`trignear::edge_near`), else the near search of
/// `trignear::edge_smooth_near`; None where the wide search (`refine_wide`) must decide.
#[allow(clippy::too_many_arguments)]
fn refine_near(v: &[f32], base: usize, len: usize, pre: usize, near_only: bool, level: f32, hyst: f32, rising: bool, either: bool, half: usize, col: usize) -> Option<(Option<f64>, Option<oos_dsp::edges::Edge>)> {
    let t = std::time::Instant::now();
    let r = if near_only {
        Some(crate::trignear::edge_near(v, base, len, pre, level, hyst, rising, either, half, col))
    } else {
        crate::trignear::edge_near_window(v, base, len, pre, level, hyst, rising, either, half, col, |_| true)
    };
    cost::add(Part::Near, t);
    r
}

/// The wide part: `fastdraw::edge_smooth` on all samples `v` of the source, timed from `t` (the codes path includes
/// its decode).
#[allow(clippy::too_many_arguments)]
fn refine_wide(t: std::time::Instant, v: &[f32], pre: usize, level: f32, hyst: f32, rising: bool, either: bool, half: usize, col: usize) -> (Option<f64>, Option<oos_dsp::edges::Edge>) {
    let r = crate::fastdraw::edge_smooth(v, pre, level, hyst, rising, either, half, col, |_| true);
    cost::add(Part::Wide, t);
    r
}

#[cfg(test)]
thread_local! {
    /// wide searches of the codes path (tests)
    static WIDE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// the trigger source's volts that the codes path decodes (the near window, or all of them for the wide search)
    static TRIG: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `draw_frame` straight from the read-out `bytes` of a frame (`lay`, meta data in `f`, whose channel vectors stay
/// empty): the same picture and statistics without the f32 copy of every channel. The trigger source is decoded only
/// around the hardware trigger point (the near search, `trignear::edge_near_window`), all of it only when the wide
/// search is needed (never with `near_only`); a filtered trigger path (AC, LF reject) takes its level from
/// `board::MEAN_SAMPLES` words of the source. None where this path does not apply (sparse, pairs, roll or scan, dots): then decode and call
/// `draw_frame`.
pub fn draw_frame_codes(ph: &mut Phosphor, m: &Model, f: &Frame, bytes: &[u8], lay: &CodeLayout, stats: Option<Stats>) -> Option<bool> {
    let (w, h) = (ph.w, ph.h);
    let tdiv = m.time_per_div() as f64;
    let x_per_sample = f.dt / tdiv * w as f64 / HDIV as f64;
    if x_per_sample > 0.5 || f.envelope || f.roll || f.scan || m.dots_at(x_per_sample) {
        return None;
    }
    let present: [bool; NCH] = std::array::from_fn(|c| lay.channel(bytes, c).n > 0);
    let refine = |hyst: f32, rising: bool, either: bool, half: usize, col: usize| {
        use crate::fastdraw::Samples;
        let src = lay.channel(bytes, m.trig_src);
        let t = std::time::Instant::now();
        let level = crate::board::sw_trig_level_of(m, src.n, |i| src.at(i));
        cost::add(Part::Level, t);
        TRIG.with(|t| {
            let v = &mut *t.borrow_mut();
            let (a, b) = crate::trignear::near_window(f.pre, src.n, col);
            let t0 = std::time::Instant::now();
            (lay.volts)(bytes, src.stride, src.word, a, b, src.affine, v);
            cost::add(Part::Decode, t0);
            if let Some(r) = refine_near(v, a, src.n, f.pre, near_only(m, f), level, hyst, rising, either, half, col) {
                return r;
            }
            #[cfg(test)]
            WIDE.with(|n| n.set(n.get() + 1));
            let t = std::time::Instant::now();
            (lay.volts)(bytes, src.stride, src.word, 0, src.n, src.affine, v);
            refine_wide(t, v, f.pre, level, hyst, rising, either, half, col)
        })
    };
    let deposit = |c: usize, map: &mut [u16], xm: &crate::fastdraw::Map| crate::fastdraw::deposit_codes(map, w, h, &lay.channel(bytes, c), xm, 24);
    Some(dense(ph, m, f, x_per_sample, false, present, stats, refine, deposit))
}

/// Frames acquired, used (drawn into the picture) and dropped, since the start; in both modes.
#[derive(Default)]
pub struct Counters {
    pub acquired: AtomicU64,
    pub used: AtomicU64,
    pub dropped: AtomicU64,
    /// frames whose cheap measurements went into the statistics (framestats.rs)
    pub measured: AtomicU64,
}

/// The process's counters (the acquisition thread, the workers and the UI add to them).
pub fn counters() -> &'static Counters {
    static C: OnceLock<Counters> = OnceLock::new();
    C.get_or_init(Counters::default)
}

impl Counters {
    pub fn add(c: &AtomicU64, n: u64) {
        c.fetch_add(n, Ordering::Relaxed);
    }
    /// (acquired, used, dropped)
    pub fn read(&self) -> (u64, u64, u64) {
        (self.acquired.load(Ordering::Relaxed), self.used.load(Ordering::Relaxed), self.dropped.load(Ordering::Relaxed))
    }
}

/// The pipeline mode: `batch` (this pipeline) or `frames` (every frame through the UI queue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Frames,
    Batch,
}

/// `env` if set, else the value of `key` in system.conf text `conf` (a fork key: kept by `SysConf` as an unknown line).
pub fn conf_value(env: Option<&str>, conf: &str, key: &str) -> Option<String> {
    let conf_val = conf.lines().filter_map(|l| l.split('#').next()?.split_once('=')).find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string());
    env.map(str::to_string).or(conf_val)
}

/// OOS_PIPELINE, else the `acq.pipeline` line of system.conf, else `frames`.
pub fn mode_from(env: Option<&str>, conf: &str) -> Mode {
    match conf_value(env, conf, "acq.pipeline").as_deref() {
        Some("batch") => Mode::Batch,
        _ => Mode::Frames,
    }
}

/// A read-out buffer of the board (the acquisition thread's DMA target), lent to the workers.
pub trait RawBatch: Send + Sync {
    fn bytes(&self) -> &[u8];
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
}

/// Decodes the frame bytes into a kept frame (the board's decoder with the batch's settings).
pub type DecodeInto = Arc<dyn Fn(&[u8], &mut Frame) + Send + Sync>;

/// The buffer while the workers have it: back to the acquisition thread's pool when the last share is done.
struct Lease {
    raw: Option<Box<dyn RawBatch>>,
    home: Sender<Box<dyn RawBatch>>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(r) = self.raw.take() {
            // the pool is gone only when the pipe is: nothing to return to
            let _ = self.home.send(r);
        }
    }
}

/// A batch with the workers: its frames are claimed a few at a time by whoever is free (the workers, and the
/// acquisition thread while it waits for the next batch: `help`). Frames never claimed count as dropped.
struct Batch {
    lease: Lease,
    fb: usize,
    frames: usize,
    decode: DecodeInto,
    /// the board's view without decoding (`draw_frame_codes`), where it has one
    codes: Option<CodesInto>,
    stamp: u64,
    next: AtomicUsize,
    counters: &'static Counters,
    /// batches not done yet (`Pipe::live`)
    live: Arc<AtomicUsize>,
    /// what the acquisition thread wants done once every frame is drawn (the newest frame to the UI queue), put on
    /// `ready` for the UI's next `merge`
    deliver: Mutex<Option<Deliver>>,
    ready: Arc<Mutex<Vec<Deliver>>>,
    /// a number of its own (`Acc::batches`, partial merges), its settings generation (`pacing::APPLIED` when fed:
    /// stale frames) and when it was fed, right after its read (picture age)
    id: u64,
    cfg: u64,
    fed: std::time::Instant,
}

/// Run by the UI after the merge that holds a batch's frames (`Pipe::feed_with`).
pub type Deliver = Box<dyn FnOnce() + Send>;

impl Batch {
    /// The next `n` frames nobody has taken yet.
    fn claim(&self, n: usize) -> Option<Range<usize>> {
        let i = self.next.fetch_add(n, Ordering::Relaxed);
        (i < self.frames).then(|| i..(i + n).min(self.frames))
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        let left = self.frames.saturating_sub(self.next.load(Ordering::Relaxed));
        Counters::add(&self.counters.dropped, left as u64);
        // every frame is in a mailbox now (the workers hand on before they let go of the batch)
        if let Some(d) = self.deliver.lock().unwrap().take() {
            crate::pacing::ready();
            self.ready.lock().unwrap().push(d);
        }
        self.live.fetch_sub(1, Ordering::Release);
    }
}

/// Frames a worker claims at a time.
const CLAIM: usize = 4;

/// The screen mapping the workers draw with, and a generation that changes with anything the drawing depends on.
struct View {
    gen: u64,
    m: Model,
    w: usize,
    h: usize,
}

/// The fields of the model `draw_frame` depends on (a change of any other field keeps the workers' maps).
#[derive(Clone, PartialEq)]
struct Key {
    ch: [Channel; NCH],
    tscale: usize,
    delay: f32,
    trig: (usize, Option<u8>, TrigType, Slope, f32, TrigCoup, bool, bool),
    acq: AcqMode,
    dots: DotsMode,
    size: (usize, usize),
}

impl Key {
    fn of(m: &Model, w: usize, h: usize) -> Key {
        Key { ch: m.ch.clone(), tscale: m.tscale, delay: m.delay, trig: (m.trig_src, m.trig_line(), m.trig_type, m.slope, m.trig_level, m.trig_coup, m.trig_nrej, m.trig_on_la()), acq: m.acq, dots: m.dots, size: (w, h) }
    }
}

/// What a worker drew and the UI has not merged yet.
struct Acc {
    gen: u64,
    ph: Phosphor,
    /// statistics of the cheap measurements over the frames (framestats.rs)
    folds: crate::framestats::Folds,
    frames: u64,
    triggered: u64,
    edges: [u64; 4],
    off: (f64, f64),
    /// the channels drawn into (on in the view): only their maps are added and cleared
    used: [bool; NCH],
    /// the batches with frames in these maps (`Batch::id`) and when the newest of them was fed
    batches: Vec<u64>,
    newest: Option<std::time::Instant>,
}

impl Acc {
    fn new(gen: u64, w: usize, h: usize) -> Acc {
        Acc { gen, ph: Phosphor::new(w, h), folds: Vec::new(), frames: 0, triggered: 0, edges: [0; 4], off: (0.0, 0.0), used: [true; NCH], batches: Vec::new(), newest: None }
    }
    /// Empty again, keeping the maps' memory.
    fn reset(&mut self, gen: u64) {
        for (m, u) in self.ph.map.iter_mut().zip(self.used) {
            if u {
                m.iter_mut().for_each(|x| *x = 0);
            }
        }
        self.folds.clear();
        self.batches.clear();
        self.newest = None;
        (self.gen, self.frames, self.triggered, self.edges, self.off) = (gen, 0, 0, [0; 4], (0.0, 0.0));
    }
    fn add(&mut self, o: &Acc) {
        add(&mut self.ph, &o.ph, o.used);
        for (a, b) in self.used.iter_mut().zip(o.used) {
            *a |= b;
        }
        crate::framestats::add_folds(&mut self.folds, &o.folds);
        self.frames += o.frames;
        self.triggered += o.triggered;
        for (a, b) in self.edges.iter_mut().zip(o.edges) {
            *a += b;
        }
        self.off = (self.off.0 + o.off.0, self.off.1.max(o.off.1));
        for &b in &o.batches {
            self.note(b, o.newest);
        }
    }
    /// Frames of batch `id`, fed at `fed`, drawn into these maps.
    fn note(&mut self, id: u64, fed: Option<std::time::Instant>) {
        if !self.batches.contains(&id) {
            self.batches.push(id);
        }
        self.newest = self.newest.max(fed);
    }
}

/// A worker's mailbox: the drawn maps the UI takes, and the maps it gives back to be cleared and reused.
#[derive(Default)]
struct Mail {
    /// handed on, in order; the worker never adds under the lock (the UI waited 18-34 ms for that on the unit)
    full: Vec<Acc>,
    spare: Vec<Acc>,
}

/// Maps handed on that a mailbox holds at most before a worker adds into the last one.
const MAIL_MAX: usize = 4;

/// The acquisition thread's state as a helper (`Pipe::help`): its maps and its kept frame.
#[derive(Default)]
pub struct Helper {
    acc: Option<Acc>,
    spare: Option<Acc>,
    f: Frame,
}

/// The view and the settings stamp the UI is at; None while the fast path does not apply.
#[derive(Default)]
struct ViewState {
    view: Option<Arc<View>>,
    stamp: u64,
    key: Option<Key>,
    gen: u64,
}

pub struct Pipe {
    view: Arc<Mutex<ViewState>>,
    counters: &'static Counters,
    workers: Vec<SyncSender<Arc<Batch>>>,
    /// the workers' mailboxes and, last, the helper's
    mails: Vec<Arc<Mutex<Mail>>>,
    /// batches the helper may take frames of, oldest first
    open: Mutex<std::collections::VecDeque<std::sync::Weak<Batch>>>,
    /// batches fed and not done (drawn and handed on, or dropped)
    live: Arc<AtomicUsize>,
    /// `Batch::deliver` of the batches done, for the next `merge`
    ready: Arc<Mutex<Vec<Deliver>>>,
    home: Sender<Box<dyn RawBatch>>,
    pool: Mutex<Receiver<Box<dyn RawBatch>>>,
    /// merges so far, and the batches seen in them: (`Batch::id`, merge, counted as partial)
    merges: AtomicU64,
    seen: Mutex<std::collections::VecDeque<(u64, u64, bool)>>,
    /// batches of this pipe merged in parts (also counted in `pacing::PARTIAL`)
    partial: AtomicU64,
}

/// Batches `Pipe::seen` remembers: more than are ever in flight.
const SEEN_KEEP: usize = 64;

/// Merged frames: how many, how many found their trigger, the statistics of the cheap measurements over them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merged {
    pub frames: u64,
    pub triggered: u64,
    pub folds: crate::framestats::Folds,
    /// when the newest batch with frames in this merge was fed (picture age)
    pub newest: Option<std::time::Instant>,
}

impl Pipe {
    /// A pipe with `n` workers counting into `counters`; `little`: the workers go to the little cores.
    pub fn start(n: usize, little: bool, counters: &'static Counters) -> Pipe {
        let view: Arc<Mutex<ViewState>> = Default::default();
        let mut mails = Vec::new();
        let mut workers = Vec::new();
        for k in 0..n {
            // batches are bounded by the read-out buffers (`hw::POOL`), a few at most
            let (tx, rx) = sync_channel::<Arc<Batch>>(16);
            let mail: Arc<Mutex<Mail>> = Default::default();
            let (v, m) = (view.clone(), mail.clone());
            match std::thread::Builder::new().name(format!("deposit{}", k)).spawn(move || {
                if little {
                    crate::http::to_little_cores();
                }
                work(rx, &v, &m)
            }) {
                Ok(_) => {
                    workers.push(tx);
                    mails.push(mail);
                }
                Err(e) => eprintln!("oos-ui: deposit worker: {}", e),
            }
        }
        mails.push(Default::default());
        let (home, pool) = channel();
        Pipe { view, counters, workers, mails, open: Default::default(), live: Default::default(), ready: Default::default(), home, pool: Mutex::new(pool), merges: AtomicU64::new(0), seen: Default::default(), partial: AtomicU64::new(0) }
    }

    /// The UI's mapping for the frames to come (None: the fast path does not apply).
    pub fn set_view(&self, m: Option<&Model>, w: usize, h: usize) {
        let mut v = self.view.lock().unwrap();
        let Some(m) = m else {
            (v.view, v.key) = (None, None);
            drop(v);
            self.release();
            return;
        };
        let key = Key::of(m, w, h);
        if v.key.as_ref() != Some(&key) {
            crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
            v.gen += 1;
            v.view = Some(Arc::new(View { gen: v.gen, m: m.clone(), w, h }));
            v.key = Some(key);
        }
        v.stamp = m.hw_stamp;
    }

    /// The fast path applies: the workers take frames.
    pub fn accepting(&self) -> bool {
        self.view.lock().unwrap().view.is_some()
    }

    /// A buffer back from the workers, to read the next batch into (None: all are still out).
    pub fn take_buffer(&self) -> Option<Box<dyn Any + Send>> {
        self.pool.lock().unwrap().try_recv().ok().map(|r| r.into_any())
    }

    /// `feed_codes` without the board's view of the read-out words (tests).
    #[cfg(test)]
    pub fn feed(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto) {
        self.feed_batch(raw, fb, frames, stamp, decode, None);
    }

    /// From the acquisition thread: the first `frames` frames of `fb` bytes in `raw`, recorded under settings stamp
    /// `stamp`, for the workers (and the helper) to claim, decoded with `decode` or, without per-frame measurements,
    /// drawn from their words with `codes` (`draw_frame_codes`). The buffer comes back through `take_buffer` when the
    /// batch is done. Frames nobody claims are counted as dropped. The unit feeds with `feed_with`.
    #[cfg(test)]
    pub fn feed_codes(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto, codes: Option<CodesInto>) {
        self.feed_batch(raw, fb, frames, stamp, decode, codes);
    }

    /// `feed_codes`, and `deliver` run by the UI right after the merge that brings the batch's frames: the newest
    /// frame of the batch reaches the screen in the same picture as the others (docs/fork/stutter.md).
    #[allow(clippy::too_many_arguments)]
    pub fn feed_with(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto, codes: Option<CodesInto>, deliver: Deliver) {
        let b = self.feed_batch(raw, fb, frames, stamp, decode, codes);
        *b.deliver.lock().unwrap() = Some(deliver);
    }

    /// `feed`, returning the batch (tests hold it to let the helper alone draw it).
    fn feed_batch(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto, codes: Option<CodesInto>) -> Arc<Batch> {
        let frames = if fb == 0 { 0 } else { frames.min(raw.bytes().len() / fb) };
        static IDS: AtomicU64 = AtomicU64::new(0);
        let (id, cfg) = (IDS.fetch_add(1, Ordering::Relaxed) + 1, crate::pacing::APPLIED.load(Ordering::Relaxed));
        let b = Arc::new(Batch { lease: Lease { raw: Some(raw), home: self.home.clone() }, fb, frames, decode, codes, stamp, next: AtomicUsize::new(0), counters: self.counters, live: self.live.clone(), deliver: Mutex::new(None), ready: self.ready.clone(), id, cfg, fed: std::time::Instant::now() });
        self.live.fetch_add(1, Ordering::Acquire);
        {
            let mut open = self.open.lock().unwrap();
            open.retain(|w| w.strong_count() > 0);
            open.push_back(Arc::downgrade(&b));
        }
        for w in &self.workers {
            // full only if the workers are far behind: the others take its frames
            if let Err(TrySendError::Disconnected(_)) = w.try_send(b.clone()) {
                eprintln!("oos-ui: deposit worker gone");
            }
        }
        b
    }

    /// From the acquisition thread while the ring fills (`seq::acquire_batch_idle`): draws one frame of the oldest open batch into the
    /// helper's maps. False when there is none (the helper's maps then go to the UI).
    pub fn help(&self, h: &mut Helper) -> bool {
        let b = {
            let mut open = self.open.lock().unwrap();
            loop {
                match open.front().map(|w| w.upgrade()) {
                    None => break None,
                    Some(Some(b)) if b.next.load(Ordering::Relaxed) < b.frames => break Some(b),
                    Some(_) => {
                        open.pop_front();
                    }
                }
            }
        };
        let Some(b) = b else {
            self.help_done(h);
            return false;
        };
        let Some(r) = b.claim(1) else { return true };
        let (v, stamp) = self.view_now();
        let mails = self.mails.last().expect("helper mailbox");
        match v.filter(|_| b.stamp >= stamp) {
            None => Counters::add(&self.counters.dropped, r.len() as u64),
            Some(v) => {
                if h.acc.as_ref().is_some_and(|a| a.gen != v.gen) {
                    self.help_done(h);
                }
                if h.acc.is_none() {
                    let spare = h.spare.take().or_else(|| mails.lock().unwrap().spare.pop());
                    h.acc = Some(fresh_acc(spare, &v));
                }
                let a = h.acc.as_mut().expect("helper maps");
                draw_range(&b, r, &v, a, &mut h.f, self.counters);
            }
        }
        true
    }

    /// The helper's maps to the UI.
    pub fn help_done(&self, h: &mut Helper) {
        match h.acc.take() {
            Some(a) if a.frames > 0 => h.spare = publish(self.mails.last().expect("helper mailbox"), a, self.counters),
            Some(a) => h.spare = Some(a),
            None => {}
        }
    }

    fn view_now(&self) -> (Option<Arc<View>>, u64) {
        let s = self.view.lock().unwrap();
        (s.view.clone(), s.stamp)
    }

    /// Adds what the workers drew under the current mapping to `ph` and the trigger statistics to `edges`, `off`.
    /// `clear`: minimum persistence, the picture is replaced: cleared first if anything is merged.
    pub fn merge(&self, ph: &mut Phosphor, clear: &mut bool, edges: &mut [u64; 4], off: &mut (f64, f64)) -> Merged {
        self.merges.fetch_add(1, Ordering::Relaxed);
        let gen = self.view.lock().unwrap().view.as_ref().map(|v| v.gen);
        let mut got = Merged::default();
        for mail in &self.mails {
            let full = std::mem::take(&mut mail.lock().unwrap().full);
            let mut back = Vec::with_capacity(full.len());
            for a in full {
                if Some(a.gen) == gen && (a.ph.w, a.ph.h) == (ph.w, ph.h) && a.frames > 0 {
                    if *clear {
                        ph.clear();
                        *clear = false;
                    }
                    add(ph, &a.ph, a.used);
                    for (x, y) in edges.iter_mut().zip(a.edges) {
                        *x += y;
                    }
                    *off = (off.0 + a.off.0, off.1.max(a.off.1));
                    got.frames += a.frames;
                    got.triggered += a.triggered;
                    got.newest = got.newest.max(a.newest);
                    self.note_merged(&a.batches);
                    crate::framestats::add_folds(&mut got.folds, &a.folds);
                } else {
                    Counters::add(&self.counters.dropped, a.frames);
                }
                back.push(a);
            }
            // back to the worker, which clears it
            mail.lock().unwrap().spare.extend(back);
        }
        // after the maps: a batch hands its maps on before it goes on `ready`
        self.release();
        got
    }

    /// Batches `ids` reached the picture in this merge; one that reached it in an earlier merge too came in parts
    /// (`pacing::PARTIAL`, docs/fork/draw-architecture.md step 0).
    fn note_merged(&self, ids: &[u64]) {
        let n = self.merges.load(Ordering::Relaxed);
        let mut seen = self.seen.lock().unwrap();
        for &id in ids {
            match seen.iter_mut().find(|e| e.0 == id) {
                Some(e) if e.1 != n && !e.2 => {
                    e.2 = true;
                    self.partial.fetch_add(1, Ordering::Relaxed);
                    crate::pacing::PARTIAL.fetch_add(1, Ordering::Relaxed);
                }
                Some(_) => {}
                None => {
                    if seen.len() >= SEEN_KEEP {
                        seen.pop_front();
                    }
                    seen.push_back((id, n, false));
                }
            }
        }
    }

    /// Runs the `deliver` of the batches done.
    pub fn release(&self) {
        let ready = std::mem::take(&mut *self.ready.lock().unwrap());
        for d in ready {
            d();
        }
    }

}

/// Empty maps for view `v`, reusing `spare` when it has the size.
fn fresh_acc(spare: Option<Acc>, v: &View) -> Acc {
    let mut a = match spare {
        Some(mut a) if (a.ph.w, a.ph.h) == (v.w, v.h) => {
            a.reset(v.gen);
            a
        }
        _ => Acc::new(v.gen, v.w, v.h),
    };
    a.used = std::array::from_fn(|c| v.m.ch[c].on);
    a.folds = crate::framestats::folds_for(&v.m);
    a
}

/// Frames `r` of batch `b` decoded into `f` and drawn into `a`.
fn draw_range(b: &Batch, r: Range<usize>, v: &View, a: &mut Acc, f: &mut Frame, counters: &Counters) {
    let Some(bytes) = b.lease.raw.as_ref().map(|x| x.bytes()) else { return };
    a.note(b.id, Some(b.fed));
    crate::pacing::drawn(b.cfg, r.len() as u64);
    for i in r {
        let t_frame = std::time::Instant::now();
        let chunk = &bytes[i * b.fb..(i + 1) * b.fb];
        // without per-frame measurements a large frame is drawn from its read-out words where the board has a view of them
        if a.folds.is_empty() && chunk.len() >= CODES_MIN_BYTES {
            let lay = b.codes.as_ref().and_then(|c| c(chunk, f)).filter(|l| l.stride <= CODES_MAX_STRIDE);
            let st = Stats { edges: &mut a.edges, off: &mut a.off };
            if let Some(t) = lay.and_then(|l| draw_frame_codes(&mut a.ph, &v.m, f, chunk, &l, Some(st))) {
                a.triggered += t as u64;
                a.frames += 1;
                cost::add(Part::Frame, t_frame);
                continue;
            }
        }
        let t = std::time::Instant::now();
        (b.decode)(chunk, f);
        cost::add(Part::Decode, t);
        let st = Stats { edges: &mut a.edges, off: &mut a.off };
        a.triggered += draw_frame(&mut a.ph, &v.m, f, Some(st)) as u64;
        if crate::framestats::fold_frame(f, &v.m, &mut a.folds) {
            Counters::add(&counters.measured, 1);
        }
        a.frames += 1;
        cost::add(Part::Frame, t_frame);
    }
}

/// Hands drawn maps to `mail` (added to what the UI has not taken yet); returns maps to draw the next ones into.
fn publish(mail: &Mutex<Mail>, a: Acc, counters: &Counters) -> Option<Acc> {
    let mut mb = mail.lock().unwrap();
    // maps of an older mapping: the UI would drop them
    let (keep, old): (Vec<Acc>, Vec<Acc>) = std::mem::take(&mut mb.full).into_iter().partition(|f| f.gen == a.gen && (f.ph.w, f.ph.h) == (a.ph.w, a.ph.h));
    mb.full = keep;
    for o in old {
        Counters::add(&counters.dropped, o.frames);
        mb.spare.push(o);
    }
    if mb.full.len() < MAIL_MAX {
        mb.full.push(a);
        return mb.spare.pop();
    }
    // the UI is far behind: added under the lock, rare
    if let Some(last) = mb.full.last_mut() {
        last.add(&a);
    }
    Some(a)
}

/// A worker: claims frames of each batch until none are left, draws them into its own maps, then hands them on.
fn work(rx: Receiver<Arc<Batch>>, view: &Mutex<ViewState>, mail: &Mutex<Mail>) {
    let mut f = Frame::default();
    let mut spare: Option<Acc> = None;
    while let Ok(b) = rx.recv() {
        let (v, stamp) = {
            let s = view.lock().unwrap();
            (s.view.clone(), s.stamp)
        };
        let Some(v) = v.filter(|_| b.stamp >= stamp) else {
            // older settings or no fast path: the frames are dropped (counted when the batch goes)
            while let Some(r) = b.claim(b.frames.max(1)) {
                Counters::add(&b.counters.dropped, r.len() as u64);
            }
            continue;
        };
        let mut a = fresh_acc(spare.take().or_else(|| mail.lock().unwrap().spare.pop()), &v);
        while let Some(r) = b.claim(CLAIM) {
            draw_range(&b, r, &v, &mut a, &mut f, b.counters);
        }
        // handed on before the batch goes: once it is gone, its frames are in a mailbox
        if a.frames == 0 {
            spare = Some(a);
        } else {
            spare = publish(mail, a, b.counters);
        }
    }
}

/// `dst` += `src`, saturating, per channel map.
fn add(dst: &mut Phosphor, src: &Phosphor, used: [bool; NCH]) {
    for ((d, s), u) in dst.map.iter_mut().zip(src.map.iter()).zip(used) {
        if u {
            for (x, y) in d.iter_mut().zip(s.iter()) {
                *x = x.saturating_add(*y);
            }
        }
    }
}

/// Workers where the cores are all alike (the host): two.
const WORKERS: usize = 2;

/// The pipe in `batch` mode (started on first use), else None.
pub fn pipe() -> Option<&'static Pipe> {
    static P: OnceLock<Option<Pipe>> = OnceLock::new();
    P.get_or_init(|| {
        let env = std::env::var("OOS_PIPELINE").ok();
        let conf = crate::sysconf::fork_text();
        (mode_from(env.as_deref(), &conf) == Mode::Batch).then(|| Pipe::start(crate::placement::little_count().unwrap_or(WORKERS), true, counters()))
    })
    .as_ref()
}

/// The mode in force, for the status line.
pub fn mode() -> Mode {
    if pipe().is_some() { Mode::Batch } else { Mode::Frames }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oos_hal::dho::hz;
    use oos_hal::dho::seq::Settings;

    /// A read-out buffer for the tests.
    struct Bytes(Vec<u8>);

    impl RawBatch for Bytes {
        fn bytes(&self) -> &[u8] {
            &self.0
        }
        fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
            self
        }
    }

    fn fresh() -> &'static Counters {
        Box::leak(Box::default())
    }

    fn dec(s: &Settings, p: &hz::Params) -> DecodeInto {
        let (s, p) = (s.clone(), *p);
        Arc::new(move |c: &[u8], f: &mut Frame| crate::board::dho::hw::decode_into(c, &s, &p, 0, f))
    }

    /// A raw batch of `frames` frames, CH1 (and CH2 with `two`): a sine with a rising zero crossing near the hardware
    /// trigger point, a per-frame jitter, every fifth frame shifted up, every seventh without the crossing.
    fn batch(frames: usize, memsel: u32, two: bool) -> (Vec<u8>, usize, Settings, hz::Params) {
        let r = if two { 2 } else { 1 };
        let p = hz::Params { ratio: r, rate: 1_250_000_000 / r as u64, memsel, dot_fs: 800_000, seg: 0, max_frames: 1000, pre: memsel / 2, post: memsel / 2 };
        let s = Settings { mask: if two { 3 } else { 1 }, window: None, compress: 1, header: false, ..Settings::default() };
        let fb = oos_hal::dho::seq::frame_bytes(&s, &p);
        let mut bytes = Vec::new();
        for k in 0..frames {
            let jit = (k % 7) as f64 * 0.13 - 0.4;
            for i in 0..memsel as usize {
                let ph = (i as f64 - memsel as f64 / 2.0 + jit) / 400.0 * std::f64::consts::TAU;
                for c in 0..r as usize {
                    let v = if k % 7 == 6 { 0.7 } else { 0.9 * (ph * (1.0 + c as f64 * 0.5)).sin() + if k % 5 == 0 { 0.05 } else { 0.0 } };
                    bytes.extend(((32768.0 + v * 7500.0) as u16).to_le_bytes());
                }
            }
        }
        assert_eq!(bytes.len(), frames * fb, "frame layout");
        (bytes, fb, s, p)
    }

    fn model(tdiv: f64, two: bool, dots: bool) -> Model {
        let mut m = Model::default();
        m.tscale = tscales().iter().position(|&t| (t as f64 - tdiv).abs() < tdiv * 1e-3).unwrap();
        (m.trig_level, m.dots) = (0.0, if dots { DotsMode::On } else { DotsMode::Off });
        for c in 0..NCH {
            (m.ch[c].on, m.ch[c].offset) = (c == 0 || c == 1 && two, 0.0);
        }
        m
    }

    /// Frames of channels `mask` (ratio from the mask), `n` points each: per channel a signal from `sig(frame, channel,
    /// point)` in volts at 1 V/div (7500 codes per volt).
    fn frames_of(mask: u32, n: u32, frames: usize, sig: &dyn Fn(usize, usize, usize) -> f64) -> (Vec<u8>, usize, Settings, hz::Params) {
        let r = hz::ratio(mask);
        let p = hz::Params { ratio: r, rate: 1_250_000_000 / r as u64, memsel: n, dot_fs: 800_000 * r as u64, seg: 0, max_frames: 1000, pre: n / 2, post: n / 2 };
        let s = Settings { mask, window: None, compress: 1, header: false, ..Settings::default() };
        let fb = oos_hal::dho::seq::frame_bytes(&s, &p);
        let map = oos_hal::dho::seq::code_map(mask, s.ch.map(|k| (k.scale, k.offset)));
        let mut bytes = vec![0u8; frames * fb];
        for k in 0..frames {
            for i in 0..n as usize {
                for c in 0..NCH {
                    if let Some(wd) = map.word[c] {
                        let x = (32768.0 + sig(k, c, i) * 7500.0).round().clamp(0.0, 65535.0) as u16;
                        let j = k * fb + 2 * (i * r as usize + wd);
                        bytes[j..j + 2].copy_from_slice(&x.to_le_bytes());
                    }
                }
            }
        }
        (bytes, fb, s, p)
    }

    // Break it catches: the workers' codes path (large frames, no per-frame measurements) drawing another picture or
    // other trigger statistics than the frames mode, or frames lost on it
    #[test]
    fn batch_picture_from_the_codes_equals_the_frames_mode() {
        let (w, h) = (970, 448);
        let sig = |k: usize, c: usize, i: usize| 0.9 * (((i as f64 + (k % 7) as f64 * 0.13 - 35_000.0) / 400.0) * (1.0 + c as f64 * 0.5) * std::f64::consts::TAU).sin() + if k % 5 == 0 { 0.05 } else { 0.0 };
        for (mask, n) in [(1u32, 100_000u32), (3, 70_312)] {
            let mut m = model(1e-5, mask == 3, false);
            m.meas = Vec::new();
            let (bytes, fb, s, p) = frames_of(mask, n, 9, &sig);
            assert!(fb >= CODES_MIN_BYTES);
            let pipe = Pipe::start(2, false, fresh());
            pipe.set_view(Some(&m), w, h);
            let (cs, cp) = (s.clone(), p);
            let codes: CodesInto = Arc::new(move |c: &[u8], f: &mut Frame| crate::board::dho::hw::codes_into(c, &cs, &cp, 0, f));
            pipe.feed_codes(Box::new(Bytes(bytes.clone())), fb, 9, 0, dec(&s, &p), Some(codes));
            settle(&pipe);
            let mut got = Phosphor::new(w, h);
            let (mut clear, mut edges, mut off) = (false, [0u64; 4], (0.0, 0.0));
            let n_got = pipe.merge(&mut got, &mut clear, &mut edges, &mut off);
            let (want, trig, wedges, woff, _) = reference(&bytes, fb, &s, &p, &m, w, h);
            assert_eq!((n_got.frames, n_got.triggered, edges), (9, trig, wedges), "mask {}", mask);
            assert!((off.0 - woff.0).abs() <= 1e-9 * woff.0.abs().max(1.0) && off.1 == woff.1);
            for c in 0..NCH {
                assert!(got.map[c] == want.map[c], "mask {}: CH{} differs", mask, c + 1);
            }
        }
    }

    // Break it catches (`draw_frame_codes`): a picture or trigger statistics from the read-out words that differ from
    // decoding the frame first (column extremes, join, interleaved channels, the near window, the wide fallback),
    // or the codes path taken where it must not be.
    #[test]
    fn drawing_from_the_codes_equals_drawing_the_decoded_frame() {
        let (w, h) = (970, 448);
        let sine = |k: usize, c: usize, i: usize| 0.9 * (((i as f64 - 50_000.0 + (k % 7) as f64 * 0.13) / 400.0) * (1.0 + c as f64 * 0.5) * std::f64::consts::TAU).sin();
        let square = |k: usize, c: usize, i: usize| {
            let x = i as f64 + 37.0 * k as f64 + 101.0 * c as f64;
            (if (x / 9_000.0).fract() < 0.5 { 1.2 } else { -1.2 }) + 0.03 * ((i * 7919 + k * 31) % 101) as f64 / 101.0
        };
        let flat = |_k: usize, c: usize, i: usize| 0.2 + 0.001 * (i % 3) as f64 + c as f64 * 0.1;
        let mut taken = 0;
        let mut cases = 0;
        for (mask, n, tdiv) in [(1u32, 100_000u32, 1e-5), (1, 12_512, 1e-6), (3, 70_312, 1e-5), (3, 12_512, 1e-6), (15, 30_000, 1e-5), (5, 25_000, 1e-5), (1, 1408, 1e-7)] {
            for (name, sig) in [("sine", &sine as &dyn Fn(usize, usize, usize) -> f64), ("square", &square), ("flat", &flat)] {
                // exact false: as a decimated read, where the wide search stays
                for (src, dots, coup, exact) in [(0usize, false, TrigCoup::Dc, true), (1, false, TrigCoup::Dc, true), (0, true, TrigCoup::Dc, true), (0, false, TrigCoup::Ac, true), (0, false, TrigCoup::LfReject, true), (0, false, TrigCoup::HfReject, true), (0, false, TrigCoup::Dc, false), (1, false, TrigCoup::Dc, false), (0, false, TrigCoup::HfReject, false)] {
                    let mut m = model(tdiv, false, dots);
                    for c in 0..NCH {
                        m.ch[c].on = mask & (1 << c) != 0;
                    }
                    m.trig_src = src;
                    m.trig_coup = coup;
                    let (bytes, fb, s, p) = frames_of(mask, n, 6, sig);
                    let (mut a, mut b) = (Phosphor::new(w, h), Phosphor::new(w, h));
                    let (mut sa, mut sb) = (([0u64; 4], (0.0, 0.0)), ([0u64; 4], (0.0, 0.0)));
                    let (mut ta, mut tb) = (0, 0);
                    for chunk in bytes.chunks_exact(fb) {
                        let mut f = crate::board::dho::hw::decode(chunk, &s, &p, 0);
                        f.pre_exact &= exact;
                        ta += draw_frame(&mut a, &m, &f, Some(Stats { edges: &mut sa.0, off: &mut sa.1 })) as u32;
                        let mut g = Frame::default();
                        let lay = crate::board::dho::hw::codes_into(chunk, &s, &p, 0, &mut g).expect("raw frame");
                        g.pre_exact &= exact;
                        assert!(g.ch.iter().all(|v| v.is_empty()) && (g.dt, g.pre, g.acq) == (f.dt, f.pre, f.acq));
                        match draw_frame_codes(&mut b, &m, &g, chunk, &lay, Some(Stats { edges: &mut sb.0, off: &mut sb.1 })) {
                            Some(t) => {
                                tb += t as u32;
                                taken += 1;
                            }
                            None => {
                                assert!(dots || f.dt / m.time_per_div() as f64 * w as f64 / HDIV as f64 > 0.5, "{} {} codes path refused", name, mask);
                                tb += draw_frame(&mut b, &m, &f, Some(Stats { edges: &mut sb.0, off: &mut sb.1 })) as u32;
                            }
                        }
                    }
                    let what = format!("{} mask {} n {} src {} dots {} {:?} exact {}", name, mask, n, src, dots, coup, exact);
                    assert_eq!((ta, sa.0), (tb, sb.0), "{}", what);
                    assert!(sa.1 == sb.1, "{}: {:?} vs {:?}", what, sa.1, sb.1);
                    for c in 0..NCH {
                        assert!(a.map[c] == b.map[c], "{}: CH{} differs", what, c + 1);
                    }
                    cases += 1;
                }
            }
        }
        assert!(taken > 150, "codes path taken {} times in {} cases", taken, cases);
        // the wide fallback was compared too
        assert!(WIDE.with(|n| n.get()) > 5, "{} wide searches", WIDE.with(|n| n.get()));
    }

    // Break it catches: a frame moved to an edge outside the near window where the search must stay near (AC, LF
    // reject; an edge trigger on an exact `pre`), on the decoded frame or from the words; a far edge no longer found
    // where the wide search must stay (decimated reads, other trigger types, noise reject, holdoff); a DC refinement
    // that differs from `trignear::edge_smooth_near` (the refinement before the near-only paths).
    #[test]
    fn near_only_where_the_hardware_point_stands_wide_elsewhere() {
        let (w, h) = (970, 448);
        // -1 V up to 10 k samples after the hardware point (50 k), then +1 V: the only rising edge is far away
        let far = |_k: usize, _c: usize, i: usize| if i < 60_000 { -1.0 } else { 1.0 } + 0.002 * ((i * 7919) % 13) as f64;
        let (bytes, fb, s, p) = frames_of(1, 100_000, 1, &far);
        let f = crate::board::dho::hw::decode(&bytes, &s, &p, 0);
        let mut g0 = Frame::default();
        let lay = crate::board::dho::hw::codes_into(&bytes, &s, &p, 0, &mut g0).expect("raw frame");
        assert!(f.pre == 50_000 && f.pre_exact && g0.pre_exact, "a full-rate raw read is exact");
        // at the hardware point: a level no sample reaches finds no edge
        let mut at_pre = Phosphor::new(w, h);
        let mut m = model(1e-5, false, false);
        m.trig_level = 5.0;
        assert!(!draw_frame(&mut at_pre, &m, &f, None));
        let cases: [(&str, TrigCoup, bool, &dyn Fn(&mut Model), bool); 10] = [
            ("dc exact", TrigCoup::Dc, true, &|_| {}, true),
            ("hf reject exact", TrigCoup::HfReject, true, &|_| {}, true),
            ("ac", TrigCoup::Ac, false, &|_| {}, true),
            ("lf reject", TrigCoup::LfReject, false, &|_| {}, true),
            ("dc decimated", TrigCoup::Dc, false, &|_| {}, false),
            ("dc decimated noise reject", TrigCoup::Dc, false, &|m| m.trig_nrej = true, false),
            ("hf reject decimated", TrigCoup::HfReject, false, &|_| {}, false),
            ("dc noise reject exact", TrigCoup::Dc, true, &|m| m.trig_nrej = true, true),
            ("dc holdoff exact", TrigCoup::Dc, true, &|m| m.holdoff = 1e-6, true),
            ("pulse", TrigCoup::Dc, true, &|m| m.trig_type = TrigType::Pulse, false),
        ];
        for (name, coup, exact, set, near) in cases {
            let mut m = model(1e-5, false, false);
            m.trig_coup = coup;
            set(&mut m);
            let (mut fe, mut g) = (f.clone(), g0.clone());
            (fe.pre_exact, g.pre_exact) = (exact, exact);
            let (mut a, mut b) = (Phosphor::new(w, h), Phosphor::new(w, h));
            let (mut sa, mut sb) = (([0u64; 4], (0.0, 0.0)), ([0u64; 4], (0.0, 0.0)));
            let ta = draw_frame(&mut a, &m, &fe, Some(Stats { edges: &mut sa.0, off: &mut sa.1 }));
            let tb = draw_frame_codes(&mut b, &m, &g, &bytes[..fb], &lay, Some(Stats { edges: &mut sb.0, off: &mut sb.1 })).expect("codes path");
            assert!(ta == tb && a.map[0] == b.map[0] && sa == sb, "{}: codes path differs", name);
            if near {
                assert_eq!(sa.0, [0, 0, 0, 1], "{}", name);
                assert!(a.map[0] == at_pre.map[0], "{}: not drawn at the hardware point", name);
            } else {
                assert_eq!(sa.0, [0, 0, 1, 0], "{}", name);
                assert!((sa.1 .1 - 10_000.0).abs() < 2.0, "{}: offset {:?}", name, sa.1);
                assert!(a.map[0] != at_pre.map[0], "{}", name);
            }
        }
        // a read at the full rate is exact, compressed too (to its pair); a decimated one is not, compressed or not
        for (rate, compress, exact) in [(1_250_000_000u64, 4u32, true), (625_000_000, 1, false), (625_000_000, 4, false)] {
            let q = hz::Params { rate, ..p };
            let t = Settings { compress, ..s.clone() };
            assert_eq!(crate::board::dho::hw::decode(&bytes, &t, &q, 0).pre_exact, exact, "rate {} compress {}", rate, compress);
        }
        // DC: the composed near and wide refinement is `edge_smooth_near`, wherever the hardware point lies
        let sine: Vec<f32> = (0..100_000).map(|i| (0.9 * (i as f64 / 3000.0 * std::f64::consts::TAU).sin() + 0.2 * ((i * 7919) % 17) as f64 / 17.0) as f32).collect();
        for v in [&f.ch[0], &sine] {
            for pre in (1000..99_000).step_by(3331) {
                for (col, half) in [(1, 600), (16, 9000), (128, 62_500)] {
                    for level in [0.0f32, 0.5, -0.99] {
                        let got = refine_near(v, 0, v.len(), pre, false, level, 0.1, true, false, half, col).unwrap_or_else(|| refine_wide(std::time::Instant::now(), v, pre, level, 0.1, true, false, half, col));
                        let want = crate::trignear::edge_smooth_near(v, pre, level, 0.1, true, false, half, col, |_| true);
                        assert!(got.0 == want.0 && got.1.map(|e| (e.t, e.rising)) == want.1.map(|e| (e.t, e.rising)), "pre {} col {} level {}", pre, col, level);
                    }
                }
            }
        }
    }

    // Break it catches: a compressed read (min/max pairs of the live compression) at the full rate whose search still
    // runs wide, or moves to an edge outside the near window; a decimated compressed read that no longer searches
    // wide; a near window (in points) that does not cover the pair of the hardware point at the largest k the full
    // rate can have (a record of at most `compress::MAX_DEPTH` points).
    #[test]
    fn compressed_full_rate_reads_search_near_decimated_ones_wide() {
        let (w, h) = (970, 448);
        // the largest k at the full rate, over every time base and channel ratio, and the near window there
        let mut worst = (0u32, usize::MAX);
        for ratio in [1u64, 2, 4] {
            let rate = 1_250_000_000 / ratio;
            for &t in tscales().iter() {
                let memsel = (rate as f64 * t as f64 * HDIV as f64).round();
                if memsel > crate::compress::MAX_DEPTH as f64 {
                    continue;
                }
                let k = crate::compress::k_for(rate, t as f64, memsel as u32);
                if k < 2 {
                    continue;
                }
                let x_per_point = k as f64 / rate as f64 / t as f64 * w as f64 / HDIV as f64;
                let col = crate::compress::smooth_len(x_per_point);
                let near = crate::trignear::NEAR_COLS * col + crate::trignear::NEAR_PAD;
                if k > worst.0 || (k == worst.0 && near < worst.1) {
                    worst = (k, near);
                }
            }
        }
        println!("largest k at the full rate {}, near window there {} points", worst.0, worst.1);
        assert!(worst.0 >= 2, "no compressed full-rate read");
        assert!(worst.1 >= 8 * crate::compress::PAIR, "k {}: near window {} points against a pair of {}", worst.0, worst.1, crate::compress::PAIR);
        // a compressed frame: pairs (min, max) of a step from -1 to +1 V, the only rising edge 2 k points after the
        // hardware point (far outside the near window), or in the point's pair
        let n = 20_000usize;
        let pairs = |edge_at: usize| -> Vec<f32> {
            (0..n).flat_map(|j| {
                let base = if j >= edge_at { 1.0 } else { -1.0 };
                [base - 0.01, base + 0.01]
            }).collect()
        };
        let mut m = model(1e-5, false, false);
        m.trig_level = 0.0;
        let dt = 1e-5 * HDIV as f64 / (2 * n) as f64;
        let frame = |v: Vec<f32>, exact: bool| Frame { dt, pre: n, ch: [v, Vec::new(), Vec::new(), Vec::new()], envelope: true, acq: (1.25e9, 1_000_000), pre_exact: exact, ..Default::default() };
        assert!(crate::compress::live(&frame(Vec::new(), true)));
        for (exact, near) in [(true, true), (false, false)] {
            let f = frame(pairs(n / 2 + 1000), exact);
            let mut ph = Phosphor::new(w, h);
            let mut st = ([0u64; 4], (0.0, 0.0));
            draw_frame(&mut ph, &m, &f, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
            let want = if near { [0, 0, 0, 1] } else { [0, 0, 1, 0] };
            assert_eq!(st.0, want, "exact {}: {:?}", exact, st);
            // the edge in the pair of the hardware point is found either way
            let f = frame(pairs(n / 2), exact);
            let mut st = ([0u64; 4], (0.0, 0.0));
            draw_frame(&mut ph, &m, &f, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
            assert_eq!(st.0, [1, 0, 0, 0], "exact {}: edge at the point", exact);
        }
    }

    /// A batch of 20 frames for 2 workers where one worker waits in its first decode until the returned gate opens;
    /// returns once the other worker has handed on its 16 frames.
    fn one_worker_held(pipe: &Pipe) -> Arc<(Mutex<bool>, std::sync::Condvar)> {
        let (bytes, fb, s, p) = batch(20, 12_512, false);
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (blocked_tx, blocked) = std::sync::mpsc::channel::<()>();
        let first = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let inner = dec(&s, &p);
        let (g, tx) = (gate.clone(), Mutex::new(blocked_tx));
        let decode: DecodeInto = Arc::new(move |c: &[u8], f: &mut Frame| {
            if first.swap(false, Ordering::AcqRel) {
                let _ = tx.lock().unwrap().send(());
                let mut open = g.0.lock().unwrap();
                while !*open {
                    open = g.1.wait(open).unwrap();
                }
            }
            inner(c, f)
        });
        pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, decode);
        blocked.recv_timeout(std::time::Duration::from_secs(20)).expect("a worker in its first decode");
        let t0 = std::time::Instant::now();
        while pipe.mails.iter().map(|m| m.lock().unwrap().full.iter().map(|a| a.frames).sum::<u64>()).sum::<u64>() < 16 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(20), "the other worker's share");
            std::thread::yield_now();
        }
        gate
    }

    // Break it catches (step 0 counters, docs/fork/draw-architecture.md): a batch whose frames reach the picture in two
    // merges not counted as partial, a whole batch counted, or the picture age of a merge not reported.
    #[test]
    fn partial_merges_and_picture_age_are_counted() {
        let m = model(1e-6, false, false);
        let pipe = Pipe::start(2, false, fresh());
        pipe.set_view(Some(&m), 970, 448);
        let mut ph = Phosphor::new(970, 448);
        let mut merge = |pipe: &Pipe| pipe.merge(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        // whole: drawn, then merged once
        let (bytes, fb, s, p) = batch(20, 12_512, false);
        pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
        settle(&pipe);
        let whole = merge(&pipe);
        assert_eq!(whole.frames, 20);
        assert!(whole.newest.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(20)), "picture age");
        assert_eq!(pipe.partial.load(Ordering::Relaxed), 0);
        // in parts: 16 frames in one merge, the held worker's 4 in the next
        let gate = one_worker_held(&pipe);
        let first = merge(&pipe).frames;
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        settle(&pipe);
        let second = merge(&pipe).frames;
        assert_eq!((first, second), (16, 4));
        assert_eq!(pipe.partial.load(Ordering::Relaxed), 1);
        // an empty merge reports no age
        assert_eq!(merge(&pipe).newest, None);
    }

    // Timing guard (tools/perf-guard.sh): a worker frame drawn from the read-out words against decode + draw, lines,
    // no per-frame measurements, also with AC trigger coupling (its level from the words). The A53 gains more than the
    // host (the f32 copy is 2x the read-out per channel).
    #[test]
    #[ignore]
    fn perf_guard_codes_per_frame() {
        let (w, h) = (970, 448);
        let sig = |k: usize, c: usize, i: usize| 0.9 * (((i as f64 + (k % 7) as f64 * 0.13) / 400.0) * (1.0 + c as f64 * 0.5) * std::f64::consts::TAU).sin();
        let mut worst: f64 = 0.0;
        for (mask, n, tdiv, coup) in [(1u32, 100_000u32, 1e-5, TrigCoup::Dc), (3, 70_312, 1e-5, TrigCoup::Dc), (5, 50_000, 1e-5, TrigCoup::Dc), (1, 40_000, 2e-6, TrigCoup::Dc), (1, 100_000, 1e-5, TrigCoup::Ac)] {
            let mut m = model(tdiv, false, false);
            m.trig_coup = coup;
            for c in 0..NCH {
                m.ch[c].on = mask & (1 << c) != 0;
            }
            let frames = (2_000_000 / (n as usize * 2 * hz::ratio(mask) as usize)).clamp(4, 100);
            let (bytes, fb, s, p) = frames_of(mask, n, frames, &sig);
            let (mut pa, mut pb) = (Phosphor::new(w, h), Phosphor::new(w, h));
            let (mut fa, mut fbm) = (Frame::default(), Frame::default());
            // one round: every frame once (each round's time divided by the frames below)
            let (old, new) = crate::perfguard::best_pair(
                crate::perfguard::ROUNDS,
                1,
                || {
                    for chunk in bytes.chunks_exact(fb) {
                        crate::board::dho::hw::decode_into(chunk, &s, &p, 0, &mut fa);
                        std::hint::black_box(draw_frame(&mut pa, &m, &fa, None));
                    }
                },
                || {
                    for chunk in bytes.chunks_exact(fb) {
                        let lay = crate::board::dho::hw::codes_into(chunk, &s, &p, 0, &mut fbm).unwrap();
                        std::hint::black_box(draw_frame_codes(&mut pb, &m, &fbm, chunk, &lay, None).unwrap());
                    }
                },
            );
            let (old, new) = (old / frames as u32, new / frames as u32);
            let r = new.as_secs_f64() / old.as_secs_f64();
            worst = worst.max(r);
            assert!(fb >= CODES_MIN_BYTES && hz::ratio(mask) as usize <= CODES_MAX_STRIDE);
            println!("codes mask {:#x} {} pt {:?}: decode + draw {:?}/frame, from the codes {:?}/frame (ratio {:.2})", mask, n, coup, old, new, r);
        }
        assert!(worst <= 0.9, "drawing from the codes must be cheaper: worst ratio {:.2}", worst);
    }

    /// Until every batch fed is done (drawn and handed on, or dropped).
    fn settle(p: &Pipe) {
        let t0 = std::time::Instant::now();
        while p.live.load(Ordering::Acquire) > 0 {
            assert!(t0.elapsed() < std::time::Duration::from_secs(20), "batches still open");
            std::thread::yield_now();
        }
    }

    /// The frames mode for the same frames: `Ui::fast_draw` and the per-frame statistics on every frame.
    fn reference(bytes: &[u8], fb: usize, s: &Settings, p: &hz::Params, m: &Model, w: usize, h: usize) -> (Phosphor, u64, [u64; 4], (f64, f64), crate::framestats::Folds) {
        let mut want = Phosphor::new(w, h);
        let (mut trig, mut edges, mut off) = (0, [0u64; 4], (0.0, 0.0));
        let mut folds = crate::framestats::folds_for(m);
        for chunk in bytes.chunks_exact(fb) {
            let f = crate::board::dho::hw::decode(chunk, s, p, 0);
            trig += draw_frame(&mut want, m, &f, Some(Stats { edges: &mut edges, off: &mut off })) as u64;
            crate::framestats::fold_frame(&f, m, &mut folds);
        }
        (want, trig, edges, off, folds)
    }

    // Break it catches: a batch picture that differs from the frames mode for the same frames (decode into kept
    // vectors, frames claimed by several workers and the helper, dense and sparse frames, dots, two channels),
    // trigger or measurement statistics that leave frames out, frames lost or counted twice, a merge that does not
    // clear for minimum persistence, a buffer that does not come back.
    #[test]
    fn batch_picture_equals_the_frames_mode() {
        let (w, h) = (970, 448);
        for (tdiv, memsel, two, dots, workers, helper) in [(1e-6, 12_512u32, false, false, 2, false), (1e-6, 12_512, true, true, 4, true), (1e-7, 1408, false, false, 1, true), (2e-8, 400, true, false, 1, true)] {
            let mut m = model(tdiv, two, dots);
            m.meas = vec![(Meas::Vpp, 0), (Meas::Vavg, 0), (Meas::Vrms, 1)];
            let (bytes, fb, s, p) = batch(41, memsel, two);
            let c = fresh();
            let pipe = Pipe::start(workers, false, c);
            pipe.set_view(Some(&m), w, h);
            assert!(pipe.accepting());
            pipe.feed(Box::new(Bytes(bytes[..17 * fb].to_vec())), fb, 17, 0, dec(&s, &p));
            pipe.feed(Box::new(Bytes(bytes[17 * fb..].to_vec())), fb, 24, 0, dec(&s, &p));
            // the acquisition thread's share, as while it waits for the next batch
            let mut hp = Helper::default();
            if helper {
                while pipe.help(&mut hp) {}
            }
            settle(&pipe);
            pipe.help_done(&mut hp);
            let mut got = Phosphor::new(w, h);
            got.map[0][0] = 7;
            let (mut clear, mut edges, mut off) = (true, [0u64; 4], (0.0, 0.0));
            let n = pipe.merge(&mut got, &mut clear, &mut edges, &mut off);
            let (want, trig, wedges, woff, folds) = reference(&bytes, fb, &s, &p, &m, w, h);
            let what = format!("{} s/div, {} workers, helper {}", tdiv, workers, helper);
            assert_eq!((n.frames, n.triggered), (41, trig), "{}", what);
            assert!(!clear && trig > 30 && trig < 41, "{}: {} triggered", what, trig);
            assert_eq!(edges, wedges, "{}", what);
            assert!((off.0 - woff.0).abs() <= 1e-9 * woff.0.abs().max(1.0) && off.1 == woff.1);
            for c in 0..NCH {
                assert!(got.map[c] == want.map[c], "{}: CH{} differs", what, c + 1);
            }
            // per-frame statistics: the same frames counted, sums in another order
            assert_eq!(n.folds.len(), folds.len());
            for ((k, a), (k2, b)) in n.folds.iter().zip(&folds) {
                assert_eq!((k, a.n, a.min, a.max), (k2, b.n, b.min, b.max), "{}", what);
                assert!((a.sum - b.sum).abs() <= 1e-12 * b.sum.abs().max(1.0) && (a.sumsq - b.sumsq).abs() <= 1e-12 * b.sumsq.abs().max(1.0));
            }
            assert!(tdiv < 1e-6 || folds.iter().all(|x| x.1.n > 30), "{}: {:?}", what, folds);
            assert_eq!(c.read(), (0, 0, 0), "{}: nothing dropped", what);
            assert_eq!(c.measured.load(Ordering::Relaxed), folds.first().map_or(0, |x| x.1.n));
            // merged once; both buffers back in the pool
            assert_eq!(pipe.merge(&mut got, &mut clear, &mut edges, &mut off), Merged::default());
            let back: Vec<_> = std::iter::from_fn(|| pipe.take_buffer()).collect();
            assert_eq!(back.len(), 2);
            assert!(back.into_iter().all(|b| b.downcast::<Bytes>().is_ok()));
        }
    }

    // Break it catches: the newest frame of a batch reaching the UI before the batch's other frames (a picture with
    // the newest frame alone, the rest a picture later: docs/fork/stutter.md), or not at all.
    #[test]
    fn a_batch_is_delivered_with_its_frames() {
        let (w, h) = (970, 448);
        let m = model(1e-6, false, false);
        let (bytes, fb, s, p) = batch(30, 12_512, false);
        let pipe = Pipe::start(2, false, fresh());
        pipe.set_view(Some(&m), w, h);
        let got = Arc::new(Mutex::new(Vec::new()));
        for k in 0..3u32 {
            let g = got.clone();
            pipe.feed_with(Box::new(Bytes(bytes.clone())), fb, 30, 0, dec(&s, &p), None, Box::new(move || g.lock().unwrap().push(k)));
        }
        // nothing before a merge, even with every frame drawn
        settle(&pipe);
        assert!(got.lock().unwrap().is_empty());
        let mut ph = Phosphor::new(w, h);
        let n = pipe.merge(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(n.frames, 90);
        let mut d = got.lock().unwrap().clone();
        d.sort();
        assert_eq!(d, [0, 1, 2]);
        // no view any more: what is done is delivered too
        let g = got.clone();
        pipe.feed_with(Box::new(Bytes(bytes.clone())), fb, 30, 0, dec(&s, &p), None, Box::new(move || g.lock().unwrap().push(9)));
        settle(&pipe);
        pipe.set_view(None, 0, 0);
        assert_eq!(got.lock().unwrap().last(), Some(&9));
    }

    // Break it catches: the acquisition thread's share drawn differently from the workers', or not handed to the UI.
    #[test]
    fn the_helper_alone_draws_the_same_picture() {
        let (w, h) = (970, 448);
        for (tdiv, memsel) in [(1e-6, 12_512u32), (1e-7, 1408)] {
            let mut m = model(tdiv, false, false);
            m.meas = vec![(Meas::Vpp, 0)];
            let (bytes, fb, s, p) = batch(20, memsel, false);
            let pipe = Pipe::start(0, false, fresh());
            pipe.set_view(Some(&m), w, h);
            let b = pipe.feed_batch(Box::new(Bytes(bytes.clone())), fb, 20, 0, dec(&s, &p), None);
            let mut hp = Helper::default();
            let mut k = 0;
            while pipe.help(&mut hp) {
                k += 1;
                assert!(k < 100);
            }
            drop(b);
            let mut got = Phosphor::new(w, h);
            let n = pipe.merge(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
            let (want, trig, ..) = reference(&bytes, fb, &s, &p, &m, w, h);
            assert_eq!((k, n.frames, n.triggered), (20, 20, trig), "{} s/div", tdiv);
            assert!((0..NCH).all(|c| got.map[c] == want.map[c]), "{} s/div", tdiv);
        }
    }

    // Break it catches: frames of older settings drawn, maps of an older mapping merged, frames nobody drew not
    // counted, batches taken while the fast path does not apply.
    #[test]
    fn stale_work_is_dropped_and_counted() {
        let (w, h) = (970, 448);
        let mut m = model(1e-6, false, false);
        let (bytes, fb, s, p) = batch(10, 12_512, false);
        let c = fresh();
        let pipe = Pipe::start(2, false, c);
        let mut ph = Phosphor::new(w, h);
        let merge = |ph: &mut Phosphor| pipe.merge(ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert!(!pipe.accepting());
        // settings stamp older than the view's: dropped
        m.hw_stamp = 3;
        pipe.set_view(Some(&m), w, h);
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 2, dec(&s, &p));
        settle(&pipe);
        assert_eq!(merge(&mut ph), Merged::default());
        assert_eq!(c.read(), (0, 0, 10));
        // drawn, then the mapping changes before the merge: dropped at the merge
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        settle(&pipe);
        m.ch[0].offset = 0.5;
        pipe.set_view(Some(&m), w, h);
        assert_eq!(merge(&mut ph), Merged::default());
        assert!(ph.map.iter().all(|x| x.iter().all(|&y| y == 0)));
        assert_eq!(c.read(), (0, 0, 20));
        // a change the drawing does not depend on keeps the maps
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        settle(&pipe);
        m.status = TrigStatus::Wait;
        pipe.set_view(Some(&m), w, h);
        assert_eq!(merge(&mut ph).frames, 10);
        // many batches at once: all drawn, nothing more dropped
        let (big, fb2, s2, p2) = batch(200, 12_512, false);
        for _ in 0..6 {
            pipe.feed(Box::new(Bytes(big.clone())), fb2, 200, 3, dec(&s2, &p2));
        }
        settle(&pipe);
        assert_eq!(merge(&mut ph).frames, 1200);
        assert_eq!(c.read().2, 20);
        // a batch dropped before anybody claimed a frame counts all of them
        let pipe0 = Pipe::start(0, false, c);
        pipe0.set_view(Some(&m), w, h);
        pipe0.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        assert_eq!(c.read().2, 30);
        // no view: the acquisition thread queues the frames itself
        pipe.set_view(None, 0, 0);
        assert!(!pipe.accepting());
    }

    #[test]
    fn pipeline_mode_from_env_and_system_conf() {
        assert_eq!(mode_from(None, ""), Mode::Frames);
        assert_eq!(mode_from(None, "ntp = off\nacq.pipeline = batch   # new path\n"), Mode::Batch);
        assert_eq!(mode_from(Some("frames"), "acq.pipeline = batch\n"), Mode::Frames);
        assert_eq!(mode_from(Some("batch"), ""), Mode::Batch);
        assert_eq!(mode_from(None, "acq.pipeline = bogus\n"), Mode::Frames);
    }

    /// Host cost per frame of a worker (decode into kept vectors, trigger refinement, deposit) against the frames
    /// mode's per-frame work on the UI thread (`decode` + `Ui::fast_draw`), and the rate of 1, 2 and 4 workers (the
    /// helper runs the same code per frame). Run by tools/perf-guard.sh.
    #[test]
    #[ignore]
    fn perf_guard_deposit_worker_per_frame() {
        let (w, h) = (970, 448);
        for (tdiv, memsel, meas) in [(1e-7, 1408u32, false), (1e-6, 12_512, false), (1e-6, 12_512, true), (1e-5, 125_000, false), (1e-5, 125_000, true)] {
            let mut m = model(tdiv, false, false);
            // the measurements made on every frame cost on top (reported, not guarded): the worst case, all of them on
            m.meas = if meas { crate::framestats::CHEAP.iter().map(|&k| (k, 0)).collect() } else { Vec::new() };
            let frames = (2_000_000 / (memsel as usize * 2)).clamp(8, 500);
            let (bytes, fb, s, p) = batch(frames, memsel, false);
            // the frames mode: decode + fast_draw on one thread (its measurements: a few records a second), against
            // one worker of the pipeline, alternating (perfguard.rs); 2 and 4 workers are reported only
            let mut ph = Phosphor::new(w, h);
            let one = Pipe::start(1, false, fresh());
            one.set_view(Some(&m), w, h);
            // the buffers fed are copied before, outside the timing (the copy is not the pipeline's work)
            let mut copies: Vec<Bytes> = (0..crate::perfguard::ROUNDS).map(|_| Bytes(bytes.clone())).collect();
            let (old, one_t) = crate::perfguard::best_pair(
                crate::perfguard::ROUNDS,
                1,
                || {
                    for chunk in bytes.chunks_exact(fb) {
                        std::hint::black_box(draw_frame(&mut ph, &m, &crate::board::dho::hw::decode(chunk, &s, &p, 0), None));
                    }
                },
                || {
                    one.feed(Box::new(copies.pop().expect("a copy per round")), fb, frames, 0, dec(&s, &p));
                    settle(&one);
                },
            );
            let (old, mut per) = (old / frames as u32, vec![one_t / frames as u32]);
            for n in [2usize, 4] {
                let pipe = Pipe::start(n, false, fresh());
                pipe.set_view(Some(&m), w, h);
                let t0 = std::time::Instant::now();
                for _ in 0..3 {
                    pipe.feed(Box::new(Bytes(bytes.clone())), fb, frames, 0, dec(&s, &p));
                    settle(&pipe);
                }
                per.push(t0.elapsed() / (3 * frames) as u32);
            }
            let r = per[0].as_secs_f64() / old.as_secs_f64();
            println!("{} s/div {} pt, frame statistics {}: frames mode {:?}/frame; one core in the pipeline {:?}/frame (ratio {:.2}); 1/2/4 workers {:.0}/{:.0}/{:.0} frames/s", tdiv, memsel, meas, old, per[0], r, 1.0 / per[0].as_secs_f64(), 1.0 / per[1].as_secs_f64(), 1.0 / per[2].as_secs_f64());
            assert!(meas || r <= 1.5, "a frame in the pipeline must not cost more than in the frames mode: {:.2}", r);
        }
    }
}
