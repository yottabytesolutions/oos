// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! System > Graphics = GPU: the picture put together on the Mali by `oos-gpu` (gpu/oos-gpu.c).
//!
//! oos-ui draws the screen in two layers (`ui::Layers`: under the traces, over them) and leaves the traces as
//! phosphor maps (`ui::TraceJob`). This module hands the three to `oos-gpu`, a separate process (oos-ui is one static
//! musl binary, Mesa needs glibc), through one shared memory arena and a Unix socket on the helper's stdin. The
//! helper draws straight into the display's scan-out buffers (dma-bufs, `drm::Display::export`), or into its own
//! memory and reads the picture back (host tests, and the picture for the web page and screenshots).
//!
//! The protocol is a few fixed C structs (`#[repr(C)]` here, the same in oos-gpu.c, sizes checked by a test), each
//! frame answered by a render completion, followed by a pixel completion when requested.
//! A failed helper is replaced by CPU rendering.

mod process;
pub mod recovery;
#[cfg(any(target_os = "linux", test))]
mod response;
#[cfg(any(target_os = "linux", test))]
mod upload;
use crate::gfx::{Canvas, Rect};
use crate::ui::TraceJob;
use crate::wave::Phosphor;

pub const MAGIC: u32 = 0x4753_4f4f;
pub const VERSION: u32 = 4;
const K_HELLO: u32 = 1;
const K_TARGET: u32 = 2;
const K_FRAME: u32 = 4;
const K_DONE: u32 = 5;
const K_READBACK: u32 = 6;
pub const MAX_QUADS: usize = 4;
pub const MAX_JOBS: usize = 16;
const NCH: usize = 4;
const NONE: u32 = u32::MAX;
pub const FOURCC_XRGB8888: u32 = 0x3432_5258;
/// Shared memory for one frame: two layers and the read-back picture at 1920 x 1080, maps, tables.
const ARENA: usize = 96 << 20;
/// The helper as the image has it.
pub const HELPER: &str = "/bin/oos-gpu";

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Hello {
    magic: u32,
    version: u32,
    kind: u32,
    pad: u32,
    arena_size: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct TargetMsg {
    magic: u32,
    kind: u32,
    index: u32,
    w: i32,
    h: i32,
    pitch: i32,
    fourcc: u32,
    memory: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct QuadMsg {
    under_off: u32,
    over_off: u32,
    lw: i32,
    lh: i32,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
    dx: i32,
    dy: i32,
    dw: i32,
    dh: i32,
    filter: u32,
    first_job: u32,
    n_jobs: u32,
    reuse: u32,
    under_y0: i32,
    under_y1: i32,
    over_y0: i32,
    over_y1: i32,
    /// NONE: the layer is pixels; else that many commands at its offset
    under_n: u32,
    over_n: u32,
    /// the pixels of the image commands of both layers, rows of `ATLAS_W` words
    img_off: u32,
    img_rows: u32,
}

/// Width of the glyph atlas (bytes) and of the rows of image pixels (words) in the helper.
const ATLAS_W: usize = 4096;
const ATLAS_H: usize = 512;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct JobMsg {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    mw: i32,
    mh: i32,
    map_off: [u32; NCH],
    graded: u32,
    lut_off: u32,
    shift: [u32; NCH],
    col: [u32; NCH],
    map_y0: [i32; NCH],
    map_y1: [i32; NCH],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameMsg {
    magic: u32,
    kind: u32,
    target: u32,
    clear: u32,
    n_quads: u32,
    n_jobs: u32,
    readback: u32,
    readback_off: u32,
    /// glyph alphas new to the atlas: rows of `ATLAS_W` bytes to its row `atlas_row`
    atlas_off: u32,
    atlas_rows: u32,
    atlas_row: u32,
    /// the helper's set of textures: 0 the panel's, 1 the monitor's (each keeps what its last frame uploaded)
    set: u32,
    q: [QuadMsg; MAX_QUADS],
    j: [JobMsg; MAX_JOBS],
}

/// Two rows differ: eight bytes at a time (musl's memcmp, which `==` on byte slices calls, goes byte by byte and
/// took 7.5 ms for the 5 MB of a frame on the unit).
fn differ(a: &[u8], b: &[u8]) -> bool {
    let ((ca, ra), (cb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    let words = ca.iter().zip(cb).fold(0u64, |d, (p, q)| d | (u64::from_ne_bytes(*p) ^ u64::from_ne_bytes(*q)));
    words != 0 || ra != rb
}

fn bytes<T: ?Sized>(v: &T) -> &[u8] {
    // SAFETY: callers pass plain #[repr(C)] integer messages or integer slices, with no uninitialized bytes
    unsafe { std::slice::from_raw_parts(v as *const T as *const u8, std::mem::size_of_val(v)) }
}

/// A rectangle of a layer pair put into a rectangle of the target, with the traces of its plot windows.
pub struct Quad<'a> {
    pub under: Option<&'a Canvas>,
    pub over: Option<&'a Canvas>,
    /// the layers' size, also when they are not sent again (`reuse`)
    pub size: (i32, i32),
    pub src: Rect,
    pub dst: Rect,
    /// filtered scaling (a monitor), else nearest
    pub filter: bool,
    /// layers, maps and tables as the panel's last frame left them for this quad (nothing is copied or uploaded)
    pub reuse: bool,
    pub jobs: &'a [TraceJob],
    pub ph: &'a Phosphor,
    pub phz: &'a Phosphor,
}

/// Helper timings in milliseconds.
#[derive(Clone, Copy, Default, Debug)]
pub struct Times {
    pub copy: f32,
    pub upload: f32,
    pub draw: f32,
    /// The last completed pixel read, or zero before the first read.
    pub read: f32,
    /// the helper's whole time (upload and draw) of the last frame of the panel and of the monitor
    pub panel: f32,
    pub monitor: f32,
}

/// A frame sent to the helper and not collected yet (`Gpu::submit`, `Gpu::collect`, in the order sent).
pub struct Ticket {
    /// where the picture comes back, and its size, when it was asked for
    readback: Option<(usize, i32, i32)>,
    sent: std::time::Instant,
    /// a frame of the monitor (its time goes to `Times::monitor`)
    monitor: bool,
    #[cfg(target_os = "linux")]
    response: response::Response,
}

impl Ticket {
    #[cfg(target_os = "linux")]
    fn new(readback: Option<(usize, i32, i32)>, sent: std::time::Instant, monitor: bool) -> Self {
        Self { readback, sent, monitor, response: Default::default() }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn submitted(&self) -> std::time::Instant {
        self.sent
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn deadline(&self) -> std::time::Instant {
        self.sent + std::time::Duration::from_millis(1500)
    }

    /// The picture of this frame comes back (`collect` puts it into its canvas).
    pub fn reads(&self) -> bool {
        self.readback.is_some()
    }
}

#[cfg(all(target_os = "linux", test))]
pub(crate) use imp::tests::HelperFixture;
#[cfg(target_os = "linux")]
pub use imp::Gpu;

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use crate::gfx::{Cmd, Rec, CMD_GLYPH, CMD_IMAGE, CMD_IMAGE_KEY};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    use upload::UploadData as Source;

    #[cfg(test)]
    pub(crate) mod tests;

    pub struct Gpu {
        child: Option<std::process::Child>,
        stopped: Option<bool>,
        sock: UnixStream,
        arena: *mut u8,
        /// Target dimensions known to the helper.
        targets: [Option<(i32, i32)>; 8],
        /// the blocks of the last frame in each half of the arena (the panel's, the monitor's): while a frame has
        /// the same blocks, only the rows that changed are copied and uploaded
        blocks: [Vec<upload::Block>; 2],
        spare_blocks: Vec<upload::Block>,
        /// The reserved arena tail of the one outstanding pixel request, by half and offset.
        pending_read: Option<(usize, usize)>,
        /// the first of the monitor's targets: the panel's are below it (`monitor_targets_from`)
        monitor_from: usize,
        /// the fonts in the helper's glyph atlas (their alpha bytes, the byte of the atlas they start at) and its
        /// first free row
        atlas: Vec<(usize, u32)>,
        atlas_rows: usize,
        pub times: Times,
    }

    // SAFETY: the arena pointer is only used by the thread that owns the Gpu
    unsafe impl Send for Gpu {}

    fn send_fd(sock: &UnixStream, data: &[u8], fd: Option<i32>) -> std::io::Result<()> {
        let mut iov = libc::iovec { iov_base: data.as_ptr() as *mut libc::c_void, iov_len: data.len() };
        let space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<i32>() as u32) } as usize;
        let mut ctl = vec![0u8; space];
        let mut m: libc::msghdr = unsafe { std::mem::zeroed() };
        m.msg_iov = &mut iov;
        m.msg_iovlen = 1;
        if let Some(fd) = fd {
            m.msg_control = ctl.as_mut_ptr() as *mut libc::c_void;
            m.msg_controllen = space as _;
            // SAFETY: the control buffer is CMSG_SPACE for one int
            unsafe {
                let c = libc::CMSG_FIRSTHDR(&m);
                (*c).cmsg_level = libc::SOL_SOCKET;
                (*c).cmsg_type = libc::SCM_RIGHTS;
                (*c).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as _;
                std::ptr::write_unaligned(libc::CMSG_DATA(c) as *mut i32, fd);
            }
        }
        let n = unsafe { libc::sendmsg(sock.as_raw_fd(), &m, libc::MSG_NOSIGNAL) };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if n as usize != data.len() {
            return Err(std::io::Error::other("short write"));
        }
        Ok(())
    }

    impl Gpu {
        fn arena_file() -> Result<OwnedFd, String> {
            // SAFETY: the name is NUL-terminated and only the close-on-exec flag is set.
            let fd = unsafe { libc::memfd_create(c"oos-gpu".as_ptr(), libc::MFD_CLOEXEC) };
            if fd < 0 {
                return Err(format!("memfd: {}", std::io::Error::last_os_error()));
            }
            // SAFETY: a fresh descriptor we own
            let mem = unsafe { OwnedFd::from_raw_fd(fd) };
            // SAFETY: fd is the live descriptor owned by mem.
            if unsafe { libc::ftruncate(fd, ARENA as libc::off_t) } != 0 {
                return Err(format!("arena: {}", std::io::Error::last_os_error()));
            }
            Ok(mem)
        }

        fn map_arena(mem: &OwnedFd) -> io::Result<*mut u8> {
            // SAFETY: mem is a live arena descriptor sized for this mapping.
            let arena = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    ARENA,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    mem.as_raw_fd(),
                    0,
                )
            };
            if arena == libc::MAP_FAILED {
                Err(io::Error::last_os_error())
            } else {
                Ok(arena.cast())
            }
        }

        /// Starts the helper; it answers once it has a GPU context (a few hundred ms).
        pub fn start(helper: &str, cpu: Option<usize>) -> Result<Gpu, String> {
            let mem = Self::arena_file()?;
            let (mut child, ours) = lifecycle::spawn(helper, cpu)?;
            let arena = Self::map_arena(&mem).map_err(|error| {
                process::stop(&mut child, std::time::Instant::now());
                format!("arena map: {error}")
            })?;
            let mut g = Self::attach(Some(child), ours, arena);
            let h = Hello { magic: MAGIC, version: VERSION, kind: K_HELLO, pad: 0, arena_size: ARENA as u64 };
            send_fd(&g.sock, bytes(&h), Some(mem.as_raw_fd())).map_err(|e| format!("hello: {}", e))?;
            drop(mem);
            // the helper opens Mesa and the GPU first
            g.wait(K_DONE, Duration::from_secs(10)).map_err(|e| format!("start: {}", e))?;
            Ok(g)
        }

        fn wait(&mut self, kind: u32, timeout: Duration) -> Result<Duration, String> {
            self.sock.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
            let mut response = response::Response::default();
            self.sock.read_exact(response.remaining()).map_err(|e| format!("no answer: {e}"))?;
            response.decode(kind, &mut self.times)
        }

        pub(crate) fn reply_fd(&self) -> i32 {
            self.sock.as_raw_fd()
        }

        /// Poll one reply in FIFO order. Keep any partial stream reply with its ticket.
        pub(crate) fn try_done(
            &mut self,
            ticket: &mut Ticket,
            now: std::time::Instant,
        ) -> Result<Option<Duration>, String> {
            let done = self.poll_reply(K_DONE, ticket, now)?;
            if done.is_some() {
                self.finish_render(ticket);
                ticket.response = Default::default();
            }
            Ok(done)
        }

        pub(crate) fn try_readback(
            &mut self,
            ticket: &mut Ticket,
            now: std::time::Instant,
            canvas: &mut Canvas,
        ) -> Result<bool, String> {
            if self.poll_reply(K_READBACK, ticket, now)?.is_none() {
                return Ok(false);
            }
            self.finish_readback(ticket, Some(canvas))?;
            Ok(true)
        }

        fn poll_reply(
            &mut self,
            kind: u32,
            ticket: &mut Ticket,
            now: std::time::Instant,
        ) -> Result<Option<Duration>, String> {
            let fd = self.reply_fd();
            let expired = now >= ticket.deadline();
            ticket.response.poll(kind, &mut self.times, expired, |buf| {
                // SAFETY: buf is writable for its length; recv does not retain it.
                let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), libc::MSG_DONTWAIT) };
                if n < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            })
        }

        fn finish_render(&mut self, t: &Ticket) {
            let ms = self.times.upload + self.times.draw;
            if t.monitor {
                self.times.monitor = ms;
            } else {
                self.times.panel = ms;
            }
        }

        fn finish_readback(&mut self, t: &Ticket, back: Option<&mut Canvas>) -> Result<(), String> {
            self.pending_read = None;
            if let (Some((off, tw, th)), Some(cv)) = (t.readback, back) {
                if (cv.w, cv.h) != (tw, th) {
                    return Err("read-back canvas of another size".into());
                }
                // SAFETY: the helper wrote tw * th pixels there
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.arena.add(off) as *const u32,
                        cv.px.as_mut_ptr(),
                        (tw * th) as usize,
                    )
                };
            }
            Ok(())
        }

        fn unmap_arena(&mut self) {
            // SAFETY: this process owns this ARENA-byte mapping. Helper mappings are separate.
            unsafe { libc::munmap(self.arena as *mut libc::c_void, ARENA) };
        }
    }

    /// Uploads fill stable blocks from the bottom and transient recordings from the top of one half.
    struct ArenaRange {
        base: *mut u8,
        start: usize,
        tail: usize,
    }

    impl ArenaRange {
        fn reserve(&mut self, len: usize) -> Result<usize, String> {
            self.tail = self
                .tail
                .checked_sub(len)
                .filter(|&offset| offset >= self.start)
                .ok_or("frame too large for the arena")?
                & !63;
            Ok(self.tail)
        }

        fn images(&mut self, recordings: [Option<&Rec>; 2]) -> Result<(u32, u32), String> {
            let pixels = recordings.into_iter().flatten().map(|recording| recording.img.len()).sum::<usize>();
            if pixels == 0 {
                return Ok((NONE, NONE));
            }
            let rows = pixels.div_ceil(ATLAS_W);
            let size = rows * ATLAS_W * 4;
            let offset = self.reserve(size)?;
            let mut position = offset;
            for recording in recordings.into_iter().flatten() {
                // SAFETY: the reserved rows fit both recordings and are disjoint from their sources.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        recording.img.as_ptr(),
                        self.base.add(position).cast::<u32>(),
                        recording.img.len(),
                    )
                };
                position += recording.img.len() * 4;
            }
            // SAFETY: zero the remaining bytes of the reserved block for the helper's image hash.
            unsafe { std::ptr::write_bytes(self.base.add(position), 0, offset + size - position) };
            Ok((offset as u32, rows as u32))
        }

        fn commands(&mut self, recording: &Rec, atlas: &[(usize, u32)], image_base: u32) -> Result<(u32, u32), String> {
            let mut bases = [0; 256];
            for (base, font) in bases.iter_mut().zip(&recording.fonts) {
                *base = atlas.iter().find(|a| a.0 == font.as_ptr() as usize).map_or(0, |a| a.1);
            }
            let len = recording.cmds.len();
            let offset = self.reserve(len * std::mem::size_of::<Cmd>())?;
            // SAFETY: reserve returns aligned storage for these Cmds, exclusively used until submission.
            let destination = unsafe { std::slice::from_raw_parts_mut(self.base.add(offset).cast::<Cmd>(), len) };
            for (out, command) in destination.iter_mut().zip(&recording.cmds) {
                *out = *command;
                match command.kind & 255 {
                    CMD_GLYPH => out.off += bases[usize::from(command.kind >> 8)],
                    CMD_IMAGE | CMD_IMAGE_KEY => out.off += image_base,
                    _ => {}
                }
            }
            Ok((offset as u32, len as u32))
        }
    }

    impl Gpu {
        fn pack_atlas(&mut self, frame: &mut FrameMsg, quads: &[Quad], arena: &mut ArenaRange) -> Result<(), String> {
            let first_font = self.atlas.len();
            let first_row = self.atlas_rows;
            let mut new_fonts = Vec::new();
            let fonts = quads
                .iter()
                .filter(|quad| !quad.reuse)
                .flat_map(|quad| [quad.under, quad.over].into_iter().flatten())
                .filter_map(|canvas| canvas.rec.as_deref())
                .flat_map(|recording| recording.fonts.iter().copied());
            for font in fonts {
                let id = font.as_ptr() as usize;
                if self.atlas.iter().any(|entry| entry.0 == id) {
                    continue;
                }
                let rows = font.len().div_ceil(ATLAS_W);
                if self.atlas_rows + rows > ATLAS_H {
                    return Err("glyph atlas full".into());
                }
                self.atlas.push((id, (self.atlas_rows * ATLAS_W) as u32));
                self.atlas_rows += rows;
                new_fonts.push(font);
            }
            if !new_fonts.is_empty() {
                let rows = self.atlas_rows - first_row;
                let offset = arena.reserve(rows * ATLAS_W)?;
                for (font, entry) in new_fonts.into_iter().zip(&self.atlas[first_font..]) {
                    let position = offset + entry.1 as usize - first_row * ATLAS_W;
                    // SAFETY: reserve provides enough arena rows for every new font.
                    unsafe { std::ptr::copy_nonoverlapping(font.as_ptr(), arena.base.add(position), font.len()) };
                }
                (frame.atlas_off, frame.atlas_rows, frame.atlas_row) = (offset as u32, rows as u32, first_row as u32);
            }
            Ok(())
        }
    }

    struct BlockUpload<'a> {
        base: *mut u8,
        at: usize,
        tail: usize,
        known: bool,
        previous: &'a [upload::Block],
        blocks: &'a mut Vec<upload::Block>,
    }

    impl BlockUpload<'_> {
        fn put(&mut self, source: Source<'_>) -> Result<(u32, i32, i32), String> {
            self.at = (self.at + 63) & !63;
            let len = source.bytes().len();
            if self.at + len > self.tail {
                return Err("frame too large for the arena".into());
            }
            let previous = self.previous.get(self.blocks.len());
            self.known &= previous.is_some_and(|block| block.matches(self.at, len));
            // SAFETY: the preceding render in this half finished, and the block fits below the reserved tail.
            let dst = unsafe { std::slice::from_raw_parts_mut(self.base.add(self.at), len) };
            let (block, band) = upload::Block::publish(self.at, dst, source, previous.filter(|_| self.known));
            self.blocks.push(block);
            let offset = self.at as u32;
            self.at += len;
            Ok((offset, band.start as i32, band.end as i32))
        }
    }

    mod frame;

    mod lifecycle;
    impl Drop for Gpu {
        fn drop(&mut self) {
            #[cfg(test)]
            if self.child.is_none() {
                self.unmap_arena();
                return;
            }
            self.shutdown();
            self.unmap_arena();
        }
    }
}

/// No GPU path off the scope's Linux (the macOS host build).
#[cfg(not(target_os = "linux"))]
pub struct Gpu {
    pub times: Times,
}

#[cfg(not(target_os = "linux"))]
impl Gpu {
    pub fn start(_: &str, _: Option<usize>) -> Result<Gpu, String> {
        Err("the GPU path runs on Linux only".into())
    }
    pub fn target_memory(&mut self, _: usize, _: i32, _: i32) -> Result<(), String> {
        Err("no GPU".into())
    }
    pub fn compose(&mut self, _: usize, _: Option<u32>, _: &[Quad], _: Option<&mut Canvas>) -> Result<(), String> {
        Err("no GPU".into())
    }
    pub fn submit(&mut self, _: usize, _: Option<u32>, _: &[Quad], _: bool) -> Result<Ticket, String> {
        Err("no GPU".into())
    }
    pub fn collect(&mut self, _: Ticket, _: Option<&mut Canvas>) -> Result<(), String> {
        Err("no GPU".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    impl Ticket {
        pub(crate) fn fixture(sent: std::time::Instant) -> Self {
            Self::new(None, sent, false)
        }
    }

    #[test]
    fn rows_that_differ_anywhere_are_found() {
        let a: Vec<u8> = (0..1027).map(|i| i as u8).collect();
        assert!(!differ(&a, &a.clone()));
        for at in [0, 7, 8, 1023, 1024, 1026] {
            let mut b = a.clone();
            b[at] ^= 1;
            assert!(differ(&a, &b), "{}", at);
        }
    }

    /// The same sizes as the structs of gpu/oos-gpu.c.
    #[test]
    fn messages_have_the_helpers_layout() {
        use crate::gfx::Cmd;
        use std::mem::size_of;
        assert_eq!(
            (
                size_of::<Hello>(),
                size_of::<TargetMsg>(),
                size_of::<QuadMsg>(),
                size_of::<JobMsg>(),
                size_of::<FrameMsg>(),
                response::SIZE,
                size_of::<Cmd>()
            ),
            (24, 32, 96, 112, 2224, 32, 24)
        );
        let _ = (K_HELLO, K_TARGET, K_FRAME, K_DONE, NONE);
    }
}
