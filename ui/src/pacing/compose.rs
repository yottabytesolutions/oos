// SPDX-License-Identifier: GPL-2.0-or-later

use super::{pct, push, state};
use std::{collections::VecDeque, fmt::Write, time::Duration};

#[derive(Clone, Default)]
pub(super) struct History {
    fast: VecDeque<[f32; 2]>,
    phases: VecDeque<[f32; 5]>,
    rejected: VecDeque<f32>,
    remote: VecDeque<f32>,
    pub(super) presentation: super::presentation::History,
}

impl History {
    pub(super) fn record_fast(&mut self, draw_ms: f32, merge_ms: f32) {
        push(&mut self.fast, [draw_ms, merge_ms]);
    }

    pub(super) fn fast_costs(&self) -> [Vec<f32>; 2] {
        std::array::from_fn(|column| self.fast.iter().map(|sample| sample[column]).collect())
    }

    /// Rolling p50/p90/p99/max in microseconds, one composition sample per changed picture.
    pub(super) fn text(&self) -> String {
        let mut text = String::from(" compose.v1");
        for (column, name) in
            ["handoff_composed", "owner_wait", "merge_blend", "rebuild", "other"].into_iter().enumerate()
        {
            let mut values: Vec<_> = self.phases.iter().map(|sample| sample[column]).collect();
            quantiles(&mut text, name, &mut values);
        }
        quantiles(&mut text, "rejected_wait", &mut self.rejected.iter().copied().collect::<Vec<_>>());
        quantiles(&mut text, "remote_intake", &mut self.remote.iter().copied().collect::<Vec<_>>());
        text
    }
}

fn quantiles(text: &mut String, name: &str, values: &mut [f32]) {
    let [p50, p90, p99, max] = [0.5, 0.9, 0.99, 1.0].map(|q| pct(values, q));
    let _ = write!(text, " {name}={p50:.0}/{p90:.0}/{p99:.0}/{max:.0}");
}

fn micros(elapsed: Duration) -> f32 {
    elapsed.as_secs_f32() * 1e6
}

/// Per picture, retain the longest handoff wait among the batches actually shown.
pub(crate) fn note_composition(
    wait: Option<Duration>,
    rejected_wait: Option<Duration>,
    merge: Duration,
    rebuild: Duration,
    elapsed: Duration,
) {
    let wait = wait.unwrap_or_default();
    let other = elapsed.saturating_sub(merge + rebuild);
    let sample = [wait + elapsed, wait, merge, rebuild, other].map(micros);
    let mut pacing = oos_hal::sync::lock(state());
    push(&mut pacing.compose.phases, sample);
    if let Some(wait) = rejected_wait {
        push(&mut pacing.compose.rejected, micros(wait));
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn note_remote_intake(elapsed: Duration) {
    push(&mut oos_hal::sync::lock(state()).compose.remote, micros(elapsed));
}
