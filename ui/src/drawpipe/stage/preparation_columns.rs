// SPDX-License-Identifier: GPL-2.0-or-later
//! Use idle placement workers for the column ranges of a single deep record.

use super::*;
use crate::fastdraw::Spans;
use crate::picture::deposit::envelope::{self, CodePlacement, ColumnEdges};
use crate::trignear::{SearchResult, WideRefine, WideSearch, WideShard};
use kernel::{Projection, SamplesRef};
use route::Route;
use std::ops::Range;

const SPLIT_SAMPLES: usize = 64 * 1024;

pub(super) struct Columns {
    workers: usize,
    decoded: Option<Arc<Frame>>,
    shared_codes: Arc<SharedCodes>,
    tasks: Vec<Option<Box<Task>>>,
    completed: usize,
}

struct Dispatch<'a> {
    pool: &'a mut Pool<Job, super::average::Scratch>,
    jobs: &'a mut Vec<Job>,
    worker_time: Duration,
    wall: Duration,
}

impl Painter {
    pub(super) fn place_underfilled(&mut self, b: &Arc<Batch>, v: &Arc<View>, first: usize, count: usize) {
        let arena = Arc::get_mut(&mut self.arena).expect("all tile jobs returned their arena references");
        arena.resize_with(arena.len().max(count), || None);
        let mut dispatch =
            Dispatch { pool: &mut self.pool, jobs: &mut self.jobs, worker_time: Duration::ZERO, wall: Duration::ZERO };
        let started = Instant::now();
        let mut refine = Duration::ZERO;
        for (index, slot) in (first..first + count).zip(&mut arena[..count]) {
            let mut p = Prepared::reuse(slot, v, index);
            #[cfg(test)]
            let _counters = b.header.trace.as_ref().map(|_| test_scope::WorkerCounters::enter(b.counters));
            let (_, marks) = fallback::capture(|| {
                #[cfg(test)]
                fixture_bench::record(index, || self.columns.place(b, v, &mut p, &mut dispatch));
                #[cfg(not(test))]
                self.columns.place(b, v, &mut p, &mut dispatch);
            });
            *p.fallbacks.get_mut() = marks;
            refine += p.refine;
            *slot = Some(p);
        }
        let elapsed = started.elapsed();
        let coordinator = elapsed.saturating_sub(dispatch.wall);
        self.cost.refine += refine;
        self.cost.envelope += (dispatch.worker_time + coordinator).saturating_sub(refine);
        self.cost.pool += elapsed;
    }
}

impl Columns {
    pub(super) fn new(workers: usize) -> Self {
        Self { workers, decoded: None, shared_codes: Arc::default(), tasks: Vec::new(), completed: 0 }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn workers(&self) -> usize {
        self.workers
    }

    pub(super) fn should_split(&self, b: &Batch, v: &View, count: usize) -> bool {
        count < self.workers
            && v.zoom.is_none()
            && v.w > 1
            && v.m.acq != AcqMode::HiRes
            && b.fb / size_of::<u16>() > SPLIT_SAMPLES
    }

    fn place(&mut self, b: &Arc<Batch>, v: &Arc<View>, p: &mut Prepared, dispatch: &mut Dispatch<'_>) {
        self.completed = 0;
        #[cfg(test)]
        for task in self.tasks.iter_mut().flatten() {
            task.allocations = 0;
        }
        p.refine = Duration::ZERO;
        p.zoom = None;
        let previous = p.plan.take().and_then(|frame| frame.plan.recycle(&mut p.screen));
        let mut scratch = self.decoded.take().unwrap_or_default();
        let bytes = chunk(b, p.index);
        let (frame, placement) =
            placement_frame(b, v, p, Arc::get_mut(&mut scratch).expect("previous columns returned"), bytes);
        if placement.words.is_some_and(|(_, raw, _)| {
            matches!(placement.route, Route::Dense | Route::Peak)
                && !placement.projection.dots
                && bytes.len() / size_of::<u16>() / raw.stride > SPLIT_SAMPLES
        }) {
            p.decoded_dots = false;
            p.plan = Some(self.codes(b, v, p, (frame, placement), previous, dispatch));
        } else if let Some(grid) = ScreenGrid::for_placement(frame, &placement) {
            Cause::SplitDecodedScreen.mark();
            let partial = frame.partial;
            let retained = b.newest.as_ref().filter(|newest| std::ptr::eq(frame, &***newest));
            let source = retained.unwrap_or(&scratch).clone();
            let mut fallback =
                |search, fits: &dyn Fn(f64) -> bool| self.wide_trigger(dispatch, &source, v.m.trig_src, search, fits);
            let wide: Option<&mut WideRefine<'_>> = (placement.route == Route::Pairs).then_some(&mut fallback);
            let (ti, outcome) =
                standard::screen_trigger(&v.m, &source, &placement.projection, wide, Some(&mut p.refine));
            let triggered = outcome != kernel::TrigOutcome::NotFound;
            p.plan = Some(self.screen(source, placement, grid, (ti, triggered, partial), &mut p.screen, dispatch));
            p.decoded_dots = false;
        } else {
            finish_placement(v, p, frame, placement, previous);
        }
        self.decoded = Some(scratch);
    }

    fn wide_trigger(
        &mut self,
        dispatch: &mut Dispatch<'_>,
        frame: &Arc<Frame>,
        channel: usize,
        search: WideSearch,
        fits: &dyn Fn(f64) -> bool,
    ) -> SearchResult {
        let samples = &frame.ch[channel];
        let windows = search.windows(samples.len());
        if windows < SPLIT_SAMPLES {
            return search.serial(samples, fits);
        }
        self.dispatch(dispatch, Source::Trigger(frame.clone(), channel, search), windows);
        search.resolve(samples, self.tasks[..self.completed].iter_mut().flatten().map(|task| &mut task.trigger), fits)
    }

    fn codes(
        &mut self,
        b: &Arc<Batch>,
        v: &View,
        p: &mut Prepared,
        (frame, placement): (&Frame, standard::BatchPlacement<'_>),
        previous: Option<kernel::RawTrace>,
        dispatch: &mut Dispatch<'_>,
    ) -> standard::FastFrame {
        let standard::BatchPlacement { projection, route, words } = placement;
        let (bytes, raw, layout) = words.expect("only complete code routes enter column placement");
        let samples = code_samples(bytes, raw);
        let meta = standard::frame_timing(&v.m, frame);
        let source = &samples[v.m.trig_src];
        let trigger = standard::batch_trigger(&v.m, frame, &projection, &meta, (source, route), Some(&mut p.refine));
        let placement = CodePlacement::new(layout, projection, trigger.0);
        let shared = Arc::get_mut(&mut self.shared_codes).expect("previous columns returned");
        let columns = placement.update_edges(&samples, &mut shared.edges);
        let placed = if columns > 1 {
            shared.placement = Some((raw, placement));
            let source = Source::Codes(b.clone(), p.index, self.shared_codes.clone());
            self.dispatch(dispatch, source, columns);
            let mut spans = previous.map_or_else(Default::default, |trace| trace.spans);
            for (c, span) in spans.iter_mut().enumerate() {
                let SamplesRef::Codes(codes) = samples[c] else { unreachable!("code source supplies code views") };
                if projection.ch[c].is_none() || codes.n == 0 {
                    *span = None;
                    continue;
                }
                let span = span.get_or_insert_with(Spans::default);
                fixture_stage!(
                    Split,
                    join_spans(
                        span,
                        self.results()
                            .map(|task| task.spans[c].as_ref().expect("the active lane has spans in every column job")),
                    )
                );
            }
            let (trigger_at, outcome) = trigger;
            let trace = kernel::RawTrace { trigger_at, outcome, spans };
            kernel::Placed { projection, route, trace, fold: [None; NCH] }
        } else {
            kernel::place_at(samples, &projection, (route, Some(layout)), trigger, previous)
        };
        standard::FastFrame::placed(placed, &meta)
    }

    fn screen(
        &mut self,
        frame: Arc<Frame>,
        placement: standard::BatchPlacement<'_>,
        grid: ScreenGrid,
        (ti, triggered, partial): (f64, bool, bool),
        points: &mut [Vec<f32>; NCH],
        dispatch: &mut Dispatch<'_>,
    ) -> standard::FastFrame {
        let pairs = grid.points.div_ceil(2);
        self.dispatch(dispatch, Source::Screen(frame, grid, ti), pairs);
        for (c, points) in points.iter_mut().enumerate() {
            fixture_stage!(Split, join_screen(points, self.results().map(|task| task.screen[c].as_slice())));
        }
        let rec = crate::wave::Record {
            t0: grid.first,
            dt: grid.dt,
            ch: std::mem::take(points),
            triggered,
            partial,
            raw: None,
            la: None,
            pairs: true,
        };
        let mut plan = standard::FastFrame::record(rec, placement.projection, [None; NCH]);
        plan.route = placement.route;
        plan
    }

    fn dispatch(&mut self, dispatch: &mut Dispatch<'_>, source: Source, columns: usize) {
        #[cfg(test)]
        let started = fixture_bench::split_dispatch_start();
        let count = self.workers.min(columns);
        // Refinement includes the whole dispatch in its own wall measurement.
        let time_shards = !matches!(source, Source::Trigger(..));
        self.tasks.resize_with(self.tasks.len().max(count), || None);
        for (i, slot) in self.tasks[..count].iter_mut().enumerate() {
            let mut task = slot.take().unwrap_or_default();
            task.index = i;
            task.columns = columns * i / count..columns * (i + 1) / count;
            task.source = Some(source.clone());
            dispatch.jobs.push(Job::Columns(task));
        }
        let timing = dispatch.pool.map(dispatch.jobs, time_shards);
        if time_shards {
            dispatch.wall += timing.wall;
            dispatch.worker_time += timing.worker_time;
        }
        for job in dispatch.jobs.drain(..) {
            let Job::Columns(task) = job else { unreachable!("only column jobs were dispatched") };
            fallback::merge(task.fallbacks);
            let index = task.index;
            self.tasks[index] = Some(task);
        }
        self.completed = count;
        #[cfg(test)]
        if let Some(started) = started.filter(|_| time_shards) {
            fixture_bench::note_split_dispatch(started, timing.worker_time);
        }
    }

    fn results(&self) -> impl Iterator<Item = &Task> {
        self.tasks[..self.completed].iter().flatten().map(Box::as_ref)
    }

    #[cfg(test)]
    pub(super) fn task_allocations(&self) -> impl Iterator<Item = usize> + '_ {
        self.results().map(|task| task.allocations)
    }
}

fn join_screen<'a>(out: &mut Vec<f32>, parts: impl Iterator<Item = &'a [f32]>) {
    out.clear();
    for part in parts {
        out.extend_from_slice(part);
    }
}

fn join_spans<'a>(out: &mut Spans, parts: impl Iterator<Item = &'a Spans>) {
    out.rows.clear();
    for part in parts {
        if out.rows.is_empty() {
            out.c0 = part.c0;
        }
        out.rows.extend_from_slice(&part.rows);
    }
}

fn code_samples(bytes: &[u8], raw: CodeLayout) -> [SamplesRef<'_>; NCH] {
    std::array::from_fn(|c| SamplesRef::Codes(raw.channel(bytes, c)))
}

#[derive(Clone, Copy)]
struct ScreenGrid {
    points: usize,
    dt: f64,
    first: f64,
}

impl ScreenGrid {
    fn for_placement(frame: &Frame, placement: &standard::BatchPlacement<'_>) -> Option<Self> {
        let Projection { time_per_div, delay, .. } = placement.projection;
        let window = time_per_div * f64::from(HDIV);
        let points = crate::sparse::POINTS;
        let dt = window / points as f64;
        (matches!(placement.route, Route::Screen | Route::Pairs)
            && placement.route.screen_raster(placement.projection.dots, frame)
            && dt / frame.dt >= 2.0
            && frame.ch.iter().any(|ch| ch.len() > SPLIT_SAMPLES))
        .then_some(Self { points, dt, first: delay - window / 2.0 })
    }
}

#[derive(Clone)]
enum Source {
    Codes(Arc<Batch>, usize, Arc<SharedCodes>),
    Screen(Arc<Frame>, ScreenGrid, f64),
    Trigger(Arc<Frame>, usize, WideSearch),
}

#[derive(Default)]
struct SharedCodes {
    edges: ColumnEdges,
    placement: Option<(CodeLayout, CodePlacement)>,
}

#[derive(Default)]
pub(super) struct Task {
    source: Option<Source>,
    index: usize,
    columns: Range<usize>,
    spans: [Option<Spans>; NCH],
    screen: [Vec<f32>; NCH],
    trigger: WideShard,
    fallbacks: u64,
    #[cfg(test)]
    allocations: usize,
}

impl Task {
    pub(super) fn run(&mut self) {
        let (_, marks) = fallback::capture(|| {
            #[cfg(not(test))]
            self.reduce();
            #[cfg(test)]
            {
                self.allocations += crate::system::test_allocator::count_own(|| self.reduce()).0;
            }
        });
        self.fallbacks = marks;
    }

    fn reduce(&mut self) {
        match self.source.take().expect("a column job holds its source until it runs") {
            Source::Codes(batch, index, shared) => {
                let (raw, placement) = shared.placement.expect("code jobs share the prepared projection");
                let samples = code_samples(chunk(&batch, index), raw);
                crate::fastdraw::with_scratch(|scratch| {
                    scratch.envelope.place_columns(
                        placement,
                        &samples,
                        &shared.edges,
                        self.columns.clone(),
                        &mut self.spans,
                    );
                });
            }
            Source::Screen(frame, grid, ti) => {
                for (samples, out) in frame.ch.iter().zip(&mut self.screen) {
                    if samples.len() >= 2 {
                        fixture_stage!(
                            Resample,
                            envelope::adapt::pairs::screen_envelope_columns(
                                samples,
                                (frame.dt, grid.dt / frame.dt),
                                (grid.points, grid.dt, grid.first),
                                ti,
                                self.columns.clone(),
                                out,
                            )
                        );
                    } else {
                        out.clear();
                    }
                }
            }
            Source::Trigger(frame, channel, search) => {
                search.smooth(&frame.ch[channel], self.columns.clone(), &mut self.trigger);
            }
        }
    }
}
