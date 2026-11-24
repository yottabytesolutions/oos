// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

type PlacementCodes<'a> = (&'a [u8], CodeLayout, crate::picture::deposit::envelope::Layout);

pub(in crate::drawpipe) fn input<'batch, 'bytes>(
    batch: &'batch Arc<Batch>,
    index: usize,
    bytes: &'bytes [u8],
    scratch: &mut Frame,
    have_decoded: bool,
) -> (Option<PlacementCodes<'bytes>>, impl Fn(&mut Frame) + use<'batch, 'bytes>) {
    let words = batch.codes.as_ref().and_then(|codes| {
        let raw = codes(index, bytes, scratch)?;
        Some((bytes, raw, route::layout(batch, &raw)?))
    });
    let decode = move |frame: &mut Frame| {
        batch.decode.decode(index, bytes, frame);
        batch.header.tag(frame);
    };
    if words.is_some() {
        batch.header.tag(scratch);
    } else if !have_decoded {
        decode(scratch);
    }
    (words, decode)
}

#[cfg(test)]
#[test]
fn placement_reuses_the_source_waveform_when_a_code_view_is_unavailable() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let count = Arc::new(AtomicUsize::new(0));
    let called = count.clone();
    let decode: IndexedDecodeInto = Arc::new(move |_, _, _| {
        called.fetch_add(1, Ordering::Relaxed);
    });
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    let identity = oos_core::change::Identity::default();
    pipe.publish_view(&Model::default(), 128, 64, crate::sparse::Interp::Exact, identity);
    let mut newest = Frame { dt: 1e-9, seq: 1, ..Frame::default() };
    newest.ch[0].resize(1024, 0.0);
    let raw = Box::new(crate::drawpipe::tests::Bytes(vec![0]));
    let fallback = pipe.feed_source(
        (raw, 1, 0..1),
        (identity, Instant::now()),
        (decode, Some(Arc::new(|_, _, _| None))),
        (Some(Arc::new(newest)), None),
        true,
    );
    assert!(fallback.is_none());
    crate::drawpipe::host::finish_batch(&pipe, 1).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert_eq!(pipe.take_waveform((128, 64)).unwrap().seq, 1);
}
