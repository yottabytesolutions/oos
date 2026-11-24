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
    let remaining = b.next.load(Ordering::Relaxed) < b.frames;
    let v = batch_view(b, &state, reset_before);
    let identity = state.identity;
    let reader = v.is_none().then(|| state.view.clone()).flatten();
    drop(state);
    let Some(v) = v else {
        if remaining {
            b.counters.metrics.discarded(b.header.identity(), clock);
        }
        if same_pair(b.header.identity(), identity) {
            let _ = painter.send_tap(slot, b, TapProjection::Unpainted, reader.as_deref(), identity);
        }
        return None;
    };
    Some((v, identity))
}
