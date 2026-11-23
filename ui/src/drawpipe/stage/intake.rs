// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::sync::atomic::AtomicBool;

pub(in crate::drawpipe) fn work(
    rx: Receiver<Message>,
    (queued, wake_pending): (&AtomicUsize, &AtomicBool),
    view: &Mutex<ViewState>,
    slot: &Handoff,
    n: usize,
    little: u64,
    counters: &'static Counters,
) {
    let mut painter = Painter::new(n, little);
    let mut current: Option<Deposit> = None;
    let mut statistics = None;
    loop {
        if wake_pending.swap(false, Ordering::AcqRel) {
            current = current.and_then(|d| owner::publish_pending(slot, d, view, counters));
        }
        let Ok(message) = rx.recv() else { break };
        let dequeued = Instant::now();
        let state = oos_hal::sync::lock(view);
        let reset_before = slot.reset_before();
        if let Some(deposit) = current.take_if(|d| d.predates(reset_before)) {
            deposit.reject(state.identity, counters);
        }
        let b = match message {
            Message::Batch(b) => {
                queued.fetch_sub(1, Ordering::AcqRel);
                b
            }
            Message::Wake => continue,
            #[cfg(test)]
            Message::Barrier(reply) => {
                let _ = reply.send(());
                continue;
            }
        };
        let clock = b.clock.dequeue(dequeued);
        let Some((v, identity)) = claim_batch(&mut painter, slot, &b, state, reset_before, clock) else { continue };
        if crate::drawpipe::average::software(&v.m) {
            average::publish_average(&mut painter, slot, &b, &v, clock);
            continue;
        }
        painter.prepare_statistics(&mut statistics, &b, &v);
        let Some(deposit) = painter.batch_with_clock(current.take(), slot, &b, &v, identity, clock) else {
            if slot.closed() {
                break;
            }
            continue;
        };
        current = owner::publish_completed(slot, b, deposit, view, counters);
    }
    if let Some(deposit) = current {
        deposit.reject(oos_hal::sync::lock(view).identity, counters);
    }
}

fn claim_batch(
    painter: &mut Painter,
    slot: &Handoff,
    b: &Batch,
    state: std::sync::MutexGuard<'_, ViewState>,
    reset_before: u64,
    clock: clock::StageClock,
) -> Option<(Arc<View>, Option<oos_core::change::Identity>)> {
    let v = batch_view(b, &state, reset_before);
    let identity = state.identity;
    let reader = v.is_err().then(|| state.view.clone()).flatten();
    drop(state);
    match v {
        Ok(v) => Some((v, identity)),
        Err(lost) => {
            if lost != 0 {
                b.counters.metrics.discarded(b.header.identity(), clock);
            }
            if same_pair(b.header.identity(), identity) {
                let projection = if lost != 0 { TapProjection::Recoverable } else { TapProjection::Unpainted };
                let _ = painter.send_tap(slot, b, projection, reader.as_deref(), identity);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::tests::Bytes;

    #[test]
    fn rejected_intake_tap_uses_the_same_count_as_the_loss() {
        for first in [1, 3] {
            let counters = Box::leak(Box::<Counters>::default());
            let pipe = Pipe::start(0, 0, counters);
            let id = Default::default();
            pipe.publish_view(&Model::default(), 8, 8, Interp::Exact, id);
            let decode: DecodeInto = Arc::new(|_, _| unreachable!("rejected before placement"));
            let mut batch = pipe.feed_batch(Box::new(Bytes(vec![0; 3])), 1, first..3, (0, Some(id)), decode, None);
            Arc::get_mut(&mut batch).unwrap().newest = Some(Arc::new(Frame { seq: 3, ..Default::default() }));
            let mut painter = Painter::new(0, 0);
            let reset_before = batch.view_gen_at_submit + 1;
            let view = oos_hal::sync::lock(&pipe.view);
            assert!(claim_batch(&mut painter, &pipe.slot, &batch, view, reset_before, batch.clock).is_none());
            assert_eq!(batch.next.load(Ordering::Relaxed), 3);
            let expected = if first < 3 { PictureEligibility::Recoverable } else { PictureEligibility::Drawable };
            assert_eq!(pipe.take_waveform((8, 8)).unwrap().eligibility(), expected);
            drop(batch);
            let text = counters.records_text();
            assert!(text.contains(&format!("lost.settings={} ", 3 - first)), "{text}");
            assert!(text.contains("lost.unclaimed=0 "), "{text}");
        }
    }
}
