// SPDX-License-Identifier: GPL-2.0-or-later

use super::report::{Stages, Timing};
use serde::Deserialize;
use std::{borrow::Cow, collections::BTreeMap};

#[derive(Deserialize)]
struct Sample<'a> {
    #[serde(borrow)]
    fixture: Cow<'a, str>,
    stage: &'a str,
    mode: &'a str,
    record: u64,
    ns_per_record: Option<f64>,
}

type StageSamples<'a> = BTreeMap<(Cow<'a, str>, u64), Option<f64>>;
type Round = BTreeMap<String, Vec<f64>>;

fn parse(output: &str) -> Result<Round, String> {
    let mut stages = BTreeMap::<&str, StageSamples<'_>>::new();
    for (index, line) in output.lines().enumerate().filter(|(_, line)| line.starts_with('{')) {
        let sample: Sample<'_> =
            serde_json::from_str(line).map_err(|_| format!("invalid JSON sample on line {}", index + 1))?;
        if sample.mode != "cold" {
            continue;
        }
        if sample.ns_per_record.is_some_and(|ns| !ns.is_finite() || ns < 0.0) {
            return Err(format!("invalid timing on line {}", index + 1));
        }
        let records = stages.entry(sample.stage).or_default();
        // The benchmark prints the same total on every enabled channel's row.
        if records
            .insert((sample.fixture, sample.record), sample.ns_per_record)
            .is_some_and(|old| old != sample.ns_per_record)
        {
            return Err(format!("conflicting timings for one record on line {}", index + 1));
        }
    }
    if stages.get("place").is_none_or(|stage| stage.values().flatten().all(|&ns| ns == 0.0)) {
        return Err("no positive cold place timings".into());
    }
    Ok(stages.into_iter().map(|(stage, samples)| (stage.into(), samples.into_values().flatten().collect())).collect())
}

fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_unstable_by(f64::total_cmp);
    let middle = values.len() / 2;
    let &upper = values.get(middle)?;
    Some(if values.len().is_multiple_of(2) { values[middle - 1].midpoint(upper) } else { upper })
}

#[derive(Default)]
struct AcrossRounds {
    samples: Vec<f64>,
    medians: Vec<f64>,
}

impl AcrossRounds {
    fn add(&mut self, mut values: Vec<f64>) {
        self.medians.extend(median(&mut values));
        self.samples.append(&mut values);
    }

    fn finish(mut self) -> Option<Timing> {
        let median = median(&mut self.samples)?;
        self.medians.sort_unstable_by(f64::total_cmp);
        Some(Timing::new(median, *self.medians.first()?, *self.medians.last()?, self.medians.len(), self.samples.len()))
    }
}

pub(super) fn summarize(rounds: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Stages, String> {
    let mut stages: BTreeMap<String, AcrossRounds> = BTreeMap::new();
    for (index, output) in rounds.into_iter().enumerate() {
        let round = parse(output.as_ref()).map_err(|e| format!("round {}: {e}", index + 1))?;
        if index == 0 {
            stages.extend(round.keys().map(|stage| (stage.clone(), AcrossRounds::default())));
        }
        super::report::same_keys(&stages, &round, "stage", "first round", "later round")?;
        for (stage, values) in round {
            stages.get_mut(&stage).expect("stage keys match the first round").add(values);
        }
    }
    if stages.is_empty() {
        return Err("no benchmark rounds".into());
    }
    Ok(stages.into_iter().map(|(stage, times)| (stage, times.finish())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(stage: &str, mode: &str, fixture: &str, record: u64, ns: Option<u64>) -> String {
        serde_json::json!({ "stage": stage, "mode": mode, "fixture": fixture, "record": record, "ns_per_record": ns })
            .to_string()
    }

    #[test]
    fn medians_of_cold_record_totals_include_fixture_and_round_variation() {
        let round = |a, b| {
            [
                "running 1 test".into(),
                sample("place", "cold", "a\"1", 1, Some(a)),
                sample("place", "cold", "a\"1", 1, Some(a)),
                sample("place", "cold", "b", 1, Some(b)),
                sample("place", "instructions", "a", 1, Some(900)),
                sample("absent", "cold", "a", 1, None),
                sample("partial", "cold", "a", 1, None),
                sample("partial", "cold", "a", 2, (a == 10).then_some(8)),
            ]
            .join("\n")
        };
        let stages = summarize([round(10, 14), round(20, 100)]).unwrap();
        assert_eq!(stages["place"], Some(Timing::new(17.0, 12.0, 60.0, 2, 4)));
        assert_eq!(stages["absent"], None);
        assert_eq!(stages["partial"], Some(Timing::new(8.0, 8.0, 8.0, 1, 1)));
    }

    #[test]
    fn invalid_samples_and_rounds_are_errors() {
        let good = sample("place", "cold", "a", 1, Some(10));
        let extra = sample("extra", "cold", "a", 1, None);
        for (rounds, expected) in [
            (vec!["".into()], "no positive cold place timings"),
            (vec!["{broken".into()], "invalid JSON"),
            (vec![format!("{good}\n{}", sample("place", "cold", "a", 1, Some(11)))], "conflicting timings"),
            (vec![good.clone(), format!("{good}\n{extra}")], "missing stage 'extra' in first round"),
        ] {
            let err = summarize(rounds).unwrap_err();
            assert!(err.contains(expected), "{err}");
        }
    }
}
