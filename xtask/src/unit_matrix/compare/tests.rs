// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_matrix::{
    report::Summary,
    state::PRESENTATION,
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
            sections: [(PRESENTATION.into(), Default::default())].into(),
        }),
    };
    // Only the per-window deltas carry flip counters.
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
    let reference = report(10.0, 1000, 300, 0);
    for (records, pictures, expected) in [(2000, 600, 0), (1900, 580, 0), (1899, 600, 1), (2000, 579, 1)] {
        let result = report(20.0, records, pictures, 0);
        let comparison = compare(&reference, &result, 0.95).unwrap();
        assert_eq!(comparison.exit(), ExitCode::from(expected));
        let text = table(&comparison);
        assert_eq!(text.lines().count(), 3);
        assert_eq!(text.contains("repeat group, then investigate"), expected == 1);
    }
    assert_eq!(per_minute(10.0, [9.0, 11.0]), 60.0);
}

#[test]
fn applies_an_absolute_late_flip_limit_to_the_result_group() {
    for (shown, reference_late, result_late, expected) in
        [(300, 0, 0, 0), (300, 10, 3, 0), (300, 0, 4, 1), (300, 4, 4, 1), (300, 10, 4, 1), (399, 4, 4, 1)]
    {
        let reference = report(10.0, 100, shown, reference_late);
        let result = report(10.0, 100, shown, result_late);
        let comparison = compare(&reference, &result, 0.95).unwrap();
        assert_eq!(comparison.exit(), ExitCode::from(expected), "{reference_late} vs {result_late}");
        assert_eq!(comparison.rows[0].regression, expected == 1);
    }
}

#[test]
fn a_group_over_the_late_limit_fails_even_if_the_run_total_is_below_it() {
    let mut reference = report(10.0, 100, 300, 0);
    let mut result = report(10.0, 100, 300, 4);
    for report in [&mut reference, &mut result] {
        report.groups.insert("sparse".into(), group((10.0, 100, 300, 700, 0)));
    }
    let comparison = compare(&reference, &result, 0.95).unwrap();
    assert_eq!(comparison.exit(), ExitCode::FAILURE);
    let text = table(&comparison);
    let mut lines = text.lines().skip(1);
    assert!(lines.next().unwrap().contains("REGRESSION (late flip limit)"));
    assert!(lines.next().unwrap().ends_with("pass"));
    let summary = lines.next().unwrap();
    assert!(summary.contains("reference 0/1000 (0.00%)"));
    assert!(summary.contains("result 4/1000 (0.40%)"));
}

#[test]
fn a_group_without_shown_pictures_cannot_be_compared() {
    let mut reference = report(10.0, 100, 300, 0);
    let mut result = report(10.0, 100, 300, 0);
    reference.groups.insert("sparse".into(), group((10.0, 100, 0, 0, 0)));
    result.groups.insert("sparse".into(), group((10.0, 100, 300, 300, 0)));
    let error = compare(&reference, &result, 0.95).unwrap_err();
    assert_eq!(error, "reference group 'sparse': no shown pictures");
    let error = compare(&result, &reference, 0.95).unwrap_err();
    assert_eq!(error, "result group 'sparse': no shown pictures");
}

#[test]
fn needs_target_refresh_counts_in_every_window_and_prints_group_percentages() {
    let mut reference = report(10.0, 100, 300, 3);
    let result = report(10.0, 100, 300, 6);
    let table = table(&compare(&reference, &result, 0.95).unwrap());
    let row = table.lines().nth(1).unwrap();
    assert!(row.contains("1.00%"));
    assert!(row.contains("2.00%"));
    let WindowSample::Valid(sample) = &mut reference.groups.get_mut("dense").unwrap().windows[0].sample else {
        panic!("expected a valid window");
    };
    sample.sections = [("present.v1".into(), Default::default())].into();
    let error = compare(&reference, &result, 0.95).unwrap_err();
    assert!(error.contains("reference group 'dense': requires present.v2"));
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
