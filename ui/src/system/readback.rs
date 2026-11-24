// SPDX-License-Identifier: GPL-2.0-or-later
//! A submitted picture takes the current batch of pixel requests. Each batch owns its completed snapshot.

use crate::gfx::Canvas;
use std::{
    cell::RefCell,
    sync::{Arc, OnceLock, Weak},
};

// Unset while the picture is pending. A completed batch holds its pixels;
// abandoning the picture sets None so its readers can request another.
type Batch = OnceLock<Option<Canvas>>;

#[derive(Default)]
pub(crate) struct Readback {
    queued: RefCell<Weak<Batch>>,
}

pub(crate) struct Request(Arc<Batch>);

pub(crate) struct Capture(Weak<Batch>);

impl Readback {
    pub(crate) fn request(&self) -> Request {
        let mut waiting = self.queued.borrow_mut();
        let batch = waiting.upgrade().unwrap_or_else(|| {
            let batch = Arc::new(Batch::new());
            *waiting = Arc::downgrade(&batch);
            batch
        });
        Request(batch)
    }

    pub(crate) fn take(&self) -> Option<Capture> {
        let batch = self.queued.take();
        (batch.strong_count() != 0).then_some(Capture(batch))
    }
}

impl Request {
    pub(crate) fn ready(&mut self, readback: &Readback) -> bool {
        if matches!(self.0.get(), Some(None)) {
            *self = readback.request();
        }
        matches!(self.0.get(), Some(Some(_)))
    }

    pub(crate) fn pixels(&mut self, readback: &Readback) -> Option<&Canvas> {
        self.ready(readback);
        self.0.get()?.as_ref()
    }
}

impl Capture {
    pub(crate) fn complete(self, canvas: &Canvas) {
        let Some(waiting) = self.0.upgrade() else { return };
        let mut snapshot = Canvas::new(canvas.w, canvas.h);
        snapshot.px.copy_from_slice(&canvas.px);
        let _ = waiting.set(Some(snapshot));
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(waiting) = self.0.upgrade() {
            let _ = waiting.set(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_requests_complete_while_old_readers_keep_their_snapshot() {
        let readback = Readback::default();
        assert!(readback.take().is_none());
        drop(readback.request());
        assert!(readback.take().is_none());
        let mut first = readback.request();
        let mut second = readback.request();
        let capture = readback.take().unwrap();
        let mut next = readback.request();
        assert!(!first.ready(&readback));
        let mut cv = Canvas::new(1, 1);
        cv.clear(0x123456);
        capture.complete(&cv);
        cv.clear(0x654321);
        assert!(first.ready(&readback));
        assert!(second.ready(&readback));
        assert_eq!(first.pixels(&readback).unwrap().px[0], 0x123456);
        assert!(!next.ready(&readback));
        drop(readback.take().unwrap());
        assert!(!next.ready(&readback));
        readback.take().unwrap().complete(&cv);
        assert!(next.ready(&readback));
        assert_eq!(next.pixels(&readback).unwrap().px[0], 0x654321);
        let first_pixels = first.pixels(&readback).unwrap();
        assert_eq!(first_pixels.px[0], 0x123456);
        assert!(std::ptr::eq(first_pixels, second.pixels(&readback).unwrap()));
        assert!(readback.take().is_none());
    }
}
