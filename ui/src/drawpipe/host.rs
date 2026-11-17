// SPDX-License-Identifier: GPL-2.0-or-later

//! Host-only completion barrier: a returned read-out means every worker has published its share.

use super::{Pipe, RawBatch};
use std::time::Duration;

pub fn finish_batch(pipe: &Pipe, id: u64) -> Result<Box<dyn RawBatch>, String> {
    oos_hal::sync::lock(&pipe.pool).recv_timeout(Duration::from_secs(30)).map_err(|e| {
        format!(
            "batch {}: worker batch completion missing at buffer return: {}; counters {:?}, live {}",
            id,
            e,
            pipe.counters.read(),
            pipe.live.load(std::sync::atomic::Ordering::Acquire)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_return_follows_decoder_cleanup() {
        use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
        use std::sync::{Arc, Barrier};

        struct Cleanup(Arc<Barrier>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.wait();
                self.0.wait();
            }
        }

        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        let mut source = scripted::source(BatchKind::Raw, 123);
        let batch = source.next_batch(&Default::default(), &Default::default(), 3, None).unwrap();
        let fb = batch.frame_bytes();
        let barrier = Arc::new(Barrier::new(2));
        let cleanup = Cleanup(barrier.clone());
        let decode: super::super::DecodeInto = Arc::new(move |_, _| {
            std::hint::black_box(&cleanup);
        });
        let batch = pipe.feed_batch(Box::new(batch.into_bytes()), fb, 0..3, (0, None), decode, None);
        let worker = std::thread::spawn(move || drop(batch));
        barrier.wait();
        let returned_early = oos_hal::sync::lock(&pipe.pool).try_recv().is_ok();
        barrier.wait();
        worker.join().unwrap();
        assert!(!returned_early, "the decoder's captured settings must be freed before the buffer returns");
        assert!(oos_hal::sync::lock(&pipe.pool).try_recv().is_ok());
    }

    #[test]
    fn returned_buffer_precedes_delivery_until_merge() {
        use crate::board::scripted::{self, Batch, BatchKind, BatchSource, ScriptBytes};
        use crate::model::Model;
        use crate::sparse::Interp;
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let m = Model::default();
        let pipe = Pipe::start(2, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&m), 970, 448, Interp::Exact);
        let mut source = scripted::source(BatchKind::Raw, 123);
        let batch = source.next_batch(&m, &Default::default(), 27, None).unwrap();
        let (fb, decode) = (batch.frame_bytes(), batch.decode());
        let delivered = Arc::new(AtomicUsize::new(0));
        let d = delivered.clone();
        pipe.set_identity(Default::default());
        pipe.feed_with(
            Box::new(batch.into_bytes()),
            fb,
            0..26,
            Default::default(),
            decode,
            None,
            Box::new(move || {
                d.fetch_add(1, Ordering::Relaxed);
            }),
        );
        let back = finish_batch(&pipe, 1).unwrap().into_any().downcast::<ScriptBytes>().unwrap();
        assert_eq!(delivered.load(Ordering::Relaxed), 0);
        let merged = pipe.merge(&mut crate::wave::Phosphor::new(970, 448), &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(merged.frames, 26);
        assert_eq!(delivered.load(Ordering::Relaxed), 1);
        assert_eq!(pipe.counters.read().2, 0);
        let next = source.next_batch(&m, &Default::default(), 3, Some(*back)).unwrap();
        assert_eq!(next.newest(0).seq, 30);
    }
}
