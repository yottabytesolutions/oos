// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::average::{self, Fold};
use std::ops::Range;

#[derive(Default)]
pub(super) struct Scratch {
    pub(super) frame: Frame,
    screen: [Vec<f32>; NCH],
}

#[derive(Default)]
pub(super) struct Partial {
    start: usize,
    fold: Fold,
    trigger: (f64, bool),
}

impl Partial {
    pub(super) fn run(&mut self, scratch: &mut Scratch, b: &Batch, v: &View, range: Range<usize>) {
        self.start = range.start;
        self.fold.reset();
        for index in range {
            #[cfg(test)]
            fixture_bench::record(index, || self.push(scratch, b, v, index));
            #[cfg(not(test))]
            self.push(scratch, b, v, index);
        }
    }

    fn push(&mut self, scratch: &mut Scratch, b: &Batch, v: &View, index: usize) {
        let frame = newest_at(b, index).unwrap_or_else(|| {
            fixture_stage!(Decode, b.decode.decode(index, chunk(b, index), &mut scratch.frame));
            &scratch.frame
        });
        let (points, window, first) = crate::board::record::hw_grid(&v.m);
        self.trigger =
            fixture_stage!(Trigger, crate::board::record::record_trigger(frame, &v.m, points, window, first));
        crate::board::record::resample_channels(
            frame,
            &v.m,
            (points, window, first),
            self.trigger.0,
            &mut scratch.screen,
        );
        fixture_stage!(Accumulate, self.fold.push(&scratch.screen, v.m.avg_count));
    }
}

pub(super) fn average_batch(
    painter: &mut Painter,
    b: &Arc<Batch>,
    v: &Arc<View>,
) -> (average::Output, crate::picture::deposit::pool::Timing) {
    let first = b.next.load(Ordering::Relaxed);
    let records = b.frames - first;
    let width = records.div_ceil(painter.pool.shards()).max(1);
    for start in (first..b.frames).step_by(width) {
        let partial = painter.averages.pop().unwrap_or_default();
        painter.jobs.push(Job::Average(b.clone(), v.clone(), start..(start + width).min(b.frames), partial));
    }
    let mut timing = painter.pool.map(&mut painter.jobs, true);
    let combining = Instant::now();
    // The pool returns its inline shard first. Restore capture order before composing.
    painter.jobs.sort_unstable_by_key(|job| match job {
        Job::Average(_, _, _, partial) => partial.start,
        _ => unreachable!("only average jobs were dispatched"),
    });
    let mut output = painter.average_storage.take(b.counters);
    let mut trigger = (0.0, false);
    for job in painter.jobs.drain(..) {
        if let Job::Average(_, _, _, partial) = job {
            output.join(&partial.fold);
            trigger = partial.trigger;
            painter.averages.push(partial);
        }
    }
    output.finish(trigger, crate::board::record::hw_grid(&v.m));
    timing.wall += combining.elapsed();
    (output, timing)
}

pub(super) fn publish_average(
    painter: &mut Painter,
    slot: &Handoff,
    b: &Arc<Batch>,
    v: &Arc<View>,
    mut clock: clock::StageClock,
) {
    let records = b.frames - b.next.load(Ordering::Relaxed);
    if records == 0 {
        return;
    }
    if !newest(b).is_some_and(|f| average::accepts_grid(&v.m, f.dt, f.envelope)) {
        b.next.store(b.frames, Ordering::Relaxed);
        b.counters.discard(Lost::Error, records as u64);
        b.counters.metrics.discarded(b.header.identity(), clock);
        return;
    }
    let (mut output, timing) = average_batch(painter, b, v);
    b.next.store(b.frames, Ordering::Relaxed);
    clock.deposit();
    output.publish_clock(b.header.identity(), clock);
    let started = Instant::now();
    let published = painter.send_tap(slot, b, TapProjection::Average(output), Some(v), b.header.identity());
    if published.is_some() {
        b.counters.routes.add(route::Route::Average, records as u64);
        #[cfg(any(target_os = "linux", test))]
        if let Some(trace) = &b.header.trace {
            trace.claimed(v, painter.pool.shards(), 0);
            trace.count(route::Route::Average, records as u64);
        }
        let work = metrics::Work { pool: timing.wall, envelope: timing.worker_time, ..Default::default() };
        b.counters.metrics.work(b.header.identity(), &work, records as u64, false);
    }
    if let Some(flight) = oos_hal::sync::lock(&b.header.flight).as_mut() {
        flight.worked(records as u64, timing.wall);
        if published.is_some() {
            flight.published(started.elapsed());
        }
    }
}

#[cfg(test)]
mod tests;
