// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) enum Job {
    Place(Arc<Batch>, Arc<View>, Box<Prepared>),
    Columns(Box<preparation_columns::Task>),
    Tile(Arc<Batch>, Arena, (usize, bool), DiffBand, Duration),
}

pub(super) fn run(scratch: &mut Frame, job: &mut Job) {
    match job {
        Job::Place(b, v, p) => {
            #[cfg(test)]
            let _counters = b.header.trace.as_ref().map(|_| test_scope::WorkerCounters::enter(b.counters));
            #[cfg(test)]
            fixture_bench::record(p.index, || place(b, v, p, scratch));
            #[cfg(not(test))]
            place(b, v, p, scratch);
        }
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
                #[cfg(test)]
                fixture_bench::at(p.index, || fixture_stage!(Writes, band.channel, band.paint(samples, &plan.plan)));
                #[cfg(not(test))]
                band.paint(samples, &plan.plan);
            }
            *elapsed = started.elapsed();
        }
    }
}
