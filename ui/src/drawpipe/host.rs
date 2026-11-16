// SPDX-License-Identifier: GPL-2.0-or-later

//! Host-only completion barrier: a returned read-out means every worker has published its share.

use super::{Pipe, RawBatch};
use std::time::Duration;

pub fn finish_batch(pipe: &Pipe, id: u64) -> Result<Box<dyn RawBatch>, String> {
    oos_hal::sync::lock(&pipe.pool).recv_timeout(Duration::from_secs(30)).map_err(|e| {
        format!(
            "batch {}: buffer return failed: {}; counters {:?}, live {}",
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
            26,
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
