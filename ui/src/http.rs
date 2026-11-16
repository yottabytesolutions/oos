// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Web remote (HTTP, port 8080): live screen in the browser, clicks act as touch taps, buttons for keys and knobs.
//!   GET /             page            GET /screen.png   current screen (PNG, 60 to 175 KB; the page's live view and saving;
//!                                     made off the frame loop, with an ETag: If-None-Match gets 304; see `Shots`)
//!   GET /screen.bmp   current screen (24-bit BMP, 1.8 MB; the page can show this instead)
//!   GET /screen.jpg   the live picture as JPEG from the VPU (`vpu.rs`; PNG where there is none), as /screen.png
//!   GET /wave.csv, /la.csv   the last record, as Storage > CSV writes it
//!   POST /waveform?name=...&slot=1   load a waveform into a chosen reference (no file written)
//!   GET /ui-state     source, acquisition state and capture count for the remote controls
//!   GET /cmd?c=...    one key, knob or touch command (cmd.rs syntax: "key runstop", "knob vscale +1", "tap 100 200");
//!                     other commands are refused (`cmd::remote_ok`)
//!   GET /lxi/identification   LXI identification XML (LXI Device Specification 2011, 10.2)
//!   POST /mcp         Model Context Protocol (JSON-RPC 2.0, streamable HTTP transport, plain JSON responses): tools
//!                     `scpi` (any SCPI command of scpi.rs, returns the response) and `screenshot` (PNG)
//!   GET /update       update state and upload form; POST /update with a signed package (.oosupd) installs it;
//!                     (ui/update.rs: the header is checked here, the image off the frame loop, before anything is
//!                     written). Requests are read by threads with time limits (`reader`, `Limits`); every limit
//!                     ends in an answer (docs/update.md).
//! Unauthenticated: bench LAN only (an update must carry a trusted signature, but the community key is public).
//! `net.allow` (`netacl`, docs/network.md 6) limits `POST /update` and `/mcp` to the listed address prefixes.

use crate::gfx::Canvas;
use crate::ui::Ui;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Longest request: `POST /update` (a package is ~31 MB, the image at most 64 MiB), every other request.
const MAX_UPDATE: usize = 70 << 20;
const MAX_WAVEFORM: usize = crate::store::MAX_WAVEFORM_BYTES;
const MAX_OTHER: usize = 1 << 20;
/// Requests being read at once (more are answered 503 at accept).
const MAX_PENDING: usize = 8;
/// Longest request head; a longer one is answered 431.
const MAX_HEAD: usize = 16384;

/// Time limits of a request (`reader`). Requests are read in threads of their own, not in the frame loop: a frame loop
/// that stalls (a long step, a stuck lock) cannot keep a client waiting for ever; every limit ends in an answer.
#[derive(Clone, Copy, Debug)]
struct Limits {
    /// no data for this long: 408 (the socket's read time-out)
    idle: Duration,
    /// the whole request, head and body (the upload of a package over 10 Mbit/s takes ~30 s): 408
    deadline: Duration,
    /// a complete request the frame loop has not taken after this long: 503
    take: Duration,
}

const LIMITS: Limits =
    Limits { idle: Duration::from_secs(10), deadline: Duration::from_secs(120), take: Duration::from_secs(10) };

/// A body above this (an update package, a waveform) is read one at a time: a second one is answered 503 when its
/// head is in, so the readers hold at most one large body and `MAX_PENDING` small ones.
const LARGE_BODY: usize = MAX_OTHER;
/// Answers above this size are sent from a thread of their own: over WLAN a picture takes long enough to stall the
/// frame loop, and with it the scope's own screen and keys. A screen as PNG is 60 to 175 KB (a BMP 1.8 MB), so it
/// goes the same way; the page and the small answers fit into the socket's buffer and go at once.
const SEND_INLINE: usize = 48 << 10;
/// At most this many at once; more are answered 503, and the page tries again.
const MAX_SENDERS: usize = 2;
mod shutdown;
const SEND_TIMEOUT_S: u64 = 30;
static SENDERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

use shutdown::send_in_background;

/// Which instrument addresses the identification page lists. The default is the scope: VXI-11 and the SCPI socket.
#[derive(Clone, Debug)]
pub struct Offer {
    pub scpi_port: Option<u16>,
    pub vxi11: bool,
    pub hislip_port: Option<u16>,
}

impl Default for Offer {
    fn default() -> Offer {
        Offer { scpi_port: Some(5555), vxi11: true, hislip_port: None }
    }
}

fn busy_answer(s: &TcpStream) {
    let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
    let msg = "busy, try again";
    let _ = (&*s).write_all(format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", msg.len(), msg).as_bytes());
}

/// Nice value of the threads of the web remote (Linux; a thread's own value), where `SCHED_IDLE` is not to be had.
const WEB_NICE: i32 = 10;

/// The threads of the web remote (encoder, senders) run only on what the scope leaves over: scheduling class
/// `SCHED_IDLE` (they get a core only while nothing else wants it), and only on the slower cores of a big.LITTLE SoC
/// (RK3399: the four A53 in `little`, `Cores::little`; the frame loop keeps the two A72 for itself). Nice 10 where
/// that fails.
fn lower_priority(little: u64) {
    // not in the tests: with every core busy with tests in parallel (CI), a thread at SCHED_IDLE got no time for
    // seconds, and a viewer waiting for a picture got 304 or 503 instead
    #[cfg(target_os = "linux")]
    if !cfg!(test) {
        // SAFETY: plain system calls on the calling thread; the parameter and the CPU set are local values.
        unsafe {
            let tid = libc::syscall(libc::SYS_gettid) as libc::pid_t;
            // priority 0, the only one SCHED_IDLE takes (musl's struct has more fields)
            let p: libc::sched_param = std::mem::zeroed();
            // the system call itself: musl's sched_setscheduler() is a stub that always fails
            if libc::syscall(libc::SYS_sched_setscheduler, tid, libc::SCHED_IDLE, &p as *const libc::sched_param) != 0 {
                libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, WEB_NICE);
            }
            if little != 0 {
                let mut set: libc::cpu_set_t = std::mem::zeroed();
                for c in crate::placement::cpus(little) {
                    libc::CPU_SET(c, &mut set);
                }
                libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = little;
}

/// The slower cores of a big.LITTLE SoC (RK3399: 0-3), empty where the cores are all alike.
pub fn little_core_list() -> Vec<usize> {
    crate::placement::cpus(crate::placement::Cores::read().little)
}

/// The calling thread onto `cores` (nothing when it is empty).
pub fn pin_to(cores: &[usize]) {
    #[cfg(target_os = "linux")]
    if !cores.is_empty() {
        // SAFETY: a local CPU set for the calling thread
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            for &c in cores {
                libc::CPU_SET(c, &mut set);
            }
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = cores;
}

/// The calling thread onto the slower cores of a big.LITTLE SoC (`little`, `Cores::little`; RK3399: the four A53), at
/// normal priority: the measurements in the background (`ui::MeasWorker`); the threads it starts after this stay
/// there too. Nothing without little cores.
pub fn to_little_cores(little: u64) {
    #[cfg(target_os = "linux")]
    {
        if little == 0 {
            return;
        }
        // SAFETY: a local CPU set for the calling thread
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            for c in crate::placement::cpus(little) {
                libc::CPU_SET(c, &mut set);
            }
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = little;
}

/// The calling thread onto the fastest cores (RK3399: the two A72), and the processes it starts after this with it.
/// System > Graphics = GPU: the frame loop and the GPU helper compare and send the layers there (on an A53 a
/// frame's compare took three times as long).
#[cfg(target_os = "linux")]
pub fn pin_to_big_cores(cores: crate::placement::Cores) {
    let big = crate::placement::cpus(cores.big);
    if big.is_empty() || cores.little == 0 {
        return;
    }
    // SAFETY: a local CPU set for the calling thread
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for c in big {
            libc::CPU_SET(c, &mut set);
        }
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
    }
}

/// The fastest the live picture is made: 20 a second.
const SHOT_MIN: Duration = Duration::from_millis(50);
/// The encoder's share of its core at most (half of a slow core, which is idle otherwise): the time between two copies
/// is at least this many times the last encode.
const SHOT_DUTY: u32 = 2;
/// While the screen stands still, it is looked at again only this often.
const SHOT_STILL: Duration = Duration::from_millis(200);
/// After a request, pictures are made ahead for this long (a viewer that left costs no more than this).
const SHOT_AHEAD: Duration = Duration::from_secs(1);
/// A viewer that has the newest picture waits this long for the next one (long poll), then gets 304.
const LONG_POLL: Duration = Duration::from_secs(2);
/// A picture from a copy at most this old goes out at once to a viewer that does not have it; else a fresh copy first.
const SHOT_FRESH: Duration = Duration::from_millis(300);
/// Connections waiting for a picture at most (more are answered 503).
const SHOT_WAITING: usize = 8;
/// After the VPU failed, JPEG is tried again this much later (PNG meanwhile).
const VPU_RETRY: Duration = Duration::from_secs(60);

/// The live picture of the web remote.
///
/// A viewer asks with the ETag of the picture it has (If-None-Match). Has it an older one, or none, it gets the newest
/// at once. Has it the newest, the request waits (long poll) until the screen has changed and a new picture is made,
/// at most `LONG_POLL`, then 304. So every new picture reaches the page without delay, and nothing is asked for in
/// vain.
///
/// The frame loop only copies the screen (2.4 MB), and only while somebody waits, and not more often than the encoder
/// can keep up with on a third of a slow core (`SHOT_DUTY`), at most 20 times a second. The encoder thread compares the
/// copy with the last picture, makes a PNG only of a changed screen and hands it to the waiting viewers; it and the
/// senders get only the time the scope leaves over (`lower_priority`). The PNG used to be made in the frame loop:
/// one viewer at 15 pictures a second took the scope's screen from 27 to 16 frames a second.
pub struct Shots {
    st: Mutex<ShotState>,
    wake: Condvar,
    /// the little cores the encoder and the senders run on (`Cores::little`)
    little: u64,
    tasks: Arc<shutdown::Tasks>,
}

#[derive(Default)]
struct ShotState {
    /// a copy of the screen for the encoder: width, height, pixels
    next: Option<(u32, u32, Vec<u32>)>,
    /// the encoder works on a copy
    busy: bool,
    /// connections waiting for a picture newer than the ETag they have, since when, and in which format (JPEG; PNG
    /// once the VPU failed)
    waiting: Vec<(TcpStream, Option<String>, Instant, bool)>,
    /// the last picture: its pixels (taken out by the encoder while it compares), size, PNG, ETag
    px: Vec<u32>,
    size: (u32, u32),
    png: Option<Arc<[u8]>>,
    tag: String,
    /// when the last copy was taken, and the time to wait after it before the next
    at: Option<Instant>,
    /// the last request for the live picture: while it is recent, the next picture is made ahead of the next request
    asked: Option<Instant>,
    gap: Duration,
    /// pixel buffer for the next copy
    spare: Vec<u32>,
    /// PNGs made so far
    encodes: u64,
    /// the live picture as JPEG from the VPU (`/screen.jpg`, `vpu.rs`) rather than PNG: as the last request asked;
    /// the format of `png`; the VPU failed (PNG until `no_jpeg_at` + `VPU_RETRY`)
    jpeg: bool,
    pic_jpeg: bool,
    no_jpeg: bool,
    /// when the VPU failed: it is tried again after `VPU_RETRY` (one timeout must not end JPEG for the session)
    no_jpeg_at: Option<Instant>,
    /// tests: a long poll other than `LONG_POLL`
    #[cfg(test)]
    long_poll: Option<Duration>,
    /// the frame loop hands its screens over (`hand_over`) instead of `tick` copying them
    frames: bool,
    thread: bool,
    quit: bool,
}

impl Shots {
    fn new(little: u64) -> Arc<Shots> {
        Arc::new(Shots { st: Default::default(), wake: Default::default(), little, tasks: Default::default() })
    }

    fn lock(&self) -> MutexGuard<'_, ShotState> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The answer to one `GET /screen.png` (`have`: the ETag of If-None-Match); the frame loop never encodes here.
    fn request(self: &Arc<Self>, s: &TcpStream, have: Option<String>, cv: &Canvas, jpeg: bool) {
        let mut st = self.lock();
        st.asked = Some(Instant::now());
        if st.no_jpeg && st.no_jpeg_at.is_some_and(|t| t.elapsed() >= VPU_RETRY) {
            st.no_jpeg = false;
        }
        st.jpeg = jpeg && !st.no_jpeg;
        // an older picture or none: the newest at once, when it is fresh (a screenshot must not be an old one) and in
        // the format asked for
        let fresh =
            st.at.is_some_and(|t| t.elapsed() < SHOT_FRESH) && !st.busy && st.next.is_none() && st.pic_jpeg == st.jpeg;
        if let Some(png) = st
            .png
            .clone()
            .filter(|_| fresh && have.as_deref() != Some(st.tag.as_str()) && st.size == (cv.w as u32, cv.h as u32))
        {
            let tag = st.tag.clone();
            drop(st);
            return answer_shot(s, have.as_deref(), png, &tag, self.little, &self.tasks);
        }
        if st.waiting.len() >= SHOT_WAITING {
            drop(st);
            return busy_answer(s);
        }
        if !st.thread {
            let me = self.clone();
            if self.tasks.spawn("web-png", None, move || me.encoder()).is_err() {
                // no thread: the old way, in the frame loop
                drop(st);
                return answer_shot(s, None, crate::store::png_of(cv).into(), "", self.little, &self.tasks);
            }
            st.thread = true;
        }
        let Ok(c) = s.try_clone() else { return };
        let want = st.jpeg;
        st.waiting.push((c, have, Instant::now(), want));
        drop(st);
        self.tick(cv);
    }

    /// Every frame: a copy for the encoder when the time has come and somebody waits, or asked within `SHOT_AHEAD` (the
    /// picture is then ready when the viewer's next request comes); 304 to whoever waited too long.
    fn tick(&self, cv: &Canvas) {
        let mut st = self.lock();
        let watched = st.asked.is_some_and(|t| t.elapsed() < SHOT_AHEAD);
        if st.waiting.is_empty() && !watched {
            return;
        }
        let now = Instant::now();
        #[cfg(test)]
        let long_poll = st.long_poll.unwrap_or(LONG_POLL);
        #[cfg(not(test))]
        let long_poll = LONG_POLL;
        let (old, keep): (Vec<_>, Vec<_>) =
            std::mem::take(&mut st.waiting).into_iter().partition(|w| now - w.2 >= long_poll);
        st.waiting = keep;
        if !old.is_empty() {
            let (png, tag) = (st.png.clone().unwrap_or_else(|| Arc::from(&[][..])), st.tag.clone());
            let fit = |w: bool| Self::fits(w, st.pic_jpeg, st.no_jpeg);
            let fits: Vec<bool> = old.iter().map(|w| fit(w.3)).collect();
            drop(st);
            for ((c, have, _, _), ok) in old.into_iter().zip(fits) {
                // the last picture in the format asked for, else "busy" (the page asks again)
                if ok {
                    answer_shot(&c, have.as_deref(), png.clone(), &tag, self.little, &self.tasks);
                } else {
                    busy_answer(&c);
                }
            }
            st = self.lock();
        }
        if !st.frames && Self::wanted(&st, now, watched) {
            Self::give_copy(&mut st, cv, now);
            self.wake.notify_one();
        }
    }

    /// A copy of the screen for the encoder (the caller wakes it).
    fn give_copy(st: &mut ShotState, cv: &Canvas, now: Instant) {
        let mut px = std::mem::take(&mut st.spare);
        px.clear();
        px.extend_from_slice(&cv.px);
        st.next = Some((cv.w as u32, cv.h as u32, px));
        st.at = Some(now);
    }

    /// A picture in JPEG (`pic_jpeg`) or PNG answers a request for `want_jpeg`: the same format, or PNG for JPEG once
    /// the VPU failed. A screenshot asked for as PNG must never get JPEG bytes (it is saved as .png).
    fn fits(want_jpeg: bool, pic_jpeg: bool, no_jpeg: bool) -> bool {
        want_jpeg == pic_jpeg || (want_jpeg && no_jpeg && !pic_jpeg)
    }

    /// Whether the encoder should get the screen now.
    fn wanted(st: &ShotState, now: Instant, watched: bool) -> bool {
        (!st.waiting.is_empty() || watched)
            && !st.busy
            && st.next.is_none()
            && st.at.map_or(true, |t| now - t >= st.gap)
    }

    /// Right before the frame loop renders the next screen (which clears and draws all of it): the shown screen goes to
    /// the encoder by swapping buffers, no copy. A copy of 2.4 MB in the frame loop pushed everything else of the frame
    /// out of the fast core's cache and made the whole frame about 10 % slower. `whole`: the next render draws every
    /// pixel (not the start-up screen); else nothing is swapped and `tick` copies.
    fn hand_over(&self, cv: &mut Canvas, whole: bool) {
        let mut st = self.lock();
        st.frames = whole;
        let now = Instant::now();
        let watched = st.asked.is_some_and(|t| t.elapsed() < SHOT_AHEAD);
        if !whole || !Self::wanted(&st, now, watched) {
            return;
        }
        let mut px = std::mem::take(&mut st.spare);
        px.resize(cv.px.len(), 0);
        std::mem::swap(&mut px, &mut cv.px);
        st.next = Some((cv.w as u32, cv.h as u32, px));
        st.at = Some(now);
        self.wake.notify_one();
    }

    fn encoder(&self) {
        lower_priority(self.little);
        let mut vpu: Option<crate::vpu::JpegEnc> = None;
        let mut vpu_size = (0, 0);
        let boot = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64 & 0xffff_ffff);
        loop {
            let mut st = self.lock();
            while st.next.is_none() && !st.quit {
                st = self.wake.wait(st).unwrap_or_else(|e| e.into_inner());
            }
            if st.quit {
                return;
            }
            let Some((w, h, px)) = st.next.take() else { continue };
            st.busy = true;
            let last = std::mem::take(&mut st.px);
            // the format of the oldest request waiting (one viewer may ask for JPEG, another for a PNG screenshot)
            let jpeg = st.waiting.first().map_or(st.jpeg, |w| w.3) && !st.no_jpeg;
            let known = st.png.is_some() && st.size == (w, h) && st.pic_jpeg == jpeg;
            drop(st);
            // the comparison and the encoding without the lock: the frame loop may ask meanwhile
            let t0 = Instant::now();
            let mut vpu_failed = false;
            let png: Option<Arc<[u8]>> = (!known || !same_pixels(&last, &px)).then(|| {
                if jpeg {
                    if vpu_size != (w, h) {
                        vpu = None;
                    }
                    if vpu.is_none() {
                        match crate::vpu::JpegEnc::open(w as usize, h as usize) {
                            Ok(e) => {
                                eprintln!("oos-ui: JPEG encoder: VPU, {} in", e.input());
                                (vpu, vpu_size) = (Some(e), (w, h));
                            }
                            Err(e) => eprintln!("oos-ui: JPEG encoder: {}", e),
                        }
                    }
                    match vpu.as_mut().map(|e| e.encode(&px)) {
                        Some(Ok(j)) => return j.into(),
                        Some(Err(e)) => eprintln!("oos-ui: JPEG encoder: {}", e),
                        None => {}
                    }
                    (vpu, vpu_failed) = (None, true);
                }
                crate::store::png(w, h, &px).into()
            });
            let took = t0.elapsed();
            let mut st = self.lock();
            st.busy = false;
            if vpu_failed {
                (st.no_jpeg, st.jpeg, st.no_jpeg_at) = (true, false, Some(Instant::now()));
            }
            let Some(png) = png else {
                // the same screen: who has an older picture gets this one, who has it waits on; the next look later
                st.gap = SHOT_STILL.max(took * SHOT_DUTY);
                st.px = last;
                st.spare = px;
                let tag = st.tag.clone();
                let (pj, nj) = (st.pic_jpeg, st.no_jpeg);
                let (lack, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut st.waiting)
                    .into_iter()
                    .partition(|w| w.1.as_deref() != Some(tag.as_str()) && Self::fits(w.3, pj, nj));
                st.waiting = keep;
                let png = st.png.clone().unwrap_or_else(|| Arc::from(&[][..]));
                drop(st);
                for (c, have, _, _) in lack {
                    answer_shot(&c, have.as_deref(), png.clone(), &tag, self.little, &self.tasks);
                }
                continue;
            };
            st.gap = SHOT_MIN.max(took * SHOT_DUTY);
            st.encodes += 1;
            st.tag = format!("\"{:x}-{}\"", boot, st.encodes);
            st.pic_jpeg = png.starts_with(&[0xff, 0xd8]);
            st.png = Some(png.clone());
            st.size = (w, h);
            st.px = px;
            st.spare = last;
            // who asked for the other format waits for the next picture, made in that format
            let (pj, nj) = (st.pic_jpeg, st.no_jpeg);
            let (waiting, other): (Vec<_>, Vec<_>) =
                std::mem::take(&mut st.waiting).into_iter().partition(|w| Self::fits(w.3, pj, nj));
            st.waiting = other;
            let tag = st.tag.clone();
            drop(st);
            for (c, have, _, _) in waiting {
                answer_shot(&c, have.as_deref(), png.clone(), &tag, self.little, &self.tasks);
            }
        }
    }
}

/// Whether two screens are the same, eight bytes at a step. `==` on the slices ends in musl's memcmp, which goes byte
/// by byte: 12 ms for a screen on an A53, against about 1 ms here.
pub fn same_pixels(a: &[u32], b: &[u32]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let (ca, cb) = (a.chunks_exact(8), b.chunks_exact(8));
    let (ra, rb) = (ca.remainder(), cb.remainder());
    // 32 bytes at a time with an OR of the differences: no early exit per word, which lets the compiler use vectors
    ca.zip(cb).all(|(x, y)| x.iter().zip(y).fold(0u32, |d, (p, q)| d | (p ^ q)) == 0) && ra == rb
}

/// 304 when the viewer has this picture, else the PNG through a sender thread (503 when all are busy).
fn answer_shot(s: &TcpStream, have: Option<&str>, png: Arc<[u8]>, tag: &str, little: u64, tasks: &shutdown::Tasks) {
    let etag = if tag.is_empty() { String::new() } else { format!("ETag: {}\r\n", tag) };
    if !tag.is_empty() && have == Some(tag) {
        let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
        let _ = (&*s).write_all(
            format!("HTTP/1.1 304 Not Modified\r\n{}Cache-Control: no-store\r\nConnection: close\r\n\r\n", etag)
                .as_bytes(),
        );
        return;
    }
    let ctype = if png.starts_with(&[0xff, 0xd8]) { "image/jpeg" } else { "image/png" };
    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Cache-Control: no-store\r\nConnection: close\r\n\r\n", ctype, png.len(), etag);
    if !send_in_background(s, head, png, little, tasks) {
        busy_answer(s);
    }
}

pub struct Http {
    /// the accept thread's address and stop flag while the port is open
    open: Option<(std::net::SocketAddr, Arc<std::sync::atomic::AtomicBool>)>,
    /// complete requests from the reader threads
    inbox: Arc<Inbox>,
    limits: Limits,
    shots: Arc<Shots>,
    /// who may install an update or use MCP (empty: everyone); the list Remote services > Allowed sets, checked per
    /// request, so a change counts at once
    acl: crate::netacl::Shared,
    /// the list the reader threads see (refused before the body is read): `acl`, swapped by `set_acl`
    acl_shared: Arc<Mutex<crate::netacl::Shared>>,
    /// the addresses the identification page names
    offer: Offer,
}

impl Drop for Http {
    fn drop(&mut self) {
        self.shots.lock().quit = true;
        self.shots.wake.notify_all();
        if let Some((_, stop)) = self.open.take() {
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// A complete request from a reader thread: its number, the connection, the head (as sent) and the body.
struct Req {
    id: u64,
    s: TcpStream,
    head: String,
    body: Vec<u8>,
}

/// Requests read and waiting for the frame loop; the reader waits on `taken` until the frame loop took its request.
#[derive(Default)]
struct Inbox {
    q: Mutex<std::collections::VecDeque<Req>>,
    taken: Condvar,
    next: std::sync::atomic::AtomicU64,
    /// Permanent process shutdown also wakes readers waiting for main intake.
    quit: std::sync::atomic::AtomicBool,
    /// reader threads running
    active: std::sync::atomic::AtomicUsize,
    /// a reader holds the one large body (`LARGE_BODY`)
    large: std::sync::atomic::AtomicBool,
}

/// The large-body place of a reader, given back when the reader ends.
struct LargeSlot<'a>(Option<&'a std::sync::atomic::AtomicBool>);

impl<'a> LargeSlot<'a> {
    /// One large body at a time: a second is answered at its head, so the readers hold at most one. None: taken.
    fn take(h: &Head, taken: &'a std::sync::atomic::AtomicBool) -> Option<Self> {
        let large = h.len.is_some_and(|n| n > LARGE_BODY);
        if large && taken.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        Some(Self(large.then_some(taken)))
    }
}

impl Drop for LargeSlot<'_> {
    fn drop(&mut self) {
        if let Some(b) = self.0 {
            b.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl Inbox {
    fn lock(&self) -> MutexGuard<'_, std::collections::VecDeque<Req>> {
        self.q.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A short answer and the end of the connection (every refused or dropped request gets one): at most 1 s to write.
/// The rest of an unread body is read away for up to 1 s, so the client sees the answer and not a reset.
fn answer(s: &TcpStream, status: &str, msg: &str) {
    answer_now(s, status, msg);
    let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
    let end = Instant::now() + Duration::from_secs(1);
    let mut b = [0u8; 16384];
    while Instant::now() < end {
        match std::io::Read::read(&mut &*s, &mut b) {
            Ok(n) if n > 0 => {}
            _ => break,
        }
    }
}

/// `answer` without reading the rest away (the accept thread: it must not wait).
fn answer_now(s: &TcpStream, status: &str, msg: &str) {
    let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = (&*s).write_all(
        format!(
            "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            msg.len(),
            msg
        )
        .as_bytes(),
    );
    let _ = s.shutdown(std::net::Shutdown::Write);
}

/// The parts of a request head the reader needs (names without case).
#[derive(Debug, PartialEq)]
struct Head {
    post: bool,
    route: String,
    /// Content-Length; None without one
    len: Option<usize>,
    chunked: bool,
    expect_continue: bool,
}

fn parse_head(text: &str) -> Head {
    let mut words = text.split_whitespace();
    let post = words.next() == Some("POST");
    let path = words.next().unwrap_or("/");
    let route = path.split_once('?').map_or(path, |x| x.0).to_string();
    let len = header(text, "content-length").and_then(|v| v.parse::<usize>().ok());
    let chunked = header(text, "transfer-encoding").is_some_and(|v| !v.eq_ignore_ascii_case("identity"));
    let expect_continue = header(text, "expect").is_some_and(|v| v.eq_ignore_ascii_case("100-continue"));
    Head { post, route, len, chunked, expect_continue }
}

/// Why a request is answered before its body is read: the status line and text, or None to read it.
fn refuse_early(h: &Head, text: &str, peer_ok: bool) -> Option<(&'static str, &'static str)> {
    let limit = match (h.post, h.route.as_str()) {
        (true, "/update") => MAX_UPDATE,
        (true, "/waveform") => MAX_WAVEFORM,
        _ => MAX_OTHER,
    };
    if h.post && (h.chunked || h.len.is_none()) {
        return Some(("411 Length Required", "a body needs a Content-Length"));
    }
    if h.len.is_some_and(|n| n > limit) {
        return Some(("413 Payload Too Large", "request too large"));
    }
    if foreign(text, &h.route, h.post)
        || (matches!(h.route.as_str(), "/update" | "/mcp") && (h.post || h.route == "/mcp") && !peer_ok)
    {
        return Some(("403 Forbidden", "refused"));
    }
    None
}

/// One connection: the head (`MAX_HEAD`), the early refusals, `100 Continue` when asked for, the body within the
/// limits, then the request to the frame loop; 503 when the frame loop does not take it in `Limits::take`.
fn reader(s: TcpStream, inbox: &Inbox, acl: &Mutex<crate::netacl::Shared>, lim: Limits) {
    use std::io::Read;
    let t0 = Instant::now();
    let _ = s.set_nonblocking(false);
    let _ = s.set_nodelay(true);
    let mut req: Vec<u8> = Vec::new();
    let mut buf = [0u8; 65536];
    // one read, within the idle time and what is left of the deadline; Err: answered or gone
    let read_some = |req: &mut Vec<u8>, buf: &mut [u8]| -> Result<(), ()> {
        let left = lim.deadline.saturating_sub(t0.elapsed());
        if left.is_zero() {
            answer(&s, "408 Request Timeout", "request took too long");
            return Err(());
        }
        let _ = s.set_read_timeout(Some(lim.idle.min(left)));
        match (&s).read(buf) {
            Ok(0) => Err(()),
            Ok(n) => {
                req.extend_from_slice(&buf[..n]);
                Ok(())
            }
            Err(e) if crate::net::later(&e) => {
                answer(&s, "408 Request Timeout", "request timed out");
                Err(())
            }
            Err(_) => Err(()),
        }
    };
    let end = loop {
        if let Some(p) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        if req.len() > MAX_HEAD {
            return answer(&s, "431 Request Header Fields Too Large", "request head too long");
        }
        if read_some(&mut req, &mut buf).is_err() {
            return;
        }
    };
    let text = String::from_utf8_lossy(&req[..end]).into_owned();
    let h = parse_head(&text);
    let peer_ok = acl.lock().unwrap_or_else(|e| e.into_inner()).clone().permits_peer(s.peer_addr());
    if let Some((status, msg)) = refuse_early(&h, &text, peer_ok) {
        return answer(&s, status, msg);
    }
    let Some(_slot) = LargeSlot::take(&h, &inbox.large) else {
        return answer(&s, "503 Service Unavailable", "busy, try again");
    };
    let need = end + h.len.unwrap_or(0);
    // the whole request in one allocation, not a doubling past it
    req.reserve_exact(need.saturating_sub(req.len()));
    if h.expect_continue && req.len() < need {
        let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
        if (&s).write_all(b"HTTP/1.1 100 Continue\r\n\r\n").is_err() {
            return;
        }
    }
    while req.len() < need {
        if read_some(&mut req, &mut buf).is_err() {
            return;
        }
    }
    req.truncate(need);
    let body = req.split_off(end);
    shutdown::queue_request(s, inbox, text, body, lim);
}

/// The accept thread of an open port: a reader thread per connection, at most `MAX_PENDING` (503 beyond). Polls the
/// listener every 20 ms so that `stop` ends it.
fn acceptor(
    l: TcpListener,
    inbox: Arc<Inbox>,
    acl: Arc<Mutex<crate::netacl::Shared>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    lim: Limits,
    tasks: Arc<shutdown::Tasks>,
) {
    use std::sync::atomic::Ordering::SeqCst;
    let _ = l.set_nonblocking(true);
    while !stop.load(SeqCst) {
        match l.accept() {
            Ok((s, _)) => {
                if inbox.active.fetch_add(1, SeqCst) >= MAX_PENDING {
                    inbox.active.fetch_sub(1, SeqCst);
                    let _ = s.set_nonblocking(false);
                    answer_now(&s, "503 Service Unavailable", "busy, try again");
                    continue;
                }
                let (ib, acl) = (inbox.clone(), acl.clone());
                let tracked = s.try_clone().ok();
                let spawned = tasks.spawn("web-read", tracked.as_ref(), move || {
                    reader(s, &ib, &acl, lim);
                    ib.active.fetch_sub(1, SeqCst);
                });
                if spawned.is_err() {
                    inbox.active.fetch_sub(1, SeqCst);
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Style of both pages (the update page uses it too).
pub const STYLE: &str = r#"*{box-sizing:border-box}
body{background:#14171c;color:#e4e7ea;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;margin:0 auto;max-width:1048px;padding:12px}
header{display:flex;align-items:center;gap:14px;margin-bottom:10px}header b{font-size:17px}header nav{margin-left:auto;display:flex;gap:14px}
a{color:#5cb2f0;text-decoration:none}a:hover{text-decoration:underline}
#st{color:#9aa4af;font-size:13px}#st.bad{color:#f0a93b}
button{background:#242a33;color:#e4e7ea;border:1px solid #3a424d;border-radius:6px;padding:7px 11px;font:inherit;cursor:pointer;touch-action:manipulation}
button:hover{background:#2f3743}button:active{background:#3a4452}button.on{border-color:#f0a93b;color:#f0a93b}
button:focus-visible,a:focus-visible,select:focus-visible{outline:2px solid #f0a93b;outline-offset:3px}button:disabled{opacity:.55;cursor:wait}
select{background:#242a33;color:#e4e7ea;border:1px solid #3a424d;border-radius:6px;padding:7px;font:inherit}
pre{background:#0d0f13;border:1px solid #2a3038;border-radius:6px;padding:12px;overflow-x:auto;white-space:pre-wrap}
code{font:13px ui-monospace,Menlo,Consolas,monospace}"#;

const PAGE: &str = r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OOS remote</title>
<link rel="stylesheet" href="/style.css">
<style>img{display:block;width:100%;height:auto;border:1px solid #3a424d;border-radius:6px;cursor:crosshair;touch-action:none;user-select:none;-webkit-user-select:none}
.g{display:flex;flex-wrap:wrap;gap:12px 22px;margin-top:12px}.k span{display:block;font-size:12px;color:#9aa4af;margin-bottom:3px}.k div{display:flex;flex-wrap:wrap;gap:4px}
.workflow{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin:0 0 8px}.workflow label{display:flex;align-items:center;gap:6px;color:#b8c0ca;font-size:13px}.workflow .primary{border-color:#f0a93b;color:#f0a93b}.session{margin-left:auto;color:#b8c0ca;font-size:13px;font-variant-numeric:tabular-nums}.file-hint{min-height:20px;margin:0 0 8px;color:#b8c0ca;font-size:13px}.file-hint.bad{color:#ffab91}header{flex-wrap:wrap}@media(max-width:620px){.session{margin-left:0;flex-basis:100%}.workflow{gap:6px}button{min-height:40px}}</style></head>
<body><header><b>OOS remote</b><span id="st">connecting ...</span><nav><a href="/update">Update</a></nav></header>
<div class="workflow" aria-label="Waveform and source actions">
<button id="load-wave" class="primary" onclick="askFile()">Load waveform</button>
<input id="wave-file" type="file" accept=".csv,.bin,.wfm,.trc,.isf,.tr0,.wfmx" hidden>
<label>Into <select id="wave-slot" aria-label="Reference slot"><option value="" selected>Choose slot</option><option value="1">R1</option><option value="2">R2</option><option value="3">R3</option><option value="4">R4</option><option value="5">R5</option><option value="6">R6</option><option value="7">R7</option><option value="8">R8</option><option value="9">R9</option><option value="10">R10</option></select></label>
<button id="sim-mode" onclick="c('workflow sim')">SIM</button><button id="return-live" onclick="c('workflow live')" hidden>Return to live</button>
<button id="acquire-toggle" onclick="c('key runstop')" aria-label="Run or stop acquisition">Run/Stop</button><button onclick="c('key single')">Single</button>
<button onclick="c('workflow measure')">Add measurement</button><span id="session-state" class="session" aria-live="polite">Reading source...</span></div>
<p id="wave-status" class="file-hint" role="status">Choose a slot, then CSV, BIN, WFM, TRC, ISF, TR0 or WFMX · up to 128 MiB. You can also drop a file on the waveform.</p>
<img id="s" width="1024" height="600" alt="scope screen" src="/screen.png">
<div class="g"><div class="k"><span>Acquisition</span><div>
<button onclick="c('key runstop')">Run/Stop</button><button onclick="c('key single')">Single</button><button onclick="c('key auto')">Autoset</button><button onclick="c('key clear')">Clear</button><button onclick="c('key default')">Default</button></div></div>
<div class="k"><span>Channels</span><div>
<button onclick="c('key ch1')">CH1</button><button onclick="c('key ch2')">CH2</button><button onclick="c('key ch3')">CH3</button><button onclick="c('key ch4')">CH4</button><button onclick="c('key math')">Math</button><button onclick="c('key ref')">Ref</button><button onclick="c('key la')">LA</button><button onclick="c('key gi')">Gen</button></div></div>
<div class="k"><span>Tools</span><div>
<button onclick="c('workflow settings')">Measure settings</button><button onclick="c('key cursor')">Cursor</button><button onclick="c('key analyse')">Decode</button><button onclick="c('key tmenu')">Trigger</button><button onclick="c('key force')">Force trigger</button></div></div></div>
<div class="g">
<div class="k"><span>V scale</span><div><button onclick="c('knob vscale -1')">-</button><button onclick="c('knob vscale +1')">+</button></div></div>
<div class="k"><span>V pos</span><div><button onclick="c('knob vpos -5')">-</button><button onclick="c('knob vpos +5')">+</button></div></div>
<div class="k"><span>H scale</span><div><button onclick="c('knob hscale -1')">-</button><button onclick="c('knob hscale +1')">+</button><button onclick="c('knob hscale push')">zoom</button></div></div>
<div class="k"><span>H pos</span><div><button onclick="c('knob hpos -5')">-</button><button onclick="c('knob hpos +5')">+</button></div></div>
<div class="k"><span>Trig level</span><div><button onclick="c('knob tlevel -5')">-</button><button onclick="c('knob tlevel +5')">+</button><button onclick="c('knob tlevel push')">50%</button></div></div>
<div class="k"><span>Multi 1</span><div><button onclick="c('knob multi1 -1')">-</button><button onclick="c('knob multi1 +1')">+</button><button onclick="c('knob multi1 push')">push</button></div></div>
<div class="k"><span>Multi 2</span><div><button onclick="c('knob multi2 -1')">-</button><button onclick="c('knob multi2 +1')">+</button></div></div></div>
<div class="g"><div class="k"><span>Save to this computer</span><div>
<button onclick="dl('/screen.png','png')">Screenshot (PNG)</button><button onclick="dl('/wave.csv','csv')">Waveform (CSV)</button><button onclick="dl('/wave.bin','bin')">Waveform (binary)</button><button onclick="dl('/la.csv','la.csv')">LA lines (CSV)</button></div></div>
<div class="k"><span>Browser picture only</span><div><button id="p" onclick="pause()">Pause picture</button></div></div>
<div class="k"><span>Live picture format</span><div><button id="fp" onclick="setfmt('png')" title="compressed, about 90 KB per picture">PNG</button><button id="fb" onclick="setfmt('bmp')" title="uncompressed, 1.8 MB per picture">BMP</button><button id="fj" onclick="setfmt('jpg')" title="made by the scope's video encoder, about 70 KB per picture: up to 20 a second, but text a little less sharp">JPEG</button></div></div></div>
<script>
const s=document.getElementById('s'),st=document.getElementById('st'),pb=document.getElementById('p'),fp=document.getElementById('fp'),fb=document.getElementById('fb'),fj=document.getElementById('fj');
let paused=false,running=false,n=0,msg='',until=0,fmt='png';
try{const f=localStorage.getItem('fmt');if(f=='bmp'||f=='jpg')fmt=f;}catch(e){}
function setfmt(f){fmt=f;try{localStorage.setItem('fmt',f);}catch(e){}fp.className=f=='png'?'on':'';fb.className=f=='bmp'?'on':'';fj.className=f=='jpg'?'on':'';}
setfmt(fmt);
function say(t){msg=t;until=Date.now()+5000;show();}
function show(bad){st.textContent=Date.now()<until?msg:(paused?'picture paused':bad?'no connection':'connected, '+n+' pictures/s');st.className=bad?'bad':'';}
function c(x){return fetch('/cmd?c='+encodeURIComponent(x)).then(async r=>{const text=await r.text();if(!r.ok||text==='refused')throw Error(text);refreshState();}).catch(()=>say('Command not sent. Check the connection.'));}
let statePending=false,uploading=false,workflowState=null;
function askFile(){const slot=document.getElementById('wave-slot'),status=document.getElementById('wave-status');if(!slot.value){status.textContent='Choose a slot from R1 to R10 before the file.';status.className='file-hint bad';return;}document.getElementById('wave-file').click();}
function refreshState(){if(statePending||document.hidden)return;statePending=true;fetch('/ui-state').then(r=>{if(!r.ok)throw Error();return r.json();}).then(v=>{
const mode=v.mode==='sim'?'SIM':v.mode==='reference'?'Stored waveform':'Live';
workflowState=v;
document.getElementById('session-state').textContent=[mode,v.source,v.status,(v.captures>0?'Capture '+v.captures:'')].filter(Boolean).join(' · ');
document.getElementById('sim-mode').className=v.mode==='sim'?'on':'';document.getElementById('return-live').hidden=v.mode==='live';
document.getElementById('acquire-toggle').textContent=v.running?'Stop':'Run';
}).catch(()=>{}).finally(()=>{statePending=false;});}
async function uploadWave(file){if(!file||uploading)return;const status=document.getElementById('wave-status'),button=document.getElementById('load-wave'),slot=document.getElementById('wave-slot');
if(!slot.value){status.textContent='Choose a slot from R1 to R10 before the file.';status.className='file-hint bad';document.getElementById('wave-file').value='';return;}
if(file.size>128*1024*1024){status.textContent='That file is larger than 128 MiB. Export a shorter waveform and try again.';status.className='file-hint bad';return;}
uploading=true;button.disabled=true;slot.disabled=true;button.textContent='Loading...';status.className='file-hint';status.textContent='Loading '+file.name+' into R'+slot.value+'...';
try{const response=await fetch('/waveform?name='+encodeURIComponent(file.name)+'&slot='+encodeURIComponent(slot.value),{method:'POST',headers:{'Content-Type':'application/octet-stream'},body:file});const text=await response.text();if(!response.ok)throw Error(text);status.textContent=text;refreshState();}
catch(error){status.textContent='Could not load waveform: '+(error.message||'check the connection and try again.');status.className='file-hint bad';}
finally{uploading=false;button.disabled=false;slot.disabled=false;button.textContent='Load waveform';document.getElementById('wave-file').value='';}}
document.getElementById('wave-file').addEventListener('change',event=>uploadWave(event.target.files[0]));
s.addEventListener('dragover',event=>{if(Array.from(event.dataTransfer?.types||[]).includes('Files'))event.preventDefault();});
s.addEventListener('drop',event=>{event.preventDefault();uploadWave(event.dataTransfer.files[0]);});
document.addEventListener('keydown',event=>{if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='o'){event.preventDefault();askFile();}});
refreshState();
function dl(u,ext){say('saving ...');Promise.all([fetch(u).then(r=>r.ok?r.blob():r.text().then(t=>Promise.reject(t))),fetch('/stamp').then(r=>r.ok?r.text():'').catch(()=>'')]).then(([b,t])=>{
const a=document.createElement('a'),d=t.trim()||new Date(Date.now()-new Date().getTimezoneOffset()*60000).toISOString().slice(0,19).replace(/-|:/g,'').replace('T','-');
a.href=URL.createObjectURL(b);a.download='oos-'+d+'.'+ext;document.body.appendChild(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(a.href),10000);say('saved');
}).catch(e=>say(typeof e=='string'?e:'not saved: no connection'));}
function pause(){paused=!paused;pb.textContent=paused?'Resume picture':'Pause picture';pb.className=paused?'on':'';show();go();}
function pt(e){const r=s.getBoundingClientRect();const p=e.changedTouches?e.changedTouches[0]:e;return [Math.round((p.clientX-r.left)*(s.naturalWidth||1024)/r.width),Math.round((p.clientY-r.top)*(s.naturalHeight||600)/r.height)];}
let d0=null;
function down(e){e.preventDefault();d0=pt(e);}
function up(e){if(!d0)return;e.preventDefault();const p=pt(e),start=d0;d0=null;if(Math.abs(p[0]-start[0])+Math.abs(p[1]-start[1])<8){c('tap '+start[0]+' '+start[1]);}else c('drag '+start[0]+' '+start[1]+' '+p[0]+' '+p[1]);}
s.addEventListener('mousedown',down);s.addEventListener('mouseup',up);s.addEventListener('touchstart',down,{passive:false});s.addEventListener('touchend',up,{passive:false});
s.ondragstart=()=>false;
let count=0;setInterval(()=>{n=count;count=0;if(!document.hidden){refreshState();if(!paused)show(st.className=='bad');}},1000);
// no pictures while paused or in a background tab: each one is 90 KB as PNG, 1.8 MB as BMP, which the scope has to send
// a PNG comes with an ETag: sent back, the scope holds the request until its screen has changed (304 after 2 s), so
// the next request can go at once; a BMP has none, so a pause between them
let tag='',shown='';
function next(ok){running=false;show(!ok);setTimeout(go,!ok?1000:fmt!='bmp'?0:150);}
function go(){if(running||paused||document.hidden)return;running=true;const f=fmt;
fetch('/screen.'+f+'?t='+Date.now(),{cache:'no-store',headers:tag&&f!='bmp'?{'If-None-Match':tag}:{}}).then(r=>{
if(r.status==304)return next(true);
if(!r.ok)return next(false);
const t=f!='bmp'&&r.headers.get('ETag')||'';
return r.blob().then(b=>{const u=URL.createObjectURL(b),i=new Image();
i.onload=()=>{s.src=u;if(shown)URL.revokeObjectURL(shown);shown=u;tag=t;count++;next(true);};
i.onerror=()=>{URL.revokeObjectURL(u);next(false);};i.src=u;});
}).catch(()=>next(false));}
document.addEventListener('visibilitychange',go);go();
</script></body></html>"#;

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                // on the bytes: the two characters after % need not be ASCII
                let hex = |c: u8| (c as char).to_digit(16);
                if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                    out.push((h * 16 + l) as u8);
                }
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A browser supplies only a display name. It never selects a path on the instrument.
fn waveform_name(query: &str) -> String {
    let decoded = query.split('&').find_map(|part| part.strip_prefix("name=")).map(unescape).unwrap_or_default();
    let leaf = decoded.rsplit(['/', '\\']).next().unwrap_or("");
    let name: String = leaf.chars().filter(|c| !c.is_control()).take(128).collect();
    if name.trim().is_empty() {
        "waveform.csv".into()
    } else {
        name
    }
}

impl Http {
    /// The web remote on `port`, with the switches' IPv6 and little cores.
    pub fn open(port: impl Into<crate::net6::Port>, sw: &crate::sysconf::Switches) -> Http {
        Self::open_with(port, LIMITS, sw)
    }

    pub fn port(&self) -> Option<u16> {
        self.open.as_ref().map(|(addr, _)| addr.port())
    }

    /// The address list to check (`Ui::acl`, shared with the other servers), for the frame loop and the readers.
    pub fn set_acl(&mut self, acl: crate::netacl::Shared) {
        *self.acl_shared.lock().unwrap_or_else(|e| e.into_inner()) = acl.clone();
        self.acl = acl;
    }

    fn open_with(port: impl Into<crate::net6::Port>, limits: Limits, sw: &crate::sysconf::Switches) -> Http {
        let acl = crate::netacl::Shared::default();
        let mut h = Http {
            open: None,
            inbox: Arc::new(Inbox::default()),
            limits,
            shots: Shots::new(sw.cores.little),
            acl_shared: Arc::new(Mutex::new(acl.clone())),
            acl,
            offer: Offer::default(),
        };
        h.start(port, sw.ipv6);
        h
    }

    fn start(&mut self, port: impl Into<crate::net6::Port>, v6: bool) {
        let Some(l) = crate::net6::tcp(port, v6) else { return };
        let Ok(addr) = l.local_addr() else { return };
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (inbox, acl, st, lim) = (self.inbox.clone(), self.acl_shared.clone(), stop.clone(), self.limits);
        let tasks = self.shots.tasks.clone();
        if self.shots.tasks.spawn("web-accept", None, move || acceptor(l, inbox, acl, st, lim, tasks)).is_err() {
            eprintln!("oos-ui: web remote: no accept thread");
            return;
        }
        eprintln!("oos-ui: web remote on {}", addr);
        self.open = Some((addr, stop));
    }

    /// The address the port is open on (tests).
    #[cfg(test)]
    fn addr(&self) -> std::net::SocketAddr {
        self.open.as_ref().expect("web port open").0
    }

    /// The addresses the identification page names. Set from the listeners that actually came up.
    pub fn offer(&mut self, o: Offer) {
        self.offer = o;
    }

    /// System > Network > Remote services: the port open (`Some`), or closed with the requests waiting on it.
    pub fn set_open(&mut self, port: Option<u16>, v6: bool) {
        match port {
            Some(p) if self.open.is_none() => self.start(p, v6),
            Some(_) => {}
            None => {
                if let Some((_, stop)) = self.open.take() {
                    stop.store(true, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("oos-ui: web remote off");
                }
                // the readers waiting for the frame loop answer 503 when they give up
                self.inbox.lock().clear();
                self.inbox.taken.notify_all();
            }
        }
    }

    /// Call right before rendering the next screen: hands the shown one to the live picture (`Shots::hand_over`).
    /// `whole`: the render draws every pixel.
    pub fn before_render(&mut self, cv: &mut Canvas, whole: bool) {
        self.shots.hand_over(cv, whole);
    }

    /// System > Graphics = GPU: the frame loop hands over the pictures that come back from the GPU
    /// (`picture_read_back`), `tick` copies none and nothing is swapped. `cv` holds the last picture read back, not
    /// one drawn every frame: a buffer swapped out of it would stay in it, an older picture, until the next read.
    pub fn before_gpu_frame(&mut self) {
        self.shots.lock().frames = true;
    }

    /// System > Graphics = GPU: `cv` was just read back whole; a copy goes to the encoder when it wants one.
    pub fn picture_read_back(&self, cv: &Canvas) {
        let mut st = self.shots.lock();
        let watched = st.asked.is_some_and(|t| t.elapsed() < SHOT_AHEAD);
        let now = Instant::now();
        if Shots::wanted(&st, now, watched) {
            Shots::give_copy(&mut st, cv, now);
            self.shots.wake.notify_one();
        }
    }

    /// The live picture would take the next screen (System > Graphics = GPU: read it back for it).
    pub fn picture_wanted(&self) -> bool {
        let st = self.shots.lock();
        let watched = st.asked.is_some_and(|t| t.elapsed() < SHOT_AHEAD);
        Shots::wanted(&st, Instant::now(), watched)
    }

    /// Accepts connections, answers complete requests; text commands are returned for the caller to run.
    pub fn poll(&mut self, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi) -> Vec<String> {
        let mut cmds = Vec::new();
        self.shots.tick(cv);
        let reqs: Vec<Req> = self.inbox.lock().drain(..).collect();
        if !reqs.is_empty() {
            self.inbox.taken.notify_all();
        }
        for Req { s, head: text, body, .. } in reqs {
            let s = &s;
            {
                // only the header as text: the body may be a 31 MB binary package
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (route, query) = path.split_once('?').unwrap_or((&path, ""));
                let mut req_body = body;
                let post = text.starts_with("POST ");
                // what changes the scope beyond keys and knobs: only from the allowed prefixes (also at the head)
                let peer_ok = self.acl.permits_peer(s.peer_addr());
                // an answer that is text for the page to show, not the data asked for
                let mut failed = false;
                let mut csv = |r: Result<String, String>| match r {
                    Ok(t) => ("text/csv; charset=utf-8", t.into_bytes()),
                    Err(e) => {
                        failed = true;
                        ("text/plain", e.into_bytes())
                    }
                };
                let (ctype, body): (&str, Vec<u8>) = match route {
                    _ if foreign(&text, route, post) => ("text/plain", b"refused".to_vec()),
                    "/update" | "/mcp" if (post || route == "/mcp") && !peer_ok => ("text/plain", b"refused".to_vec()),
                    "/" => ("text/html; charset=utf-8", PAGE.as_bytes().to_vec()),
                    "/style.css" => ("text/css", STYLE.as_bytes().to_vec()),
                    "/ui-state" if !post => ("application/json", ui.workflow_state().into_bytes()),
                    "/waveform" if post => {
                        let slot = query
                            .split('&')
                            .find_map(|part| part.strip_prefix("slot="))
                            .and_then(|value| value.parse::<usize>().ok());
                        let name = waveform_name(query);
                        let previous = ui.ref_slot();
                        let result = if let Some(slot) = slot.filter(|s| (1..=10).contains(s)) {
                            ui.load_waveform_bytes_at(&req_body, &name, slot - 1)
                        } else {
                            Err("Choose a reference slot from R1 to R10.".into())
                        };
                        match result {
                            Ok(()) => {
                                let hint = if ui.record_mode() {
                                    "Use Run/Stop or Single to acquire from this record; Return to live restores the live source."
                                } else {
                                    "It is on with the live channels. Fit shows its full record; Take over enters stored acquisition."
                                };
                                (
                                    "text/plain; charset=utf-8",
                                    format!("Loaded {} into R{}. {}", name, ui.ref_slot() + 1, hint).into_bytes(),
                                )
                            }
                            Err(error) => {
                                ui.ref_slot_set(previous);
                                failed = true;
                                ("text/plain; charset=utf-8", error.into_bytes())
                            }
                        }
                    }
                    "/screen.png" | "/screen.jpg" => {
                        let have = header(&text, "if-none-match").map(str::to_string);
                        self.shots.request(s, have, cv, route == "/screen.jpg");
                        continue;
                    }
                    // the scope's local time for the names of the page's downloads; empty while its clock is not set
                    "/stamp" => ("text/plain", ui.file_stamp().unwrap_or_default().into_bytes()),
                    "/wave.csv" => csv(ui.web_wave_csv()),
                    "/la.csv" => csv(ui.web_la_csv()),
                    "/wave.bin" => match ui.web_wave_bin() {
                        Ok(b) => ("application/octet-stream", b),
                        Err(e) => {
                            failed = true;
                            ("text/plain", e.into_bytes())
                        }
                    },
                    "/update" if post => {
                        let (refused, kind, body) = ui.update_upload_reply(std::mem::take(&mut req_body), query);
                        failed = refused;
                        (kind, body)
                    }
                    "/update" => ("text/html; charset=utf-8", ui.update_page().into_bytes()),
                    "/lxi/identification" => ("text/xml", lxi_identification(&self.offer).into_bytes()),
                    "/mcp" => match mcp(&String::from_utf8_lossy(&req_body), ui, cv, scpi) {
                        Some(r) => ("application/json", r.into_bytes()),
                        None => ("", Vec::new()),
                    },
                    "/screen.bmp" => ("image/bmp", crate::scpi::bmp(cv)),
                    "/cmd" => {
                        let mut refused = false;
                        for kv in query.split('&') {
                            if let Some(v) = kv.strip_prefix("c=") {
                                let c = unescape(v);
                                if crate::cmd::remote_ok(&c) {
                                    cmds.push(c);
                                } else {
                                    refused = true;
                                }
                            }
                        }
                        ("text/plain", if refused { b"refused".to_vec() } else { b"ok".to_vec() })
                    }
                    _ => ("text/plain", b"not found".to_vec()),
                };
                let status = match (failed, body.as_slice(), ctype.is_empty()) {
                    (true, _, _) => "409 Conflict",
                    (_, b"not found", _) => "404 Not Found",
                    (_, b"refused", _) => "403 Forbidden",
                    (_, _, true) => "202 Accepted",
                    _ => "200 OK",
                };
                let head = format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", status, ctype, body.len());
                if body.len() > SEND_INLINE {
                    if !send_in_background(s, head, body, self.shots.little, &self.shots.tasks) {
                        busy_answer(s);
                    }
                } else {
                    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                    let _ = (&*s).write_all(head.as_bytes());
                    let _ = (&*s).write_all(&body);
                }
            }
        }
        cmds
    }
}

/// The value of a header of the request head (name without case).
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

fn lxi_identification(o: &Offer) -> String {
    let mut addrs = String::new();
    if o.vxi11 {
        addrs.push_str("<InstrumentAddressString>TCPIP::INSTR</InstrumentAddressString>\n");
    }
    if let Some(p) = o.scpi_port {
        addrs.push_str(&format!("<InstrumentAddressString>TCPIP::{p}::SOCKET</InstrumentAddressString>\n"));
    }
    if let Some(port) = o.hislip_port {
        let device = if port == crate::hislip::PORT { "hislip0".into() } else { format!("hislip0,{port}") };
        addrs.push_str(&format!("<InstrumentAddressString>TCPIP::{device}::INSTR</InstrumentAddressString>\n"));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<LXIDevice xmlns="http://www.lxistandard.org/InstrumentIdentification/1.0">
<Manufacturer>OOS</Manufacturer><Model>DHO924S</Model><SerialNumber>0</SerialNumber>
<FirmwareRevision>oos-ui {}</FirmwareRevision><ManufacturerDescription>OOS oscilloscope</ManufacturerDescription>
<HomepageURL>/</HomepageURL><DriverURLs/>
<UserDescription>OOS</UserDescription>
<IdentificationURL>/lxi/identification</IdentificationURL>
<Interface InterfaceType="LXI" IPType="IPv4" xsi:type="NetworkInformation" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
{addrs}</Interface>
<LXIVersion>1.5</LXIVersion>
</LXIDevice>
"#,
        env!("CARGO_PKG_VERSION")
    )
}

pub fn base64(d: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(d.len().div_ceil(3) * 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            o.push(if k <= c.len() { T[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    o
}

/// One MCP JSON-RPC message; None for notifications (answered with 202 and no body).
pub fn mcp(body: &str, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi) -> Option<String> {
    use crate::json::{parse, J};
    let err = |id: J, code: f64, msg: &str| {
        J::obj(&[
            ("jsonrpc", J::s("2.0")),
            ("id", id),
            ("error", J::obj(&[("code", J::Num(code)), ("message", J::s(msg))])),
        ])
        .to_string()
    };
    let Some(req) = parse(body) else { return Some(err(J::Null, -32700.0, "parse error")) };
    let id = req.get("id").cloned()?;
    let method = req.get("method").and_then(J::str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(J::Null);
    let text = |s: &str| J::obj(&[("content", J::Arr(vec![J::obj(&[("type", J::s("text")), ("text", J::s(s))])]))]);
    let result = match method {
        "initialize" => {
            let ver = params.get("protocolVersion").and_then(J::str).unwrap_or("2025-06-18").to_string();
            J::obj(&[
                ("protocolVersion", J::Str(ver)),
                ("capabilities", J::obj(&[("tools", J::obj(&[]))])),
                ("serverInfo", J::obj(&[("name", J::s("oos-ui")), ("version", J::s(env!("CARGO_PKG_VERSION")))])),
                ("instructions", J::s("Rigol DHO924S running OOS. Use the scpi tool with Rigol-style commands (e.g. :TIM:SCAL 1e-3, :CHAN1:SCAL 0.5, :MEAS:ITEM? VPP,CHAN1, :WAV:DATA?) and screenshot to see the screen.")),
            ])
        }
        "ping" => J::obj(&[]),
        "tools/list" => {
            let schema = |props: J, req: Vec<J>| {
                J::obj(&[("type", J::s("object")), ("properties", props), ("required", J::Arr(req))])
            };
            J::obj(&[(
                "tools",
                J::Arr(vec![
                    J::obj(&[
                        ("name", J::s("scpi")),
                        ("description", J::s("Run one SCPI program message on the oscilloscope (commands separated by ';'); returns the response of the queries.")),
                        ("inputSchema", schema(J::obj(&[("command", J::obj(&[("type", J::s("string"))]))]), vec![J::s("command")])),
                    ]),
                    J::obj(&[
                        ("name", J::s("screenshot")),
                        ("description", J::s("The current oscilloscope screen as a PNG image.")),
                        ("inputSchema", schema(J::obj(&[]), vec![])),
                    ]),
                ]),
            )])
        }
        "tools/call" => match params.get("name").and_then(J::str) {
            Some("scpi") => {
                let c = params.get("arguments").and_then(|a| a.get("command")).and_then(J::str).unwrap_or("");
                match scpi.exec_message_checked(ui, cv, c) {
                    Ok((r, has_reply)) => text(if r.is_empty() && !has_reply {
                        "ok"
                    } else {
                        std::str::from_utf8(&r).unwrap_or("(binary response)")
                    }),
                    Err(error) => J::obj(&[
                        ("isError", J::Bool(true)),
                        ("content", J::Arr(vec![J::obj(&[("type", J::s("text")), ("text", J::s(error))])])),
                    ]),
                }
            }
            Some("screenshot") => J::obj(&[(
                "content",
                J::Arr(vec![J::obj(&[
                    ("type", J::s("image")),
                    ("mimeType", J::s("image/png")),
                    ("data", J::Str(base64(&crate::store::png_of(cv)))),
                ])]),
            )]),
            _ => return Some(err(id, -32602.0, "unknown tool")),
        },
        _ => return Some(err(id, -32601.0, "method not found")),
    };
    Some(J::obj(&[("jsonrpc", J::s("2.0")), ("id", id), ("result", result)]).to_string())
}

/// A request a web page on another site made through the browser of the bench PC, not the page of the scope or a tool:
/// it may press keys or install a package without the owner's doing. The Host names the scope by address or local
/// name (a foreign name that resolves here is DNS rebinding), an Origin matches the Host, and the browser's own
/// Sec-Fetch-Site says the rest, also for the GET of an image tag, which has no Origin. A link to the page itself may
/// come from anywhere. curl and the like send neither header.
fn foreign(head: &str, route: &str, post: bool) -> bool {
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_ascii_lowercase())
        })
    };
    let name_of = |h: &str| {
        h.rsplit_once(':')
            .filter(|(n, p)| !n.ends_with(':') && p.bytes().all(|b| b.is_ascii_digit()))
            .map_or(h.to_string(), |(n, _)| n.to_string())
    };
    let host = header("host");
    if let Some(h) = &host {
        let n = name_of(h);
        let n = n.trim_matches(['[', ']']);
        let local =
            n.parse::<std::net::IpAddr>().is_ok() || n == "localhost" || !n.contains('.') || n.ends_with(".local");
        if !local {
            return true;
        }
    }
    if let Some(o) = header("origin") {
        let o = o.split_once("://").map_or(o.as_str(), |x| x.1).to_string();
        if host.as_deref() != Some(o.as_str()) {
            return true;
        }
    }
    let page = !post && matches!(route, "/" | "/style.css" | "/update" | "/lxi/identification");
    !page && matches!(header("sec-fetch-site").as_deref(), Some("cross-site" | "same-site"))
}

#[cfg(test)]
mod tests {
    mod empty_tests;
    use super::*;
    use std::io::Read;

    #[test]
    fn a_foreign_site_gets_nowhere() {
        let f = |h: &str, route: &str, post: bool| {
            foreign(&format!("GET {} HTTP/1.1\r\n{}\r\n\r\n", route, h), route, post)
        };
        // tools and the page of the scope itself
        assert!(!f("Host: 192.0.2.64:8080", "/cmd", false));
        assert!(!f("Host: x", "/cmd", false));
        assert!(!f("Host: dho924.local:8080", "/cmd", false));
        assert!(!f("Host: [fe80::1]:8080", "/cmd", false));
        // the scope by its global IPv6 address (net.ipv6 = on), with and without the port, and the page's own requests
        assert!(!f("Host: [2001:db8:5:7:1d2e:3f4a:5b6c:7d8e]:8080", "/cmd", false));
        assert!(!f("Host: [2001:db8:5:7::7d8e]", "/cmd", false));
        assert!(!f("Host: [2001:DB8:5:7::7D8E]:8080\r\nOrigin: http://[2001:db8:5:7::7d8e]:8080\r\nSec-Fetch-Site: same-origin", "/update", true));
        assert!(f("Host: [2001:db8:5:7::7d8e]:8080\r\nOrigin: http://[2001:db8:5:7::1]:8080", "/update", true));
        assert!(f("Host: [2001:db8::1].evil.example:8080", "/cmd", false));
        assert!(!f("", "/cmd", false));
        assert!(!f(
            "Host: 192.0.2.64:8080\r\nOrigin: http://192.0.2.64:8080\r\nSec-Fetch-Site: same-origin",
            "/update",
            true
        ));
        // a page elsewhere
        assert!(f("Host: 192.0.2.64:8080\r\nOrigin: https://evil.example", "/update", true));
        assert!(f("Host: 192.0.2.64:8080\r\nOrigin: null", "/mcp", true));
        assert!(f("Host: 192.0.2.64:8080\r\nSec-Fetch-Site: cross-site", "/cmd", false));
        assert!(f("Host: 192.0.2.64:8080\r\nSec-Fetch-Site: cross-site", "/screen.png", false));
        // rebinding: a foreign name that resolves to the scope
        assert!(f("Host: evil.example:8080", "/wave.csv", false));
        // a link to the page
        assert!(!f("Host: 192.0.2.64:8080\r\nSec-Fetch-Site: cross-site", "/", false));
        assert!(f("Host: 192.0.2.64:8080\r\nSec-Fetch-Site: cross-site", "/update", true));
        assert!(f("Host: 192.0.2.64:8080\r\nOrigin: https://evil.example", "/waveform", true));
        assert!(f("Host: 192.0.2.64:8080\r\nSec-Fetch-Site: cross-site", "/ui-state", false));
    }

    #[test]
    fn waveform_names_are_display_names() {
        assert_eq!(waveform_name("name=C%3A%5Ccaptures%5Cclock.wfm"), "clock.wfm");
        assert_eq!(waveform_name("name=..%2F..%2Fclock%0A.csv"), "clock.csv");
        assert_eq!(waveform_name("slot=2"), "waveform.csv");
    }

    /// One request over a real socket; the server is polled until the answer is complete.
    fn request(h: &mut Http, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi, req: Vec<u8>) -> String {
        String::from_utf8_lossy(&request_raw(h, ui, cv, scpi, req)).into_owned()
    }

    fn request_raw(h: &mut Http, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi, req: Vec<u8>) -> Vec<u8> {
        let addr = h.addr();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", addr.port())).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
            let _ = c.write_all(&req);
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        while !t.is_finished() {
            h.poll(ui, cv, scpi);
            std::thread::sleep(Duration::from_millis(1));
        }
        t.join().unwrap()
    }

    /// Sends `req` to `port` (in pieces of `pieces`, `gap` apart) and reads the answer until the server closes.
    fn talk(port: u16, req: &[u8], pieces: usize, gap: Duration) -> String {
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let n = req.len().div_ceil(pieces.max(1)).max(1);
        for p in req.chunks(n) {
            if c.write_all(p).is_err() {
                break;
            }
            std::thread::sleep(gap);
        }
        let mut out = Vec::new();
        let _ = c.read_to_end(&mut out);
        String::from_utf8_lossy(&out).into_owned()
    }

    fn quick(idle_ms: u64, deadline_ms: u64, take_ms: u64) -> Http {
        let ms = Duration::from_millis;
        Http::open_with(
            0,
            Limits { idle: ms(idle_ms), deadline: ms(deadline_ms), take: ms(take_ms) },
            &Default::default(),
        )
    }

    // Break it catches: a client held for ever when the frame loop does not run (the 55 min upload of 2026-11-14 is
    // the case this is for): the reader answers 503 once `take` passed.
    #[test]
    fn a_request_the_frame_loop_does_not_take_gets_503() {
        let h = quick(10_000, 120_000, 300);
        let r = talk(h.addr().port(), b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", 1, Duration::ZERO);
        assert!(r.starts_with("HTTP/1.1 503"), "{}", r);
        assert!(h.inbox.lock().is_empty(), "the request left the inbox");
    }

    // Break it catches: no answer to `Expect: 100-continue` (curl waits a second, other clients longer), the 100 sent
    // for a request that is refused anyway.
    #[test]
    fn expect_continue_is_answered_and_refusals_come_first() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        let body = b"OOS-UPDATE 1\ntarget x\n\nxx".to_vec();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
            c.write_all(
                format!(
                    "POST /update HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nExpect: 100-continue\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .unwrap();
            let mut first = [0u8; 25];
            c.read_exact(&mut first).unwrap();
            c.write_all(&body).unwrap();
            let mut rest = Vec::new();
            let _ = c.read_to_end(&mut rest);
            (String::from_utf8_lossy(&first).into_owned(), String::from_utf8_lossy(&rest).into_owned())
        });
        while !t.is_finished() {
            h.poll(&mut ui, &cv, &mut scpi);
            std::thread::sleep(Duration::from_millis(1));
        }
        let (first, rest) = t.join().unwrap();
        assert_eq!(first, "HTTP/1.1 100 Continue\r\n\r\n");
        assert!(rest.starts_with("HTTP/1.1 409") && rest.contains("REFUSED"), "{}", rest);
        // too large or from a refused address: the answer at once, no 100, no body read
        let r = talk(
            port,
            format!(
                "POST /update HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nExpect: 100-continue\r\n\r\n",
                MAX_UPDATE + 1
            )
            .as_bytes(),
            1,
            Duration::ZERO,
        );
        assert!(r.starts_with("HTTP/1.1 413"), "{}", r);
        // (the address list always lets the scope's own loopback in, so a page elsewhere stands in for a refusal)
        let r = talk(port, b"POST /update HTTP/1.1\r\nHost: x\r\nOrigin: https://evil.example\r\nContent-Length: 2\r\nExpect: 100-continue\r\n\r\n", 1, Duration::ZERO);
        assert!(r.starts_with("HTTP/1.1 403"), "{}", r);
    }

    // Break it catches: an update or MCP request from an address outside `net.allow` read in full before it is
    // refused (31 MB for nothing), the length limit of other requests applied to /update.
    #[test]
    fn early_refusals_from_the_head() {
        let t = |r: &str| format!("{}\r\nHost: x\r\n\r\n", r);
        let head = |r: &str, len: Option<usize>| Head {
            post: r.starts_with("POST"),
            route: r.split_whitespace().nth(1).unwrap().into(),
            len,
            chunked: false,
            expect_continue: true,
        };
        let h = head("POST /update HTTP/1.1", Some(31 << 20));
        assert_eq!(refuse_early(&h, &t("POST /update HTTP/1.1"), true), None);
        assert_eq!(refuse_early(&h, &t("POST /update HTTP/1.1"), false).map(|x| x.0), Some("403 Forbidden"));
        assert_eq!(
            refuse_early(&head("POST /mcp HTTP/1.1", Some(10)), &t("POST /mcp HTTP/1.1"), false).map(|x| x.0),
            Some("403 Forbidden")
        );
        assert_eq!(
            refuse_early(&head("POST /mcp HTTP/1.1", Some(31 << 20)), &t("POST /mcp HTTP/1.1"), true).map(|x| x.0),
            Some("413 Payload Too Large")
        );
        assert_eq!(refuse_early(&head("GET /screen.png HTTP/1.1", None), &t("GET /screen.png HTTP/1.1"), false), None);
        let p = parse_head("POST /update?x=1 HTTP/1.1\r\ncontent-LENGTH: 12\r\nEXPECT: 100-Continue\r\n\r\n");
        assert_eq!(
            p,
            Head { post: true, route: "/update".into(), len: Some(12), chunked: false, expect_continue: true }
        );
    }

    // Break it catches: a POST without a length read as an empty body (a chunked upload was cut to nothing).
    #[test]
    fn a_body_without_content_length_gets_411() {
        let h = Http::open(0, &Default::default());
        for req in [
            "POST /update HTTP/1.1\r\nHost: x\r\n\r\n",
            "POST /update HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n",
        ] {
            let r = talk(h.addr().port(), req.as_bytes(), 1, Duration::ZERO);
            assert!(r.starts_with("HTTP/1.1 411"), "{:?}: {}", req, r);
        }
    }

    // Break it catches: a silent close where the limits end a request (the client sees no reason), a slow trickle that
    // keeps a connection longer than the deadline, a head without an end read for ever.
    #[test]
    fn every_limit_answers() {
        // idle: half a head, then nothing
        let h = quick(300, 120_000, 10_000);
        let r = talk(h.addr().port(), b"GET / HTTP/1.1\r\nHo", 1, Duration::ZERO);
        assert!(r.starts_with("HTTP/1.1 408"), "idle: {}", r);
        // deadline: a byte every 100 ms keeps the idle time from running out
        let h = quick(10_000, 600, 10_000);
        let head = b"GET / HTTP/1.1\r\nHost: x\r\nX-Slow: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n\r\n";
        let r = talk(h.addr().port(), head, head.len(), Duration::from_millis(100));
        assert!(r.starts_with("HTTP/1.1 408"), "deadline: {}", r);
        // head too long
        let h = Http::open(0, &Default::default());
        let r = talk(
            h.addr().port(),
            format!("GET / HTTP/1.1\r\nX: {}", "a".repeat(MAX_HEAD + 100)).as_bytes(),
            1,
            Duration::ZERO,
        );
        assert!(r.starts_with("HTTP/1.1 431"), "head: {}", r);
    }

    // Break it catches: more connections than `MAX_PENDING` closed without a word.
    #[test]
    fn too_many_connections_get_503() {
        let h = Http::open(0, &Default::default());
        let port = h.addr().port();
        let held: Vec<TcpStream> = (0..MAX_PENDING)
            .map(|_| {
                let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
                c.write_all(b"GET / HTTP/1.1\r\n").unwrap();
                c
            })
            .collect();
        let until = Instant::now() + Duration::from_secs(10);
        while h.inbox.active.load(std::sync::atomic::Ordering::SeqCst) < MAX_PENDING && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let r = talk(port, b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", 1, Duration::ZERO);
        assert!(r.starts_with("HTTP/1.1 503"), "{}", r);
        drop(held);
    }

    /// The web remote with the demo signals, to try it without a scope: `cargo test web_demo -- --ignored --nocapture`,
    /// then open http://localhost:8080/. Runs for OOS_WEB_DEMO_S seconds (default 300), on OOS_WEB_DEMO_PORT.
    #[test]
    #[ignore = "manual web server: set OOS_WEB_DEMO_S"]
    fn web_demo() {
        let _: u64 =
            std::env::var("OOS_WEB_DEMO_S").expect("set OOS_WEB_DEMO_S").parse().expect("OOS_WEB_DEMO_S seconds");
        let var = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let mut h = Http::open(var("OOS_WEB_DEMO_PORT", 8080) as u16, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let end = Instant::now() + Duration::from_secs(var("OOS_WEB_DEMO_S", 300));
        while Instant::now() < end {
            ui.step(0.04);
            ui.render(&mut cv);
            for c in h.poll(&mut ui, &cv, &mut scpi) {
                println!("web: {}", c);
                crate::cmd::run(&mut ui, &c);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    /// An explicitly started, private backend for tools/scpi_validate.py.
    /// It opens no desktop, hardware device or LAN discovery announcer.
    #[test]
    #[ignore = "Owned headless backend for explicit local SCPI acceptance"]
    fn scpi_validation_fixture() {
        let var = |name: &str, default: u64| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
        let mut h = Http::open(var("OOS_VALIDATION_WEB_PORT", 18881) as u16, &Default::default());
        let mut scpi = crate::scpi::Scpi::open(var("OOS_VALIDATION_SCPI_PORT", 15556) as u16, false);
        assert!(h.port().is_some() && scpi.port().is_some(), "private acceptance ports must bind");
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        ui.set_offline(true);
        ui.store_dir = std::env::var_os("OOS_VALIDATION_STORE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("oos-scpi-validation-{}", std::process::id())));
        let mut cv = Canvas::new(crate::ui::W, crate::ui::H);
        let end = std::time::Instant::now() + Duration::from_secs(var("OOS_VALIDATION_SECONDS", 90));
        while !ui.quit && std::time::Instant::now() < end {
            ui.step(0.04);
            ui.render(&mut cv);
            scpi.poll(&mut ui, &cv);
            for command in h.poll(&mut ui, &cv, &mut scpi) {
                crate::cmd::run(&mut ui, &command);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn get(h: &mut Http, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi, path: &str) -> String {
        request(h, ui, cv, scpi, format!("GET {} HTTP/1.1\r\nHost: x\r\n\r\n", path).into_bytes())
    }

    #[test]
    fn waveform_upload_loads_the_chosen_reference_and_preserves_it_on_error() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let store = std::env::temp_dir().join(format!("oos-http-no-write-{}", std::process::id()));
        ui.store_dir = store.clone();
        let cv = Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let post = |path: &str, body: &[u8]| {
            let mut data = format!(
                "POST {path} HTTP/1.1\r\nHost: x\r\nOrigin: http://x\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .into_bytes();
            data.extend_from_slice(body);
            data
        };
        let mut csv = String::from("Time (s),Volts\n");
        for k in 0..1000 {
            csv.push_str(&format!("{},{}\n", k as f64 * 1e-9, (k as f64 * std::f64::consts::TAU / 20.0).sin()));
        }
        let reply = request(&mut h, &mut ui, &cv, &mut scpi, post("/waveform?name=clock.csv&slot=3", csv.as_bytes()));
        assert!(reply.starts_with("HTTP/1.1 200") && reply.contains("Loaded clock.csv into R3"), "{reply}");
        assert!(!ui.record_mode(), "loading into a live session adds a reference overlay");
        assert_eq!(ui.ref_slot(), 2);
        let reply = request(&mut h, &mut ui, &cv, &mut scpi, post("/waveform?name=no-slot.csv", csv.as_bytes()));
        assert!(reply.starts_with("HTTP/1.1 409") && reply.contains("R1 to R10"), "{reply}");
        assert_eq!(ui.ref_slot(), 2, "an omitted destination must not replace R1");
        assert_eq!(ui.ref_shown(), Some(true));
        assert!(!store.exists(), "an upload is read into memory, not written to a server-selected path");
        let reply =
            request(&mut h, &mut ui, &cv, &mut scpi, post("/waveform?name=broken.csv&slot=4", b"not a waveform"));
        assert!(reply.starts_with("HTTP/1.1 409"), "{reply}");
        assert_eq!(ui.ref_slot(), 2, "failed upload keeps the previous selection");
        assert_eq!(ui.ref_shown(), Some(true));
        let mut malformed = Vec::from(&b"OOSW"[..]);
        malformed.extend_from_slice(&1u32.to_le_bytes());
        malformed.extend_from_slice(&1u32.to_le_bytes());
        malformed.extend_from_slice(&u64::MAX.to_le_bytes());
        malformed.extend_from_slice(&1e-9f64.to_le_bytes());
        malformed.extend_from_slice(&0f64.to_le_bytes());
        let reply = request(&mut h, &mut ui, &cv, &mut scpi, post("/waveform?name=broken.bin&slot=4", &malformed));
        assert!(reply.starts_with("HTTP/1.1 409") && reply.contains("32-million-sample import limit"), "{reply}");
        assert_eq!(ui.ref_slot(), 2);
        let reply = request(&mut h, &mut ui, &cv, &mut scpi, post("/waveform?name=clock.csv", csv.as_bytes()));
        assert!(reply.starts_with("HTTP/1.1 409") && reply.contains("R1 to R10"), "{reply}");
        assert_eq!(ui.ref_slot(), 2, "a file with no slot does not replace R1");
        assert!(ui.meas_at(crate::model::WaveId::Ref(0), oos_dsp::measure::Meas::Vpp).is_nan());
        assert!(ui.meas_at(crate::model::WaveId::Ref(2), oos_dsp::measure::Meas::Vpp).is_finite());
        for slot in ["0", "11", "nope", ""] {
            let reply =
                request(&mut h, &mut ui, &cv, &mut scpi, post(&format!("/waveform?slot={slot}"), csv.as_bytes()));
            assert!(reply.starts_with("HTTP/1.1 409") && reply.contains("R1 to R10"), "{reply}");
        }
        let state = get(&mut h, &mut ui, &cv, &mut scpi, "/ui-state");
        let body = state.split("\r\n\r\n").nth(1).unwrap();
        let parsed = crate::json::parse(body).unwrap();
        assert_eq!(parsed.get("mode").and_then(crate::json::J::str), Some("live"));
    }

    #[test]
    fn waveform_upload_has_its_own_limit_and_origin_protection() {
        assert_eq!(MAX_WAVEFORM, 128 << 20, "native and browser imports share the parser byte limit");
        assert!(PAGE.contains("up to 128 MiB") && PAGE.contains("file.size>128*1024*1024"));
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let reply = request(
            &mut h,
            &mut ui,
            &cv,
            &mut scpi,
            format!("POST /waveform?name=x.bin HTTP/1.1\r\nContent-Length: {}\r\n\r\n", MAX_WAVEFORM + 1).into_bytes(),
        );
        assert!(reply.starts_with("HTTP/1.1 413"), "{reply}");
        let reply = request(
            &mut h,
            &mut ui,
            &cv,
            &mut scpi,
            b"POST /waveform HTTP/1.1\r\nHost: x\r\nOrigin: https://evil.example\r\nContent-Length: 0\r\n\r\n".to_vec(),
        );
        assert!(reply.starts_with("HTTP/1.1 403"), "{reply}");
        assert!(!ui.record_mode());
        assert!(crate::cmd::remote_ok("workflow live"));
        assert!(crate::cmd::remote_ok("workflow sim"));
        assert!(!crate::cmd::remote_ok("workflow quit"));
        assert!(!crate::cmd::remote_ok("workflow load C:/private/file"));
    }

    /// Runs the opt-in page-script checks under Node with a stand-in browser.
    fn run_page_script(script: &str, preset: &str, storage_throws: bool) -> String {
        const HARNESS: &str = r#"
const fs = require('fs'), vm = require('vm');
const log = [], inm = [], store = {};
if (process.env.PRESET) store.fmt = process.env.PRESET;
const els = {};
const el = id => els[id] || (els[id] = { id, textContent: '', className: '', addEventListener() {}, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1024, height: 600 }) });
global.localStorage = process.env.THROWS
  ? { getItem() { throw new Error('blocked'); }, setItem() { throw new Error('blocked'); } }
  : { getItem: k => (k in store ? store[k] : null), setItem: (k, v) => { store[k] = v; } };
global.document = { getElementById: el, hidden: false, addEventListener() {}, createElement: () => ({}), body: { appendChild() {} } };
global.Image = class { set src(v) { this._s = v; setTimeout(() => this.onload && this.onload(), 0); } get src() { return this._s; } };
global.URL = { createObjectURL: () => 'blob:x', revokeObjectURL() {} };
// the scope's side: a PNG has the ETag "t", which If-None-Match then gets 304 for
global.fetch = (u, o) => {
  // the remote controls' state poll (/ui-state) is not picture traffic: answered, not logged
  if (!u.startsWith('/screen')) return Promise.resolve({ ok: true, status: 200, headers: { get: () => null }, json: () => Promise.resolve({}), text: () => Promise.resolve(''), blob: () => Promise.resolve({}) });
  const have = (o && o.headers && o.headers['If-None-Match']) || '';
  log.push(u); inm.push(have);
  const png = u.startsWith('/screen.png'), status = png && have == '"t"' ? 304 : 200;
  return Promise.resolve({ ok: status == 200, status, headers: { get: k => (png && k == 'ETag' ? '"t"' : null) }, blob: () => Promise.resolve({}) });
};
global.setInterval = () => 0;
vm.runInThisContext(fs.readFileSync(process.argv[2], 'utf8'));
setTimeout(() => {
  const first = log.slice(), inm0 = inm.slice(), cls0 = [el('fp').className, el('fb').className];
  let err = '';
  try { setfmt('bmp'); } catch (e) { err = String(e); }
  const mark = log.length;
  setTimeout(() => {
    console.log(JSON.stringify({ first, inm0, cls0, cls1: [el('fp').className, el('fb').className], after: log.slice(mark), inm1: inm.slice(mark), store, err }));
    process.exit(0);
  }, 500);
}, 400);
"#;
        let dir = std::env::temp_dir().join(format!(
            "oos-ui-page-{}-{}",
            std::process::id(),
            preset.len() + storage_throws as usize
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("page.js"), script).unwrap();
        std::fs::write(dir.join("run.cjs"), HARNESS).unwrap();
        let mut cmd = std::process::Command::new("node");
        cmd.arg(dir.join("run.cjs"))
            .arg(dir.join("page.js"))
            .env("PRESET", preset)
            .env_remove("THROWS")
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH");
        if storage_throws {
            cmd.env("THROWS", "1");
        }
        let out = cmd.output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The background senders (`SENDERS`, at most `MAX_SENDERS`) are one count for the whole process: tests that
    /// send large answers in parallel took each other's slots, and a picture came back 503 (about 1 run in 12 under
    /// load, `live_picture_from_the_frame_loop_without_a_copy`). Those tests run one at a time.
    fn senders_alone() -> std::sync::MutexGuard<'static, ()> {
        static ALONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        ALONE.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn page_lets_the_viewer_pick_png_or_bmp() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let page = get(&mut h, &mut ui, &cv, &mut scpi, "/");
        assert!(page.contains("<script>"), "script in the page");
        // the picture the browser loads before the script runs is the small one, too
        assert!(
            page.contains(r#"id="s" width="1024" height="600" alt="scope screen" src="/screen.png""#),
            "first picture"
        );
        assert!(page.contains("setfmt('png')") && page.contains("setfmt('bmp')"), "buttons for both formats");
    }

    #[test]
    #[ignore = "external tool: requires Node.js"]
    fn page_script_lets_the_viewer_pick_png_or_bmp() {
        let script = PAGE.split("<script>").nth(1).unwrap().split("</script>").next().unwrap();
        let r = run_page_script(script, "", false);
        // default: PNG, then BMP after the switch, and the choice is kept
        assert!(r.contains(r#""first":["/screen.png?t="#), "{}", r);
        assert!(!r.contains(r#""first":["/screen.bmp"#), "{}", r);
        // the PNG's ETag goes back with the next request, the 304 keeps the loop going; never with a BMP
        assert!(r.contains(r#""inm0":["","\"t\"","#) || r.contains(r#""inm0":["","\"t\""]"#), "{}", r);
        let inm1 = r.split(r#""inm1":["#).nth(1).unwrap().split(']').next().unwrap();
        assert!(inm1.split(',').all(|v| v == r#""""#), "{}", r);
        let after: Vec<&str> = r.split(r#""after":["#).nth(1).unwrap().split(']').next().unwrap().split(',').collect();
        assert!(!after.is_empty() && after.iter().all(|a| a.contains("/screen.bmp?t=")), "{}", r);
        assert!(r.contains(r#""store":{"fmt":"bmp"}"#), "{}", r);
        // the button of the format in use is lit
        assert!(r.contains(r#""cls0":["on",""],"cls1":["","on"]"#), "{}", r);
        // a kept choice comes back at the next visit
        let r = run_page_script(script, "bmp", false);
        assert!(r.contains(r#""first":["/screen.bmp?t="#), "{}", r);
        // storage blocked (private window): still PNG, and switching does not fail
        let r = run_page_script(script, "", true);
        assert!(r.contains(r#""first":["/screen.png?t="#) && r.contains(r#""err":"""#), "{}", r);
        assert!(r.split(r#""after":["#).nth(1).unwrap().contains("/screen.bmp?t="), "{}", r);
    }

    #[test]
    fn pages_and_downloads() {
        let _alone = senders_alone();
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let r = get(&mut h, &mut ui, &cv, &mut scpi, "/");
        assert!(r.starts_with("HTTP/1.1 200") && r.contains("/screen.png") && r.contains("/update"), "{}", &r[..200]);
        assert!(get(&mut h, &mut ui, &cv, &mut scpi, "/style.css").contains("text/css"));
        // no record yet: the page gets the reason, not an empty file
        let r = get(&mut h, &mut ui, &cv, &mut scpi, "/wave.csv");
        assert!(r.starts_with("HTTP/1.1 409") && r.contains("No waveform"), "{}", r);
        let r = get(&mut h, &mut ui, &cv, &mut scpi, "/la.csv");
        assert!(r.starts_with("HTTP/1.1 409"), "{}", r);
        for _ in 0..30 {
            ui.step(0.02);
        }
        let r = get(&mut h, &mut ui, &cv, &mut scpi, "/wave.csv");
        assert!(
            r.starts_with("HTTP/1.1 200") && r.contains("text/csv") && r.contains("Time (s),CH1 (V)"),
            "{}",
            &r[..300.min(r.len())]
        );
        // download names: nothing while the clock is not set (the page takes the browser's time), then the scope's
        // local time in its own zone
        let body = |r: &str| r.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        assert_eq!(body(&get(&mut h, &mut ui, &cv, &mut scpi, "/stamp")), "");
        ui.clock_known = true;
        ui.sys.tz = "JST-9".into();
        let st = body(&get(&mut h, &mut ui, &cv, &mut scpi, "/stamp"));
        let want = ui.local_now().stamp();
        assert_eq!(st, want);
    }

    /// Large answers leave the frame loop: the whole body arrives, also several at once, and the count goes back.
    #[test]
    fn large_answers_go_through_a_thread() {
        let _alone = senders_alone();
        use std::sync::atomic::Ordering::SeqCst;
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        // a real screen: a blank canvas would be a few KB as a PNG, which is not a large answer
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        ui.render(&mut cv);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        for path in ["/screen.bmp", "/screen.png"] {
            let addr = h.addr();
            let t = std::thread::spawn(move || {
                let mut c = TcpStream::connect(("127.0.0.1", addr.port())).unwrap();
                c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
                c.write_all(format!("GET {} HTTP/1.1\r\nHost: x\r\n\r\n", path).as_bytes()).unwrap();
                let mut out = Vec::new();
                let _ = c.read_to_end(&mut out);
                out
            });
            while !t.is_finished() {
                h.poll(&mut ui, &cv, &mut scpi);
                std::thread::sleep(Duration::from_millis(1));
            }
            let out = t.join().unwrap();
            let e = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let head = String::from_utf8_lossy(&out[..e]).to_string();
            let n: usize =
                head.lines().find_map(|l| l.strip_prefix("Content-Length: ")).unwrap().trim().parse().unwrap();
            assert!(n > SEND_INLINE, "{}", path);
            assert_eq!(out.len() - e, n, "{}", path);
            if path.ends_with("png") {
                assert_eq!(&out[e..e + 4], b"\x89PNG");
            }
        }
        // the client has every byte, the sender only lets go of its place: a minute is a guard against a hang only
        let until = Instant::now() + Duration::from_secs(60);
        while SENDERS.load(SeqCst) != 0 {
            assert!(Instant::now() < until, "a sender did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// With a frame loop that hands its screens over before each render, the picture is the screen that was shown,
    /// taken without a copy, and the loop's canvas gets a buffer of the right size to draw into. The hand-over is told
    /// by the buffer, not its content: a blank canvas swapped for a zeroed spare has equal pixels.
    ///
    /// Break it catches as well: the test's own race. A request in before the first frame is answered with a copy of
    /// the screen (`Shots::request` -> `tick`: the loop has not yet said that it renders whole frames), and frames
    /// handed over afterwards are pictures made ahead. Run in one order or the other by the client's timing, that
    /// failed about 1 in 7 under load; both orders now run on purpose.
    #[test]
    fn live_picture_from_the_frame_loop_without_a_copy() {
        let _alone = senders_alone();
        for frames_first in [true, false] {
            live_picture_hand_over(frames_first);
        }
    }

    fn live_picture_hand_over(frames_first: bool) {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        let mut frame = 0u32;
        // the first screen handed over (the canvas got another buffer): the one before the render
        let mut shown: Option<Vec<u32>> = None;
        let mut one_frame = |h: &mut Http, cv: &mut Canvas, shown: &mut Option<Vec<u32>>| {
            let (before, buf) = (cv.px.clone(), cv.px.as_ptr());
            h.before_render(cv, true);
            if cv.px.as_ptr() != buf && shown.is_none() {
                *shown = Some(before);
            }
            assert_eq!(cv.px.len(), (cv.w * cv.h) as usize);
            frame += 1;
            cv.clear(crate::gfx::rgb(frame % 256, 40, 90));
        };
        if frames_first {
            for _ in 0..3 {
                h.poll(&mut ui, &cv, &mut scpi);
                one_frame(&mut h, &mut cv, &mut shown);
            }
            assert!(shown.is_none(), "nothing handed over before a request");
        }
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
            let _ = c.write_all(b"GET /screen.png HTTP/1.1\r\nHost: x\r\n\r\n");
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        // the request is in before the next frame, whatever the load; the screen at that moment
        while h.shots.lock().waiting.is_empty() && !t.is_finished() {
            h.poll(&mut ui, &cv, &mut scpi);
            std::thread::sleep(Duration::from_millis(1));
        }
        let at_request = cv.px.clone();
        while !t.is_finished() {
            one_frame(&mut h, &mut cv, &mut shown);
            h.poll(&mut ui, &cv, &mut scpi);
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = t.join().unwrap();
        let e = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(out.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&out[..e]));
        let png = |px: &[u32]| crate::store::png(cv.w as u32, cv.h as u32, px);
        if frames_first {
            // the frame loop runs: the next screen goes over by a buffer swap, and it is the picture
            let shown = shown.expect("a screen was handed over");
            assert!(out[e..] == png(&shown), "the PNG of the screen that was shown");
        } else {
            // before the first frame: the screen of that moment, copied
            assert!(out[e..] == png(&at_request), "the PNG of the screen at the request");
        }
    }

    /// A screenshot asked for as PNG never gets JPEG bytes, a JPEG viewer gets PNG only once the VPU failed.
    #[test]
    fn a_picture_answers_only_its_own_format() {
        let _alone = senders_alone();
        assert!(Shots::fits(false, false, false) && Shots::fits(true, true, false));
        assert!(!Shots::fits(false, true, false) && !Shots::fits(false, true, true), "PNG asked, JPEG made");
        assert!(!Shots::fits(true, false, false), "JPEG asked, PNG made while the VPU works: wait for the JPEG");
        assert!(Shots::fits(true, false, true), "the VPU failed: PNG for everyone");
    }

    /// System > Graphics = GPU: the canvas changes only when a picture comes back from the GPU (here every third
    /// frame). Nothing is swapped out of it (a swapped-in buffer stayed until the next read: the web page went back
    /// and forth between the new and an older screen), and the picture is one that came back.
    #[test]
    fn live_picture_with_the_gpu_is_a_picture_read_back() {
        let _alone = senders_alone();
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
            let _ = c.write_all(b"GET /screen.png HTTP/1.1\r\nHost: x\r\n\r\n");
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        let mut read_back = Vec::new();
        let mut frame = 0u32;
        while !t.is_finished() {
            h.poll(&mut ui, &cv, &mut scpi);
            let before = cv.px.clone();
            h.before_gpu_frame();
            assert!(cv.px == before, "the canvas keeps the last picture read back");
            frame += 1;
            if frame % 3 == 0 {
                cv.clear(crate::gfx::rgb(frame % 256, 40, 90));
                read_back.push(cv.px.clone());
                h.picture_read_back(&cv);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = t.join().unwrap();
        let e = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(out.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&out[..e]));
        assert!(
            read_back.iter().any(|p| out[e..] == crate::store::png(cv.w as u32, cv.h as u32, p)),
            "the PNG of a picture read back"
        );
    }

    /// The live picture: one encode per screen; a viewer with the newest picture waits (long poll) and gets 304 when
    /// nothing changes, or the new picture as soon as the screen changes; a viewer without it gets it at once.
    #[test]
    fn live_picture_once_per_screen_and_long_poll() {
        let _alone = senders_alone();
        // the comparison of two screens, at every length and a difference at every place
        for n in [0usize, 1, 7, 8, 9, 63, 64, 65] {
            let a: Vec<u32> = (0..n as u32).map(|i| i.wrapping_mul(2654435761)).collect();
            assert!(same_pixels(&a, &a.clone()));
            for k in 0..n {
                let mut b = a.clone();
                b[k] ^= 1 << (k % 32);
                assert!(!same_pixels(&a, &b), "{} {}", n, k);
            }
            assert!(!same_pixels(&a, &a[..n.saturating_sub(1)]) || n == 0);
        }
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        ui.render(&mut cv);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        // a viewer in a thread; the server is polled meanwhile, and `change` alters the screen after 100 ms
        let shot = |h: &mut Http,
                    ui: &mut Ui,
                    scpi: &mut crate::scpi::Scpi,
                    cv: &mut Canvas,
                    have: Option<&str>,
                    change: bool| {
            let inm = have.map_or(String::new(), |t| format!("If-None-Match: {}\r\n", t));
            let req = format!("GET /screen.png?t=1 HTTP/1.1\r\nHost: x\r\n{}\r\n", inm).into_bytes();
            let t0 = Instant::now();
            let t = std::thread::spawn(move || {
                let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
                c.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
                let _ = c.write_all(&req);
                let mut out = Vec::new();
                let _ = c.read_to_end(&mut out);
                out
            });
            let mut changed = false;
            while !t.is_finished() {
                if change && !changed && t0.elapsed() > Duration::from_millis(100) {
                    cv.fill(crate::gfx::Rect::new(10, 60, 300, 40), crate::gfx::rgb(30, 250, 30));
                    changed = true;
                }
                h.poll(ui, cv, scpi);
                std::thread::sleep(Duration::from_millis(1));
            }
            let out = t.join().unwrap();
            let e = out.windows(4).position(|w| w == b"\r\n\r\n").expect("a head") + 4;
            let head = String::from_utf8_lossy(&out[..e]).to_string();
            let tag = head.lines().find_map(|l| l.strip_prefix("ETag: ")).unwrap_or("").to_string();
            (head, tag, out[e..].to_vec(), t0.elapsed())
        };
        let encodes = |h: &Http| h.shots.lock().encodes;
        let png_of = |cv: &Canvas| crate::store::png(cv.w as u32, cv.h as u32, &cv.px);
        // the long poll is a minute but where it is waited out: a debug PNG takes most of a second, and the encoder
        // waits twice its last encode before the next copy (`SHOT_DUTY`), so the test's one second could end before a
        // picture is made on a slow or busy computer (a viewer without a picture then got an empty one)
        let long_poll = |h: &Http, d: Duration| h.shots.lock().long_poll = Some(d);
        long_poll(&h, Duration::from_secs(60));
        let (head, tag1, body, _) = shot(&mut h, &mut ui, &mut scpi, &mut cv, None, false);
        assert!(head.starts_with("HTTP/1.1 200") && head.contains("Content-Type: image/png"), "{}", head);
        assert!(tag1.starts_with('"') && tag1.ends_with('"') && tag1.len() > 2, "{}", head);
        assert!(body == png_of(&cv), "the PNG of the screen");
        // the screen unchanged, the viewer has it: it waits the long poll out, then 304 without a body, no encode
        long_poll(&h, LONG_POLL);
        let (head, tag, body, took) = shot(&mut h, &mut ui, &mut scpi, &mut cv, Some(&tag1), false);
        assert!(head.starts_with("HTTP/1.1 304") && body.is_empty() && tag == tag1, "{}", head);
        assert!(took >= LONG_POLL, "answered after {:?}", took);
        long_poll(&h, Duration::from_secs(60));
        // a viewer without it gets the same picture, without a new encode
        let (head, tag, body, _) = shot(&mut h, &mut ui, &mut scpi, &mut cv, None, false);
        assert!(head.starts_with("HTTP/1.1 200") && tag == tag1 && body == png_of(&cv), "{}", head);
        assert_eq!(encodes(&h), 1);
        // the screen changes while the viewer waits: the new picture comes before the long poll ends (a 304 would come
        // after a minute)
        let (head, tag2, body, _) = shot(&mut h, &mut ui, &mut scpi, &mut cv, Some(&tag1), true);
        assert!(head.starts_with("HTTP/1.1 200") && tag2 != tag1 && body == png_of(&cv), "{}", head);
        assert_eq!(encodes(&h), 2);
        // a foreign page still gets nowhere
        let r = request(
            &mut h,
            &mut ui,
            &cv,
            &mut scpi,
            b"GET /screen.png HTTP/1.1\r\nHost: x\r\nSec-Fetch-Site: cross-site\r\n\r\n".to_vec(),
        );
        assert!(r.starts_with("HTTP/1.1 403"), "{}", r);
    }

    #[test]
    fn update_upload_limits() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let post = |path: &str, body: &[u8]| {
            let mut r =
                format!("POST {} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n", path, body.len()).into_bytes();
            r.extend_from_slice(body);
            r
        };
        // a 5 MB body is taken on /update (and refused for its content, nothing installed) ...
        let big = vec![0xffu8; 5 << 20];
        let r = request(&mut h, &mut ui, &cv, &mut scpi, post("/update", &big));
        assert!(
            r.starts_with("HTTP/1.1 409") && r.contains("REFUSED: not an OOS update package"),
            "{}",
            &r[..r.len().min(300)]
        );
        // ... but not elsewhere
        let r = request(&mut h, &mut ui, &cv, &mut scpi, post("/mcp", &big));
        assert!(r.starts_with("HTTP/1.1 413"), "{}", &r[..r.len().min(200)]);
        // an announced length beyond the limit is answered at once, without waiting for the body
        let r = request(
            &mut h,
            &mut ui,
            &cv,
            &mut scpi,
            b"POST /update HTTP/1.1\r\nContent-Length: 999999999999\r\n\r\n".to_vec(),
        );
        assert!(r.starts_with("HTTP/1.1 413"), "{}", r);
        let r = request(
            &mut h,
            &mut ui,
            &cv,
            &mut scpi,
            b"POST /update HTTP/1.1\r\nContent-Length: 18446744073709551615\r\n\r\n".to_vec(),
        );
        assert!(r.starts_with("HTTP/1.1 413"), "{}", r);
        // ordinary requests still work
        let r = request(&mut h, &mut ui, &cv, &mut scpi, b"GET /lxi/identification HTTP/1.1\r\n\r\n".to_vec());
        assert!(r.starts_with("HTTP/1.1 200") && r.contains("<Manufacturer>OOS"), "{}", r);
        assert!(r.contains("TCPIP::INSTR") && r.contains("TCPIP::5555::SOCKET"), "{}", r);
    }

    /// The readers hold at most one large body: before, eight connections could each buffer up to 128 MiB, and
    /// `POST /update` or `/mcp` from outside `net.allow` are refused when the head is in; only a head is sent here,
    /// so the answer must not wait for a body.
    #[test]
    fn a_large_body_at_a_time_and_net_allow_before_the_body() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let head = |path: &str, n: usize| format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {n}\r\n\r\n");
        // a large upload in progress: its head and a part of its body
        let port = h.port().unwrap();
        let mut first = TcpStream::connect(("127.0.0.1", port)).unwrap();
        first.write_all(head("/update", 8 << 20).as_bytes()).unwrap();
        first.write_all(&vec![0u8; 64 << 10]).unwrap();
        let until = Instant::now() + Duration::from_secs(60);
        while !h.inbox.large.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(Instant::now() < until, "the first upload was not taken");
            h.poll(&mut ui, &cv, &mut scpi);
            std::thread::sleep(Duration::from_millis(1));
        }
        for path in ["/waveform?slot=1", "/update"] {
            let r = request(&mut h, &mut ui, &cv, &mut scpi, head(path, 2 << 20).into_bytes());
            assert!(r.starts_with("HTTP/1.1 503"), "{path}: {r}");
        }
        // a small body still goes through beside it
        let r = request(&mut h, &mut ui, &cv, &mut scpi, b"GET /lxi/identification HTTP/1.1\r\n\r\n".to_vec());
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        drop(first);
        // outside net.allow: refused at the head, the body never asked for
        h.acl.set(crate::netacl::Acl::parse("192.0.2.0/24").0);
        h.acl.peers_as(Some("198.51.100.7".parse().unwrap()));
        for path in ["/update", "/mcp"] {
            let r = request(&mut h, &mut ui, &cv, &mut scpi, head(path, 1 << 20).into_bytes());
            assert!(r.starts_with("HTTP/1.1 403") && r.ends_with("refused"), "{path}: {r}");
        }
        let r = request(&mut h, &mut ui, &cv, &mut scpi, b"GET /lxi/identification HTTP/1.1\r\n\r\n".to_vec());
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    }

    // Break it catches: the web server keeping the net.allow of its start, so a list changed on the screen
    // (Remote services > Allowed) refused or let in an update only after a restart of oos-ui
    #[test]
    fn net_allow_follows_a_change_without_a_restart() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        // the list the screen sets, shared with the server; the test's connections come from 198.51.100.7
        let acl = crate::netacl::Shared::default();
        h.set_acl(acl.clone());
        acl.peers_as(Some("198.51.100.7".parse().unwrap()));
        let update = b"POST /update HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\n\r\njunk".to_vec();
        // taken: the package itself is refused (409, task 0074), not the address
        let taken = |r: &str| r.starts_with("HTTP/1.1 409") && r.contains("REFUSED: not an OOS update package");
        let r = request(&mut h, &mut ui, &cv, &mut scpi, update.clone());
        assert!(taken(&r), "no list: everyone: {r}");
        acl.set(crate::netacl::Acl::parse("192.0.2.0/24").0);
        let r = request(&mut h, &mut ui, &cv, &mut scpi, update.clone());
        assert!(r.starts_with("HTTP/1.1 403") && r.ends_with("refused"), "outside the new list: {r}");
        acl.set(crate::netacl::Acl::parse("192.0.2.0/24 198.51.100.0/24").0);
        let r = request(&mut h, &mut ui, &cv, &mut scpi, update.clone());
        assert!(taken(&r), "inside the next one: {r}");
        acl.set(crate::netacl::Acl::from_value("").0);
        let r = request(&mut h, &mut ui, &cv, &mut scpi, update);
        assert!(taken(&r), "the list removed: {r}");
    }

    #[test]
    fn identification_names_the_listeners() {
        let mut h = Http::open(0, &Default::default());
        h.offer(Offer { scpi_port: Some(5555), vxi11: false, hislip_port: Some(crate::hislip::PORT) });
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let r = request(&mut h, &mut ui, &cv, &mut scpi, b"GET /lxi/identification HTTP/1.1\r\n\r\n".to_vec());
        assert!(r.contains("TCPIP::hislip0::INSTR"), "{}", r);
        assert!(r.contains("TCPIP::5555::SOCKET"), "{}", r);
        assert!(!r.contains("TCPIP::INSTR<"), "{}", r);
        assert!(r.contains("<IdentificationURL>/lxi/identification</IdentificationURL>"), "{}", r);
    }

    #[test]
    fn identification_names_custom_hislip_port_without_advertising_the_default() {
        let xml = lxi_identification(&Offer { scpi_port: Some(5555), vxi11: false, hislip_port: Some(14880) });
        assert!(xml.contains("TCPIP::hislip0,14880::INSTR"), "{xml}");
        assert!(!xml.contains("TCPIP::hislip0::INSTR"), "{xml}");
        let closed = lxi_identification(&Offer { scpi_port: None, vxi11: false, hislip_port: None });
        assert!(!closed.contains("InstrumentAddressString"), "{closed}");
    }

    #[test]
    fn cmd_takes_keys_knobs_and_touch_only() {
        let mut h = Http::open(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let get = |c: &str| format!("GET /cmd?c={} HTTP/1.1\r\n\r\n", c).into_bytes();
        for c in ["shot%20/dev/mmcblk1", "storedir%20/", "stat%20/etc/passwd", "quit", "frames%2010", "selfcal%20x", ""]
        {
            let r = request(&mut h, &mut ui, &cv, &mut scpi, get(c));
            assert!(r.starts_with("HTTP/1.1 403"), "{}: {}", c, r);
        }
        let r = request(&mut h, &mut ui, &cv, &mut scpi, get("key%20runstop"));
        assert!(r.starts_with("HTTP/1.1 200"), "{}", r);
        for c in ["key runstop", "knob vscale +1", "tap 100 200", "drag 1 2 3 4"] {
            assert!(crate::cmd::remote_ok(c), "{}", c);
        }
    }

    #[test]
    fn unescape_survives_any_bytes() {
        assert_eq!(unescape("key%20runstop+x"), "key runstop x");
        // two-byte characters right after the %: the slice used to fall inside one
        assert_eq!(unescape("a%é1"), "a1");
        assert_eq!(unescape("%€"), String::from_utf8_lossy(&[0xac]));
        assert_eq!(unescape("x%4"), "x%4");
    }

    #[test]
    fn mcp_idn_preserves_the_build_field() {
        use crate::json::{parse, J};
        let host = crate::ui::host::Host::fixture_with_text(|path| {
            (path == "/etc/oos-build").then(|| "20261115-143335 kernel fixture\n".into())
        });
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, host);
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scpi","arguments":{"command":"*IDN?"}}}"#;
        let reply = parse(&mcp(body, &mut ui, &cv, &mut scpi).unwrap()).unwrap();
        let Some(J::Arr(content)) = reply.get("result").and_then(|r| r.get("content")) else {
            panic!("MCP text result missing: {:?}", reply);
        };
        let text = content[0].get("text").and_then(J::str).unwrap();
        assert_eq!(text.split(',').count(), 4);
        assert_eq!(
            text.split(',').next_back().unwrap(),
            format!("oos-ui {} build 20261115-143335", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(text.as_bytes(), scpi.exec_line(&mut ui, &cv, "*IDN?"));
    }

    #[test]
    fn mcp_pending_opc_is_an_explicit_tool_error_not_success() {
        let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        ui.set_offline(true);
        let mut csv = String::from("Time (s),Volts\n");
        for i in 0..4000 {
            csv.push_str(&format!("{},{}\n", i as f64 * 1e-9, if i % 40 < 20 { -0.5 } else { 0.5 }));
        }
        ui.load_waveform_bytes(csv.as_bytes(), "opc.csv").unwrap();
        ui.m.trig_level = 10.0;
        let cv = crate::gfx::Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let reply=mcp(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":SINGLE;*OPC?;:RUN"}}}"#,&mut ui,&cv,&mut scpi).unwrap();
        let reply = crate::json::parse(&reply).unwrap();
        assert_eq!(reply.get("result").unwrap().get("isError"), Some(&crate::json::J::Bool(true)));
        assert_eq!(
            ui.m.trig_mode,
            crate::model::TrigMode::Single,
            "commands after an unsupported immediate barrier must not execute"
        );
    }

    #[test]
    fn mcp_empty_query_is_empty_text_while_an_empty_write_is_ok() {
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let empty = mcp(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":MEAS:CLEAR;:MEAS:STRIP?"}}}"#,
            &mut ui, &cv, &mut scpi).unwrap();
        assert!(empty.contains("\"text\":\"\""), "{empty}");
        assert!(!empty.contains("\"text\":\"ok\""));
        let write = mcp(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":MEAS:CLEAR"}}}"#,
            &mut ui, &cv, &mut scpi).unwrap();
        assert!(write.contains("\"text\":\"ok\""), "{write}");
        let compound = mcp(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":MEAS:STRIP?;*OPC?"}}}"#,
            &mut ui, &cv, &mut scpi).unwrap();
        assert!(compound.contains("\"text\":\";1\""), "{compound}");
    }

    #[test]
    fn mcp_session() {
        use crate::json::{parse, J};
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let mut call = |b: &str| mcp(b, &mut ui, &cv, &mut scpi).map(|r| parse(&r).unwrap());
        let r = call(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#)
            .unwrap();
        assert_eq!(r.get("result").and_then(|x| x.get("protocolVersion")).and_then(J::str), Some("2025-06-18"));
        assert!(call(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let r = call(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert!(r.to_string().contains("\"screenshot\""));
        let r = call(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scpi","arguments":{"command":":TIM:SCAL 0.001;*IDN?"}}}"#).unwrap();
        assert!(r.to_string().contains("OOS,DHO924S"), "{}", r.to_string());
        let r = call(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"screenshot","arguments":{}}}"#)
            .unwrap();
        assert!(r.to_string().contains("iVBORw0KGgo")); // PNG signature in base64
        assert!(call(r#"{"jsonrpc":"2.0","id":5,"method":"nope"}"#).unwrap().get("error").is_some());
        drop(call);
        assert!((ui.m.time_per_div() - 1e-3).abs() < 1e-9);
    }

    #[test]
    fn scpi_extensions() {
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        for _ in 0..3 {
            ui.step(0.05);
        }
        let mut q = |c: &str| String::from_utf8(scpi.exec_line(&mut ui, &cv, c)).unwrap();
        q(":VIEW SPEC");
        assert_eq!(q(":VIEW?"), "SPECTRUM");
        assert!(q(":IDEN?").starts_with("CH1 "), "{}", q(":IDEN?"));
        assert_eq!(q(":CAL:STAT?"), "needs the hardware source");
        q(":BODE:GEN \"10.0.0.9:5025\"");
        assert_eq!(q(":BODE:GEN?"), "10.0.0.9:5025");
        assert_eq!(q(":BODE:DATA?"), "");
        // built-in generator, Rigol forms
        q(":SOUR:FREQ 12345.6");
        q(":SOURce1:VOLTage:LEVel:IMMediate:AMPLitude 0.5");
        q(":SOUR:VOLT:OFFS 0.25");
        q(":SOUR:OUTP:STAT ON");
        assert_eq!(
            (q(":SOUR:FREQ?"), q(":SOUR:VOLT?"), q(":SOUR:VOLT:OFFS?"), q(":SOUR:OUTP?"), q(":SOUR:FUNC?")),
            ("1.234560E+04".into(), "5.000000E-01".into(), "2.500000E-01".into(), "1".into(), "SIN".into())
        );
        // other wave shapes: a ramp caps the frequency at 5 MHz; square duty, ramp symmetry
        q(":SOUR:FREQ 1e7");
        q(":SOUR:FUNC RAMP");
        q(":SOUR:FUNC:RAMP:SYMM 20");
        assert_eq!(
            (q(":SOUR:FUNC?"), q(":SOUR:FREQ?"), q(":SOUR:FUNC:RAMP:SYMM?")),
            ("RAMP".into(), "5.000000E+06".into(), "2.000000E+01".into())
        );
        q(":SOUR:FUNC SQU");
        q(":SOUR:FUNC:SQU:DUTY 25");
        assert_eq!((q(":SOUR:FUNC?"), q(":SOUR:FUNC:SQU:DUTY?")), ("SQU".into(), "2.500000E+01".into()));
        q(":SOUR:FUNC SIN");
        // modulation
        q(":SOUR:FREQ 1e6");
        q(":SOUR:MOD:TYP FM");
        q(":SOUR:MOD:FM:DEV 2e6");
        q(":SOUR:MOD:FM:INT:FREQ 1000");
        q(":SOUR:MOD:FM:INT:FUNC SQU");
        // the deviation stays below the carrier
        assert_eq!(
            (
                q(":SOUR:MOD:TYP?"),
                q(":SOUR:MOD:FM:DEV?"),
                q(":SOUR:MOD:FM:INT:FREQ?"),
                q(":SOUR:MOD:FM:INT:FUNC?"),
                q(":SOUR:MOD?")
            ),
            ("FM".into(), "1.000000E+06".into(), "1.000000E+03".into(), "SQU".into(), "1".into())
        );
        q(":SOUR:MOD OFF");
        assert_eq!(q(":SOUR:MOD:TYP?"), "NONE");
        q(":SOUR:FREQ 1e9");
        assert_eq!(q(":SOUR:FREQ?"), "6.000000E+07");
    }

    #[test]
    fn scpi_refuses_what_is_not_a_number() {
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let (delay, offset, probe) = (ui.m.delay, ui.m.ch[0].offset, ui.m.ch[0].probe);
        for c in [
            ":TIM:OFFS nan",
            ":TIM:OFFS inf",
            ":TIM:OFFS 1e39",
            ":CHAN1:OFFS NaN",
            ":CHAN1:PROB -inf",
            ":CHAN1:PROB 0",
            ":CHAN1:PROB -10",
            ":TRIG:EDGE:LEV nan",
            ":SOUR:FREQ nan",
            ":SOUR:VOLT inf",
        ] {
            scpi.exec_line(&mut ui, &cv, c);
        }
        assert_eq!((ui.m.delay, ui.m.ch[0].offset, ui.m.ch[0].probe), (delay, offset, probe));
        // large but finite values run through the screen and a saved setup
        scpi.exec_line(&mut ui, &cv, ":TIM:OFFS 3e38");
        scpi.exec_line(&mut ui, &cv, ":CHAN1:OFFS -3e38");
        ui.key(crate::ui::key::CLEAR);
        ui.mark_changed();
        for _ in 0..3 {
            ui.step(0.05);
        }
        ui.render(&mut cv);
        assert!(crate::store::parse_setup(&crate::store::setup_text(&ui.m)).is_ok());
    }

    #[test]
    fn empty_bode_address_keeps_the_setup_readable() {
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        scpi.exec_line(&mut ui, &cv, ":BODE:GEN \"\"");
        assert_eq!(ui.m.bode_addr, "");
        let m = crate::store::parse_setup(&crate::store::setup_text(&ui.m)).unwrap();
        assert_eq!(m.bode_addr, "");
        // two words would not fit the line of the setup file
        scpi.exec_line(&mut ui, &cv, ":BODE:GEN \"10.0.0.9:5025\"");
        scpi.exec_line(&mut ui, &cv, ":BODE:GEN \"a b\"");
        assert_eq!(ui.m.bode_addr, "10.0.0.9:5025");
        assert_eq!(crate::store::parse_setup(&crate::store::setup_text(&ui.m)).unwrap().bode_addr, "10.0.0.9:5025");
        // "usb": the generator on the USB port, the LAN address kept for later
        scpi.exec_line(&mut ui, &cv, ":BODE:GEN \"usb\"");
        assert!(ui.m.bode_usb && ui.m.bode_addr == "10.0.0.9:5025");
        assert_eq!(scpi.exec_line(&mut ui, &cv, ":BODE:GEN?"), b"usb".to_vec());
        let m = crate::store::parse_setup(&crate::store::setup_text(&ui.m)).unwrap();
        assert!(m.bode_usb && m.bode_addr == "10.0.0.9:5025");
        scpi.exec_line(&mut ui, &cv, ":BODE:GEN 10.0.0.10");
        assert!(!ui.m.bode_usb && ui.m.bode_addr == "10.0.0.10");
    }

    #[test]
    fn base64_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
