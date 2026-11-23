// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) enum Job {
    Place(Arc<Batch>, Arc<View>, Box<Prepared>),
    Average(Arc<Batch>, Arc<View>, std::ops::Range<usize>, average::Partial),
    Columns(Box<preparation_columns::Task>),
    Tile(Arc<Batch>, Arena, (usize, bool), DiffBand, Duration),
}

pub(super) fn run(scratch: &mut average::Scratch, job: &mut Job) {
    match job {
        Job::Place(b, v, p) => {
            #[cfg(test)]
            let _counters = b.header.trace.as_ref().map(|_| test_scope::WorkerCounters::enter(b.counters));
            let (_, marks) = fallback::capture(|| {
                #[cfg(test)]
                fixture_bench::record(p.index, || place(b, v, p, &mut scratch.frame));
                #[cfg(not(test))]
                place(b, v, p, &mut scratch.frame);
            });
            *p.fallbacks.get_mut() = marks;
        }
        Job::Average(b, v, range, partial) => partial.run(scratch, b, v, range.clone()),
        Job::Columns(task) => task.run(),
        Job::Tile(b, placed, (count, zoom), band, elapsed) => {
            let started = Instant::now();
            for p in placed[..*count].iter().flatten() {
                if p.decoded_dots {
                    continue;
                }
                let plan = if *zoom { p.zoom_plan() } else { p.plan.as_ref() };
                let Some(plan) = plan else { continue };
                let samples = p.codes.map_or(kernel::SamplesRef::Volts(&[]), |layout| {
                    kernel::SamplesRef::Codes(layout.channel(chunk(b, p.index), band.channel))
                });
                let (_, marks) = fallback::capture(|| {
                    #[cfg(test)]
                    fixture_bench::at(p.index, || {
                        fixture_stage!(Writes, band.channel, band.paint(samples, &plan.plan))
                    });
                    #[cfg(not(test))]
                    band.paint(samples, &plan.plan);
                });
                if marks != 0 {
                    p.fallbacks.fetch_or(marks, Ordering::Relaxed);
                }
            }
            *elapsed = started.elapsed();
        }
    }
}
