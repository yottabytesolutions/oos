// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::presentation::{FlipRequest, FLIP_TIMEOUT};
use std::time::{Duration, Instant};

const CRTC_QUEUE_SEQUENCE: libc::c_ulong = iowr(0x3c, std::mem::size_of::<SequenceRequest>());

#[repr(C)]
struct SequenceRequest {
    crtc_id: u32,
    flags: u32,
    sequence: u64,
    user_data: u64,
}

#[derive(Default)]
pub(super) struct FlipState {
    serial: u64,
    flip: Option<(u64, Instant)>,
    vblank: Option<(u64, Instant)>,
    modeset: bool,
    busy_since: Option<Instant>,
    modeset_failed: bool,
    #[cfg(all(test, target_os = "linux"))]
    emulate_scanout: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FlipEvent {
    pub(crate) sequence: u32,
    pub(crate) monotonic: Duration,
}

#[derive(Default)]
pub(crate) struct Events {
    pub(crate) vblank: Option<FlipEvent>,
    pub(crate) flip: Option<FlipEvent>,
}

fn outstanding(event: Option<(u64, Instant)>, now: Instant) -> bool {
    event.is_some_and(|(_, requested)| now < requested + FLIP_TIMEOUT)
}

impl FlipState {
    fn cookie(&mut self) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }

    pub(super) fn read(&mut self, mut buf: &[u8]) -> Events {
        let mut events = Events::default();
        while buf.len() >= 8 {
            let word = |at| u32::from_ne_bytes(buf[at..at + 4].try_into().expect("four-byte DRM field"));
            let wide = |at| u64::from_ne_bytes(buf[at..at + 8].try_into().expect("eight-byte DRM field"));
            let len = word(4) as usize;
            if len < 8 || len > buf.len() {
                break;
            }
            if len >= 32 {
                let cookie = wide(8);
                match word(0) {
                    2 if self.flip.is_some_and(|(id, _)| id == cookie) => {
                        self.flip = None;
                        let monotonic =
                            Duration::from_secs(u64::from(word(16))) + Duration::from_micros(u64::from(word(20)));
                        events.flip = Some(FlipEvent { sequence: word(24), monotonic });
                    }
                    3 if self.vblank.is_some_and(|(id, _)| id == cookie) => {
                        self.vblank = None;
                        events.vblank =
                            Some(FlipEvent { sequence: wide(24) as u32, monotonic: Duration::from_nanos(wide(16)) });
                    }
                    _ => {}
                }
            }
            buf = &buf[len..];
        }
        events
    }
}

impl ModeInfo {
    fn refresh_period(&self) -> Duration {
        if self.clock == 0 || self.htotal == 0 || self.vtotal == 0 {
            let hz = if self.vrefresh > 0 { self.vrefresh } else { 60 };
            return Duration::from_secs_f64(1.0 / f64::from(hz));
        }
        const INTERLACE: u32 = 0x10;
        const DBLSCAN: u32 = 0x20;
        let pixels = f64::from(self.htotal) * f64::from(self.vtotal);
        let scan = if self.flags & DBLSCAN != 0 { 2.0 } else { 1.0 };
        let fields = if self.flags & INTERLACE != 0 { 2.0 } else { 1.0 };
        let hz = f64::from(self.clock) * 1_000.0 * fields / (pixels * scan * f64::from(self.vscan.max(1)));
        Duration::from_secs_f64(1.0 / hz)
    }
}

impl Display {
    pub(crate) fn refresh_period(&self) -> Duration {
        self.mode.refresh_period()
    }

    /// An independent vblank event leaves the page-flip slot available for a completed picture.
    pub(crate) fn poll_flip_fd(&mut self, clock: bool, now: Instant) -> Option<i32> {
        if clock && !outstanding(self.flips.vblank, now) {
            let user_data = self.flips.cookie();
            let mut request = SequenceRequest { crtc_id: self.crtc_id, flags: 1, sequence: 1, user_data };
            if ioc(self.fd, CRTC_QUEUE_SEQUENCE, &mut request).is_ok() {
                self.flips.vblank = Some((user_data, now));
            }
        }
        (self.flips.vblank.is_some() || self.flips.flip.is_some()).then_some(self.fd)
    }

    pub(super) fn wait_flip(&mut self) {
        let deadline = Instant::now() + FLIP_TIMEOUT * 2;
        while self.flips.flip.is_some() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                eprintln!("oos-ui: page flip event late at display shutdown");
                break;
            }
            self.read_flips(left.as_millis().max(1) as i32);
        }
    }

    /// Shows a rendered buffer on the next vblank, retaining both scanout buffers while busy.
    pub(crate) fn request_flip(&mut self, back: usize, now: Instant, modeset_due: bool) -> FlipRequest {
        self.request_flip_with(
            back,
            now,
            modeset_due,
            |display, page_cookie| {
                #[cfg(all(test, target_os = "linux"))]
                if display.flips.emulate_scanout {
                    return Ok(());
                }
                match page_cookie {
                    Some(user_data) => {
                        let mut flip = PageFlip {
                            crtc_id: display.crtc_id,
                            fb_id: display.bufs[back].fb,
                            flags: PAGE_FLIP_EVENT,
                            user_data,
                            ..PageFlip::default()
                        };
                        ioc(display.fd, PAGE_FLIP, &mut flip)
                    }
                    None => display.set_crtc(back),
                }
            },
            |error| eprintln!("oos-ui: SETCRTC failed: {error}"),
        )
    }

    fn request_flip_with(
        &mut self,
        back: usize,
        now: Instant,
        modeset_due: bool,
        mut ioctl: impl FnMut(&mut Self, Option<u64>) -> io::Result<()>,
        report_failure: impl FnOnce(io::Error),
    ) -> FlipRequest {
        if outstanding(self.flips.flip, now) {
            return FlipRequest::Busy;
        }
        self.flips.flip = None;
        let result = if self.flips.modeset {
            self.modeset_flip(modeset_due, ioctl, report_failure)
        } else {
            let user_data = self.flips.cookie();
            match ioctl(self, Some(user_data)) {
                Ok(()) => {
                    self.flips.flip = Some((user_data, now));
                    FlipRequest::Queued
                }
                Err(e)
                    if e.raw_os_error() == Some(libc::EBUSY)
                        && now < *self.flips.busy_since.get_or_insert(now) + FLIP_TIMEOUT =>
                {
                    FlipRequest::Busy
                }
                Err(e) => {
                    self.flips.modeset = e.raw_os_error() != Some(libc::EBUSY);
                    if self.flips.modeset {
                        eprintln!("oos-ui: page flip failed ({e}), using SETCRTC");
                    }
                    self.modeset_flip(modeset_due, ioctl, report_failure)
                }
            }
        };
        if matches!(result, FlipRequest::Queued | FlipRequest::Modeset) {
            self.flips.busy_since = None;
            self.flips.modeset_failed = false;
            (self.prev, self.front) = (self.front, back);
        }
        result
    }

    fn modeset_flip(
        &mut self,
        due: bool,
        ioctl: impl FnOnce(&mut Self, Option<u64>) -> io::Result<()>,
        report_failure: impl FnOnce(io::Error),
    ) -> FlipRequest {
        if !due {
            return FlipRequest::AwaitTarget;
        }
        if let Err(error) = ioctl(self, None) {
            if !std::mem::replace(&mut self.flips.modeset_failed, true) {
                report_failure(error);
            }
            return FlipRequest::Failed;
        }
        FlipRequest::Modeset
    }

    pub(super) fn flip_quiet(&mut self, back: usize) {
        let b = &self.bufs[back];
        let mut pf = PageFlip { crtc_id: self.crtc_id, fb_id: b.fb, flags: 0, reserved: 0, user_data: 0 };
        if quiet_flip(ioc(self.fd, PAGE_FLIP, &mut pf), || self.set_crtc(back)) {
            (self.prev, self.front) = (self.front, back);
        }
    }
}

fn quiet_flip(queued: io::Result<()>, modeset: impl FnOnce() -> io::Result<()>) -> bool {
    match queued {
        Ok(()) => true,
        Err(e) if e.raw_os_error() == Some(libc::EBUSY) => false,
        Err(_) => modeset().is_ok(),
    }
}

#[cfg(test)]
mod tests;
