// SPDX-License-Identifier: GPL-2.0-or-later
//! Whole-batch placement and owned column differences. The UI takes one deposit per step.

mod config;
pub(crate) mod funding;
pub use config::{conf_value, mode_from, Mode};
mod decoder;
mod frame;
#[cfg(test)]
pub use frame::draw_frame;
pub(crate) use frame::draw_frame_reusing;
mod feed;
pub(crate) mod reuse;
mod stage;
pub use decoder::IndexedDecodeInto;
mod losses;
pub use losses::Losses;
pub(crate) use losses::Lost;
#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod counter_tests;
#[cfg(test)]
mod identity_tests;
pub(crate) use feed::same_pair;
use feed::Header;
pub mod host;
pub mod shutdown;
use crate::board::{Frame, PictureEligibility, QueuedFrame};
use crate::model::*;
use crate::picture::deposit::{Deposit, Handoff, TapProjection, TapView};
pub(crate) use crate::picture::deposit::{TapRead, TapReservation};
use crate::picture::DepositWindow;
use crate::sparse::Interp;
use crate::wave::Phosphor;
use std::any::Any;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Stats<'a> {
    pub edges: &'a mut [u64; 4],
    pub off: &'a mut (f64, f64),
}

/// Where a frame's read-out holds each channel (`fastdraw::Codes`), as the board gives it (`CodesInto`).
#[derive(Clone, Copy)]
pub struct CodeLayout {
    pub stride: usize,
    pub word: [Option<usize>; NCH],
    pub affine: [(f32, f32); NCH],
    pub volt: fn(u16, (f32, f32)) -> f32,
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

/// Frames acquired, used (drawn into the picture) and dropped, since the start; in both modes.
#[derive(Default)]
pub struct Counters {
    pub acquired: AtomicU64,
    pub used: AtomicU64,
    pub dropped: AtomicU64,
    /// frames whose cheap measurements went into the statistics (framestats.rs)
    pub measured: AtomicU64,
    pub deposited: AtomicU64,
    pub shown: AtomicU64,
    pub timing: crate::claim_timing::Timing,
    pub losses: Losses,
    pub acquisition: crate::board::AcquisitionCounters,
    #[cfg(test)]
    pub(crate) diagnostics_disabled: std::sync::atomic::AtomicBool,
}

/// The process's counters (the acquisition thread, the workers and the UI add to them).
pub fn counters() -> &'static Counters {
    #[cfg(test)]
    if let Some(state) = test_scope::state() {
        return state.counters;
    }
    static C: OnceLock<Counters> = OnceLock::new();
    C.get_or_init(Counters::default)
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

enum Message {
    Batch(Arc<Batch>),
    Wake,
    #[cfg(test)]
    Barrier(Sender<()>),
}

struct Batch {
    fb: usize,
    frames: usize,
    decode: decoder::Decoder,
    codes: Option<CodesInto>,
    header: Header,
    view_gen_at_submit: u64,
    next: AtomicUsize,
    counters: &'static Counters,
    live: Arc<AtomicUsize>,
    newest: Option<Arc<Frame>>,
    fed: std::time::Instant,
    lease: Lease,
}

impl Drop for Batch {
    fn drop(&mut self) {
        let left = self.frames.saturating_sub(self.next.load(Ordering::Relaxed));
        self.counters.discard(Lost::Unclaimed, left as u64);
        self.live.fetch_sub(1, Ordering::Release);
    }
}

struct View {
    gen: u64,
    m: Model,
    interp: Interp,
    w: usize,
    h: usize,
}

#[derive(Default)]
struct ViewState {
    view: Option<Arc<View>>,
    #[cfg(test)]
    stamp: u64,
    identity: Option<oos_core::change::Identity>,
    key: Option<oos_core::change::WorkerViewKey>,
    gen: u64,
    mapping_key: Option<oos_core::change::MappingKey>,
    mapping_gen: u64,
}

impl ViewState {
    fn tap_view(&self, size: (usize, usize)) -> Option<TapView> {
        self.view.as_ref().map(|v| TapView::new(v.gen, self.mapping_gen, size))
    }
}

pub struct Pipe {
    view: Arc<Mutex<ViewState>>,
    counters: &'static Counters,
    workers: Mutex<shutdown::Workers>,
    slot: Arc<Handoff>,
    live: Arc<AtomicUsize>,
    home: Sender<Box<dyn RawBatch>>,
    pool: Mutex<Receiver<Box<dyn RawBatch>>>,
}

#[derive(Default)]
pub struct Accepted {
    pub frames: u64,
    pub triggered: u64,
    pub folds: crate::framestats::Folds,
    pub newest: Option<std::time::Instant>,
    pub stale: u64,
    pub(crate) window_frames: Option<u64>,
}

impl Pipe {
    pub fn set_view(&self, m: Option<&Model>, w: usize, h: usize, interp: Interp) {
        feed::set_view(self, m, w, h, interp, None);
    }

    pub fn publish_view(&self, m: &Model, w: usize, h: usize, interp: Interp, identity: oos_core::change::Identity) {
        feed::set_view(self, Some(m), w, h, interp, Some(identity));
    }

    pub fn set_identity(&self, identity: oos_core::change::Identity) {
        let mut view = oos_hal::sync::lock(&self.view);
        view.identity = Some(identity);
        self.slot.set_tap_identity(identity);
    }

    pub fn accepting(&self) -> bool {
        !oos_hal::sync::lock(&self.workers).closed && oos_hal::sync::lock(&self.view).view.is_some()
    }

    pub fn take_buffer(&self) -> Option<Box<dyn Any + Send>> {
        oos_hal::sync::lock(&self.pool).try_recv().ok().map(|r| r.into_any())
    }

    #[cfg(test)]
    pub fn accept(&self, ph: &mut Phosphor, clear: &mut bool, edges: &mut [u64; 4], off: &mut (f64, f64)) -> Accepted {
        self.accept_into(ph, clear, Stats { edges, off }, None)
    }

    pub(crate) fn discard_window(&self, window: &mut DepositWindow) {
        window.reset(&self.slot);
    }

    #[cfg(test)]
    pub(crate) fn take_recycled_deposit(&self) -> Option<Deposit> {
        self.slot.spare()
    }

    pub(crate) fn tap_count(&self) -> usize {
        self.slot.tap_count()
    }

    pub(crate) fn has_waveform(&self, identity: oos_core::change::Identity) -> bool {
        self.slot.has_tap(identity)
    }

    #[cfg(test)]
    pub(crate) fn take_waveform(&self, size: (usize, usize)) -> Option<QueuedFrame> {
        let identity = oos_hal::sync::lock(&self.view).identity;
        match self.read_waveform(size, identity, None, true) {
            TapRead::Ready((frame, _)) => Some(frame),
            _ => None,
        }
    }

    pub(crate) fn read_waveform(
        &self,
        size: (usize, usize),
        identity: Option<oos_core::change::Identity>,
        fallback: Option<u64>,
        can_take: bool,
    ) -> TapRead<(QueuedFrame, Option<crate::framestats::TapFolds>)> {
        let state = oos_hal::sync::lock(&self.view);
        let view = state.tap_view(size);
        let (tap, charge) = match self.slot.take_tap(view, identity, fallback, can_take) {
            TapRead::Ready(tap) => tap,
            TapRead::Stale => return TapRead::Stale,
            TapRead::Pending => return TapRead::Pending,
            TapRead::Empty => return TapRead::Empty,
        };
        let eligibility = tap.projection.eligibility(view);
        TapRead::Ready((QueuedFrame::from_tap(tap.frame, eligibility, charge), tap.folds))
    }

    pub(crate) fn take_preview(&self, frame: &Arc<Frame>, size: (usize, usize)) -> Option<PictureEligibility> {
        let state = oos_hal::sync::lock(&self.view);
        let id = state.identity?;
        if (id.gen(), id.epoch()) != (frame.cfg, frame.stamp) {
            return None;
        }
        self.slot.take_preview(state.tap_view(size), frame)
    }

    pub(crate) fn accept_into(
        &self,
        ph: &mut Phosphor,
        clear: &mut bool,
        stats: Stats<'_>,
        window: Option<(&mut DepositWindow, std::time::Instant)>,
    ) -> Accepted {
        let mut claim = self.slot.claim();
        let mut window = window;
        if claim.is_none() && window.as_ref().is_none_or(|(retained, _)| retained.is_empty()) {
            if let Some((retained, _)) = window {
                retained.idle();
            }
            return Accepted::default();
        }
        let state = oos_hal::sync::lock(&self.view);
        let view = state.view.clone();
        let mapping_gen = state.mapping_gen;
        let identity = state.identity;
        drop(state);
        let size = (ph.w, ph.h);
        let mapping_matches =
            |d: &Deposit| d.size() == size && view.as_ref().is_some_and(|v| d.matches(v.gen, v.w, v.h));
        let mut accepted = Accepted::default();
        let mut rebuild = false;
        if let Some((retained, now)) = &mut window {
            rebuild = retained.expire(*now, &self.slot, |d| {
                mapping_matches(d) && d.identity.map(|id| id.epoch()) == identity.map(|id| id.epoch())
            });
        }
        if let Some(deposit) = claim.as_mut().and_then(|claim| claim.deposit.take()) {
            let same_acquisition = same_pair(deposit.identity, identity);
            if mapping_matches(&deposit) && same_acquisition {
                let taps = deposit.taps.clone();
                accepted = self.fold_pending(deposit, ph, clear, stats, identity, window.as_mut());
                self.slot.settle_taps(&taps, (accepted.frames > 0).then_some(mapping_gen));
                rebuild |= accepted.frames > 0;
            } else {
                deposit.reject(identity, self.counters);
                self.slot.recycle(deposit);
            }
        }
        if claim.is_some() {
            drop(claim);
            self.wake();
        }
        if let Some((retained, _)) = window {
            if accepted.frames == 0 {
                retained.idle();
            }
            if rebuild {
                *clear = false;
                accepted.window_frames = Some(retained.rebuild(ph));
            }
        }
        accepted
    }

    fn fold_pending(
        &self,
        mut deposit: Deposit,
        ph: &mut Phosphor,
        clear: &mut bool,
        stats: Stats<'_>,
        identity: Option<oos_core::change::Identity>,
        window: Option<&mut (&mut DepositWindow, std::time::Instant)>,
    ) -> Accepted {
        if let Some((_, now)) = window.as_ref() {
            if deposit.extent.expired(*now) {
                self.counters.reject(losses::Rejection::Age, deposit.records);
                Counters::add(&self.counters.dropped, deposit.records);
                deposit.discard_folds(self.counters);
                self.slot.recycle(deposit);
                return Accepted::default();
            }
        }
        let accepted = crate::picture::accept_deposit(&mut deposit, stats, identity, self.counters);
        if self.counters.diagnostics_enabled() {
            self.counters.timing.picture(deposit.progress.sample, accepted.newest.map(|fed| fed.elapsed()));
        }
        if let Some((retained, _)) = window.filter(|_| accepted.frames > 0) {
            retained.push(deposit, &self.slot);
        } else {
            if accepted.frames > 0 {
                if std::mem::take(clear) {
                    ph.clear();
                }
                if deposit.format() == crate::picture::deposit::count::CountFormat::Wide {
                    ph.set_format(deposit.format());
                }
                for band in &mut deposit.planes {
                    band.fold_picture([None, None], ph);
                }
            }
            self.slot.recycle(deposit);
        }
        accepted
    }

    fn wake(&self) {
        if let Some(tx) = &oos_hal::sync::lock(&self.workers).sender {
            let _ = tx.try_send(Message::Wake);
        }
    }
}

/// Workers where the cores are all alike (the host): two.
const WORKERS: usize = 2;

/// The pipe in `batch` mode (started on first use, one per process), else None.
pub enum Access<'a> {
    Existing,
    Start(&'a crate::sysconf::Switches),
}
impl<'a> From<&'a crate::sysconf::Switches> for Access<'a> {
    fn from(sw: &'a crate::sysconf::Switches) -> Self {
        Self::Start(sw)
    }
}

pub fn pipe<'a>(access: impl Into<Access<'a>>) -> Option<&'static Pipe> {
    #[cfg(test)]
    if let Some(state) = test_scope::state() {
        return state.pipe(access.into());
    }
    static P: OnceLock<Pipe> = OnceLock::new();
    let Access::Start(sw) = access.into() else { return P.get() };
    (sw.pipeline == Mode::Batch).then(|| P.get_or_init(|| funding::application(sw, counters())))
}

#[cfg(test)]
mod tests {
    use super::*;
    mod deposit;
    mod publication;
    mod window;
    use oos_hal::dho::hz;
    use oos_hal::dho::seq::Settings;

    /// A read-out buffer for the tests.
    pub(super) struct Bytes(pub(super) Vec<u8>);

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

    pub(super) fn dec(s: &Settings, p: &hz::Params) -> DecodeInto {
        let (s, p) = (s.clone(), *p);
        Arc::new(move |c: &[u8], f: &mut Frame| crate::board::dho::hw::decode_into(c, &s, &p, 0, f))
    }

    /// A raw batch of `frames` frames, CH1 (and CH2 with `two`): a sine with a rising zero crossing near the hardware
    /// trigger point, a per-frame jitter, every fifth frame shifted up, every seventh without the crossing.
    pub(super) fn batch(frames: usize, memsel: u32, two: bool) -> (Vec<u8>, usize, Settings, hz::Params) {
        let r = if two { 2 } else { 1 };
        let p = hz::Params {
            ratio: r,
            rate: 1_250_000_000 / r as u64,
            memsel,
            dot_fs: 800_000,
            seg: 0,
            max_frames: 1000,
            pre: memsel / 2,
            post: memsel / 2,
        };
        let s =
            Settings { mask: if two { 3 } else { 1 }, window: None, compress: 1, header: false, ..Settings::default() };
        let fb = oos_hal::dho::seq::frame_bytes(&s, &p);
        let mut bytes = Vec::new();
        for k in 0..frames {
            let jit = (k % 7) as f64 * 0.13 - 0.4;
            for i in 0..memsel as usize {
                let ph = (i as f64 - memsel as f64 / 2.0 + jit) / 400.0 * std::f64::consts::TAU;
                for c in 0..r as usize {
                    let v = if k % 7 == 6 {
                        0.7
                    } else {
                        0.9 * (ph * (1.0 + c as f64 * 0.5)).sin() + if k % 5 == 0 { 0.05 } else { 0.0 }
                    };
                    bytes.extend(((32768.0 + v * 7500.0) as u16).to_le_bytes());
                }
            }
        }
        assert_eq!(bytes.len(), frames * fb, "frame layout");
        (bytes, fb, s, p)
    }

    pub(super) fn model(tdiv: f64, two: bool, dots: bool) -> Model {
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
    fn frames_of(
        mask: u32,
        n: u32,
        frames: usize,
        sig: &dyn Fn(usize, usize, usize) -> f64,
    ) -> (Vec<u8>, usize, Settings, hz::Params) {
        let r = hz::ratio(mask);
        let p = hz::Params {
            ratio: r,
            rate: 1_250_000_000 / r as u64,
            memsel: n,
            dot_fs: 800_000 * r as u64,
            seg: 0,
            max_frames: 1000,
            pre: n / 2,
            post: n / 2,
        };
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
        let sig = |k: usize, c: usize, i: usize| {
            0.9 * (((i as f64 + (k % 7) as f64 * 0.13 - 35_000.0) / 400.0)
                * (1.0 + c as f64 * 0.5)
                * std::f64::consts::TAU)
                .sin()
                + if k % 5 == 0 { 0.05 } else { 0.0 }
        };
        for (mask, n) in [(1u32, 100_000u32), (3, 70_312)] {
            let mut m = model(1e-5, mask == 3, false);
            m.meas = Vec::new();
            let (bytes, fb, s, p) = frames_of(mask, n, 9, &sig);
            let pipe = Pipe::start(2, 0, fresh());
            pipe.set_view(Some(&m), w, h, Interp::Exact);
            let (cs, cp) = (s.clone(), p);
            let codes: CodesInto =
                Arc::new(move |c: &[u8], f: &mut Frame| crate::board::dho::hw::codes_into(c, &cs, &cp, 0, f));
            pipe.feed_codes(Box::new(Bytes(bytes.clone())), fb, 9, 0, dec(&s, &p), Some(codes));
            settle(&pipe);
            let mut got = Phosphor::new(w, h);
            let (mut clear, mut edges, mut off) = (false, [0u64; 4], (0.0, 0.0));
            let n_got = pipe.accept(&mut got, &mut clear, &mut edges, &mut off);
            let (want, trig, wedges, woff, _) = reference(&bytes, fb, &s, &p, &m, w, h);
            assert_eq!((n_got.frames, n_got.triggered, edges), (9, trig, wedges), "mask {}", mask);
            assert!((off.0 - woff.0).abs() <= 1e-9 * woff.0.abs().max(1.0) && off.1 == woff.1);
            for c in 0..NCH {
                assert!(got.maps()[c] == want.maps()[c], "mask {}: CH{} differs", mask, c + 1);
            }
        }
    }

    // Break it catches: a frame moved to an edge outside the near window where the search must stay near (AC, LF
    // reject; an edge trigger on an exact `pre`), on the decoded frame or from the words; a far edge no longer found
    // where the wide search must stay (decimated reads, other trigger types, noise reject, holdoff); a DC refinement
    // that differs from `trignear::edge_smooth_near` (the refinement before the near-only paths).
    #[test]
    fn near_only_where_the_hardware_point_stands_wide_elsewhere() {
        let (w, h) = (970, 448);
        // -1 V up to 10 k samples after the hardware point (50 k), then +1 V: the only rising edge is far away
        let far =
            |_k: usize, _c: usize, i: usize| if i < 60_000 { -1.0 } else { 1.0 } + 0.002 * ((i * 7919) % 13) as f64;
        let (bytes, fb, s, p) = frames_of(1, 100_000, 1, &far);
        let f = crate::board::dho::hw::decode(&bytes, &s, &p, 0);
        let mut g0 = Frame::default();
        let lay = crate::board::dho::hw::codes_into(&bytes, &s, &p, 0, &mut g0).expect("raw frame");
        assert!(f.pre == 50_000 && f.pre_exact && g0.pre_exact, "a full-rate raw read is exact");
        // at the hardware point: a level no sample reaches finds no edge
        let mut at_pre = Phosphor::new(w, h);
        let mut m = model(1e-5, false, false);
        m.trig_level = 5.0;
        assert!(!draw_frame(&mut at_pre, &m, &f, Interp::Exact, None));
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
            let mut fe = f.clone();
            fe.pre_exact = exact;
            let (mut a, mut b) = (Phosphor::new(w, h), Phosphor::new(w, h));
            let (mut sa, mut sb) = (([0u64; 4], (0.0, 0.0)), ([0u64; 4], (0.0, 0.0)));
            let ta = draw_frame(&mut a, &m, &fe, Interp::Exact, Some(Stats { edges: &mut sa.0, off: &mut sa.1 }));
            let mut meta = g0.clone();
            meta.pre_exact = exact;
            let codes = Arc::new(move |_: &[u8], f: &mut Frame| {
                f.clone_from(&meta);
                Some(lay)
            });
            let pipe = Pipe::start(1, 0, fresh());
            pipe.set_view(Some(&m), w, h, Interp::Exact);
            pipe.feed_codes(Box::new(Bytes(bytes.clone())), fb, 1, 0, dec(&s, &p), Some(codes));
            settle(&pipe);
            let accepted = pipe.accept(&mut b, &mut false, &mut sb.0, &mut sb.1);
            assert_eq!((accepted.frames, accepted.triggered), (1, u64::from(ta)), "{name}");
            assert!(a.maps()[0] == b.maps()[0] && sa == sb, "{name}: codes path differs");
            if near {
                assert_eq!(sa.0, [0, 0, 0, 1], "{}", name);
                assert!(a.maps()[0] == at_pre.maps()[0], "{}: not drawn at the hardware point", name);
            } else {
                assert_eq!(sa.0, [0, 0, 1, 0], "{}", name);
                assert!((sa.1 .1 - 10_000.0).abs() < 2.0, "{}: offset {:?}", name, sa.1);
                assert!(a.maps()[0] != at_pre.maps()[0], "{}", name);
            }
        }
        // a read at the full rate is exact, compressed too (to its pair); a decimated one is not, compressed or not
        for (rate, compress, exact) in
            [(1_250_000_000u64, 4u32, true), (625_000_000, 1, false), (625_000_000, 4, false)]
        {
            let q = hz::Params { rate, ..p };
            let t = Settings { compress, ..s.clone() };
            assert_eq!(
                crate::board::dho::hw::decode(&bytes, &t, &q, 0).pre_exact,
                exact,
                "rate {} compress {}",
                rate,
                compress
            );
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
        assert!(
            worst.1 >= 8 * crate::compress::PAIR,
            "k {}: near window {} points against a pair of {}",
            worst.0,
            worst.1,
            crate::compress::PAIR
        );
        // a compressed frame: pairs (min, max) of a step from -1 to +1 V, the only rising edge 2 k points after the
        // hardware point (far outside the near window), or in the point's pair
        let n = 20_000usize;
        let pairs = |edge_at: usize| -> Vec<f32> {
            (0..n)
                .flat_map(|j| {
                    let base = if j >= edge_at { 1.0 } else { -1.0 };
                    [base - 0.01, base + 0.01]
                })
                .collect()
        };
        let mut m = model(1e-5, false, false);
        m.trig_level = 0.0;
        let dt = 1e-5 * HDIV as f64 / (2 * n) as f64;
        let frame = |v: Vec<f32>, exact: bool| Frame {
            dt,
            pre: n,
            ch: [v, Vec::new(), Vec::new(), Vec::new()],
            envelope: true,
            acq: (1.25e9, 1_000_000),
            pre_exact: exact,
            ..Default::default()
        };
        assert!(crate::compress::live(&frame(Vec::new(), true)));
        for (exact, near) in [(true, true), (false, false)] {
            let f = frame(pairs(n / 2 + 1000), exact);
            let mut ph = Phosphor::new(w, h);
            let mut st = ([0u64; 4], (0.0, 0.0));
            draw_frame(&mut ph, &m, &f, Interp::Exact, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
            let want = if near { [0, 0, 0, 1] } else { [0, 0, 1, 0] };
            assert_eq!(st.0, want, "exact {}: {:?}", exact, st);
            // the edge in the pair of the hardware point is found either way
            let f = frame(pairs(n / 2), exact);
            let mut st = ([0u64; 4], (0.0, 0.0));
            draw_frame(&mut ph, &m, &f, Interp::Exact, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
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
        blocked.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).expect("a worker in its first decode");
        gate
    }

    #[test]
    fn shutdown_leaves_a_busy_deposit_workers_buffers_owned() {
        let pipe = Pipe::start(2, 0, fresh());
        pipe.set_view(Some(&model(1e-6, false, false)), 970, 448, Interp::Exact);
        let gate = one_worker_held(&pipe);
        let now = std::time::Instant::now();
        assert!(shutdown::close(&pipe, now) >= 1);
        assert!(!pipe.accepting());
        // The detached worker still owns its Batch, view and mailbox after the
        // UI's pipe is gone. Releasing the gate lets that owner finish safely.
        drop(pipe);
        *oos_hal::sync::lock(&gate.0) = true;
        gate.1.notify_all();
    }

    // Break it catches (step 0 counters, docs/architecture/drawing.md): a batch whose frames reach the picture in two
    // merges not counted as partial, a whole batch counted, or the picture age of a merge not reported.
    #[test]
    fn partial_merges_and_picture_age_are_counted() {
        // This legacy pacing fixture writes process-global mirrors. Give only this test
        // a child process so concurrent UI publication cannot replace its WANTED value.
        const CHILD: &str = "OOS_PARTIAL_MERGE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "drawpipe::tests::partial_merges_and_picture_age_are_counted", "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let m = model(1e-6, false, false);
        let pipe = Pipe::start(2, 0, fresh());
        pipe.set_view(Some(&m), 970, 448, Interp::Exact);
        let mut ph = Phosphor::new(970, 448);
        let mut merge = |pipe: &Pipe| pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        // whole: drawn, then merged once
        let (bytes, fb, s, p) = batch(20, 12_512, false);
        let before_feed = std::time::Instant::now();
        pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
        settle(&pipe);
        let whole = merge(&pipe);
        assert_eq!(whole.frames, 20);
        assert!(whole.newest.is_some_and(|time| time >= before_feed), "picture age starts at feed");
        assert_eq!(crate::pacing::PARTIAL.load(Ordering::Relaxed), 0);
        // in parts: 16 frames in one merge, the held worker's 4 in the next
        let gate = one_worker_held(&pipe);
        let first = merge(&pipe).frames;
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        settle(&pipe);
        let second = merge(&pipe).frames;
        assert_eq!((first, second), (0, 20));
        assert_eq!(crate::pacing::PARTIAL.load(Ordering::Relaxed), 0);
        // an empty merge reports no age
        assert_eq!(merge(&pipe).newest, None);
        // stale: frames acquired before a settings change count where their maps reach the picture, not where they
        // are drawn: maps of an older mapping are dropped and do not count
        crate::pacing::changed();
        crate::pacing::changed();
        let wanted = crate::pacing::WANTED.load(Ordering::Relaxed);
        crate::pacing::APPLIED.store(wanted - 1, Ordering::Relaxed);
        let (bytes, fb, s, p) = batch(20, 12_512, false);
        pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
        crate::pacing::APPLIED.store(0, Ordering::Relaxed);
        settle(&pipe);
        assert_eq!(merge(&pipe).stale, 20, "shown");
        let (bytes, fb, s, p) = batch(20, 12_512, false);
        crate::pacing::APPLIED.store(wanted - 1, Ordering::Relaxed);
        pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
        crate::pacing::APPLIED.store(0, Ordering::Relaxed);
        settle(&pipe);
        let mut m2 = m.clone();
        m2.tscale += 1;
        pipe.set_view(Some(&m2), 970, 448, Interp::Exact);
        assert_eq!(merge(&pipe).stale, 0, "maps of the old mapping dropped, not counted");
    }

    // Timing guard (tools/perf-guard.sh): a worker frame drawn from the read-out words against decode + draw, lines,
    // no per-frame measurements, also with AC trigger coupling (its level from the words). The A53 gains more than the
    // host (the f32 copy is 2x the read-out per channel).
    #[test]
    #[ignore = "host timing: set OOS_PERF_TESTS=1"]
    fn perf_guard_codes_per_frame() {
        assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
        let worst = [
            (1u32, 100_000u32, 1e-5, TrigCoup::Dc),
            (3, 70_312, 1e-5, TrigCoup::Dc),
            (5, 50_000, 1e-5, TrigCoup::Dc),
            (1, 40_000, 2e-6, TrigCoup::Dc),
            (1, 100_000, 1e-5, TrigCoup::Ac),
        ]
        .into_iter()
        .map(|(mask, n, tdiv, coup)| measure_codes_per_frame(mask, n, tdiv, coup))
        .fold(0.0, f64::max);
        assert!(worst <= 0.9, "drawing from the codes must be cheaper: worst ratio {:.2}", worst);
    }

    fn measure_codes_per_frame(mask: u32, n: u32, tdiv: f64, coup: TrigCoup) -> f64 {
        let (w, h) = (970, 448);
        let sig = |k: usize, c: usize, i: usize| {
            0.9 * (((i as f64 + (k % 7) as f64 * 0.13) / 400.0) * (1.0 + c as f64 * 0.5) * std::f64::consts::TAU).sin()
        };
        let mut m = model(tdiv, false, false);
        m.trig_coup = coup;
        m.meas.clear();
        m.meas2.clear();
        for c in 0..NCH {
            m.ch[c].on = mask & (1 << c) != 0;
        }
        let frames = (2_000_000 / (n as usize * 2 * hz::ratio(mask) as usize)).clamp(4, 100);
        let (bytes, fb, s, p) = frames_of(mask, n, frames, &sig);
        let mut ph = Phosphor::new(w, h);
        let mut frame = Frame::default();
        let pipe = Pipe::start(1, 0, fresh());
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        let cs = s.clone();
        let codes: CodesInto = Arc::new(move |bytes, f| crate::board::dho::hw::codes_into(bytes, &cs, &p, 0, f));
        let decode = dec(&s, &p);
        let mut buffer = Some(Box::new(Bytes(bytes.clone())));
        let mut draw_codes = || {
            let bytes = buffer.take().expect("the previous round returned its buffer");
            pipe.feed_codes(bytes, fb, frames, m.hw_stamp, decode.clone(), Some(codes.clone()));
            buffer = Some(recycle_worker_round(&pipe, frames));
        };
        draw_codes();
        // one round: every frame once (each round's time divided by the frames below)
        let (old, new) = crate::perfguard::guard_pair(
            0.9,
            crate::perfguard::ROUNDS,
            1,
            || {
                for chunk in bytes.chunks_exact(fb) {
                    crate::board::dho::hw::decode_into(chunk, &s, &p, 0, &mut frame);
                    std::hint::black_box(draw_frame(&mut ph, &m, &frame, Interp::Exact, None));
                }
            },
            draw_codes,
        );
        assert_eq!(pipe.counters.read().2, 0);
        let (old, new) = (old / frames as u32, new / frames as u32);
        let r = new.as_secs_f64() / old.as_secs_f64();
        println!(
            "codes mask {:#x} {} pt {:?}: decode + draw {:?}/frame, from the codes {:?}/frame (ratio {:.2})",
            mask, n, coup, old, new, r
        );
        r
    }

    /// Until every batch fed is done (drawn and handed on, or dropped).
    fn settle(p: &Pipe) {
        let t0 = crate::ui::lifecycle::test_wait::Wait::new("all submitted deposit batches completed");
        while p.live.load(Ordering::Acquire) > 0 {
            t0.check();
            t0.poll();
        }
    }

    fn collect(
        pipe: &Pipe,
        ph: &mut Phosphor,
        clear: &mut bool,
        edges: &mut [u64; 4],
        off: &mut (f64, f64),
    ) -> Accepted {
        let mut total = Accepted::default();
        loop {
            host::barrier(pipe);
            let next = pipe.accept(ph, clear, edges, off);
            if next.frames == 0 {
                return total;
            }
            total.frames += next.frames;
            total.triggered += next.triggered;
            total.newest = total.newest.max(next.newest);
            total.stale += next.stale;
            crate::framestats::add_folds(&mut total.folds, &next.folds);
        }
    }

    /// Buffer return follows publication and the live-count decrement. Await that event directly so timed
    /// rounds include worker completion without the ordinary test counter poll's sleep granularity.
    fn recycled(p: &Pipe) -> Box<Bytes> {
        oos_hal::sync::lock(&p.pool)
            .recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD)
            .expect("deposit buffer returned to pool after all workers completed")
            .into_any()
            .downcast::<Bytes>()
            .expect("the benchmark feeds Bytes buffers")
    }

    fn recycle_worker_round(pipe: &Pipe, frames: usize) -> Box<Bytes> {
        let buffer = recycled(pipe);
        // The fixture has one batch in flight. Consume its handoff without timing the UI fold.
        let deposit = pipe.slot.take().expect("worker published the deposit before returning the buffer");
        assert_eq!(deposit.records, frames as u64);
        pipe.slot.recycle(deposit);
        buffer
    }

    /// The frames mode for the same frames: `Ui::fast_draw` and the per-frame statistics on every frame.
    fn reference(
        bytes: &[u8],
        fb: usize,
        s: &Settings,
        p: &hz::Params,
        m: &Model,
        w: usize,
        h: usize,
    ) -> (Phosphor, u64, [u64; 4], (f64, f64), crate::framestats::Folds) {
        let mut want = Phosphor::new(w, h);
        let (mut trig, mut edges, mut off) = (0, [0u64; 4], (0.0, 0.0));
        let mut folds = crate::framestats::folds_for(m);
        for chunk in bytes.chunks_exact(fb) {
            let f = crate::board::dho::hw::decode(chunk, s, p, 0);
            trig +=
                draw_frame(&mut want, m, &f, Interp::Exact, Some(Stats { edges: &mut edges, off: &mut off })) as u64;
            crate::framestats::fold_frame(&f, m, &mut folds);
        }
        (want, trig, edges, off, folds)
    }

    #[test]
    fn trigger_level_change_refreshes_batch_worker_view() {
        let current = refreshed_batch(true);
        let mapping_only = refreshed_batch(false);
        assert_ne!(current, mapping_only, "negative control: mapping-only refresh must leave the old trigger view");
    }

    fn refreshed_batch(refresh: bool) -> Vec<u16> {
        use oos_core::change::{Layout, MappingKey};
        let (w, h) = (970, 448);
        let mut m = model(1e-6, false, false);
        let old_mapping = MappingKey::of(&m, Layout::default());
        let (bytes, fb, s, p) = batch(13, 12_512, false);
        let pipe = Pipe::start(2, 0, fresh());
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 13, 0, dec(&s, &p));
        settle(&pipe);
        let mut warm = Phosphor::new(w, h);
        pipe.accept(&mut warm, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        m.trig_level = 0.6;
        assert_eq!(MappingKey::of(&m, Layout::default()), old_mapping);
        if refresh && std::env::var_os("OOS_MAPPING_ONLY_PROBE").is_none() {
            pipe.set_view(Some(&m), w, h, Interp::Exact);
        }
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 13, 0, dec(&s, &p));
        settle(&pipe);
        let mut got = Phosphor::new(w, h);
        let result = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        let (want, trig, _, _, _) = reference(&bytes, fb, &s, &p, &m, w, h);
        if refresh {
            assert_eq!((result.frames, result.triggered), (13, trig));
            assert!(got.maps() == want.maps(), "workers must match the new-level record path");
        } else {
            assert_ne!(got.maps(), want.maps(), "negative control must actually distinguish the levels");
        }
        got.maps()[0].clone()
    }

    // Break it catches: a batch picture that differs from the frames mode for the same frames (decode into kept
    // vectors, frames claimed by several workers and the helper, dense and sparse frames, dots, two channels),
    // trigger or measurement statistics that leave frames out, frames lost or counted twice, a merge that does not
    // clear for minimum persistence, a buffer that does not come back.
    #[test]
    fn batch_picture_equals_the_frames_mode() {
        let (w, h) = (970, 448);
        for (tdiv, memsel, two, dots, workers, helper) in [
            (1e-6, 12_512u32, false, false, 2, false),
            (1e-6, 12_512, true, true, 4, true),
            (1e-7, 1408, false, false, 1, true),
            (2e-8, 400, true, false, 1, true),
        ] {
            let mut m = model(tdiv, two, dots);
            m.meas = vec![
                (Meas::Vpp, crate::model::WaveId::Ch(0)),
                (Meas::Vavg, crate::model::WaveId::Ch(0)),
                (Meas::Vrms, crate::model::WaveId::Ch(1)),
            ];
            let (bytes, fb, s, p) = batch(41, memsel, two);
            let c = fresh();
            let pipe = Pipe::start(workers, 0, c);
            pipe.set_view(Some(&m), w, h, Interp::Exact);
            assert!(pipe.accepting());
            pipe.feed(Box::new(Bytes(bytes[..17 * fb].to_vec())), fb, 17, 0, dec(&s, &p));
            pipe.feed(Box::new(Bytes(bytes[17 * fb..].to_vec())), fb, 24, 0, dec(&s, &p));
            settle(&pipe);
            let mut got = Phosphor::new(w, h);
            got.channel_mut(0)[0] = 7;
            let (mut clear, mut edges, mut off) = (true, [0u64; 4], (0.0, 0.0));
            let n = collect(&pipe, &mut got, &mut clear, &mut edges, &mut off);
            let (want, trig, wedges, woff, folds) = reference(&bytes, fb, &s, &p, &m, w, h);
            let what = format!("{} s/div, {} workers, helper {}", tdiv, workers, helper);
            assert_eq!((n.frames, n.triggered), (41, trig), "{}", what);
            assert!(!clear && trig > 30 && trig < 41, "{}: {} triggered", what, trig);
            assert_eq!(edges, wedges, "{}", what);
            assert!((off.0 - woff.0).abs() <= 1e-9 * woff.0.abs().max(1.0) && off.1 == woff.1);
            for c in 0..NCH {
                assert!(got.maps()[c] == want.maps()[c], "{}: CH{} differs", what, c + 1);
            }
            // per-frame statistics: the same frames counted, sums in another order
            assert_eq!(n.folds.len(), folds.len());
            for ((k, a), (k2, b)) in n.folds.iter().zip(&folds) {
                assert_eq!((k, a.n, a.min, a.max), (k2, b.n, b.min, b.max), "{}", what);
                assert!(
                    (a.sum - b.sum).abs() <= 1e-12 * b.sum.abs().max(1.0)
                        && (a.sumsq - b.sumsq).abs() <= 1e-12 * b.sumsq.abs().max(1.0)
                );
            }
            assert!(tdiv < 1e-6 || folds.iter().all(|x| x.1.n > 30), "{}: {:?}", what, folds);
            assert_eq!(c.read(), (0, 0, 0), "{}: nothing dropped", what);
            assert_eq!(c.measured.load(Ordering::Relaxed), folds.first().map_or(0, |x| x.1.n));
            // merged once; both buffers back in the pool
            assert_eq!(pipe.accept(&mut got, &mut clear, &mut edges, &mut off).frames, 0);
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
        let pipe = Pipe::start(2, 0, fresh());
        pipe.publish_view(&m, w, h, Interp::Exact, Default::default());
        let newest = Arc::new(Frame { seq: 30, ..crate::board::dho::hw::decode(&bytes[29 * fb..], &s, &p, 0) });
        pipe.feed_with(Box::new(Bytes(bytes)), fb, 0..30, Default::default(), dec(&s, &p), None, Some(newest.clone()));
        settle(&pipe);
        let mut ph = Phosphor::new(w, h);
        let n = pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(n.frames, 30);
        assert!(Arc::ptr_eq(&pipe.take_waveform((w, h)).unwrap(), &newest));
        assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
    }

    #[test]
    fn coordinator_alone_draws_the_same_picture() {
        let (w, h) = (970, 448);
        for (tdiv, memsel) in [(1e-6, 12_512u32), (1e-7, 1408)] {
            let m = model(tdiv, false, false);
            let (bytes, fb, s, p) = batch(20, memsel, false);
            let pipe = Pipe::start(1, 0, fresh());
            pipe.set_view(Some(&m), w, h, Interp::Exact);
            pipe.feed(Box::new(Bytes(bytes.clone())), fb, 20, 0, dec(&s, &p));
            settle(&pipe);
            let mut got = Phosphor::new(w, h);
            let n = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
            let (want, trig, ..) = reference(&bytes, fb, &s, &p, &m, w, h);
            assert_eq!((n.frames, n.triggered), (20, trig));
            assert!(got.maps() == want.maps());
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
        let pipe = Pipe::start(2, 0, c);
        let mut ph = Phosphor::new(w, h);
        let merge = |ph: &mut Phosphor| pipe.accept(ph, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert!(!pipe.accepting());
        // settings stamp older than the view's: dropped
        m.hw_stamp = 3;
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 2, dec(&s, &p));
        settle(&pipe);
        assert_eq!(merge(&mut ph).frames, 0);
        assert_eq!(c.read(), (0, 0, 10));
        // drawn, then the mapping changes before the merge: dropped at the merge
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        settle(&pipe);
        m.ch[0].offset = 0.5;
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        assert_eq!(merge(&mut ph).frames, 0);
        assert!(ph.maps().iter().all(|x| x.iter().all(|&y| y == 0)));
        assert_eq!(c.read(), (0, 0, 20));
        // a change the drawing does not depend on keeps the maps
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        settle(&pipe);
        m.status = TrigStatus::Wait;
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        assert_eq!(merge(&mut ph).frames, 10);
        // many batches at once: all drawn, nothing more dropped
        let (big, fb2, s2, p2) = batch(200, 12_512, false);
        for _ in 0..6 {
            pipe.feed(Box::new(Bytes(big.clone())), fb2, 200, 3, dec(&s2, &p2));
        }
        settle(&pipe);
        assert_eq!(collect(&pipe, &mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 1200);
        assert_eq!(c.read().2, 20);
        // a batch dropped before anybody claimed a frame counts all of them
        let pipe0 = Pipe::start(0, 0, c);
        pipe0.set_view(Some(&m), w, h, Interp::Exact);
        pipe0.feed(Box::new(Bytes(bytes.clone())), fb, 10, 3, dec(&s, &p));
        assert_eq!(c.read().2, 30);
        // no view: the acquisition thread queues the frames itself
        pipe.set_view(None, 0, 0, Interp::Exact);
        assert!(!pipe.accepting());
    }

    /// Host cost per frame of a worker (decode into kept vectors, trigger refinement, deposit) against the frames
    /// mode's per-frame work on the UI thread (`decode` + `Ui::fast_draw`), and the rate of 1, 2 and 4 workers (the
    /// helper runs the same code per frame). Run by tools/perf-guard.sh.
    #[test]
    #[ignore = "host timing: set OOS_PERF_TESTS=1"]
    fn perf_guard_deposit_worker_per_frame() {
        assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
        let (w, h) = (970, 448);
        for (tdiv, memsel, meas) in [
            (1e-7, 1408u32, false),
            (1e-6, 12_512, false),
            (1e-6, 12_512, true),
            (1e-5, 125_000, false),
            (1e-5, 125_000, true),
        ] {
            let mut m = model(tdiv, false, false);
            // the measurements made on every frame cost on top (reported, not guarded): the worst case, all of them on
            m.meas = if meas {
                crate::framestats::CHEAP.iter().map(|&k| (k, crate::model::WaveId::Ch(0))).collect()
            } else {
                Vec::new()
            };
            let frames = (2_000_000 / (memsel as usize * 2)).clamp(8, 500);
            let (bytes, fb, s, p) = batch(frames, memsel, false);
            // the frames mode: decode + fast_draw on one thread (its measurements: a few records a second), against
            // one worker of the pipeline, alternating (perfguard.rs); 2 and 4 workers are reported only
            let mut ph = Phosphor::new(w, h);
            let one = Pipe::start(1, 0, fresh());
            one.set_view(Some(&m), w, h, Interp::Exact);
            // Copy once outside timing, then recycle through the real pool: 101 rounds still use one buffer.
            let mut buffer = Some(Box::new(Bytes(bytes.clone())));
            let (old, one_t) = crate::perfguard::guard_pair(
                if meas { f64::INFINITY } else { 1.5 },
                crate::perfguard::ROUNDS,
                1,
                || {
                    for chunk in bytes.chunks_exact(fb) {
                        std::hint::black_box(draw_frame(
                            &mut ph,
                            &m,
                            &crate::board::dho::hw::decode(chunk, &s, &p, 0),
                            Interp::Exact,
                            None,
                        ));
                    }
                },
                || {
                    one.feed(
                        buffer.take().expect("the previous round returned its buffer"),
                        fb,
                        frames,
                        0,
                        dec(&s, &p),
                    );
                    buffer = Some(recycle_worker_round(&one, frames));
                },
            );
            assert_eq!(one.counters.read().2, 0);
            let (old, mut per) = (old / frames as u32, vec![one_t / frames as u32]);
            for n in [2usize, 4] {
                let pipe = Pipe::start(n, 0, fresh());
                pipe.set_view(Some(&m), w, h, Interp::Exact);
                let t0 = std::time::Instant::now();
                for _ in 0..3 {
                    pipe.feed(Box::new(Bytes(bytes.clone())), fb, frames, 0, dec(&s, &p));
                    drop(recycle_worker_round(&pipe, frames));
                }
                per.push(t0.elapsed() / (3 * frames) as u32);
                assert_eq!(pipe.counters.read().2, 0);
            }
            let r = per[0].as_secs_f64() / old.as_secs_f64();
            println!("{} s/div {} pt, frame statistics {}: frames mode {:?}/frame; one core in the pipeline {:?}/frame (ratio {:.2}); 1/2/4 workers {:.0}/{:.0}/{:.0} frames/s", tdiv, memsel, meas, old, per[0], r, 1.0 / per[0].as_secs_f64(), 1.0 / per[1].as_secs_f64(), 1.0 / per[2].as_secs_f64());
            assert!(meas || r <= 1.5, "a frame in the pipeline must not cost more than in the frames mode: {:.2}", r);
        }
    }
    #[test]
    #[ignore = "perf-guard latency: release, OOS_PERF_TESTS=1, one test thread"]
    fn perf_guard_shutdown_retains_busy_workers_within_budget() {
        crate::perfguard::latency::check(
            std::time::Duration::from_millis(250),
            shutdown_leaves_a_busy_deposit_workers_buffers_owned,
        );
    }
}

#[cfg(test)]
pub(crate) mod test_scope;
