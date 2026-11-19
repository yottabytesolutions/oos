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
pub(crate) fn barrier(pipe: &Pipe) {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(sender) = &oos_hal::sync::lock(&pipe.workers).sender {
        sender.send(super::Message::Barrier(tx)).unwrap();
    }
    rx.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_intake_returns_the_waveform_and_only_discards_its_neighbours() {
        use super::super::{sync_channel, tests::Bytes, Counters, Message};
        use std::sync::Arc;

        for disconnected in [false, true] {
            let counters = Box::leak(Box::<Counters>::default());
            let pipe = Pipe::start(0, 0, counters);
            let (tx, rx) = sync_channel(1);
            assert!(tx.try_send(Message::Wake).is_ok());
            let _receiver = (!disconnected).then_some(rx);
            oos_hal::sync::lock(&pipe.workers).sender = Some(tx);
            let waveform = Arc::new(crate::board::Frame { seq: 4, ..Default::default() });
            for prepared in [false, true] {
                let raw = Box::new(Bytes(vec![0; 4]));
                let id = Default::default();
                let newest = Some(waveform.clone());
                let returned = if prepared {
                    pipe.feed_prepared(
                        raw,
                        1,
                        1..4,
                        (id, std::time::Instant::now()),
                        Arc::new(|_, _, _| unreachable!()),
                        newest,
                    )
                } else {
                    pipe.feed_with(raw, 1, 1..4, id, Arc::new(|_, _| unreachable!()), None, newest)
                };
                assert!(Arc::ptr_eq(&returned.unwrap(), &waveform));
                assert!(pipe.take_buffer().is_some());
            }
            assert_eq!(counters.read().2, 4);
            assert!(counters.records_text().contains("lost.unclaimed=4"));
        }
    }

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
        use std::sync::Arc;
        let m = Model::default();
        let pipe = Pipe::start(2, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&m), 970, 448, Interp::Exact);
        let mut source = scripted::source(BatchKind::Raw, 123);
        let batch = source.next_batch(&m, &Default::default(), 27, None).unwrap();
        let (fb, decode) = (batch.frame_bytes(), batch.decode());
        pipe.set_identity(Default::default());
        let newest = batch.newest(0);
        pipe.feed_with(
            Box::new(batch.into_bytes()),
            fb,
            0..27,
            Default::default(),
            decode,
            None,
            Some(Arc::new(newest)),
        );
        let back = finish_batch(&pipe, 1).unwrap().into_any().downcast::<ScriptBytes>().unwrap();
        // Buffer return does not consume the pending whole deposit.
        let merged = pipe.accept(&mut crate::wave::Phosphor::new(970, 448), &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(merged.frames, 27);
        pipe.set_view(Some(&m), 969, 448, Interp::Exact);
        let tap = pipe.take_waveform((969, 448)).unwrap();
        assert_eq!(tap.seq, 27);
        assert_eq!(
            tap.eligibility(),
            crate::board::PictureEligibility::Redraw,
            "a delayed tap can replace pixels cleared by a resize"
        );
        assert_eq!(pipe.counters.read().2, 0);
        let next = source.next_batch(&m, &Default::default(), 3, Some(*back)).unwrap();
        assert_eq!(next.newest(0).seq, 30);
    }
}
