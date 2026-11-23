// SPDX-License-Identifier: GPL-2.0-or-later
//! Per-slice placement, tile dispatch and accounting share the original sampled eligibility.
use super::*;
pub(super) struct Context {
    pub(super) start: usize,
    pub(super) preflight: bool,
    pub(super) placement: Option<std::time::Duration>,
    pub(super) sampled: bool,
}
pub(super) fn paint(painter: &mut Painter, b: &Arc<Batch>, v: &Arc<View>, d: &mut Deposit, context: Context) {
    for first in (context.start..b.frames).step_by(SLICE) {
        let started = if first == context.start && context.preflight {
            context.placement.map(|_| std::time::Instant::now())
        } else {
            context.sampled.then(|| painter.sampler.start()).flatten()
        };
        let count = if first == context.start && context.preflight {
            (b.frames - first).min(SLICE)
        } else {
            painter.place_slice(b, v, first)
        };
        if crate::pacing::is_stale(b.header.cfg()) {
            d.progress.stale += count as u64;
        }
        painter.draw_tiles(b, d, count);
        account(b, d, &painter.arena[..count]);
        if let Some(started) = started {
            let elapsed = started.elapsed() + context.placement.filter(|_| first == context.start).unwrap_or_default();
            d.progress.sample = Some((elapsed, count as u64));
        }
    }
}
impl Painter {
    pub(super) fn draw_tiles(&mut self, b: &Arc<Batch>, d: &mut Deposit, count: usize) {
        self.tile_jobs(b, &mut d.planes, count, false);
        if let Some(planes) = d.zoom_planes_mut() {
            self.tile_jobs(b, planes, count, true);
        }
        self.cost.pool += self.pool.map(&mut self.jobs, false).wall;
        for job in self.jobs.drain(..) {
            if let Job::Tile(_, _, (_, zoom), band, elapsed) = job {
                self.cost.tiles += elapsed;
                if zoom {
                    d.zoom_planes_mut().expect("zoom tiles came from this deposit").push(band);
                } else {
                    d.planes.push(band);
                }
            }
        }
        self.paint_decoded_dots(b, &mut d.planes, count);
    }

    fn tile_jobs(&mut self, b: &Arc<Batch>, planes: &mut Vec<DiffBand>, count: usize, zoom: bool) {
        for band in planes.drain(..) {
            self.jobs.push(Job::Tile(b.clone(), self.arena.clone(), (count, zoom), band, Duration::ZERO));
        }
    }

    fn paint_decoded_dots(&mut self, b: &Batch, planes: &mut [DiffBand], count: usize) {
        // Keep only one decoded record in scratch while painting its dots across the bands.
        let scratch = &mut self.pool.scratch().frame;
        let mut dots = self.arena[..count].iter().flatten().filter(|p| p.decoded_dots).peekable();
        let started = dots.peek().map(|_| Instant::now());
        for p in dots {
            let Some(plan) = &p.plan else { continue };
            p.fallbacks.fetch_or(Cause::DecodedDots.bit(), Ordering::Relaxed);
            let frame = newest_at(b, p.index).unwrap_or_else(|| {
                p.fallbacks.fetch_or(Cause::DotsRedecode.bit(), Ordering::Relaxed);
                #[cfg(test)]
                fixture_bench::at(p.index, || {
                    fixture_stage!(Writes, b.decode.decode(p.index, chunk(b, p.index), scratch))
                });
                #[cfg(not(test))]
                b.decode.decode(p.index, chunk(b, p.index), scratch);
                scratch
            });
            for band in &mut *planes {
                let samples = kernel::SamplesRef::Volts(&frame.ch[band.channel]);
                #[cfg(test)]
                fixture_bench::at(p.index, || fixture_stage!(Writes, band.channel, band.paint(samples, &plan.plan)));
                #[cfg(not(test))]
                band.paint(samples, &plan.plan);
            }
        }
        if let Some(started) = started {
            let elapsed = started.elapsed();
            self.cost.tiles += elapsed;
            self.cost.pool += elapsed;
        }
    }
}
fn account(b: &Batch, d: &mut Deposit, placed: &[Option<Box<Prepared>>]) {
    for p in placed.iter().flatten() {
        let Some(plan) = &p.plan else { continue };
        d.records += 1;
        b.counters.routes.fallbacks.add(p.fallbacks.load(Ordering::Relaxed));
        b.counters.routes.add(plan.route, 1);
        #[cfg(any(target_os = "linux", test))]
        if let Some(trace) = &b.header.trace {
            trace.add(plan.route);
        }
        d.extent.add_hits(p.hit_bound(plan));
        let zoom_weight = p.zoom_plan().map(|zoom| zoom.plan.weight());
        d.add_weights(crate::picture::PerMap::new(plan.plan.weight(), zoom_weight));
        d.progress.measured += u64::from(p.measured);
        d.triggered += u64::from(plan.triggered);
        if let Some(outcome) = plan.plan.outcome() {
            Stats { edges: &mut d.edges, off: &mut d.off }.record(outcome);
        }
        if d.identity == b.header.identity() {
            crate::framestats::add_folds(&mut d.folds, &p.folds);
        } else {
            Counters::add(&b.counters.losses.folds, p.folds.iter().map(|(_, f)| f.n).sum());
        }
    }
}
