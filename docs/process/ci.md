# CI pipeline

Every pull request runs the gate on GitHub-hosted runners, needs a Codex review of its head commit and merges through
the merge queue into a linear `main` by rebase: every commit of a PR lands on `main` as it is. Settings live in Git
as files and a script; nothing in the workflows changes GitHub settings. Task 0092
(`docs/tasks/done/0092-pipeline.md`) built it.

## Workflows

All `runs-on` labels are literal `ubuntu-24.04`, `ubuntu-24.04-arm` or `ubuntu-latest` (`pr-meta` checks it),
top-level permissions are read-only, actions are pinned by commit SHA, every job has `timeout-minutes`, and no PR
workflow uses a secret.

| Workflow | Trigger | Jobs (required checks in bold) |
|---|---|---|
| pr.yml | pull_request, merge_group, push main | **fast**, **clippy**, test (debug), test (release), **test** (aggregate), **python**, **deny**, timings |
| pr-meta.yml | PR opened/edited/sync/labels, merge_group | **pr-meta** |
| privacy.yml | push, pull_request, merge_group | **privacy** |
| codex-review.yml | PR, reviews, review comments, issue comments, merge_group | codex-gate, publishes status **codex-review** |
| main.yml | nightly, dispatch | gate (the default gate split into parallel jobs, LTO release tests, coverage, no perf), deny, image, timings |
| image.yml | workflow_call, dispatch | image on ubuntu-24.04-arm |
| release.yml | tag vX.Y.Z | verify, image, release (draft, provenance, changelog) |
| canary.yml | weekly, dispatch | stable (latest stable clippy and tests), report (issue on failure) |
| scorecard.yml | push main, weekly, branch protection change, dispatch | analysis (OpenSSF Scorecard, SARIF upload) |
| stale.yml | daily | stale (PRs only) |
| labeler.yml | PR (same repository only) | label (paths, `agent` for the agent app) |

The jobs call `cargo xtask step <id>...`, so CI and the local gate share one definition
([ADR 0031](../adr/process/0031-the-gate-is-cargo-xtask-check.md)). The hooks run only the fast tier; the full gate
is these required checks. Every PR job reports its required name; `tools/ci/changes.py` skips the Rust build steps
for changes to documents, GitHub settings or CI scripts, and in the merge queue for a tree that already passed on its
PR head. A push to `main` only warms the caches. The timings jobs summarise job and step durations per run
(`tools/ci/run_summary.py`). The rules of `pr-meta` are in
`tools/ci/pr_meta.py`: branch name, title, every commit subject (Conventional, at most 72 characters, no `fixup!`,
`squash!`, `amend!` or WIP commits), DCO sign-off matching the author, runner labels; size, task link, tool details
in commit messages and the agent label are warnings.

The ruleset is `.github/rulesets/main.json`; `tools/ci/apply-repo-settings.sh` applies it and the repository
settings (dry run by default, `--apply` to write).

## Codex review gate

Codex keeps one summary comment per PR with `<!-- codex-review-state:v1 {json} -->`. `tools/ci/codex_gate.py` reads
the bot's newest marker and sets the commit status `codex-review` on the PR head:

- success: the review is of the head commit, completed, without unresolved findings;
- pending: no marker yet, a review of an older commit, or a review still running;
- failure: unresolved findings, or a marker that does not parse.

A new push makes the status pending until Codex reviews the new head (automatically, or after `@codex review`). In
the merge queue the group commit gets success: a PR enters the queue only with a passing `codex-review`. When Codex
is down, an admin merges through the ruleset bypass and states the reason in the PR. The codex-gate runs of a PR are
serialized and never cancelled (`queue: max`), so the last run publishes the current state.

For risk-path PRs the maintainer may also ask the Claude GitHub app for a review in a PR comment. It is not a
required check and needs no secret in any workflow.

## Agent identity

Agents work as the GitHub App `yottabyte-oos-agent`, installed on this repository only, with Contents, Pull
requests, Issues and Workflows read/write; Checks, Statuses, Actions and Metadata read; no Administration and no
ruleset bypass. Workflow changes are CODEOWNERS paths and still need the maintainer's approval. Agents use a short-lived installation token
per session; the app's private key stays in the maintainer's local secret store. Commits carry the DCO sign-off of
the person responsible for the agent run. PRs from `yottabyte-oos-agent[bot]` get the `agent` label, and `pr-meta`
warns when author and label disagree.
