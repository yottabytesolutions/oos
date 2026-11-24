// SPDX-License-Identifier: GPL-2.0-or-later

use super::{metrics::nanos, same_pair, source::SourceWake, Counters, Pipe, ViewState};
use crate::perfguard::ring::Ring;
use oos_core::change::Identity;
use oos_hal::sync::lock;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

mod model;
use model::{Estimate, PoolPlan};
#[cfg(any(target_os = "linux", test))]
mod cycle;
#[cfg(any(target_os = "linux", test))]
pub(crate) use cycle::{GrantBudget, WindowCycle};

#[cfg(any(target_os = "linux", test))]
const INITIAL_COST: u64 = 1_000_000;

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Grant {
    Overlapping(u32),
    Serial,
}

#[cfg(any(target_os = "linux", test))]
impl Grant {
    pub(crate) fn records(self) -> u32 {
        match self {
            Self::Overlapping(records) => records,
            Self::Serial => 1,
        }
    }

    pub(crate) fn serial(self) -> bool {
        self == Self::Serial
    }
}

#[derive(Default)]
pub(crate) struct Cost {
    identity: Option<Identity>,
    samples: Ring<16, [Option<u64>; 2]>,
}

impl Cost {
    pub(crate) fn set_identity(&mut self, identity: Identity) {
        if !same_pair(self.identity, Some(identity)) {
            *self = Self { identity: Some(identity), ..Self::default() };
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn grant(&self, identity: Identity, bound: u32, budget: impl Into<GrantBudget>) -> Grant {
        let [work, publication] = self.estimate(identity, [INITIAL_COST, 0], 1);
        Estimate::new(work.saturating_add(publication), 0).grant(bound, budget.into(), 1)
    }

    pub(crate) fn observe(&mut self, identity: Identity, records: u64, pool: Duration, publication: Duration) {
        if records != 0 {
            let per_record =
                |d: Duration| u64::try_from(d.as_nanos().div_ceil(u128::from(records))).unwrap_or(u64::MAX);
            self.record(identity, [pool, publication].map(per_record));
        }
    }

    fn record(&mut self, identity: Identity, sample: [u64; 2]) {
        if same_pair(self.identity, Some(identity)) {
            self.samples.add(sample.map(Some));
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn estimate(&self, identity: Identity, seed: [u64; 2], fixed_rank: usize) -> [u64; 2] {
        if self.samples.samples().is_empty() || !same_pair(self.identity, Some(identity)) {
            seed
        } else {
            [self.samples.quantiles(0)[1], self.samples.quantiles(1)[fixed_rank]]
        }
    }
}

#[derive(Default)]
pub(super) struct Admission {
    state: Mutex<State>,
    pool: OnceLock<Pool>,
}

#[derive(Default)]
struct State {
    cost: Cost,
    busy: usize,
    plan: Option<(u64, PoolPlan)>,
}

struct Pool {
    view: Arc<Mutex<ViewState>>,
    counters: &'static Counters,
    workers: u32,
}

pub(super) fn model_pool(pipe: &Pipe, workers: usize) {
    let _ = pipe.admission.pool.set(Pool {
        view: pipe.view.clone(),
        counters: pipe.counters,
        workers: workers.max(1) as u32,
    });
}

impl Admission {
    pub(super) fn set_identity(&self, identity: Identity) {
        let mut state = lock(&self.state);
        if !same_pair(state.cost.identity, Some(identity)) {
            state.plan = None;
            state.cost.set_identity(identity);
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn grant(&self, identity: Identity, bound: u32, budget: GrantBudget) -> Grant {
        let Some(pool) = self.pool.get() else { return lock(&self.state).cost.grant(identity, bound, budget) };
        let view = lock(&pool.view);
        let mut state = lock(&self.state);
        let Some(view_plan) = view.view.as_deref() else { return state.cost.grant(identity, bound, budget) };
        let plan =
            state.plan.filter(|(gen, _)| *gen == view.gen).map_or_else(|| PoolPlan::of(view_plan), |(_, plan)| plan);
        let key = Some((view.gen, plan));
        if same_pair(view.identity, Some(identity)) && state.plan != key {
            state.cost.samples = Ring::default();
            state.plan = key;
        }
        let [work, fixed] = state.cost.estimate(identity, plan.seed(budget.cycle.map(|cycle| cycle.played_points)), 0);
        let grant = Estimate::new(work, fixed).grant(bound, budget, pool.workers);
        pool.counters.metrics.source_model(identity, [work, fixed, u64::from(pool.workers.min(grant.records()))]);
        grant
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn busy(&self) -> bool {
        lock(&self.state).busy != 0
    }

    pub(super) fn submit(
        self: &Arc<Self>,
        source: &Arc<SourceWake>,
        identity: Identity,
        counters: &super::Counters,
    ) -> Flight {
        let mut state = lock(&self.state);
        let busy = state.busy != 0;
        if busy {
            super::Counters::add(&counters.losses.unexpected_busy, 1);
        }
        debug_assert!(!busy, "the acquisition source waits before submitting");
        state.busy += 1;
        Flight { admission: self.clone(), source: source.clone(), identity, plan: state.plan, work: None }
    }
}

pub(super) struct Flight {
    admission: Arc<Admission>,
    source: Arc<SourceWake>,
    identity: Identity,
    plan: Option<(u64, PoolPlan)>,
    work: Option<(u64, Duration)>,
}

impl Flight {
    pub(super) fn worked(&mut self, records: u64, pool: Duration) {
        self.work = Some((records, pool));
    }

    pub(super) fn published(&self, publication: Duration) {
        let Some((records, wall)) = self.work else { return };
        let admission = &self.admission;
        let Some(pool) = admission.pool.get() else {
            lock(&admission.state).cost.observe(self.identity, records, wall, publication);
            return;
        };
        let view = lock(&pool.view);
        let mut state = lock(&admission.state);
        if self.plan.is_none_or(|plan| plan.0 != view.gen) || self.plan != state.plan {
            return;
        }
        if let Some(work) = pool.counters.metrics.worker_time(self.identity, records) {
            let sample = Estimate::sample(records, work, nanos(wall), nanos(publication), pool.workers);
            state.cost.record(self.identity, sample.values());
        }
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        lock(&self.admission.state).busy -= 1;
        self.source.notify();
    }
}

#[cfg(test)]
mod tests;
