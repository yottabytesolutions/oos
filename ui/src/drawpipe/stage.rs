// SPDX-License-Identifier: GPL-2.0-or-later

use super::feed::batch_view;
use super::*;
use crate::claim_timing::Sampler;
use crate::picture::deposit::count::CountFormat;
use crate::picture::deposit::{kernel, pool::Pool, standard, DiffBand, Tap, HIT_LIMIT};
use crate::picture::deposit::{Deposit, Handoff, TapProjection};
use crate::{board::Frame, model::*};
#[cfg(test)]
use crate::{
    board::PictureEligibility,
    picture::{deposit::TapView, DepositWindow},
    sparse::Interp,
    wave::Phosphor,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod fixture_bench;
#[cfg(not(test))]
macro_rules! fixture_stage {
    ($stage:ident, $work:expr) => {
        $work
    };
    ($stage:ident, $channel:expr, $work:expr) => {
        $work
    };
}

#[cfg(test)]
macro_rules! fixture_stage {
    ($stage:ident, $work:expr) => {
        $crate::drawpipe::stage::fixture_bench::sample($crate::drawpipe::stage::fixture_bench::Stage::$stage, || $work)
    };
    ($stage:ident, $channel:expr, $work:expr) => {
        $crate::drawpipe::stage::fixture_bench::sample_channel(
            $crate::drawpipe::stage::fixture_bench::Stage::$stage,
            $channel,
            || $work,
        )
    };
}
pub(crate) use fixture_stage;

mod jobs;
use jobs::{run, Job};
mod preparation;
mod preparation_columns;
mod readout;
use readout::{chunk, decoded_newest_at, fold_codes, fold_decoded, newest, newest_at};
#[cfg(test)]
mod retirement_tests;

#[cfg(test)]
mod preparation_columns_tests;
#[cfg(test)]
mod tests;
mod tiles;
mod zoom;

struct Prepared {
    index: usize,
    decoded_dots: bool,
    plan: Option<standard::FastFrame>,
    codes: Option<CodeLayout>,
    folds: crate::framestats::Folds,
    measured: bool,
    refine: Duration,
    screen: [Vec<f32>; NCH],
    zoom: Option<ZoomPlan>,
}

#[derive(Default)]
struct ZoomPlan {
    plan: Option<standard::FastFrame>,
    screen: [Vec<f32>; NCH],
}

type Arena = Arc<Vec<Option<Box<Prepared>>>>;
pub(super) const SLICE: usize = 64;

impl View {
    pub(super) fn record_hit_bound(&self) -> u64 {
        let main = self.record_hit_bound_at(self.w);
        self.zoom.map_or(main, |zoom| main.max(self.zoom_hit_bound(zoom.size().0)))
    }

    pub(super) fn record_hit_bound_at(&self, columns: usize) -> u64 {
        let window = self.m.time_per_div() as f64 * HDIV as f64;
        let delay = if self.zoom.is_some() {
            self.m
                .deskew_ch
                .iter()
                .map(|&shift| (f64::from(self.m.delay) + shift).abs())
                .fold(f64::from(self.m.delay).abs(), f64::max)
        } else {
            f64::from(self.m.delay).abs()
        };
        standard::record_hit_bound(columns, crate::sparse::POINTS as f64, delay / window)
    }
}

impl Prepared {
    fn zoom_plan(&self) -> Option<&standard::FastFrame> {
        self.zoom.as_ref()?.plan.as_ref()
    }

    fn hit_bound(&self, main: &standard::FastFrame) -> u64 {
        self.zoom_plan().map_or(main.plan.hit_bound(), |zoom| main.plan.hit_bound().max(zoom.plan.hit_bound()))
    }
}

fn place(b: &Arc<Batch>, v: &Arc<View>, p: &mut Prepared, scratch: &mut Frame) {
    p.refine = Duration::ZERO;
    let bytes = chunk(b, p.index);
    if v.zoom.is_some() {
        return zoom::place(b, v, p, scratch, bytes);
    }
    p.zoom = None;
    let previous = p.plan.take().and_then(|frame| frame.plan.recycle(&mut p.screen));
    let (f, placement) = placement_frame(b, v, p, scratch, bytes);
    finish_placement(v, p, f, placement, previous);
}

fn finish_placement(
    v: &View,
    p: &mut Prepared,
    f: &Frame,
    placement: standard::BatchPlacement<'_>,
    previous: Option<kernel::RawTrace>,
) {
    use route::Route;
    let selected = placement.route;
    p.decoded_dots = selected == Route::Dots && p.codes.is_none();
    p.plan = Some(if selected.screen_raster(placement.projection.dots, f) {
        standard::screen_plan(&v.m, f, placement, v.interp, &mut p.screen, previous)
    } else {
        standard::batch_plan_timed(&v.m, f, placement, previous, Some(&mut p.refine))
    });
}

fn placement_frame<'frame, 'bytes>(
    b: &'frame Arc<Batch>,
    v: &Arc<View>,
    p: &mut Prepared,
    scratch: &'frame mut Frame,
    bytes: &'bytes [u8],
) -> (&'frame Frame, standard::BatchPlacement<'bytes>) {
    use route::Route;
    let newest = newest_at(b, p.index);
    // When the source supplies a code adapter, a failed code view still has a DMA decoder.
    // A separately read tap can be raw while the picture's batch is compressed.
    let decoded_newest = decoded_newest_at(b, p.index);
    p.codes = None;
    p.measured = false;
    let (words, decode) = feed::placement_input(b, p.index, bytes, scratch, decoded_newest.is_some());
    let f = if words.is_some() { &*scratch } else { decoded_newest.unwrap_or(scratch) };
    let (projections, code_geometry) = route::projection(v, f.dt);
    let projection = projections.main();
    let layout = words.as_ref().filter(|_| code_geometry).map(|(_, layout, _)| layout);
    let selected = route::route(f, &projection, v.m.acq, layout.is_some()).for_dma(&projection, layout);
    let raster = selected.screen_raster(projection.dots, f);
    let kept = words.filter(|_| code_geometry && !raster);
    let interpolation = code_geometry
        && selected == Route::Upsampled
        && !projection.dots
        && !f.envelope
        && layout.is_some_and(|raw| raw.reduced_capture() || raw.points(bytes) >= 16);
    let placement =
        standard::BatchPlacement { projection, route: selected, words: if interpolation { words } else { kept } };
    if let Some((bytes, raw, _)) = kept {
        p.codes = Some(raw);
        if newest.is_none() && !p.folds.is_empty() {
            p.measured = fold_codes(scratch, &v.m, (bytes, raw), &mut p.folds, decode);
        }
        if selected == Route::Boxcar && !route::hardware_trigger(&v.m) && !v.m.trig_on_la() {
            fixture_stage!(TriggerSource, {
                let source = &mut scratch.ch[v.m.trig_src];
                let codes = raw.channel(bytes, v.m.trig_src);
                source.clear();
                source.extend((0..codes.n).map(|i| crate::fastdraw::Samples::at(&codes, i)));
            });
        }
        return (scratch, placement);
    }
    if words.is_some() && decoded_newest.is_none() {
        decode(scratch);
    }
    let f = decoded_newest.unwrap_or(scratch);
    if newest.is_none() {
        p.measured = fold_decoded(f, &v.m, &mut p.folds);
    }
    (f, placement)
}

mod intake;
pub(super) use intake::work;

struct Painter {
    pool: Pool<Job, Frame>,
    columns: preparation_columns::Columns,
    arena: Arena,
    jobs: Vec<Job>,
    sampler: Sampler,
    tap_folds: crate::framestats::Folds,
    cost: metrics::Work,
}

impl Painter {
    fn prepare_statistics(&mut self, statistics: &mut Option<(u64, Option<u64>)>, b: &Batch, v: &View) {
        let key = (v.gen, b.header.identity().map(|id| id.stats_gen()));
        if *statistics != Some(key) {
            for p in Arc::get_mut(&mut self.arena).expect("all previous batch jobs returned").iter_mut().flatten() {
                p.folds = crate::framestats::folds_for(&v.m);
            }
            *statistics = Some(key);
        }
    }
    fn new(workers: usize, little: u64) -> Self {
        Self {
            pool: Pool::new(workers, little, run),
            columns: preparation_columns::Columns::new(workers),
            arena: Arena::default(),
            jobs: Vec::new(),
            sampler: Sampler::default(),
            tap_folds: Vec::new(),
            cost: metrics::Work::default(),
        }
    }

    fn send_tap(
        &mut self,
        slot: &Handoff,
        b: &Batch,
        projection: TapProjection,
        view: Option<&View>,
        identity: Option<oos_core::change::Identity>,
    ) -> Option<u64> {
        let frame = newest(b)?.clone();
        let reader = view.map(|v| (v, identity)).or_else(|| b.header.reader.as_ref().map(|(v, id)| (&**v, *id)));
        let folds = reader.and_then(|(v, id)| {
            crate::framestats::TapFolds::compute(&frame, &v.m, id.map(|id| id.stats_gen()), &mut self.tap_folds)
        });
        let tap = Tap { frame, identity: b.header.identity(), projection, folds };
        slot.publish_tap(tap, b.view_gen_at_submit, b.header.tap.as_ref(), b.header.order.as_ref())
    }

    #[cfg(test)]
    fn batch(
        &mut self,
        current: Option<Deposit>,
        slot: &Handoff,
        b: &Arc<Batch>,
        v: &Arc<View>,
        identity: Option<oos_core::change::Identity>,
    ) -> Option<Deposit> {
        self.batch_with_clock(current, slot, b, v, identity, b.clock)
    }

    fn batch_with_clock(
        &mut self,
        current: Option<Deposit>,
        slot: &Handoff,
        b: &Arc<Batch>,
        v: &Arc<View>,
        identity: Option<oos_core::change::Identity>,
        mut clock: clock::StageClock,
    ) -> Option<Deposit> {
        self.cost = metrics::Work::default();
        #[cfg(any(target_os = "linux", test))]
        if let Some(trace) = &b.header.trace {
            trace.claimed(v, self.columns.workers(), current.as_ref().map_or(0, |d| d.extent.hits()));
        }
        let start = b.next.load(Ordering::Relaxed);
        if start == b.frames {
            let _ = self.send_tap(slot, b, TapProjection::Unpainted, Some(v), identity);
            return current;
        }
        let pass = preparation::Start { first: start, sampled: b.counters.diagnostics_enabled() };
        let prepared = match preparation::batch(self, current, slot, b, v, identity, pass) {
            preparation::Preparation::Ready(prepared) => prepared,
            preparation::Preparation::Retain(current) => {
                // A budget discard consumes the batch. Unclaimed work is reported when the batch drops.
                if b.next.load(Ordering::Relaxed) == b.frames {
                    b.counters.metrics.discarded(b.header.identity(), clock);
                }
                return current;
            }
        };
        let mut d = prepared.deposit;
        let before = d.records;
        tiles::paint(self, b, v, &mut d, prepared.paint);

        b.next.store(b.frames, Ordering::Relaxed);
        let fed = clock.read_at();
        d.extent.include(fed);
        #[cfg(feature = "private-picture-counts")]
        d.extent.contributed(fed, d.records - before);

        let records = d.records - before;
        b.counters.publish(&d.progress, records);
        d.progress.measured = 0;
        clock.deposit();
        let truncated = d.extent.clocks().push(clock);
        b.counters.metrics.work(b.header.identity(), &self.cost, records, truncated);
        if let Some(flight) = oos_hal::sync::lock(&b.header.flight).as_mut() {
            let pool = self.cost.pool;
            #[cfg(test)]
            let pool = b.clock.pool_override().unwrap_or(pool);
            flight.worked(records, pool);
        }
        Some(d)
    }

    fn plan(
        &mut self,
        b: &Arc<Batch>,
        v: &Arc<View>,
        slices: impl DoubleEndedIterator<Item = usize>,
    ) -> (u64, CountFormat) {
        // Keep at most one slice of plans. Reuse the first slice when painting starts.
        let mut hits = 0u64;
        let mut format = CountFormat::Narrow;
        for first in slices.rev() {
            let count = self.place_slice(b, v, first);
            for p in self.arena[..count].iter().flatten() {
                if let Some(frame) = &p.plan {
                    hits = hits.saturating_add(p.hit_bound(frame));
                    if frame.plan.format() == CountFormat::Wide {
                        format = CountFormat::Wide;
                    }
                }
            }
        }
        (hits, format)
    }

    fn place_slice(&mut self, b: &Arc<Batch>, v: &Arc<View>, first: usize) -> usize {
        #[cfg(any(target_os = "linux", test))]
        if let Some(trace) = &b.header.trace {
            let before = b.counters.routes.wide_searches();
            let count = self.place_records(b, v, first);
            trace.add_wide_searches(b.counters.routes.wide_searches().wrapping_sub(before));
            return count;
        }
        self.place_records(b, v, first)
    }

    fn place_records(&mut self, b: &Arc<Batch>, v: &Arc<View>, first: usize) -> usize {
        let count = (b.frames - first).min(SLICE);
        if self.columns.should_split(b, v, count) {
            self.place_underfilled(b, v, first, count);
            return count;
        }
        let placed = Arc::get_mut(&mut self.arena).expect("all tile jobs returned their arena references");
        placed.resize_with(placed.len().max(count), || None);
        for (index, slot) in (first..first + count).zip(&mut placed[..count]) {
            self.jobs.push(Job::Place(b.clone(), v.clone(), Prepared::reuse(slot, v, index)));
        }
        let timing = self.pool.map(&mut self.jobs, true);
        let mut refine = Duration::ZERO;
        for (slot, job) in placed[..count].iter_mut().zip(self.jobs.drain(..)) {
            if let Job::Place(_, _, p) = job {
                refine += p.refine;
                *slot = Some(p);
            }
        }
        self.cost.refine += refine;
        self.cost.envelope += timing.worker_time.saturating_sub(refine);
        self.cost.pool += timing.wall;
        count
    }
}

pub(crate) mod owner;
