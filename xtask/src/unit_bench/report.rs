// SPDX-License-Identifier: GPL-2.0-or-later

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write};

pub(super) type Stages = BTreeMap<String, Option<Timing>>;

pub(super) const MIN_ROUNDS: usize = 5;
const MIN_SAMPLES: usize = 10;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Report {
    commit: String,
    binary_sha256: String,
    cpu: u32,
    core_type: String,
    groups: BTreeMap<String, Group>,
}

impl Report {
    pub(super) fn new(
        commit: String,
        binary_sha256: String,
        cpu: u32,
        core_type: String,
        groups: BTreeMap<String, Group>,
    ) -> Self {
        Self { commit, binary_sha256, cpu, core_type, groups }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Group {
    fixture_sha256: String,
    stages: Stages,
}

impl Group {
    pub(super) fn new(fixture_sha256: String, stages: Stages) -> Self {
        Self { fixture_sha256, stages }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct Timing {
    median: f64,
    min_round_median: f64,
    max_round_median: f64,
    sampling: Option<Sampling>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
struct Sampling {
    rounds: usize,
    samples: usize,
}

impl Sampling {
    fn valid(self) -> bool {
        self.rounds > 0 && self.samples >= self.rounds
    }

    fn rounds_needed(self) -> usize {
        if self.samples < MIN_SAMPLES {
            // Valid short reports have at most nine rounds. Project their observed samples per round.
            MIN_ROUNDS.max((MIN_SAMPLES * self.rounds).div_ceil(self.samples))
        } else {
            MIN_ROUNDS
        }
    }
}

impl Timing {
    pub(super) fn new(
        median: f64,
        min_round_median: f64,
        max_round_median: f64,
        rounds: usize,
        samples: usize,
    ) -> Self {
        Self { median, min_round_median, max_round_median, sampling: Some(Sampling { rounds, samples }) }
    }

    fn spread(self) -> f64 {
        self.max_round_median - self.min_round_median
    }

    fn valid(self) -> bool {
        [self.median, self.min_round_median, self.max_round_median].into_iter().all(|ns| ns.is_finite() && ns >= 0.0)
            && self.min_round_median <= self.max_round_median
            && self.sampling.is_none_or(Sampling::valid)
    }

    fn sufficient(self) -> bool {
        self.sampling.is_some_and(|s| s.rounds >= MIN_ROUNDS && s.samples >= MIN_SAMPLES)
    }
}

pub(super) fn same_keys<T, U>(
    left: &BTreeMap<String, T>,
    right: &BTreeMap<String, U>,
    kind: &str,
    left_name: &str,
    right_name: &str,
) -> Result<(), String> {
    let missing = left
        .keys()
        .filter(|key| !right.contains_key(*key))
        .map(|key| (key, right_name))
        .chain(right.keys().filter(|key| !left.contains_key(*key)).map(|key| (key, left_name)))
        .next();
    if let Some((key, label)) = missing {
        return Err(format!("missing {kind} '{key}' in {label}"));
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Regression,
    Faster,
    Same,
    WithinLimits,
    Unmeasured,
    InsufficientData,
}

#[derive(Debug)]
pub(super) struct Row<'a> {
    group: &'a str,
    stage: &'a str,
    reference: Option<Timing>,
    result: Option<Timing>,
    verdict: Verdict,
}

impl Row<'_> {
    pub(super) fn regression(&self) -> bool {
        self.verdict == Verdict::Regression
    }

    pub(super) fn insufficient(&self) -> bool {
        self.verdict == Verdict::InsufficientData
    }
}

fn verdict(stage: &str, reference: Option<Timing>, result: Option<Timing>, place: f64, tolerance: f64) -> Verdict {
    let (Some(reference), Some(result)) = (reference, result) else { return Verdict::Unmeasured };
    if !reference.sufficient() || !result.sufficient() {
        return Verdict::InsufficientData;
    }
    timing_verdict(stage, reference, result, place, tolerance)
}

fn timing_verdict(stage: &str, reference: Timing, result: Timing, place: f64, tolerance: f64) -> Verdict {
    let delta = result.median - reference.median;
    let limit = (reference.median * tolerance).max(reference.spread() + result.spread());
    if delta.abs() <= limit {
        Verdict::Same
    } else if stage != "place" && delta.abs() <= place * tolerance {
        Verdict::WithinLimits
    } else if delta > 0.0 {
        Verdict::Regression
    } else {
        Verdict::Faster
    }
}

pub(super) fn compare<'a>(reference: &'a Report, result: &Report, tolerance: f64) -> Result<Vec<Row<'a>>, String> {
    if reference.cpu != result.cpu {
        return Err(format!("CPU differs: reference {}, result {}", reference.cpu, result.cpu));
    }
    if reference.core_type != result.core_type {
        return Err(format!("CPU core type differs: reference {}, result {}", reference.core_type, result.core_type));
    }
    same_keys(&reference.groups, &result.groups, "group", "reference", "result")?;
    if reference.groups.is_empty() {
        return Err("reports have no groups".into());
    }
    let mut rows = Vec::new();
    for (group, baseline) in &reference.groups {
        let candidate = &result.groups[group];
        if baseline.fixture_sha256 != candidate.fixture_sha256 {
            return Err(format!("group '{group}': fixture SHA-256 differs between reference and result"));
        }
        compare_group(&mut rows, group, &baseline.stages, &candidate.stages, tolerance)?;
    }
    Ok(rows)
}

fn compare_group<'a>(
    rows: &mut Vec<Row<'a>>,
    group: &'a str,
    stages: &'a Stages,
    candidate: &Stages,
    tolerance: f64,
) -> Result<(), String> {
    same_keys(stages, candidate, "stage", "reference", "result").map_err(|e| format!("group '{group}': {e}"))?;
    let place = placement(stages, group, "reference")?;
    placement(candidate, group, "result")?;
    for (stage, &reference) in stages {
        let result = candidate[stage];
        if reference.is_some() != result.is_some() {
            let missing = if reference.is_none() { "reference" } else { "result" };
            return Err(format!("group '{group}', stage '{stage}': unmeasured in {missing} only"));
        }
        if [reference, result].into_iter().flatten().any(|timing| !timing.valid()) {
            return Err(format!("group '{group}', stage '{stage}': invalid timing"));
        }
        rows.push(Row {
            group,
            stage,
            reference,
            result,
            verdict: verdict(stage, reference, result, place, tolerance),
        });
    }
    Ok(())
}

fn placement(stages: &Stages, group: &str, label: &str) -> Result<f64, String> {
    stages
        .get("place")
        .copied()
        .flatten()
        .filter(|place| place.median > 0.0)
        .map(|place| place.median)
        .ok_or_else(|| format!("group '{group}' requires a positive 'place' median in {label}"))
}

pub(super) fn table(rows: &[Row<'_>]) -> String {
    let group_width = rows.iter().map(|row| row.group.len()).max().unwrap_or_default().max(5);
    let stage_width = rows.iter().map(|row| row.stage.len()).max().unwrap_or_default().max(5);
    let mut text = format!(
        "{:<group_width$}  {:<stage_width$}  {:>12}  {:>12}  {:>9}  {:>10}  {:>10}  {}\n",
        "Group", "Stage", "Ref ns/rec", "New ns/rec", "Change", "Ref spread", "New spread", "Status"
    );
    for row in rows {
        let number = |timing: Option<Timing>, value: fn(Timing) -> f64| {
            timing.map_or_else(|| "n/a".into(), |t| format!("{:.1}", value(t)))
        };
        let change = row
            .reference
            .zip(row.result)
            .filter(|(old, _)| old.median > 0.0)
            .map_or_else(|| "n/a".into(), |(old, new)| format!("{:+.2}%", 100.0 * (new.median / old.median - 1.0)));
        let status = status(row);
        writeln!(
            text,
            "{:<group_width$}  {:<stage_width$}  {:>12}  {:>12}  {:>9}  {:>10}  {:>10}  {status}",
            row.group,
            row.stage,
            number(row.reference, |t| t.median),
            number(row.result, |t| t.median),
            change,
            number(row.reference, Timing::spread),
            number(row.result, Timing::spread)
        )
        .expect("writing to a String succeeds");
    }
    text
}

fn status(row: &Row<'_>) -> String {
    match row.verdict {
        Verdict::Regression => "REGRESSION",
        Verdict::Faster => "faster",
        Verdict::Same => "same",
        Verdict::WithinLimits => "within limits",
        Verdict::Unmeasured => "unmeasured",
        Verdict::InsufficientData => {
            let needs = [("reference", row.reference), ("result", row.result)]
                .into_iter()
                .filter_map(|(label, timing)| sampling_need(label, timing?))
                .collect::<Vec<_>>()
                .join("; ");
            return format!("insufficient data ({needs})");
        }
    }
    .into()
}

fn sampling_need(label: &str, timing: Timing) -> Option<String> {
    if timing.sufficient() {
        return None;
    }
    Some(match timing.sampling {
        None => {
            format!("{label} counts unavailable, rerun with at least {MIN_ROUNDS} rounds and {MIN_SAMPLES} samples")
        }
        Some(s) => format!(
            "{label} has {} rounds and {} samples, needs at least {} rounds at this sampling rate",
            s.rounds,
            s.samples,
            s.rounds_needed()
        ),
    })
}

#[cfg(test)]
mod tests;
