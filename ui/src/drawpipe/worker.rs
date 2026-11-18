// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// A worker: claims frames of each batch until none are left, draws them into its own maps, then hands them on.
pub(super) fn work(rx: Receiver<Arc<Batch>>, view: &Mutex<ViewState>, mail: &Mutex<Mail>) {
    let mut f = Frame::default();
    let mut spare: Option<Acc> = None;
    while let Ok(b) = rx.recv() {
        let (v, stamp, identity) = {
            let s = oos_hal::sync::lock(view);
            (s.view.clone(), feed::legacy_stamp(&s), s.identity)
        };
        let Some(v) = v.filter(|_| accepts(&b, stamp, identity)) else {
            // older settings or no fast path: the frames are dropped (counted when the batch goes)
            while let Some(r) = b.claim(b.frames.max(1)) {
                if !same_pair(b.header.identity(), identity) {
                    Counters::add(&b.counters.losses.pipe, r.len() as u64);
                }
                Counters::add(&b.counters.dropped, r.len() as u64);
            }
            continue;
        };
        let mut a = fresh_acc(spare.take().or_else(|| oos_hal::sync::lock(mail).spare.pop()), &v);
        a.identity = b.header.identity();
        while let Some(r) = b.claim(CLAIM) {
            draw_range(&b, r, &v, &mut a, &mut f, b.counters);
        }
        // handed on before the batch goes: once it is gone, its frames are in a mailbox
        if a.frames == 0 {
            spare = Some(a);
        } else {
            spare = publish(mail, a, b.counters);
        }
    }
}

/// Frames `r` of batch `b` decoded into `f` and drawn into `a`.
pub(super) fn draw_range(b: &Batch, r: Range<usize>, v: &View, a: &mut Acc, f: &mut Frame, counters: &Counters) {
    let Some(bytes) = b.lease.raw.as_ref().map(|x| x.bytes()) else { return };
    a.identity = b.header.identity();
    a.note(b.id, Some(b.fed));
    if crate::pacing::is_stale(b.header.cfg()) {
        a.stale += r.len() as u64;
    }
    for i in r {
        let t_frame = std::time::Instant::now();
        let chunk = &bytes[i * b.fb..(i + 1) * b.fb];
        // without per-frame measurements a large frame is drawn from its read-out words where the board has a view of them
        if a.folds.is_empty() && chunk.len() >= CODES_MIN_BYTES {
            let lay = b.codes.as_ref().and_then(|c| c(chunk, f)).filter(|l| l.stride <= CODES_MAX_STRIDE);
            b.header.tag(f);
            let st = Stats { edges: &mut a.edges, off: &mut a.off };
            if let Some(t) = lay.and_then(|l| draw_frame_codes(&mut a.ph, &v.m, f, chunk, &l, Some(st))) {
                a.triggered += t as u64;
                a.frames += 1;
                cost::add(Part::Frame, t_frame);
                continue;
            }
        }
        let t = std::time::Instant::now();
        b.decode.decode(i, chunk, f);
        b.header.tag(f);
        cost::add(Part::Decode, t);
        let st = Stats { edges: &mut a.edges, off: &mut a.off };
        a.triggered += draw_frame(&mut a.ph, &v.m, f, v.interp, Some(st)) as u64;
        if crate::framestats::fold_frame(f, &v.m, &mut a.folds) {
            Counters::add(&counters.measured, 1);
        }
        a.frames += 1;
        cost::add(Part::Frame, t_frame);
    }
}
