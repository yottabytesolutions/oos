// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    report::{GroupReport, Report},
    window::{per_minute, WindowSample},
};
use crate::unit_tools::report::{read, same_keys};
use std::{fmt::Write, path::Path, process::ExitCode};

// The paired 26-group runs differed by 1.998 percentage points (535/12058 vs 294/12054).
const LATE_FLIP_FLOOR_DIVISOR: u128 = 50;

#[derive(Debug)]
struct Row<'a> {
    group: &'a str,
    reference: Metrics,
    result: Metrics,
    ratio: f64,
    regression: bool,
}

#[derive(Debug)]
struct Metrics {
    first_records_per_s: f64,
    fps: f64,
    late_per_minute: f64,
    flips: Flips,
}

#[derive(Debug, Default)]
struct Flips {
    late: u128,
    shown: u128,
}

impl Flips {
    fn add(&mut self, other: &Self) {
        self.late += other.late;
        self.shown += other.shown;
    }

    fn regression_against(&self, reference: &Self) -> Result<bool, String> {
        if reference.shown == 0 || self.shown == 0 {
            let side = if reference.shown == 0 { "reference" } else { "result" };
            return Err(format!("{side} run has no shown pictures"));
        }
        let denominator = self.shown.checked_mul(reference.shown).ok_or("picture totals are too large to compare")?;
        // Both late counts are bounded by their shown counts. Compare the exact fractions at a common denominator.
        let old = reference.late * self.shown;
        let new = self.late * reference.shown;
        let increase = new.saturating_sub(old);
        Ok(increase > old && increase > denominator / LATE_FLIP_FLOOR_DIVISOR)
    }

    fn percentage(&self) -> f64 {
        100.0 * self.late as f64 / self.shown as f64
    }
}

#[derive(Debug)]
struct Comparison<'a> {
    rows: Vec<Row<'a>>,
    reference: Flips,
    result: Flips,
    late_regression: bool,
}

impl Comparison<'_> {
    fn exit(&self) -> ExitCode {
        if self.late_regression || self.rows.iter().any(|row| row.regression) {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    }
}

pub(super) fn run(reference: &Path, result: &Path, floor: f64) -> Result<ExitCode, String> {
    let reference =
        read(reference, "reference").map_err(|_| "could not read a current unit-matrix reference report")?;
    let result = read(result, "result").map_err(|_| "could not read a current unit-matrix result report")?;
    let comparison = compare(&reference, &result, floor)?;
    print!("{}", table(&comparison));
    Ok(comparison.exit())
}

fn compare<'a>(reference: &'a Report, result: &Report, floor: f64) -> Result<Comparison<'a>, String> {
    if reference.base_setup != result.base_setup {
        return Err("reports use different base setups".into());
    }
    if reference.bench_sha256 != result.bench_sha256 {
        return Err("reports must use the same bench setup".into());
    }
    if reference.skipped_setup_keys != result.skipped_setup_keys {
        return Err("reports skipped different setup keys".into());
    }
    if reference.window_s != result.window_s || !reference.window_s.is_finite() || reference.window_s <= 0.0 {
        return Err("reports must use the same positive window length".into());
    }
    if reference.error.is_some() || result.error.is_some() {
        return Err("cannot compare a run that aborted or failed to restore".into());
    }
    if reference.groups.is_empty() || result.groups.is_empty() {
        return Err("both reports must contain at least one group".into());
    }
    same_keys(&reference.groups, &result.groups, "group", "reference", "result")?;
    let rows = reference
        .groups
        .iter()
        .map(|(id, old)| Row::compare(id, old, &result.groups[id], floor))
        .collect::<Result<Vec<_>, _>>()?;
    let mut reference = Flips::default();
    let mut result = Flips::default();
    for row in &rows {
        reference.add(&row.reference.flips);
        result.add(&row.result.flips);
    }
    let late_regression = result.regression_against(&reference)?;
    Ok(Comparison { rows, reference, result, late_regression })
}

impl<'a> Row<'a> {
    fn compare(id: &'a str, old: &GroupReport, new: &GroupReport, floor: f64) -> Result<Self, String> {
        if old.overrides != new.overrides {
            return Err(format!("group '{id}' uses different setups"));
        }
        if old.expected_windows != new.expected_windows {
            return Err(format!("group '{id}' uses different window counts"));
        }
        let reference = Metrics::from(old).ok_or_else(|| format!("missing or invalid reference metrics for '{id}'"))?;
        let result = Metrics::from(new).ok_or_else(|| format!("missing or invalid result metrics for '{id}'"))?;
        let ratio = result.first_records_per_s / reference.first_records_per_s;
        if !ratio.is_finite() {
            return Err(format!("group '{id}' requires a positive reference first-use records/s rate"));
        }
        let regression = ratio < floor || reference.fps - result.fps > 1.0;
        Ok(Self { group: id, reference, result, ratio, regression })
    }
}

impl Metrics {
    fn from(group: &GroupReport) -> Option<Self> {
        if group.error.is_some() || group.expected_windows == 0 || group.windows.len() != group.expected_windows {
            return None;
        }
        let summary = group.summary.as_ref()?;
        let [low, high] = summary.host_s;
        if !low.is_finite() || !high.is_finite() || low <= 0.0 || high < low {
            return None;
        }
        let mut flips = Flips::default();
        for window in &group.windows {
            // Older reports retain presentation counters only in the windows.
            let WindowSample::Valid(sample) = &window.sample else { return None };
            let late = u64::try_from(*sample.delta.get("flip.late")?).ok()?;
            let shown = u64::try_from(*sample.delta.get("flip.shown")?).ok()?;
            if late > shown {
                return None;
            }
            flips.add(&Flips { late: late.into(), shown: shown.into() });
        }
        let metrics = Self {
            first_records_per_s: summary.rates.first_records_per_s,
            fps: *summary.delta.get("fresh")? as f64 / f64::midpoint(low, high),
            late_per_minute: per_minute(flips.late as f64, summary.host_s),
            flips,
        };
        [metrics.first_records_per_s, metrics.fps, metrics.late_per_minute]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
            .then_some(metrics)
    }
}

fn table(comparison: &Comparison<'_>) -> String {
    let rows = &comparison.rows;
    let width = rows.iter().map(|row| row.group.len()).max().unwrap_or_default().max(5);
    let mut text = format!(
        "{:<width$}  {:>10}  {:>10}  {:>8}  {:>8}  {:>8}  {:>12}  {:>12}  {}\n",
        "Group", "Ref first/s", "New first/s", "Ratio", "Ref fps", "New fps", "Ref late/min", "New late/min", "Status"
    );
    for row in rows {
        let old = &row.reference;
        let new = &row.result;
        let verdict = if row.regression { "REGRESSION: repeat group, then investigate" } else { "pass" };
        writeln!(
            text,
            "{:<width$}  {:>10.1}  {:>10.1}  {:>8.3}  {:>8.2}  {:>8.2}  {:>12.2}  {:>12.2}  {verdict}",
            row.group,
            old.first_records_per_s,
            new.first_records_per_s,
            row.ratio,
            old.fps,
            new.fps,
            old.late_per_minute,
            new.late_per_minute
        )
        .expect("writing to a String succeeds");
    }
    let totals = |flips: &Flips| format!("{}/{} ({:.2}%)", flips.late, flips.shown, flips.percentage());
    let status = if comparison.late_regression { "REGRESSION" } else { "pass" };
    writeln!(
        text,
        "Run late flips/pictures  reference {}  result {}  {status} (floor {:.1} percentage points)",
        totals(&comparison.reference),
        totals(&comparison.result),
        100.0 / LATE_FLIP_FLOOR_DIVISOR as f64
    )
    .expect("writing to a String succeeds");
    text
}

#[cfg(test)]
mod tests;
