// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_matrix::{
    report::Summary,
    window::{Rates, Sample, Window, WindowSample},
};

type RecordedMetrics = (f64, i128, i128, i128, i128);

fn group((seconds, records, fresh, shown, late): RecordedMetrics) -> GroupReport {
    let delta = [
        ("records.shown".into(), records),
        ("records.read".into(), records),
        ("first_records".into(), records),
        ("fresh".into(), fresh),
        ("flip.shown".into(), shown),
        ("flip.late".into(), late),
    ]
    .into();
    let host_s = [seconds; 2];
    let rates = Rates::calculate(host_s, &delta).unwrap();
    let window = Window {
        start_s: 0.0,
        host_s,
        sample: WindowSample::Valid(Sample {
            rates: Rates::calculate(host_s, &delta).unwrap(),
            delta: delta.clone(),
            fallback_delta: Default::default(),
            sections: Default::default(),
        }),
    };
    // The original reports omitted flip counters from the pooled summary.
    let mut delta = delta;
    delta.remove("flip.shown");
    delta.remove("flip.late");
    let summary = Summary { host_s, delta, fallback_delta: Default::default(), rates };
    GroupReport { expected_windows: 1, windows: vec![window], summary: Some(summary), ..GroupReport::default() }
}

fn report(seconds: f64, records: i128, pictures: i128, late: i128) -> Report {
    Report {
        build: "test".into(),
        base_setup: [("cores".into(), "true".into())].into(),
        bench_sha256: "fixture".into(),
        window_s: 10.0,
        skipped_setup_keys: Vec::new(),
        groups: [("dense".into(), group((seconds, records, pictures, pictures, late)))].into(),
        error: None,
    }
}

#[test]
fn compares_threshold_boundaries_and_normalizes_for_measured_duration() {
    let reference = report(10.0, 1000, 300, 10);
    for (records, pictures, expected) in [(2000, 600, 0), (1900, 580, 0), (1899, 600, 1), (2000, 579, 1)] {
        let result = report(20.0, records, pictures, 20);
        let comparison = compare(&reference, &result, 0.95).unwrap();
        assert_eq!(comparison.exit(), ExitCode::from(expected));
        let text = table(&comparison);
        assert_eq!(text.lines().count(), 3);
        assert_eq!(text.contains("repeat group, then investigate"), expected == 1);
    }
    assert_eq!(per_minute(10.0, [9.0, 11.0]), 60.0);
}

#[test]
fn applies_both_late_flip_thresholds_strictly() {
    for (shown, reference_late, result_late, expected) in [
        (300, 0, 0, 0),
        (300, 0, 6, 0),
        (300, 0, 7, 1),
        (300, 3, 9, 0),
        (300, 3, 10, 1),
        (300, 12, 24, 0),
        (300, 12, 25, 1),
        (1000, 9, 29, 0),
    ] {
        let reference = report(10.0, 100, shown, reference_late);
        let result = report(10.0, 100, shown, result_late);
        let comparison = compare(&reference, &result, 0.95).unwrap();
        assert_eq!(comparison.exit(), ExitCode::from(expected), "{reference_late} vs {result_late}");
        assert!(!comparison.rows[0].regression);
    }
}

#[test]
fn recorded_bursts_pass_but_a_more_than_doubled_run_fraction_fails() {
    let recorded: Vec<(String, RecordedMetrics, RecordedMetrics)> =
        serde_json::from_str(include_str!("recorded.json")).unwrap();
    let mut reference = report(5.0, 0, 0, 0);
    let mut result = report(5.0, 0, 0, 0);
    let mut doubled = report(5.0, 0, 0, 0);
    [&mut reference, &mut result, &mut doubled].into_iter().for_each(|report| report.groups.clear());
    for (name, old, new) in recorded {
        reference.groups.insert(name.clone(), group(old));
        result.groups.insert(name.clone(), group(new));
        let (seconds, records, fresh, shown, late) = new;
        // Doubling the count with one fewer picture makes the run fraction strictly more than double.
        doubled
            .groups
            .insert(name.clone(), group((seconds, records, fresh, shown - i128::from(name == "deep"), late * 2)));
    }
    for (old, new, counts, expected) in [
        (&reference, &result, ["535/12058", "294/12054"], ExitCode::SUCCESS),
        (&result, &doubled, ["294/12054", "588/12053"], ExitCode::FAILURE),
    ] {
        let comparison = compare(old, new, 0.95).unwrap();
        assert_eq!(comparison.exit(), expected);
        assert!(comparison.rows.iter().all(|row| !row.regression));
        let text = table(&comparison);
        let summary = text.lines().last().unwrap();
        assert!(counts.iter().all(|count| summary.contains(count)), "{summary}");
    }
}

#[test]
fn compares_first_use_throughput_even_if_shown_and_read_are_higher() {
    let reference = report(10.0, 100, 300, 0);
    let mut result = report(10.0, 100, 300, 0);
    let summary = result.groups.get_mut("dense").unwrap().summary.as_mut().unwrap();
    summary.rates.first_records_per_s = 9.4;
    summary.rates.shown_records_per_s = 12.0;
    summary.rates.read_per_s = 15.0;
    assert_eq!(compare(&reference, &result, 0.95).unwrap().exit(), ExitCode::FAILURE);
}

#[test]
fn compares_an_older_reference_with_a_result_reporting_recoveries() {
    let mut reference = report(10.0, 100, 300, 0);
    let mut result = report(10.0, 100, 300, 0);
    for (report, deposited) in [(&mut reference, 104), (&mut result, 100)] {
        let WindowSample::Valid(sample) = &mut report.groups.get_mut("dense").unwrap().windows[0].sample else {
            panic!("expected valid window")
        };
        sample.delta.insert("records.deposited".into(), deposited);
        sample.delta.insert("rejected.stale_key".into(), 4);
    }
    let group = result.groups.get_mut("dense").unwrap();
    let WindowSample::Valid(sample) = &mut group.windows[0].sample else { panic!("expected valid window") };
    sample.delta.insert("records.recovered".into(), 1);
    group.summarize().unwrap();
    assert_eq!(group.summary.as_ref().unwrap().delta["records.recovered"], 1);
    let reference: Report = serde_json::from_str(&serde_json::to_string(&reference).unwrap()).unwrap();
    assert_eq!(compare(&reference, &result, 0.95).unwrap().exit(), ExitCode::SUCCESS);
}

#[test]
fn compares_a_stored_reference_without_disarm_counters() {
    let reference = report(10.0, 100, 300, 0);
    let mut result = report(10.0, 100, 300, 0);
    let group = result.groups.get_mut("dense").unwrap();
    let WindowSample::Valid(sample) = &mut group.windows[0].sample else { panic!("expected valid window") };
    let state =
        super::super::state::State::parse("disarm.v1 total=3 unknown=1 no_completed_record=1 recorded=1 dropped=2");
    sample.delta.extend(state.counters);
    sample.sections.extend(state.sections);
    group.summarize().unwrap();
    let stored = serde_json::to_string(&reference).unwrap();
    let reference = serde_json::from_str(&stored).unwrap();
    assert_eq!(compare(&reference, &result, 0.95).unwrap().exit(), ExitCode::SUCCESS);
}

#[test]
fn rejects_one_sided_groups_and_incompatible_or_incomplete_reports() {
    for case in [
        "missing reference group",
        "missing result group",
        "base",
        "bench",
        "window length",
        "window count",
        "skipped setup",
        "target",
        "failed group",
        "invalid window",
        "restore",
        "zero reference",
        "no shown pictures",
        "invalid flip count",
        "missing flip count",
    ] {
        let mut reference = report(10.0, if case == "zero reference" { 0 } else { 100 }, 300, 10);
        let mut result = report(10.0, 100, 300, 10);
        match case {
            "missing reference group" => {
                result.groups.insert("extra".into(), GroupReport::default());
            }
            "missing result group" => {
                reference.groups.insert("extra".into(), GroupReport::default());
            }
            "base" => {
                result.base_setup.clear();
            }
            "bench" => result.bench_sha256 = "other".into(),
            "window length" => result.window_s = 5.0,
            "window count" => result.groups.get_mut("dense").unwrap().expected_windows = 2,
            "skipped setup" => result.skipped_setup_keys.push("ch1.label".into()),
            "target" => {
                result.groups.get_mut("dense").unwrap().overrides = result.base_setup.clone();
            }
            "failed group" => result.groups.get_mut("dense").unwrap().error = Some("failed".into()),
            "invalid window" => {
                result.groups.get_mut("dense").unwrap().windows[0].sample = WindowSample::CounterReset;
            }
            "restore" => result.error = Some("restore failed".into()),
            "no shown pictures" => result = report(10.0, 100, 0, 0),
            "invalid flip count" => result = report(10.0, 100, 300, 301),
            "missing flip count" => {
                let WindowSample::Valid(sample) = &mut result.groups.get_mut("dense").unwrap().windows[0].sample else {
                    panic!("expected a valid window")
                };
                sample.delta.remove("flip.shown");
            }
            _ => {}
        }
        let error = compare(&reference, &result, 0.95).unwrap_err();
        if matches!(case, "bench" | "window length" | "window count") {
            assert!(error.contains(case), "{error}");
        }
    }
}
