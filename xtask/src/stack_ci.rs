// SPDX-License-Identifier: GPL-2.0-or-later
//! Every required check runs on a stacked pull request (CONTRIBUTING.md#stacked-pull-requests). A layer of a stack targets the branch below it,
//! not `main`, and changes only some paths. The workflows of the required checks (`.github/rulesets/main.json`) must
//! therefore run on every pull request, whatever its base and paths, and report the required name. Test-only: the
//! workflows are read with a YAML 1.2 parser, so quoting, escapes, indentation and aliases read as GitHub reads them.
//! Scope: the triggers, whether a required job runs and passes only by its own result, its check name, and that the
//! `codex-review` status gets published. What the steps of a running job check is that job's own review.

use yaml_rust2::{Yaml, YamlLoader};

#[cfg(test)]
mod tests;

/// Where a required check comes from: a job whose key is the check name, or a commit status that a job publishes
/// (`codex-review`, from `tools/ci/codex_gate.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Job(&'static str),
    Status(&'static str),
}

/// Each required check: its workflow and its source.
pub const REQUIRED: [(&str, &str, Source); 8] = [
    ("fast", "pr.yml", Source::Job("fast")),
    ("clippy", "pr.yml", Source::Job("clippy")),
    ("test", "pr.yml", Source::Job("test")),
    ("python", "pr.yml", Source::Job("python")),
    ("deny", "pr.yml", Source::Job("deny")),
    ("pr-meta", "pr-meta.yml", Source::Job("pr-meta")),
    ("privacy", "privacy.yml", Source::Job("privacy")),
    ("codex-review", "codex-review.yml", Source::Status("codex-gate")),
];
/// The files the check reads, built in: `tools/ci/changes.py` then reruns the xtask tests when one changes.
pub const RULESET: &str = include_str!("../../.github/rulesets/main.json");
pub const CODEX_GATE: &str = include_str!("../../tools/ci/codex_gate.py");
pub const WORKFLOWS: [(&str, &str); 4] = [
    ("pr.yml", include_str!("../../.github/workflows/pr.yml")),
    ("pr-meta.yml", include_str!("../../.github/workflows/pr-meta.yml")),
    ("privacy.yml", include_str!("../../.github/workflows/privacy.yml")),
    ("codex-review.yml", include_str!("../../.github/workflows/codex-review.yml")),
];
/// The GitHub Actions app: every required check must come from it, or no workflow here can satisfy it.
const ACTIONS_APP: i64 = 15368;
/// The `codex_gate.py` calls that publish the `codex-review` status, and the step condition each must carry: the PR
/// call runs on every event but merge groups, the queue call on merge groups. Each call fails on the other's event.
const PUBLISHERS: [(&str, &str); 2] = [
    ("tools/ci/codex_gate.py pr --repo \"$REPO\" --pr \"$PR\" --publish", "github.event_name != 'merge_group'"),
    ("tools/ci/codex_gate.py queue --repo \"$REPO\" --sha \"$SHA\" --publish", "github.event_name == 'merge_group'"),
];
/// Job conditions that hold for every pull request event: `always()`, and the Codex gate's filter of comment events.
const EVERY_PR: [&str; 2] = ["always()", "github.event_name != 'issue_comment' || github.event.issue.pull_request"];
/// Filters that skip a pull request by its base branch or its paths.
const FILTERS: [&str; 4] = ["branches", "branches-ignore", "paths", "paths-ignore"];
/// The activity types a stacked layer needs: opened, and updated by a push or a rebase.
const UPDATES: [&str; 3] = ["opened", "synchronize", "reopened"];

/// One workflow file, parsed. Aliases are resolved by the parser; a merge key or an unresolved alias is refused,
/// so the check never reads less than GitHub does.
pub fn parse(text: &str) -> Result<Yaml, String> {
    let mut docs = YamlLoader::load_from_str(text).map_err(|e| format!("not YAML: {e}"))?;
    if docs.len() != 1 {
        return Err(format!("{} YAML documents, expected one", docs.len()));
    }
    let doc = docs.remove(0);
    plain(&doc)?;
    Ok(doc)
}

fn plain(node: &Yaml) -> Result<(), String> {
    match node {
        Yaml::BadValue | Yaml::Alias(_) => Err("unresolved alias or bad value".to_string()),
        Yaml::Array(items) => items.iter().try_for_each(plain),
        Yaml::Hash(map) => map.iter().try_for_each(|(k, v)| {
            if k.as_str() == Some("<<") {
                return Err("merge key `<<`".to_string());
            }
            plain(k).and_then(|()| plain(v))
        }),
        _ => Ok(()),
    }
}

fn get<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    node.as_hash()?.get(&Yaml::String(key.to_string()))
}

fn strings(node: &Yaml) -> Option<Vec<&str>> {
    match node {
        Yaml::String(s) => Some(vec![s.as_str()]),
        Yaml::Array(items) => items.iter().map(Yaml::as_str).collect(),
        _ => None,
    }
}

/// The workflow runs on every pull request: any base branch, any paths, opened and every update.
pub fn runs_on_every_pull_request(doc: &Yaml) -> Result<(), String> {
    let on = get(doc, "on").ok_or("no `on`")?;
    if let Some(events) = strings(on) {
        return if events.contains(&"pull_request") { Ok(()) } else { Err("no pull_request trigger".to_string()) };
    }
    let pr = get(on, "pull_request").ok_or("no pull_request trigger")?;
    if pr.is_null() {
        return Ok(());
    }
    if pr.as_hash().is_none() {
        return Err(format!("pull_request is {pr:?}"));
    }
    if let Some(filter) = FILTERS.iter().find(|f| get(pr, f).is_some()) {
        return Err(format!("pull_request has a `{filter}` filter: a stacked layer would lose the check"));
    }
    match get(pr, "types") {
        None => Ok(()),
        Some(types) => {
            let types = strings(types).ok_or("pull_request types are not a list of names")?;
            match UPDATES.iter().find(|t| !types.contains(t)) {
                Some(missing) => Err(format!("pull_request types lack `{missing}`")),
                None => Ok(()),
            }
        }
    }
}

/// The job `job` exists and runs on every pull request. GitHub reports a job skipped by its `if`, or by a failed or
/// skipped `needs` without `if: always()`, as a passing check, so its gate would never run.
pub fn runs_always<'a>(doc: &'a Yaml, job: &str) -> Result<&'a Yaml, String> {
    let jobs = get(doc, "jobs").ok_or("no jobs")?;
    let found = get(jobs, job).ok_or_else(|| format!("no job `{job}`"))?;
    let condition = get(found, "if").map(|c| c.as_str().ok_or_else(|| format!("job `{job}` has if {c:?}")));
    let condition = condition.transpose()?;
    if condition.is_some_and(|c| !EVERY_PR.contains(&c)) {
        return Err(format!("job `{job}` runs only if {condition:?}: a skipped job passes its required check"));
    }
    if get(found, "needs").is_some() && condition != Some("always()") {
        return Err(format!("job `{job}` needs other jobs without `if: always()`: their failure skips it"));
    }
    let soft = |node: &Yaml| get(node, "continue-on-error").is_some_and(|c| c.as_bool() != Some(false));
    let soft_step = |step: &Yaml| {
        let informational = job == "fast"
            && get(step, "name").and_then(Yaml::as_str) == Some("Production code change against the base");
        soft(step) && !informational
    };
    if soft(found) || steps(doc, job).into_iter().any(soft_step) {
        return Err(format!("job `{job}` or a step of it continues on error: a failure would pass the required check"));
    }
    Ok(found)
}

/// The job `job` runs on every pull request and reports `context`: GitHub names the check after the job key unless
/// `name` overrides it.
pub fn reports(doc: &Yaml, job: &str, context: &str) -> Result<(), String> {
    let found = runs_always(doc, job)?;
    match get(found, "name") {
        None => Ok(()),
        Some(name) if name.as_str() == Some(context) => Ok(()),
        Some(name) => Err(format!("job `{job}` is named {name:?}, not {context:?}")),
    }
}

/// The steps of `job`, empty when it has none.
pub fn steps<'a>(doc: &'a Yaml, job: &str) -> Vec<&'a Yaml> {
    get(doc, "jobs")
        .and_then(|jobs| get(jobs, job))
        .and_then(|j| get(j, "steps"))
        .and_then(Yaml::as_vec)
        .map(|s| s.iter().collect())
        .unwrap_or_default()
}

/// The `with` value `key` of the checkout step of `job`.
pub fn checkout<'a>(doc: &'a Yaml, job: &str, key: &str) -> Option<&'a Yaml> {
    steps(doc, job)
        .into_iter()
        .find(|s| get(s, "uses").and_then(Yaml::as_str).is_some_and(|u| u.starts_with("actions/checkout@")))
        .and_then(|s| get(s, "with"))
        .and_then(|w| get(w, key))
}

/// A value of the workflow's top-level `env`.
pub fn env<'a>(doc: &'a Yaml, key: &str) -> Option<&'a str> {
    get(doc, "env").and_then(|e| get(e, key)).and_then(Yaml::as_str)
}

/// The required checks of the ruleset.
pub fn required_contexts(ruleset: &str) -> Result<Vec<String>, String> {
    let json: serde_json::Value = serde_json::from_str(ruleset).map_err(|e| e.to_string())?;
    let rules = json["rules"].as_array().ok_or("ruleset has no rules")?;
    let checks: Vec<serde_json::Value> = rules
        .iter()
        .filter(|r| r["type"] == "required_status_checks")
        .flat_map(|r| r["parameters"]["required_status_checks"].as_array().cloned().unwrap_or_default())
        .collect();
    if let Some(other) = checks.iter().find(|c| c["integration_id"].as_i64() != Some(ACTIONS_APP)) {
        return Err(format!("required check {other} is not bound to GitHub Actions ({ACTIONS_APP})"));
    }
    let mut out: Vec<String> = checks.iter().filter_map(|c| c["context"].as_str().map(str::to_string)).collect();
    out.sort();
    Ok(out)
}

/// The built-in text of a workflow the check reads.
pub fn workflow(name: &str) -> Result<&'static str, String> {
    WORKFLOWS.iter().find(|(n, _)| *n == name).map(|(_, text)| *text).ok_or_else(|| format!("{name} is not built in"))
}

/// The rules for one required check in the workflow `doc`.
pub fn required(doc: &Yaml, context: &str, source: Source) -> Result<(), String> {
    runs_on_every_pull_request(doc)?;
    match source {
        Source::Job(job) => reports(doc, job, context),
        Source::Status(job) => runs_always(doc, job).and_then(|_| publishes(doc, job)),
    }
}

/// The job `job` publishes the status on pull requests and merge groups: each `codex_gate.py` call with `--publish`,
/// in a step that runs on its events.
pub fn publishes(doc: &Yaml, job: &str) -> Result<(), String> {
    // A job's `permissions` replaces the workflow's; without `statuses: write` the POST fails and the gate only warns.
    let permissions = get(doc, "jobs").and_then(|j| get(j, job)).and_then(|j| get(j, "permissions"));
    let statuses = permissions.or_else(|| get(doc, "permissions")).and_then(|p| match p {
        Yaml::String(all) => Some(all.as_str()),
        _ => get(p, "statuses").and_then(Yaml::as_str),
    });
    if !matches!(statuses, Some("write" | "write-all")) {
        return Err(format!("job `{job}` cannot write commit statuses ({statuses:?})"));
    }
    let steps = steps(doc, job);
    for (call, condition) in PUBLISHERS {
        let found = steps.iter().any(|s| {
            let run = get(s, "run").and_then(Yaml::as_str).unwrap_or_default();
            let when = get(s, "if").and_then(Yaml::as_str);
            run.contains(call) && when == Some(condition) && has_token(doc, job, s)
        });
        if !found {
            return Err(format!("job `{job}` has no step `{call}` that runs if {condition}"));
        }
    }
    Ok(())
}

/// The step gets the workflow token as `GITHUB_TOKEN` or `GH_TOKEN` (step, job or workflow `env`): without it the gate
/// reads anonymously, fails to publish and only warns.
fn has_token(doc: &Yaml, job: &str, step: &Yaml) -> bool {
    let job = get(doc, "jobs").and_then(|j| get(j, job));
    let scopes = [Some(step), job, Some(doc)];
    ["GITHUB_TOKEN", "GH_TOKEN"].iter().any(|name| {
        let value = scopes.iter().flatten().find_map(|scope| get(scope, "env").and_then(|e| get(e, name)));
        value.and_then(Yaml::as_str).is_some_and(|v| v == "${{ github.token }}" || v == "${{ secrets.GITHUB_TOKEN }}")
    })
}

/// Every required check of the ruleset runs on a stacked pull request and reports its name.
pub fn check() -> Result<(), String> {
    let mut listed: Vec<String> = REQUIRED.iter().map(|(c, _, _)| (*c).to_string()).collect();
    listed.sort();
    if required_contexts(RULESET)? != listed {
        return Err("the ruleset's required checks differ from stack_ci::REQUIRED".to_string());
    }
    for (context, name, source) in REQUIRED {
        let doc = parse(workflow(name)?).map_err(|e| format!("{name}: {e}"))?;
        required(&doc, context, source).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}
