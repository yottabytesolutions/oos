// SPDX-License-Identifier: GPL-2.0-or-later
//! Preflight bounds and backing preparation keep publication and sampling decisions unchanged.
use super::*;
impl super::Prepared {
    pub(super) fn reuse(slot: &mut Option<Box<Self>>, v: &View, index: usize) -> Box<Self> {
        let mut p = slot.take().unwrap_or_else(|| {
            Box::new(Self {
                index,
                decoded_dots: false,
                plan: None,
                codes: None,
                folds: crate::framestats::folds_for(&v.m),
                measured: false,
                refine: Duration::ZERO,
                screen: Default::default(),
                zoom: None,
            })
        });
        p.index = index;
        for (_, fold) in &mut p.folds {
            *fold = Default::default();
        }
        p
    }
}

pub(super) struct Start {
    pub(super) first: usize,
    pub(super) sampled: bool,
}
pub(super) struct Prepared {
    pub(super) deposit: Deposit,
    pub(super) paint: tiles::Context,
}
pub(super) enum Preparation {
    Ready(Prepared),
    Retain(Option<Deposit>),
}
pub(super) fn batch(
    painter: &mut Painter,
    current: Option<Deposit>,
    slot: &Handoff,
    b: &Arc<Batch>,
    v: &Arc<View>,
    identity: Option<oos_core::change::Identity>,
    start: Start,
) -> Preparation {
    let slices = (start.first..b.frames).step_by(SLICE);
    let prior_hits = current.as_ref().map_or(0, |d| d.extent.hits());
    #[cfg(test)]
    let prior_hits = b.header.trace.as_deref().and_then(route::RouteTrace::replay_prior_hits).unwrap_or(prior_hits);
    let frames = b.frames - start.first;
    // Lines use one span per column or a fallback resampled to POINTS, including any newest replacement.
    // The latter bounds every tile of either route for this claimed view.
    let line_hits = || v.record_hit_bound().saturating_mul(frames as u64);
    let preflight = frames <= SLICE || v.m.dots_at(0.0) || line_hits() > HIT_LIMIT.saturating_sub(prior_hits);

    let started = (start.sampled && preflight && frames > 0).then(|| painter.sampler.start()).flatten();
    let bound = if preflight { painter.plan(b, v, slices.clone()) } else { (0, CountFormat::Narrow) };

    let placement = started.map(|t| t.elapsed());
    if bound.0 > bound.1.hit_limit() {
        let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
        b.counters.discard(Lost::Budget, records);
        let _ = painter.send_tap(slot, b, TapProjection::Unpainted, Some(v), identity);
        return Preparation::Retain(current);
    }
    let tap = painter.send_tap(slot, b, TapProjection::Projected { gen: v.gen, size: (v.w, v.h) }, Some(v), identity);

    let Some(mut d) = deposit(current, slot, b, v, identity, bound) else {
        return Preparation::Retain(None);
    };

    if let Some(number) = tap {
        d.include_tap(number);
    }
    Preparation::Ready(Prepared {
        deposit: d,
        paint: tiles::Context { start: start.first, preflight, placement, sampled: start.sampled },
    })
}
pub(super) fn deposit(
    mut current: Option<Deposit>,
    slot: &Handoff,
    b: &Batch,
    v: &View,
    identity: Option<oos_core::change::Identity>,
    (hits, format): (u64, CountFormat),
) -> Option<Deposit> {
    if current.as_ref().is_some_and(|d| !d.matches(v.gen, v.w, v.h) || !same_pair(d.identity, b.header.identity())) {
        let old = current.take().expect("the preceding check found a deposit");
        old.reject(identity, b.counters);
        slot.recycle(old);
    }
    if current.as_ref().is_some_and(|d| d.format() != format || d.extent.hits() + hits > format.hit_limit()) {
        let full = current.take().expect("the weight limit applies to an existing deposit");
        if let Err(full) = slot.send(full) {
            full.reject(identity, b.counters);
            return None;
        }
    }
    if current.is_none() {
        current = slot.acquire_picture(v.gen, v.picture_sizes(), b.header.identity(), v.m.ch.each_ref().map(|c| c.on));
    }
    let deposit = current.as_mut()?;
    deposit.set_format(format);
    if b.header.identity().map(|id| id.stats_gen()) > deposit.identity.map(|id| id.stats_gen()) {
        Counters::add(&b.counters.losses.folds, deposit.folds.iter().map(|(_, f)| f.n).sum());
        deposit.folds.clear();
        deposit.identity = b.header.identity();
    }
    current
}
