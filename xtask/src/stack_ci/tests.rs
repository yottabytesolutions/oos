// SPDX-License-Identifier: GPL-2.0-or-later
use super::{check, checkout, env, parse, required, workflow, CODEX_GATE, REQUIRED};
use yaml_rust2::Yaml;

/// The rules of `check` for one workflow text.
fn accepted(name: &str, text: &str) -> Result<(), String> {
    let doc = parse(text)?;
    REQUIRED.iter().filter(|(_, w, _)| *w == name).try_for_each(|(c, _, s)| required(&doc, c, *s))
}

fn mutated(name: &str, from: &str, to: &str) -> Result<(), String> {
    let text = workflow(name).expect("built in");
    assert!(text.contains(from), "{name} no longer contains {from:?}: update the mutation");
    accepted(name, &text.replacen(from, to, 1))
}

#[test]
fn every_required_check_runs_on_a_stacked_pull_request() {
    assert_eq!(check(), Ok(()));
}

#[test]
fn filters_are_seen_in_every_yaml_spelling() {
    let cases = [
        ("block", "  pull_request:\n    branches: [main]\n"),
        ("flow", "  pull_request: {branches: [main]}\n"),
        ("escaped key", "  pull_request: {\"bran\\u0063hes\": [main]}\n"),
        ("deep indent", "  pull_request:\n        paths-ignore: ['docs/**']\n"),
        ("merge key", "  pull_request:\n    <<: {branches: [main]}\n"),
    ];
    for (case, to) in cases {
        assert!(mutated("privacy.yml", "  pull_request:\n", to).is_err(), "{case} filter passed");
    }
    let alias =
        mutated("privacy.yml", "  push:\n  pull_request:\n", "  push: &f {branches: [main]}\n  pull_request: *f\n");
    assert!(alias.is_err(), "aliased filter passed");
}

#[test]
fn activity_types_must_cover_open_and_update() {
    let from = "types: [opened, edited, synchronize, reopened, labeled, unlabeled]";
    assert!(mutated("pr-meta.yml", from, "types: [closed]").is_err());
    assert!(mutated("pr-meta.yml", from, "types: [opened, reopened]").is_err());
    assert_eq!(mutated("pr-meta.yml", from, "types: [opened, synchronize, reopened]"), Ok(()));
}

#[test]
fn the_required_name_comes_from_the_job() {
    assert!(mutated("privacy.yml", "  privacy:\n", "  scan:\n").is_err());
    assert!(mutated("privacy.yml", "  privacy:\n    runs-on", "  privacy:\n    \"name\": scan\n    runs-on").is_err());
    assert_eq!(mutated("privacy.yml", "  privacy:\n    runs-on", "  privacy:\n    name: privacy\n    runs-on"), Ok(()));
    let condition = "  privacy:\n    if: github.base_ref == 'main'\n    runs-on";
    assert!(mutated("privacy.yml", "  privacy:\n    runs-on", condition).is_err());
    assert_eq!(mutated("privacy.yml", "  privacy:\n    runs-on", "  privacy:\n    if: always()\n    runs-on"), Ok(()));
    let deep = "on:\n  pull_request:\njobs:\n  privacy:\n        name: scan\n        runs-on: x\n";
    assert!(accepted("privacy.yml", deep).is_err());
}

#[test]
fn trigger_lists_and_names_are_read() {
    let job = "jobs:\n  privacy:\n    runs-on: x\n";
    assert_eq!(accepted("privacy.yml", &format!("on: [push, pull_request]\n{job}")), Ok(()));
    assert_eq!(accepted("privacy.yml", &format!("on: pull_request\n{job}")), Ok(()));
    assert!(accepted("privacy.yml", &format!("on: push\n{job}")).is_err());
    assert!(accepted("privacy.yml", &format!("on:\n  pull_request: x\n{job}")).is_err());
    assert!(parse("a: 1\n---\nb: 2\n").is_err());
}

#[test]
fn the_ratchet_base_is_the_layer_below() {
    let doc = parse(workflow("pr.yml").expect("built in")).expect("pr.yml");
    assert_eq!(env(&doc, "OOS_GATE_BASE"), Some("origin/${{ github.event.pull_request.base.ref || 'main' }}"));
    assert_eq!(
        env(&doc, "BASE"),
        Some("${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha }}")
    );
    // origin/<layer> exists in the checkout only with the full history of every branch.
    for job in ["fast", "clippy", "test-run", "python", "deny"] {
        assert_eq!(checkout(&doc, job, "fetch-depth"), Some(&Yaml::Integer(0)), "{job}");
    }
}

#[test]
fn the_codex_status_is_published_by_the_default_branch_gate() {
    assert!(CODEX_GATE.contains("CONTEXT: Final = \"codex-review\""));
    let doc = parse(workflow("codex-review.yml").expect("built in")).expect("codex-review.yml");
    let default = Yaml::String("${{ github.event.repository.default_branch }}".to_string());
    assert_eq!(checkout(&doc, "codex-gate", "ref"), Some(&default));
    let pr = "if: github.event_name != 'merge_group'";
    assert!(mutated("codex-review.yml", pr, "if: github.event_name == 'merge_group'").is_err());
    assert!(mutated("codex-review.yml", &format!("        {pr}\n"), "").is_err());
    assert!(mutated("codex-review.yml", " --sha \"$SHA\" --publish", " --sha \"$SHA\"").is_err());
    assert!(mutated("codex-review.yml", " --pr \"$PR\" --publish", " --publish").is_err());
    assert!(mutated("codex-review.yml", "      statuses: write\n", "      statuses: read\n").is_err());
    assert!(mutated("codex-review.yml", "      statuses: write\n", "").is_err());
    let token = "          GITHUB_TOKEN: ${{ github.token }}\n";
    assert!(mutated("codex-review.yml", token, "").is_err());
    assert!(mutated("codex-review.yml", token, "          GITHUB_TOKEN: ''\n").is_err());
}

#[test]
fn required_checks_come_from_github_actions() {
    let ruleset = super::RULESET.replacen("\"integration_id\": 15368", "\"integration_id\": 1", 1);
    assert!(super::required_contexts(&ruleset).is_err());
    assert_eq!(super::required_contexts(super::RULESET).map(|c| c.len()), Ok(REQUIRED.len()));
}

#[test]
fn the_informational_report_does_not_soften_required_steps() {
    assert_eq!(accepted("pr.yml", workflow("pr.yml").expect("built in")), Ok(()));
    let gate = "        run: cargo xtask check --fast\n";
    assert!(mutated("pr.yml", gate, &format!("        continue-on-error: true\n{gate}")).is_err());
    assert!(mutated("pr.yml", "  fast:\n", "  fast:\n    continue-on-error: true\n").is_err());
    assert!(mutated("pr.yml", "Production code change against the base", "Fast gate report").is_err());
}

#[test]
fn required_jobs_run_on_every_pull_request() {
    let soft = "  privacy:\n    continue-on-error: true\n    runs-on";
    assert!(mutated("privacy.yml", "  privacy:\n    runs-on", soft).is_err());
    let step = "      - name: Incoming additions and commit messages\n";
    assert!(mutated("privacy.yml", step, &format!("{step}        continue-on-error: true\n")).is_err());
    let needs = "  privacy:\n    needs: setup\n    runs-on";
    assert!(mutated("privacy.yml", "  privacy:\n    runs-on", needs).is_err());
    let codex = "github.event_name != 'issue_comment' || github.event.issue.pull_request";
    assert!(mutated("codex-review.yml", codex, "github.base_ref == 'main'").is_err());
    assert!(mutated("pr.yml", "  test:\n    needs: test-run\n    if: always()\n", "  test:\n    needs: test-run\n")
        .is_err());
}
