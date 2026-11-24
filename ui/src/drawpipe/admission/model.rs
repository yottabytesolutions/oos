// SPDX-License-Identifier: GPL-2.0-or-later

#[cfg(any(target_os = "linux", test))]
use crate::drawpipe::route;
use crate::drawpipe::route::Route;
#[cfg(any(target_os = "linux", test))]
use crate::drawpipe::View;
#[cfg(any(target_os = "linux", test))]
use crate::model::{ModelExt, HDIV};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PoolPlan {
    route: Route,
    channels: u64,
    points: u64,
    columns: u64,
    average_visible: u64,
}

#[cfg(any(target_os = "linux", test))]
impl PoolPlan {
    pub(super) fn of(view: &View) -> Self {
        let (rate, points) = view.m.srate_depth();
        let frame = crate::board::Frame { dt: rate.recip(), acq: (rate, points as usize), ..Default::default() };
        let spacing = frame.dt / f64::from(view.m.time_per_div()) * view.w as f64 / f64::from(HDIV);
        let projection = crate::picture::deposit::standard::projection(&view.m, view.w, view.h, spacing);
        let main = route::route(&frame, &projection, view.m.acq, projection.code_geometry());
        let average = crate::drawpipe::average::software(&view.m);
        Self {
            route: if average { Route::Average } else { view.zoom.map_or(main, |_| Route::Zoom) },
            channels: view.m.n_on().max(1) as u64,
            points,
            columns: if average {
                crate::board::record::HW_POINTS as u64
            } else {
                (view.w + view.zoom.map_or(0, |zoom| zoom.size().0)) as u64
            },
            average_visible: if average {
                (rate * f64::from(view.m.time_per_div()) * f64::from(HDIV)).ceil() as u64
            } else {
                0
            },
        }
    }

    pub(super) fn seed(self, played_points: Option<u64>) -> [u64; 2] {
        if self.route == Route::Average {
            return self.average_seed();
        }
        // Starting rates in ns, from the cold single-worker wave F groups. The short-time
        // routes spend more per column, and HiRes/deep placement spends more per input point.
        let (point_ns, column_ns) = match self.route {
            Route::Boxcar => (120, 300),
            Route::Screen => (65, 100),
            Route::Dots => (12, 900),
            Route::Upsampled => (24, 900),
            // The second map reduces only its own window, then publishes one span per column.
            Route::Zoom => (20, 180),
            _ => (9, 80),
        };
        let points = played_points.unwrap_or(self.points).min(625_000);
        [(points * point_ns + self.columns * column_ns) * self.channels, 100_000]
    }

    fn average_seed(self) -> [u64; 2] {
        // Cold A53 estimate from the measured linear and Lanczos rows. Interpolation scales
        // with the output grid. Only the trigger lane can scan the input, within the screen.
        let visible = self.average_visible.min(self.points);
        let column_ns = if self.average_visible < self.columns.div_ceil(2) { 67 } else { 50 };
        [self.columns * column_ns * self.channels + visible * 4, 100_000]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Estimate {
    work: u64,
    fixed: u64,
}

impl Estimate {
    pub(super) fn new(work: u64, fixed: u64) -> Self {
        Self { work: work.max(1), fixed }
    }

    pub(super) fn sample(records: u64, work: u64, wall: u64, publication: u64, workers: u32) -> Self {
        let lanes = records.min(u64::from(workers));
        let fixed = wall.saturating_sub(work.div_ceil(lanes)).saturating_add(publication);
        Self::new(work.div_ceil(records), fixed)
    }

    pub(super) fn values(self) -> [u64; 2] {
        [self.work, self.fixed]
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn grant(self, bound: u32, budget: super::GrantBudget, workers: u32) -> super::Grant {
        let fixed = u128::from(self.fixed);
        let available = budget.work.as_nanos().saturating_sub(fixed);
        let work = u128::from(self.work);
        let workers = u128::from(workers);
        let work_limit = available * workers / work;
        let limit = budget.cycle.map_or(work_limit, |cycle| work_limit.min(cycle.limit(work, fixed, workers)));
        if work > available || limit == 0 {
            return super::Grant::Serial;
        }
        super::Grant::Overlapping(limit.min(u128::from(bound.max(1))) as u32)
    }
}
