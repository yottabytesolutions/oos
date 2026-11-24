// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn report(groups: BTreeMap<String, Group>) -> Report {
    Report::new("offline".into(), "offline".into(), 2, "offline".into(), groups)
}

fn timing(median: f64, min: f64, max: f64) -> Timing {
    Timing::new(median, min, max, 5, 10)
}

fn single(stage: &str, stage_timing: Option<Timing>) -> Report {
    report(BTreeMap::from([(
        "group".into(),
        Group::new(
            "a".repeat(64),
            BTreeMap::from([("place".into(), Some(timing(100.0, 100.0, 100.0))), (stage.into(), stage_timing)]),
        ),
    )]))
}

#[test]
fn stage_thresholds_use_placement_cost_and_combined_spread() {
    for (stage, median, round_range, change, faster, slower) in [
        ("place", 100.0, 0.0, 2.0, Verdict::Same, Verdict::Same),
        ("place", 100.0, 0.0, 3.0, Verdict::Faster, Verdict::Regression),
        ("place", 100.0, 2.0, 4.0, Verdict::Same, Verdict::Same),
        ("place", 100.0, 2.0, 5.0, Verdict::Faster, Verdict::Regression),
        ("stage", 10.0, 2.0, 3.0, Verdict::Same, Verdict::Same),
        ("stage", 10.0, 2.0, 5.0, Verdict::Faster, Verdict::Regression),
        ("stage", 10.0, 0.0, 2.0, Verdict::WithinLimits, Verdict::WithinLimits),
        ("stage", 10.0, 0.0, 3.0, Verdict::Faster, Verdict::Regression),
        ("stage", 200.0, 0.0, 4.0, Verdict::Same, Verdict::Same),
    ] {
        let measured = |ns| timing(ns, ns - round_range / 2.0, ns + round_range / 2.0);
        for (delta, expected) in [(-change, faster), (change, slower)] {
            let actual = timing_verdict(stage, measured(median), measured(median + delta), 100.0, 0.02);
            assert_eq!(actual, expected, "{stage}: median {median}, round range {round_range}, delta {delta}");
        }
    }
}

#[test]
fn reports_sampling_shortfalls_on_either_side() {
    let measured = single("stage", Some(timing(10.0, 10.0, 10.0)));
    for (rounds, samples, needed) in [(3, 30, 5), (5, 5, 10), (5, 8, 7), (9, 9, 10)] {
        let short = single("stage", Some(Timing::new(15.0, 15.0, 15.0, rounds, samples)));
        for (reference, result, label) in [(&measured, &short, "result"), (&short, &measured, "reference")] {
            let comparison = compare(reference, result, 0.02).unwrap();
            assert_eq!(comparison.rows[0].verdict, Verdict::Same);
            assert!(comparison.rows[1].insufficient());
            assert!(comparison.table().contains(&format!(
                "insufficient data ({label} has {rounds} rounds and {samples} samples, needs at least {needed} rounds"
            )));
        }
    }
}

#[test]
fn rejects_inconsistent_sampling_counts() {
    let measured = single("stage", Some(timing(10.0, 10.0, 10.0)));
    for (rounds, samples) in [(0, 0), (5, 4)] {
        let invalid = single("stage", Some(Timing::new(15.0, 15.0, 15.0, rounds, samples)));
        assert_eq!(compare(&measured, &invalid, 0.02).unwrap_err(), "group 'group', stage 'stage': invalid timing");
    }
}

fn recorded() -> BTreeMap<String, Report> {
    serde_json::from_str(include_str!("fixtures/recorded.json")).unwrap()
}

fn deep_runs(reports: &BTreeMap<String, Report>) -> (&Report, [&Report; 3]) {
    let reference = reports
        .iter()
        .find(|(name, _)| name.strip_prefix("ref-").is_some_and(|tag| tag.bytes().all(|b| b.is_ascii_hexdigit())))
        .unwrap()
        .1;
    // The first run has no repetition suffix. The other two sort in run order.
    let runs: Vec<_> = reports.iter().filter(|(name, _)| name.starts_with("int1-")).map(|(_, report)| report).collect();
    (reference, runs.try_into().unwrap())
}

#[test]
fn archived_deep_summaries_need_fresh_counts() {
    let reports = recorded();
    let (reference, [first, _, _]) = deep_runs(&reports);
    let comparison = compare(reference, first, 0.02).unwrap();
    assert!(comparison.rows.iter().all(Row::insufficient));
    assert!(comparison.table().contains("reference counts unavailable, rerun with at least 5 rounds and 10 samples"));
    assert!(comparison.table().contains("result counts unavailable, rerun with at least 5 rounds and 10 samples"));
}

#[test]
fn archived_deep_tile_changes_are_below_the_placement_floor() {
    let reports = recorded();
    let (reference, [first, second, third]) = deep_runs(&reports);
    let repeat = &reports["ref-deep"];
    let pairs =
        [(reference, first, Verdict::WithinLimits), (repeat, second, Verdict::Same), (repeat, third, Verdict::Same)];
    for (index, (reference, result, expected)) in pairs.into_iter().enumerate() {
        let old = &reference.groups["deep"].stages;
        let new = &result.groups["deep"].stages;
        let place = old["place"].unwrap().median;
        let (old, new) = (old["tile_writes"].unwrap(), new["tile_writes"].unwrap());
        assert_eq!(timing_verdict("tile_writes", old, new, place, 0.02), expected, "run {}", index + 1);
    }
}

#[test]
fn recorded_same_binary_reports_have_no_change_beyond_the_spread() {
    let reports: BTreeMap<String, Report> = serde_json::from_str(include_str!("fixtures/same-binary.json")).unwrap();
    for (reference, result) in [("reference", "result"), ("result", "reference")] {
        let comparison = compare(&reports[reference], &reports[result], 0.02).unwrap();
        let rows = comparison.rows();
        assert_eq!(
            rows.iter().filter(|row| row.stage == "place" && row.verdict == Verdict::Same).count(),
            4,
            "{}",
            comparison.table()
        );
        assert!(
            rows.iter().all(|row| matches!(row.verdict, Verdict::Same | Verdict::Unmeasured)),
            "{}",
            comparison.table()
        );
    }
}

#[test]
fn placement_median_catches_a_common_shift_inside_every_group_spread() {
    let mut reports = recorded();
    let mut reference = reports.remove("ref-cpu2").unwrap();
    for group in reference.groups.values_mut() {
        group.stages.get_mut("place").unwrap().as_mut().unwrap().sampling = Some(Sampling { rounds: 5, samples: 10 });
    }
    for (changes, median, slower, faster, exit) in
        [([0.025; 4], 0.025, 4, 0, 1), ([-0.025, -0.025, 0.025, 0.025], 0.0, 2, 2, 0), ([-0.025; 4], -0.025, 0, 4, 0)]
    {
        let mut result: Report = serde_json::from_value(serde_json::to_value(&reference).unwrap()).unwrap();
        for (group, change) in result.groups.values_mut().zip(changes) {
            let timing = group.stages.get_mut("place").unwrap().as_mut().unwrap();
            timing.median *= 1.0 + change;
            timing.min_round_median *= 1.0 + change;
            timing.max_round_median *= 1.0 + change;
        }
        let comparison = compare(&reference, &result, 0.02).unwrap();
        assert!(comparison.rows.iter().all(|row| row.verdict == Verdict::Same), "{}", comparison.table());
        let placement = Placement::summarize(comparison.rows(), 0.02);
        assert_eq!((placement.slower, placement.faster), (slower, faster));
        assert!((placement.median - median).abs() < 1e-12);
        assert_eq!(super::super::compare(&reference, &result, 0.02).unwrap(), std::process::ExitCode::from(exit));
    }
}

#[test]
fn archived_cpu_placement_timings_trigger_the_gate_with_sufficient_sampling() {
    let mut reports = recorded();
    for name in ["a53-cpu2", "ref-cpu2"] {
        for group in reports.get_mut(name).unwrap().groups.values_mut() {
            // Supply synthetic coverage independently of the archived timing values.
            group.stages.get_mut("place").unwrap().as_mut().unwrap().sampling =
                Some(Sampling { rounds: 5, samples: 10 });
        }
    }
    for (reference, result, expected) in
        [("a53-cpu2", "ref-cpu2", Verdict::Regression), ("ref-cpu2", "a53-cpu2", Verdict::Faster)]
    {
        let comparison = compare(&reports[reference], &reports[result], 0.02).unwrap();
        assert_eq!(comparison.rows.len(), 4);
        for row in comparison.rows() {
            assert_eq!(row.stage, "place");
            let verdict = if row.group == "hires-200ns" { &expected } else { &Verdict::Same };
            assert_eq!(&row.verdict, verdict, "{}", comparison.table());
        }
    }
}

#[test]
fn compares_nulls_and_rejects_missing_groups_and_asymmetric_measurements() {
    let reference = single("trigger", None);
    let result = single("trigger", Some(timing(10.0, 10.0, 10.0)));
    assert_eq!(compare(&reference, &reference, 0.02).unwrap().rows[1].verdict, Verdict::Unmeasured);

    let missing_group = report(BTreeMap::new());
    for (left, right, expected) in [
        (&reference, &result, "group 'group', stage 'trigger': unmeasured in reference only"),
        (&result, &reference, "group 'group', stage 'trigger': unmeasured in result only"),
        (&missing_group, &result, "missing group 'group' in reference"),
        (&result, &missing_group, "missing group 'group' in result"),
    ] {
        assert_eq!(compare(left, right, 0.02).unwrap_err(), expected);
    }
}

#[test]
fn equal_stage_sets_only_show_the_comparison_table() {
    let report = single("trigger", Some(timing(10.0, 10.0, 10.0)));
    let comparison = compare(&report, &report, 0.02).unwrap();
    assert_eq!(comparison.rows.len(), 2);
    assert!(!comparison.table().contains("-only stages"));
    assert_eq!(super::super::compare(&report, &report, 0.02), Ok(std::process::ExitCode::SUCCESS));
}

#[test]
fn added_and_removed_stages_show_their_medians_without_affecting_the_exit_code() {
    let with_average = single("average_accumulate", Some(Timing::new(12.5, 12.5, 12.5, 1, 1)));
    let mut without_average = single("average_accumulate", None);
    without_average.groups.get_mut("group").unwrap().stages.remove("average_accumulate");
    for (reference, result, heading, other) in [
        (&without_average, &with_average, "Candidate-only stages", "Reference-only stages"),
        (&with_average, &without_average, "Reference-only stages", "Candidate-only stages"),
    ] {
        let comparison = compare(reference, result, 0.02).unwrap();
        assert_eq!(comparison.rows.len(), 1);
        assert_eq!(comparison.rows[0].stage, "place");
        let table = comparison.table();
        assert!(table.ends_with(&format!("\n{heading} (median ns/rec)\ngroup  average_accumulate  12.5\n")), "{table}");
        assert!(!table.contains(other), "{table}");
        assert_eq!(super::super::compare(reference, result, 0.02), Ok(std::process::ExitCode::SUCCESS));
    }
}

#[test]
fn required_placement_stage_must_be_present_on_both_sides() {
    let mut missing_place = single("trigger", Some(timing(10.0, 10.0, 10.0)));
    missing_place.groups.get_mut("group").unwrap().stages.remove("place");
    assert_eq!(
        compare(&missing_place, &missing_place, 0.02).unwrap_err(),
        "group 'group' requires a positive 'place' median in reference"
    );
}

#[test]
fn rejects_cpu_core_type_and_fixture_changes() {
    let reference = single("trigger", None);
    let mut result = single("trigger", None);
    result.cpu = 3;
    assert_eq!(compare(&reference, &result, 0.02).unwrap_err(), "CPU differs: reference 2, result 3");
    result.cpu = reference.cpu;
    result.core_type = "different".into();
    assert!(compare(&reference, &result, 0.02).unwrap_err().starts_with("CPU core type differs:"));
    result.core_type.clone_from(&reference.core_type);
    result.groups.get_mut("group").unwrap().fixture_sha256 = "b".repeat(64);
    assert_eq!(
        compare(&reference, &result, 0.02).unwrap_err(),
        "group 'group': fixture SHA-256 differs between reference and result"
    );
}

#[test]
#[ignore = "requires two recorded benchmark JSONL files outside the repository"]
fn compare_recorded_examples() {
    let groups = ["short-1ch-200ns", "dense-2ch-1us"].map(|group| {
        let path = std::path::PathBuf::from(std::env::var_os("OOS_BENCH_EXAMPLES").unwrap());
        let text = std::fs::read_to_string(path.join(format!("bench-j1-{group}.jsonl"))).unwrap();
        (group.into(), Group::new("a".repeat(64), crate::unit_bench::measure::summarize([text]).unwrap()))
    });
    let reference = report(BTreeMap::from(groups));
    let result = serde_json::to_string(&reference).unwrap();
    let result_report = serde_json::from_str(&result).unwrap();
    let comparison = compare(&reference, &result_report, 0.02).unwrap();
    assert_eq!(comparison.rows.len(), 32);
    assert!(comparison.rows.iter().all(|row| !row.regression()));
    println!("{result}");
}
