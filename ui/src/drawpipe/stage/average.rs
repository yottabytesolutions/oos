// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::average::{self, Fold};
use std::ops::Range;

mod codes;

#[derive(Default)]
pub(super) struct Scratch {
    pub(super) frame: Frame,
    screen: [Vec<f32>; NCH],
}

#[derive(Default)]
pub(super) struct Partial {
    start: usize,
    fold: Fold,
    trigger: Option<(f64, bool)>,
}

impl Partial {
    pub(super) fn run(&mut self, scratch: &mut Scratch, b: &Batch, v: &View, range: Range<usize>) {
        self.start = range.start;
        self.fold.reset();
        self.trigger = None;
        let grid = crate::board::record::hw_grid(&v.m);
        let screen_dt = grid.1 / grid.0 as f64;
        for index in range {
            #[cfg(test)]
            let trigger = {
                let mut trigger = None;
                fixture_bench::record(index, || trigger = self.push(scratch, b, &v.m, (grid, screen_dt), index));
                trigger
            };
            #[cfg(not(test))]
            let trigger = self.push(scratch, b, &v.m, (grid, screen_dt), index);
            self.trigger = trigger;
            if trigger.is_none() {
                break;
            }
        }
    }

    fn push(
        &mut self,
        scratch: &mut Scratch,
        b: &Batch,
        m: &Model,
        (grid, screen_dt): ((usize, f64, f64), f64),
        index: usize,
    ) -> Option<(f64, bool)> {
        let (dt, pre, ti, triggered) = match codes::read(scratch, b, m, grid, screen_dt, index) {
            Ok(trigger) => trigger,
            Err(cause) => {
                b.counters.routes.fallbacks.add(cause.bit());
                decoded_screen(scratch, b, m, grid, screen_dt, index)?
            }
        };
        fixture_stage!(Accumulate, self.fold.push(&scratch.screen, m.avg_count));
        let tap_ti = newest_at(b, index)
            .filter(|tap| (tap.dt, tap.pre) != (dt, pre))
            .map_or(ti, |tap| tap.pre as f64 + (ti - pre as f64) * dt / tap.dt);
        Some((tap_ti, triggered))
    }
}

fn decoded_screen(
    scratch: &mut Scratch,
    b: &Batch,
    m: &Model,
    grid: (usize, f64, f64),
    screen_dt: f64,
    index: usize,
) -> Option<(f64, usize, f64, bool)> {
    let frame = decoded_newest_at(b, index).unwrap_or_else(|| {
        fixture_stage!(Decode, b.decode.decode(index, chunk(b, index), &mut scratch.frame));
        &scratch.frame
    });
    if !average::accepts_samples(screen_dt, frame.dt, frame.envelope) {
        return None;
    }
    let (points, window, first) = grid;
    let (ti, triggered) =
        fixture_stage!(Trigger, crate::board::record::record_trigger(frame, m, points, window, first));
    crate::board::record::resample_channels(frame, m, grid, ti, &mut scratch.screen);
    Some((frame.dt, frame.pre, ti, triggered))
}

pub(super) fn average_batch(
    painter: &mut Painter,
    b: &Arc<Batch>,
    v: &Arc<View>,
) -> (Option<average::Output>, crate::picture::deposit::pool::Timing) {
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
    let mut trigger = None;
    let mut compatible = true;
    for job in painter.jobs.drain(..) {
        if let Job::Average(_, _, _, partial) = job {
            trigger = partial.trigger;
            compatible &= trigger.is_some();
            if compatible {
                output.join(&partial.fold);
            }
            painter.averages.push(partial);
        }
    }
    let output = trigger.filter(|_| compatible).map(|trigger| {
        output.finish(trigger, crate::board::record::hw_grid(&v.m));
        output
    });
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
    if newest(b).is_none() {
        b.next.store(b.frames, Ordering::Relaxed);
        b.counters.discard(Lost::Error, records as u64);
        b.counters.metrics.discarded(b.header.identity(), clock);
        return;
    }
    let (output, timing) = average_batch(painter, b, v);
    b.next.store(b.frames, Ordering::Relaxed);
    let Some(mut output) = output else {
        b.counters.reject_batch(false, records as u64);
        b.counters.metrics.discarded(b.header.identity(), clock);
        return;
    };
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
#[cfg(test)]
pub(super) use tests::verify_codes;
