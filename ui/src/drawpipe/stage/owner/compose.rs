// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::{drawpipe::clock::BatchClocks, picture::deposit::count::CountFormat};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Composition {
    clocks: [BatchClocks; PICTURE_INPUTS],
    inputs: usize,
    merge: Duration,
    rebuild: Duration,
}

struct Picture {
    view: Option<Arc<View>>,
    identity: Option<oos_core::change::Identity>,
    mapping: u64,
    sizes: PerMap<(usize, usize)>,
}
impl Picture {
    fn mapping_matches(&self, d: &Deposit) -> bool {
        d.sizes() == self.sizes && self.view.as_ref().is_some_and(|v| d.matches(v.gen, v.w, v.h))
    }
    fn retained(&self, d: &Deposit) -> bool {
        self.mapping_matches(d) && d.identity.map(|i| i.epoch()) == self.identity.map(|i| i.epoch())
    }
}
fn append_accepted(total: &mut Accepted, next: Accepted) {
    #[cfg(feature = "private-picture-counts")]
    if next.frames > 0 {
        total.oldest =
            if total.frames == 0 { next.oldest } else { total.oldest.zip(next.oldest).map(|(a, b)| a.min(b)) };
    }
    total.frames += next.frames;
    total.triggered += next.triggered;
    total.stale += next.stale;
    total.folds.extend(next.folds);
    total.newest = total.newest.max(next.newest);
}
impl OwnerContext {
    fn eligible(&self, d: Deposit, p: &Picture) -> Option<Deposit> {
        let same_acquisition = same_pair(d.identity, p.identity);
        if !p.mapping_matches(&d) || !same_acquisition {
            // A fresh tap from this acquisition can still draw with the new projection.
            let zoom_toggled = d.sizes().zoom().is_some() != p.sizes.zoom().is_some();
            if !same_acquisition || zoom_toggled {
                self.slot.settle_taps(&d.taps, None);
            }
            d.reject(p.identity, self.counters);
            self.slot.recycle(d);
            return None;
        }
        if d.records == 0 {
            self.slot.settle_taps(&d.taps, None);
            self.slot.recycle(d);
            return None;
        }
        Some(d)
    }
    fn discard(&self, deposit: Deposit) {
        Counters::add(&self.counters.dropped, deposit.records);
        deposit.reject_timing(self.counters, Instant::now());
        deposit.discard_folds(self.counters);
        self.slot.settle_taps(&deposit.taps, None);
        self.slot.recycle(deposit);
    }
    fn accept(&self, d: &mut Deposit, p: &Picture, stats: &mut Stats<'_>) -> Accepted {
        let accepted = crate::picture::accept_deposit(
            d,
            Stats { edges: &mut *stats.edges, off: &mut *stats.off },
            p.identity,
            self.counters,
        );
        self.slot.settle_taps(&d.taps, (accepted.frames > 0).then_some(p.mapping));
        if self.counters.diagnostics_enabled() {
            self.counters.timing.picture(d.progress.sample, accepted.newest.map(|t| t.elapsed()));
        }
        accepted
    }
}
impl Owner {
    fn selected(&mut self) -> usize {
        let (first, rest) = self.queue.make_contiguous().split_first().expect("prepare left a valid head");
        let mut refs = [first; PICTURE_INPUTS - 1];
        let mut count = 0;
        for other in rest.iter().take(PICTURE_INPUTS - 1) {
            refs[count] = other;
            if first.picture_totals(&refs[..=count]).is_none() {
                break;
            }
            count += 1;
        }
        count
    }
    fn commit(
        &mut self,
        ctx: &OwnerContext,
        picture: &Picture,
        stats: &mut Stats<'_>,
        window: &mut DepositWindow,
        timing: &mut Composition,
    ) -> Accepted {
        let mut accepted = Accepted::default();
        if self.queue.is_empty() {
            return accepted;
        }
        let count = self.selected();
        if let Some(mut first) = self.queue.pop_front() {
            // The complete subset has passed geometry, full identity, metadata and hit bounds before any credit.
            let mut records = first.records;
            let mut hits = first.extent.hits();
            append_accepted(&mut accepted, ctx.accept(&mut first, picture, stats));
            timing.clocks[0] = std::mem::take(first.extent.clocks());
            timing.inputs = count + 1;
            for clocks in timing.clocks.iter_mut().skip(1).take(count) {
                let mut other = self.queue.pop_front().expect("preflight selected this entry");
                append_accepted(&mut accepted, ctx.accept(&mut other, picture, stats));
                *clocks = std::mem::take(other.extent.clocks());
                let start = Instant::now();
                assert!(first.merge_picture_planes(&[&other]).is_some(), "whole subset preflight passed");
                timing.merge += start.elapsed();
                records += other.records;
                hits += other.extent.hits();
                if let Some(t) = other.extent.newest() {
                    first.extent.include(t);
                }
                ctx.slot.recycle(other);
            }
            first.records = records;
            first.extent.add_hits(hits - first.extent.hits());
            window.push(first, &ctx.slot);
        }
        accepted
    }

    fn prepare(&mut self, ctx: &OwnerContext, picture: &Picture, started: Instant) -> Option<Duration> {
        let mut rejected_wait = None;
        for _ in 0..self.queue.len() {
            let mut deposit = self.queue.pop_front().expect("bounded existing input");
            let clocks = deposit.extent.clocks();
            clocks.claimed(started);
            let wait = clocks.longest_wait(started);
            let Some(deposit) = ctx.eligible(deposit, picture) else {
                rejected_wait = rejected_wait.max(wait);
                continue;
            };
            if deposit.picture_totals(&[]).is_none() {
                ctx.counters.lose(Lost::Error, deposit.records);
                ctx.discard(deposit);
                rejected_wait = rejected_wait.max(wait);
            } else {
                self.queue.push_back(deposit);
            }
        }
        rejected_wait
    }

    fn blend(
        &mut self,
        ctx: &OwnerContext,
        picture: &Picture,
        mut maps: PerMap<&mut Phosphor>,
        clear: &mut bool,
        stats: &mut Stats<'_>,
        timing: &mut Composition,
    ) -> Accepted {
        let mut accepted = Accepted::default();
        let queued = self.queue.len();
        for (i, mut deposit) in self.queue.drain(..).enumerate() {
            let Some(clock) = timing.clocks.get_mut(timing.inputs + i) else {
                Counters::add(&ctx.counters.losses.overflow, deposit.records);
                ctx.discard(deposit);
                continue;
            };
            append_accepted(&mut accepted, ctx.accept(&mut deposit, picture, stats));
            *clock = std::mem::take(deposit.extent.clocks());
            let start = Instant::now();
            deposit.blend_picture(&mut maps, std::mem::take(clear));
            timing.merge += start.elapsed();
            ctx.slot.recycle(deposit);
        }
        debug_assert!(queued + timing.inputs <= timing.clocks.len(), "owner queue exceeds its backing capacity");
        accepted
    }

    pub(super) fn compose(
        &mut self,
        mut maps: PerMap<&mut Phosphor>,
        clear: &mut bool,
        mut stats: Stats<'_>,
        window: Option<&mut DepositWindow>,
    ) -> Accepted {
        let started = Instant::now();
        let Some(ctx) = self.context.clone() else { return Accepted::default() };
        let state = oos_hal::sync::lock(&ctx.view);
        let picture = Picture {
            view: state.view.clone(),
            identity: state.identity,
            mapping: state.mapping_gen,
            sizes: maps.sizes(),
        };
        drop(state);

        let rejected_wait = self.prepare(&ctx, &picture, started);
        let mut timing = Composition::default();
        let accepted = if let Some(window) = window {
            let invalidated = window.remove_invalid(&ctx.slot, |d| picture.retained(d));
            let mut accepted = self.commit(&ctx, &picture, &mut stats, window, &mut timing);
            if accepted.frames == 0 {
                window.idle();
            }
            if invalidated || accepted.frames > 0 {
                *clear = false;
                let start = Instant::now();
                let retained = window.rebuild_picture(maps.as_mut());
                timing.rebuild = start.elapsed();
                // A format or hit bound can prevent merging all fresh difference planes into one set.
                // Fold the remainder directly into this picture, using wide counts for the combined footprint.
                let mut visible = retained;
                if accepted.frames > 0 && !self.queue.is_empty() {
                    let (main, zoom) = maps.as_mut().into_parts();
                    main.set_format(CountFormat::Wide);
                    if let Some(zoom) = zoom {
                        zoom.set_format(CountFormat::Wide);
                    }
                    let remainder = self.blend(&ctx, &picture, maps, clear, &mut stats, &mut timing);
                    visible += remainder.frames;
                    append_accepted(&mut accepted, remainder);
                }
                accepted.window_frames = Some(visible);
            }
            accepted
        } else {
            self.blend(&ctx, &picture, maps, clear, &mut stats, &mut timing)
        };
        let elapsed = started.elapsed();
        ctx.counters.metrics.composed(picture.identity, &timing.clocks);
        if accepted.window_frames.is_some() || accepted.frames > 0 || rejected_wait.is_some() {
            let queued = timing.clocks.iter().filter_map(|clocks| clocks.longest_wait(started)).max();
            crate::pacing::note_composition(queued, rejected_wait, timing.merge, timing.rebuild, elapsed);
        }
        accepted
    }
}

#[cfg(all(test, feature = "private-picture-counts"))]
mod count_tests {
    use super::*;
    #[test]
    fn positive_oldest_is_minimum_unknown_sticky_and_zero_is_ignored() {
        let now = Instant::now();
        let older = now - std::time::Duration::from_millis(1);
        let mut total = Accepted::default();
        append_accepted(&mut total, Accepted { frames: 32, oldest: Some(now), ..Default::default() });
        append_accepted(&mut total, Accepted { frames: 32, oldest: Some(older), ..Default::default() });
        assert_eq!((total.frames, total.oldest), (64, Some(older)));
        append_accepted(&mut total, Accepted::default());
        assert_eq!(total.oldest, Some(older));
        append_accepted(&mut total, Accepted { frames: 1, oldest: None, ..Default::default() });
        append_accepted(&mut total, Accepted { frames: 1, oldest: Some(now), ..Default::default() });
        assert_eq!((total.frames, total.oldest), (66, None));
    }
}
