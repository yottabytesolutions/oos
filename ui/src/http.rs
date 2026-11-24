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
//!                     written). Netio reads requests with time limits (`Limits`); every limit
//!                     ends in an answer (docs/update.md).
//! Unauthenticated: bench LAN only (an update must carry a signature from this project's release key).
//! `net.allow` (`netacl`, docs/network.md 6) limits `POST /update` and `/mcp` to the listed address prefixes.

use crate::gfx::Canvas;
use crate::netport::http::ReplyHandle as Response;
use crate::netport::http::*;
pub use crate::netport::http::{parse_head, refuse_early, Head};
use crate::ui::Ui;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
mod listener;
mod screen;
mod shutdown;
use crate::placement::lower_priority;

/// Which instrument addresses the identification page lists.
#[derive(Clone, Debug)]
pub struct Offer {
    pub scpi_port: Option<u16>,
    pub vxi11: bool,
    pub hislip_port: Option<u16>,
}
impl Default for Offer {
    fn default() -> Self {
        Offer { scpi_port: Some(5555), vxi11: true, hislip_port: None }
    }
}
fn busy_answer(s: &Response) {
    s.answer("503 Service Unavailable", "busy, try again");
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
/// copy with the last picture, makes a PNG only of a changed screen and hands it to the waiting viewers; it gets only the time the scope leaves over (`lower_priority`). The PNG used to be made in the frame loop:
/// one viewer at 15 pictures a second took the scope's screen from 27 to 16 frames a second.
pub struct Shots {
    st: Mutex<ShotState>,
    wake: Condvar,
    /// the little cores the encoder runs on (`Cores::little`)
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
    waiting: Vec<(Response, Option<String>, Instant, bool)>,
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
    fn request(
        self: &Arc<Self>,
        s: &Response,
        have: Option<String>,
        cv: &Canvas,
        jpeg: bool,
        canvas_current: bool,
    ) -> bool {
        let mut st = self.lock();
        st.waiting.retain(|(reply, ..)| !reply.closed());
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
            answer_shot(s, have.as_deref(), png, &tag);
            return true;
        }
        if st.waiting.len() >= SHOT_WAITING {
            drop(st);
            busy_answer(s);
            return true;
        }
        if !st.thread {
            let me = self.clone();
            if self.tasks.spawn("web-png", move || me.encoder()).is_err() {
                drop(st);
                if canvas_current {
                    answer_shot(s, None, crate::store::png_of(cv).into(), "");
                }
                return canvas_current;
            }
            st.thread = true;
        }
        let c = s.clone();
        let want = st.jpeg;
        st.waiting.push((c, have, Instant::now(), want));
        drop(st);
        self.tick(cv);
        true
    }

    /// Every frame: a copy for the encoder when the time has come and somebody waits, or asked within `SHOT_AHEAD` (the
    /// picture is then ready when the viewer's next request comes); 304 to whoever waited too long.
    fn tick(&self, cv: &Canvas) {
        self.tick_with(cv, Instant::now);
    }

    #[cfg(test)]
    fn tick_at(&self, cv: &Canvas, now: Instant) {
        self.tick_with(cv, || now);
    }

    fn tick_with(&self, cv: &Canvas, now: impl Fn() -> Instant) {
        let mut st = self.lock();
        st.waiting.retain(|(reply, ..)| !reply.closed());
        let watched = st.asked.is_some_and(|t| now().saturating_duration_since(t) < SHOT_AHEAD);
        if st.waiting.is_empty() && !watched {
            return;
        }
        let now = now();
        #[cfg(test)]
        let long_poll = st.long_poll.unwrap_or(LONG_POLL);
        #[cfg(not(test))]
        let long_poll = LONG_POLL;
        if st.waiting.iter().any(|request| now - request.2 >= long_poll) {
            let (old, keep): (Vec<_>, Vec<_>) =
                std::mem::take(&mut st.waiting).into_iter().partition(|request| now - request.2 >= long_poll);
            st.waiting = keep;
            let (png, tag) = (st.png.clone().unwrap_or_else(|| Arc::from(&[][..])), st.tag.clone());
            let fit = |w: bool| Self::fits(w, st.pic_jpeg, st.no_jpeg);
            let fits: Vec<bool> = old.iter().map(|w| fit(w.3)).collect();
            drop(st);
            for ((c, have, _, _), ok) in old.into_iter().zip(fits) {
                // the last picture in the format asked for, else "busy" (the page asks again)
                if ok {
                    answer_shot(&c, have.as_deref(), png.clone(), &tag);
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
                    answer_shot(&c, have.as_deref(), png.clone(), &tag);
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
                answer_shot(&c, have.as_deref(), png.clone(), &tag);
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

/// 304 when the viewer has this picture, else the PNG through IO (503 when all are busy).
fn answer_shot(s: &Response, have: Option<&str>, png: Arc<[u8]>, tag: &str) {
    let etag = if tag.is_empty() { String::new() } else { format!("ETag: {}\r\n", tag) };
    let not_modified = !tag.is_empty() && have == Some(tag);
    let head = if not_modified {
        format!("HTTP/1.1 304 Not Modified\r\n{}Cache-Control: no-store\r\nConnection: close\r\n\r\n", etag)
    } else {
        let ctype = if png.starts_with(&[0xff, 0xd8]) { "image/jpeg" } else { "image/png" };
        format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Cache-Control: no-store\r\nConnection: close\r\n\r\n", ctype, png.len(), etag)
    };
    if !s.send(head, if not_modified { Arc::from([]) } else { png }, !not_modified) {
        busy_answer(s);
    }
}

pub struct Http {
    /// the IO service while the port is open
    open: Option<crate::netio::http::Service>,
    runtime: Option<crate::netio::Runtime>,
    /// complete owned requests from IO
    inbox: Arc<Inbox>,
    limits: Limits,
    shots: Arc<Shots>,
    /// who may install an update or use MCP (empty: everyone); the list Remote services > Allowed sets, checked per
    /// request, so a change counts at once
    acl: crate::netacl::Shared,
    /// the list IO sees (refused before the body is read): `acl`, swapped by `set_acl`
    acl_shared: Arc<Mutex<crate::netacl::Shared>>,
    /// the addresses the identification page names
    offer: Offer,
    screens: screen::Requests,
    gpu_canvas: bool,
}

impl Drop for Http {
    fn drop(&mut self) {
        self.inbox.ports.requests().clear(true);
        self.shots.lock().quit = true;
        self.shots.wake.notify_all();
        self.open.take();
    }
}

#[cfg(test)]
use crate::netport::http::PAGE;
pub use crate::netport::http::STYLE;

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
        self.open.as_ref().map(|service| service.addr().port())
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
            runtime: None,
            inbox: Arc::new(Inbox::default()),
            limits,
            shots: Shots::new(sw.cores.little),
            acl_shared: Arc::new(Mutex::new(acl.clone())),
            acl,
            offer: Offer::default(),
            screens: Default::default(),
            gpu_canvas: false,
        };
        h.start(port, sw.ipv6);
        h
    }

    pub fn wake(&self) -> &crate::system::intake::Wake {
        self.inbox.ports.requests().wake()
    }

    /// The address the port is open on (tests).
    #[cfg(test)]
    fn addr(&self) -> std::net::SocketAddr {
        self.open.as_ref().expect("web port open").addr()
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
                if self.open.take().is_some() {
                    eprintln!("oos-ui: web remote off");
                }
                // the readers waiting for the frame loop answer 503 when they give up
                self.inbox.ports.requests().clear(false);
                self.screens.clear();
            }
        }
    }

    /// Call right before rendering the next screen: hands the shown one to the live picture (`Shots::hand_over`).
    /// `whole`: the render draws every pixel.
    pub fn before_render(&mut self, cv: &mut Canvas, whole: bool) {
        self.gpu_canvas = false;
        self.shots.hand_over(cv, whole);
    }

    /// System > Graphics = GPU: the frame loop hands over the pictures that come back from the GPU
    /// (`picture_read_back`), `tick` copies none and nothing is swapped. `cv` holds the last picture read back, not
    /// one drawn every frame: a buffer swapped out of it would stay in it, an older picture, until the next read.
    pub fn before_gpu_frame(&mut self) {
        self.gpu_canvas = true;
        self.shots.lock().frames = true;
    }

    /// System > Graphics = GPU: `cv` was just read back whole; a copy goes to the encoder when it wants one.
    pub fn picture_read_back(&self, cv: &Canvas) {
        let mut st = self.shots.lock();
        st.waiting.retain(|(reply, ..)| !reply.closed());
        let now = Instant::now();
        if Shots::wanted(&st, now, false) {
            Shots::give_copy(&mut st, cv, now);
            self.shots.wake.notify_one();
        }
    }

    /// The live picture would take the next screen (System > Graphics = GPU: read it back for it).
    pub fn picture_wanted(&self) -> bool {
        let mut st = self.shots.lock();
        st.waiting.retain(|(reply, ..)| !reply.closed());
        Shots::wanted(&st, Instant::now(), false)
    }

    /// Accepts connections, answers complete requests; text commands are returned for the caller to run.
    pub fn poll(&mut self, ui: &mut Ui, cv: &Canvas, scpi: &mut crate::scpi::Scpi) -> Vec<String> {
        let mut cmds = Vec::new();
        self.screens.poll(ui, cv, scpi);
        self.shots.tick(cv);
        for request in self.inbox.take() {
            // The body can be an update package. Only parse the header as text here.
            let text = &request.head;
            let path = text.split_whitespace().nth(1).unwrap_or("/");
            let (route, query) = path.split_once('?').unwrap_or((path, ""));
            let post = text.starts_with("POST ");
            let restricted = route == "/mcp" || (route == "/update" && post);
            if foreign(text, route, post) || (restricted && !self.acl.permits_peer(Ok(request.peer))) {
                if !request.reply.content("403 Forbidden", "text/plain", b"refused".to_vec()) {
                    busy_answer(&request.reply);
                }
                continue;
            }
            if self.serve_screen(route, &request, ui, cv, scpi) {
                continue;
            }
            let mut req_body = request.body;
            let reply = request.reply;
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
                "/ui-state" if !post => ("application/json", ui.workflow_state().into_bytes()),
                "/waveform" if post => {
                    let slot = query
                        .split('&')
                        .find_map(|part| part.strip_prefix("slot="))
                        .and_then(|value| value.parse::<usize>().ok());
                    let name = waveform_name(query);
                    let previous = ui.ref_slot();
                    let result = if let Some(slot) = slot.filter(|s| (1..=10).contains(s)) {
                        ui.load_waveform_bytes_at(req_body.bytes(), &name, slot - 1)
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
                    let (refused, kind, body) = ui.update_upload_job(req_body.take_upload(), query);
                    failed = refused;
                    (kind, body)
                }
                "/update" => ("text/html; charset=utf-8", ui.update_page().into_bytes()),
                "/lxi/identification" => ("text/xml", lxi_identification(&self.offer).into_bytes()),
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
            if !reply.content(status, ctype, body) {
                busy_answer(&reply);
            }
        }
        cmds
    }
    fn serve_screen(
        &mut self,
        route: &str,
        request: &Req,
        ui: &mut Ui,
        cv: &Canvas,
        scpi: &mut crate::scpi::Scpi,
    ) -> bool {
        let reply = &request.reply;
        let (status, mime, body) = match route {
            "/screen.png" | "/screen.jpg" => {
                let have = header(&request.head, "if-none-match").map(str::to_string);
                if !self.shots.request(reply, have, cv, route == "/screen.jpg", !self.gpu_canvas) {
                    if let Some(readback) = scpi.readback() {
                        self.screens.push(reply.clone(), screen::Pending::RawPng(readback.request()));
                    } else {
                        busy_answer(reply);
                    }
                }
                return true;
            }
            "/screen.bmp" => {
                if let Some(readback) = scpi.readback() {
                    self.screens.push(reply.clone(), screen::Pending::Bmp(readback.request()));
                    return true;
                }
                ("200 OK", "image/bmp", crate::scpi::bmp(cv))
            }
            "/mcp" => match screen::mcp_reply(&String::from_utf8_lossy(request.body.bytes()), ui, cv, scpi) {
                screen::McpReply::Now(Some(result)) => ("200 OK", "application/json", result.into_bytes()),
                screen::McpReply::Now(None) => ("202 Accepted", "", Vec::new()),
                screen::McpReply::Later(pending) => {
                    self.screens.push(reply.clone(), pending);
                    return true;
                }
            },
            _ => return false,
        };
        if !reply.content(status, mime, body) {
            busy_answer(reply);
        }
        true
    }
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

pub use screen::mcp;

#[cfg(test)]
mod tests {
    #[test]
    fn exhausted_not_modified_response_budget_closes_the_screenshot_reply() {
        use std::{future::Future, task::Context, task::Poll, task::Waker};
        let inbox = Inbox::default();
        let reservation = inbox.ports.budget().reserve_available().unwrap();
        let request = inbox.envelope(String::new(), Body::Bytes(Vec::new()), "192.0.2.1:1".parse().unwrap());
        let crate::netport::RemoteRequest::Http(req) = request.into_request() else {
            panic!("HTTP request expected");
        };
        let reply = req.reply;
        let shots = Shots::new(0);
        let now = Instant::now();
        let tag = "\"unchanged\"";
        {
            let mut st = shots.lock();
            st.tag = tag.into();
            st.png = Some(Arc::from(&b"picture"[..]));
            st.waiting.push((reply.clone(), Some(tag.into()), now, false));
        }
        let mut response = std::pin::pin!(reply.receive());
        let mut cx = Context::from_waker(Waker::noop());
        assert!(response.as_mut().poll(&mut cx).is_pending());
        shots.tick_at(&Canvas::new(1, 1), now + LONG_POLL);
        assert!(shots.lock().waiting.is_empty());
        assert!(reply.closed());
        assert!(matches!(response.as_mut().poll(&mut cx), Poll::Ready(None)));
        drop(reservation);
        assert_eq!(inbox.ports.budget().used(), 0);
    }

    #[test]
    fn shot_expiry_clock_is_sampled_under_state_lock() {
        let shots = Shots::new(0);
        let now = Instant::now();
        shots.lock().asked = Some(now);
        let sampled = std::cell::Cell::new(false);
        shots.tick_with(&Canvas::new(1, 1), || {
            assert!(shots.st.try_lock().is_err(), "sample after acquiring the snapshot state");
            sampled.set(true);
            now + SHOT_AHEAD
        });
        assert!(sampled.get());
    }

    fn open_test(port: impl Into<crate::net6::Port>, sw: &crate::sysconf::Switches) -> Http {
        let guard = crate::ui::lifecycle::test_wait::HANG_GUARD;
        Http::open_with(port, Limits { idle: guard, deadline: guard, take: guard }, sw)
    }

    mod empty_tests;
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

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
        let wait = crate::ui::lifecycle::test_wait::Wait::new("HTTP response reader thread");
        let addr = h.addr();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", addr.port())).unwrap();
            c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            let _ = c.write_all(&req);
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        while !t.is_finished() {
            h.poll(ui, cv, scpi);
            wait.poll();
        }
        t.join().unwrap()
    }

    /// Sends `req` to `port` and reads the answer until the server closes.
    fn talk(port: u16, req: &[u8]) -> String {
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
        let _ = c.write_all(req);
        let mut out = Vec::new();
        let _ = c.read_to_end(&mut out);
        String::from_utf8_lossy(&out).into_owned()
    }

    // Break it catches: a client held for ever when the frame loop does not run (the 55 min upload of 2026-11-14 is
    // the case this is for): the reader answers 503 once `take` passed.
    #[test]
    fn a_request_the_frame_loop_does_not_take_gets_503() {
        crate::netio::http::tests::untaken_request_times_out();
    }

    // Break it catches: no answer to `Expect: 100-continue` (curl waits a second, other clients longer), the 100 sent
    // for a request that is refused anyway.
    #[test]
    fn expect_continue_is_answered_and_refusals_come_first() {
        let wait = crate::ui::lifecycle::test_wait::Wait::new(
            "expect_continue_is_answered_and_refusals_come_first completion",
        );
        let mut h = open_test(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        let body = b"OOS-UPDATE 1\ntarget x\n\nxx".to_vec();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
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
            wait.poll();
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
        );
        assert!(r.starts_with("HTTP/1.1 413"), "{}", r);
        // (the address list always lets the scope's own loopback in, so a page elsewhere stands in for a refusal)
        let r = talk(port, b"POST /update HTTP/1.1\r\nHost: x\r\nOrigin: https://evil.example\r\nContent-Length: 2\r\nExpect: 100-continue\r\n\r\n");
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
        let h = open_test(0, &Default::default());
        for req in [
            "POST /update HTTP/1.1\r\nHost: x\r\n\r\n",
            "POST /update HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n",
        ] {
            let r = talk(h.addr().port(), req.as_bytes());
            assert!(r.starts_with("HTTP/1.1 411"), "{:?}: {}", req, r);
        }
    }

    // Break it catches: a silent close where the limits end a request (the client sees no reason), a slow trickle that
    // keeps a connection longer than the deadline, a head without an end read for ever.
    #[test]
    fn every_limit_answers() {
        // Idle: half a head, then nothing, with an injected timeout.
        crate::netio::http::tests::idle_partial_head_expires_only_on_injected_time();
        // The partial-head total deadline fixture runs against async input with an injected clock.
        crate::netio::http::tests::partial_head_deadline();
        // head too long
        let h = open_test(0, &Default::default());
        let r = talk(h.addr().port(), format!("GET / HTTP/1.1\r\nX: {}", "a".repeat(MAX_HEAD + 100)).as_bytes());
        assert!(r.starts_with("HTTP/1.1 431"), "head: {}", r);
    }

    // Break it catches: more connections than `MAX_PENDING` closed without a word.
    #[test]
    fn too_many_connections_get_503() {
        let wait = crate::ui::lifecycle::test_wait::Wait::new("too_many_connections_get_503 completion");
        let h = open_test(0, &Default::default());
        let port = h.addr().port();
        let held: Vec<TcpStream> = (0..MAX_PENDING)
            .map(|_| {
                let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
                c.write_all(b"GET / HTTP/1.1\r\n").unwrap();
                c
            })
            .collect();
        let until = crate::ui::lifecycle::test_wait::Wait::new("too_many_connections_get_503 completion");
        while h.inbox.active.load(std::sync::atomic::Ordering::SeqCst) < MAX_PENDING {
            until.check();
            wait.poll();
        }
        let r = talk(port, b"GET / HTTP/1.1\r\nHost: x\r\n\r\n");
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
        let mut h = open_test(var("OOS_WEB_DEMO_PORT", 8080) as u16, &Default::default());
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
        let mut h = open_test(var("OOS_VALIDATION_WEB_PORT", 18881) as u16, &Default::default());
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
        let mut h = open_test(0, &Default::default());
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
        let mut h = open_test(0, &Default::default());
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
        assert!(!crate::cmd::remote_ok("workflow load"));
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
        crate::netport::http::senders_alone()
    }

    #[test]
    fn page_lets_the_viewer_pick_png_or_bmp() {
        let mut h = open_test(0, &Default::default());
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
        let mut h = open_test(0, &Default::default());
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
    fn large_answers_go_through_netio() {
        let wait = crate::ui::lifecycle::test_wait::Wait::new("HTTP sender completion");
        let _alone = senders_alone();
        use std::sync::atomic::Ordering::SeqCst;
        let mut h = open_test(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        // a real screen: a blank canvas would be a few KB as a PNG, which is not a large answer
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        ui.render(&mut cv);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        for path in ["/screen.bmp", "/screen.png"] {
            let addr = h.addr();
            let t = std::thread::spawn(move || {
                let mut c = TcpStream::connect(("127.0.0.1", addr.port())).unwrap();
                c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
                c.write_all(format!("GET {} HTTP/1.1\r\nHost: x\r\n\r\n", path).as_bytes()).unwrap();
                let mut out = Vec::new();
                let _ = c.read_to_end(&mut out);
                out
            });
            while !t.is_finished() {
                h.poll(&mut ui, &cv, &mut scpi);
                wait.poll();
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
        let until = crate::ui::lifecycle::test_wait::Wait::new("HTTP sender completion");
        while SENDERS.load(SeqCst) != 0 {
            until.check();
            wait.poll();
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
        let wait = crate::ui::lifecycle::test_wait::Wait::new("PNG encoder response after screen handover");
        let mut h = open_test(0, &Default::default());
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
            c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            let _ = c.write_all(b"GET /screen.png HTTP/1.1\r\nHost: x\r\n\r\n");
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        // the request is in before the next frame, whatever the load; the screen at that moment
        while h.shots.lock().waiting.is_empty() && !t.is_finished() {
            h.poll(&mut ui, &cv, &mut scpi);
            wait.poll();
        }
        let at_request = cv.px.clone();
        while !t.is_finished() {
            one_frame(&mut h, &mut cv, &mut shown);
            h.poll(&mut ui, &cv, &mut scpi);
            wait.poll();
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

    /// The GPU viewer accepts a picture only when it is waiting for one.
    #[test]
    fn live_picture_with_the_gpu_is_a_picture_read_back() {
        let wait = crate::ui::lifecycle::test_wait::Wait::new("GPU screenshot response");
        let _alone = senders_alone();
        let mut h = open_test(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let port = h.addr().port();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            let _ = c.write_all(b"GET /screen.png HTTP/1.1\r\nHost: x\r\n\r\n");
            let mut out = Vec::new();
            let _ = c.read_to_end(&mut out);
            out
        });
        h.before_gpu_frame();
        assert!(!h.picture_wanted());
        let mut read_back = Vec::new();
        let mut frame = 0u32;
        while !t.is_finished() {
            h.poll(&mut ui, &cv, &mut scpi);
            let before = cv.px.clone();
            h.before_gpu_frame();
            assert!(cv.px == before, "the canvas keeps the last picture read back");
            frame += 1;
            if h.picture_wanted() {
                cv.clear(crate::gfx::rgb(frame % 256, 40, 90));
                read_back.push(cv.px.clone());
                h.picture_read_back(&cv);
            }
            wait.poll();
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
        let mut h = open_test(0, &Default::default());
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
            let t = std::thread::spawn(move || {
                let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
                c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
                let _ = c.write_all(&req);
                let mut out = Vec::new();
                let _ = c.read_to_end(&mut out);
                out
            });
            let mut changed = false;
            let mut expired = false;
            let wait = crate::ui::lifecycle::test_wait::Wait::new("screen long-poll reply");
            while !t.is_finished() {
                let pending = h.shots.lock().waiting.first().map(|request| request.2);
                if !change && have.is_some() {
                    if let Some(queued) = pending {
                        assert!(!t.is_finished(), "unchanged viewer must wait for explicit expiry");
                        expired = true;
                        h.shots.tick_at(cv, queued + crate::ui::lifecycle::test_wait::HANG_GUARD);
                    }
                }
                if change && !changed && pending.is_some() {
                    cv.fill(crate::gfx::Rect::new(10, 60, 300, 40), crate::gfx::rgb(30, 250, 30));
                    changed = true;
                }
                h.poll(ui, cv, scpi);
                wait.poll();
            }
            if !change && have.is_some() {
                assert!(expired, "304 must follow the explicit long-poll deadline");
            }
            let out = t.join().unwrap();
            let e = out.windows(4).position(|w| w == b"\r\n\r\n").expect("a head") + 4;
            let head = String::from_utf8_lossy(&out[..e]).to_string();
            let tag = head.lines().find_map(|l| l.strip_prefix("ETag: ")).unwrap_or("").to_string();
            (head, tag, out[e..].to_vec())
        };
        let encodes = |h: &Http| h.shots.lock().encodes;
        let png_of = |cv: &Canvas| crate::store::png(cv.w as u32, cv.h as u32, &cv.px);
        // the long poll is a minute but where it is waited out: a debug PNG takes most of a second, and the encoder
        // waits twice its last encode before the next copy (`SHOT_DUTY`), so the test's one second could end before a
        // picture is made on a slow or busy computer (a viewer without a picture then got an empty one)
        let long_poll = |h: &Http, d: Duration| h.shots.lock().long_poll = Some(d);
        long_poll(&h, Duration::MAX);
        let (head, tag1, body) = shot(&mut h, &mut ui, &mut scpi, &mut cv, None, false);
        assert!(head.starts_with("HTTP/1.1 200") && head.contains("Content-Type: image/png"), "{}", head);
        assert!(tag1.starts_with('"') && tag1.ends_with('"') && tag1.len() > 2, "{}", head);
        assert!(body == png_of(&cv), "the PNG of the screen");
        // the screen unchanged, the viewer has it: it waits the long poll out, then 304 without a body, no encode
        long_poll(&h, crate::ui::lifecycle::test_wait::HANG_GUARD);
        let (head, tag, body) = shot(&mut h, &mut ui, &mut scpi, &mut cv, Some(&tag1), false);
        assert!(head.starts_with("HTTP/1.1 304") && body.is_empty() && tag == tag1, "{}", head);
        long_poll(&h, Duration::MAX);
        // a viewer without it gets the same picture, without a new encode
        let (head, tag, body) = shot(&mut h, &mut ui, &mut scpi, &mut cv, None, false);
        assert!(head.starts_with("HTTP/1.1 200") && tag == tag1 && body == png_of(&cv), "{}", head);
        assert_eq!(encodes(&h), 1);
        // the screen changes while the viewer waits: the new picture comes before the long poll ends (a 304 would come
        // after a minute)
        let (head, tag2, body) = shot(&mut h, &mut ui, &mut scpi, &mut cv, Some(&tag1), true);
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
        let mut h = open_test(0, &Default::default());
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
        let wait = crate::ui::lifecycle::test_wait::Wait::new(
            "a_large_body_at_a_time_and_net_allow_before_the_body completion",
        );
        let mut h = open_test(0, &Default::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let head = |path: &str, n: usize| format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {n}\r\n\r\n");
        // a large upload in progress: its head and a part of its body
        let port = h.port().unwrap();
        let mut first = TcpStream::connect(("127.0.0.1", port)).unwrap();
        first.write_all(head("/update", 8 << 20).as_bytes()).unwrap();
        first.write_all(&vec![0u8; 64 << 10]).unwrap();
        let until = crate::ui::lifecycle::test_wait::Wait::new(
            "a_large_body_at_a_time_and_net_allow_before_the_body completion",
        );
        while !h.inbox.large.occupied() {
            until.check();
            h.poll(&mut ui, &cv, &mut scpi);
            wait.poll();
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
        let mut h = open_test(0, &Default::default());
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
        let mut h = open_test(0, &Default::default());
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
        let mut h = open_test(0, &Default::default());
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
        crate::ui::change::edited(&mut ui);
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
