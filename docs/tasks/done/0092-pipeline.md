---
id: 92
title: Contribution structure and CI pipeline for the public repository
kind: chore
status: done
release: 0.2.0
owner: maintainer
depends: []
---

## Goal

Hard-fork phase 3. The public repository (yottabytesolutions/oos) takes pull requests from people and agents. Every
PR runs the gate on GitHub-hosted runners, needs a Codex review of its head commit, and merges through a merge queue
into a linear `main`. Releases are SemVer tags built in CI and signed offline. Settings live in Git as files and a
script; nothing here changes GitHub settings by itself.

## Plan

- Toolchain: `rust-toolchain.toml` already pins 1.99.1 with rustfmt, clippy, llvm-tools-preview and
  `aarch64-unknown-linux-musl`. CI installs exactly that file (`rustup toolchain install`).
- Gate split: `cargo xtask step <id>...` runs named inventory steps, so CI jobs share one definition with the local
  gate. `#[ignore]` needs a reason; `flaky:` reasons name an issue (known-failures step).
- Workflows (all `runs-on` literal `ubuntu-24.04` or `ubuntu-24.04-arm`, top-level `contents: read`, SHA-pinned
  actions, `timeout-minutes` everywhere, no secrets):

  | Workflow | Trigger | Jobs (required checks in bold) | Expected wall time, warm cache |
  |---|---|---|---|
  | pr.yml | pull_request, merge_group | **fast**, **clippy**, test (debug), test (release), **test** (aggregate), **python**, **deny** | 6-9 min (cold: 15-20) |
  | pr-meta.yml | PR opened/edited/sync/labels, merge_group | **pr-meta** | < 1 min |
  | privacy.yml | push, pull_request, merge_group | **privacy** | 1 min |
  | codex-review.yml | PR, reviews, review comments, issue comments, merge_group | codex-gate, publishes status **codex-review** | < 1 min |
  | main.yml | push main, nightly 02:17 UTC, dispatch | gate (full host gate with coverage, no perf), cache (warms PR cache keys), deny, image (nightly) | gate 25-35 min, image 40-70 min |
  | image.yml | workflow_call, dispatch | image on ubuntu-24.04-arm | 40-70 min |
  | release.yml | tag vX.Y.Z | verify, image, release (draft, provenance, changelog) | 45-75 min |
  | canary.yml | weekly Monday, dispatch | stable (latest stable clippy + tests), report (issue on failure) | 15-20 min |
  | stale.yml | daily | stale (PRs only: 14 days stale, closed 7 later) | < 1 min |
  | labeler.yml | PR (same repo only) | label (paths, `agent` for the agent app) | < 1 min |

- Checks as scripts with unit tests: `tools/ci/pr_meta.py` (branch `<type>/<ref>-<slug>`, title
  `type(scope)?: summary` up to 72 characters, DCO sign-off matching the author on every commit, runner allowlist;
  warnings for size over 400 changed lines without goldens/fixtures/Cargo.lock/ratchet data, missing issue or task
  link, tool details in commit messages, agent label vs author), `tools/ci/codex_gate.py`, `tools/ci/sync_labels.py`.
- Privacy scanner: a whole `Signed-off-by:`/`Co-authored-by:` line with a GitHub noreply address is no longer an
  email finding in commit messages. Without this no DCO-signed commit passes `privacy`.
- Ruleset `.github/rulesets/main.json` (default branch): PR required, 0 approvals except code owner review on
  CODEOWNERS paths, review thread resolution, required checks fast, clippy, test, python, deny, pr-meta, privacy,
  codex-review (all from the GitHub Actions app, id 15368), linear history, no force push, no deletion, merge queue
  (rebase, all-green groups up to 5), merge method rebase only (#29). Bypass: repository admins, PR merges only.
- Repo settings: `tools/ci/apply-repo-settings.sh [--apply]` (dry run by default).
- Releases: SemVer from v0.2.0. The tag must equal the crate versions; `release.yml` checks it.

## Status

2026-11-17: done. Steps 1 to 3 are done: the text is merged into CONTRIBUTING and AGENTS, the agent app exists,
Actions and the `main` ruleset are active. Step 4: crate versions are 0.2.0; the `v0.2.0` tag and the kernel base
asset follow as the release itself. Step 5 runs as the first pull request (`docs/t0094-community-health`); its
findings go to new issues. Lasting facts moved to
[docs/process/ci.md](../../process/ci.md) (workflows, Codex review gate, agent identity) and
[docs/releases.md](../../releases.md#versioning) (versioning). The sections below stay as the record of the design.

Done (branch `ci/t0092-pipeline` on main 441adcb6, not pushed):

- xtask `step` command; ignore reason and flaky issue rule; privacy sign-off exemption.
- pr.yml, pr-meta.yml, codex-review.yml, main.yml, image.yml (arm64 only, reusable), release.yml, canary.yml,
  stale.yml, labeler.yml; privacy.yml hardened and merge-queue aware; ci.yml removed.
- deny.toml, cliff.toml, dependabot.yml, CODEOWNERS (maintainer review on risk paths),
  issue forms, one PR template (`pull_request_template.md`; the upper-case copy is removed), labels.yml,
  labeler.yml, ruleset, settings script.
- privacy.yml no longer fetches or excludes upstream (hard fork); its tests follow.
- CONTRIBUTING.md#style: commit subjects are `type(scope): imperative summary` (every commit lands on main by rebase, #29). ADR 0005: CI runs the
  inventory split by `cargo xtask step`. Dead ci.yml links in the former lint and layered-check records point at pr.yml.
- Validated locally: actionlint (with shellcheck) clean, `cargo-deny check` passes, `git-cliff` renders, Python tests
  for the new scripts, fast gate, `codex_gate.py` against dekajoule/oos PR #38 (pending: Codex reviewed f617d62953,
  head 5e683947b9).

Next (maintainer, after main is published):

1. Merge the CONTRIBUTING/AGENTS text below into those files (they still name `tools/check.sh` layers in places).
2. Create the agent identity (below). Enable Codex automatic reviews for the repository.
3. Enable Actions, then `tools/ci/apply-repo-settings.sh` (read the dry run), then `--apply`. It replaces the
   existing `main` ruleset (id 27444711) with the file.
4. Release `v0.2.0` (versioning below) and a kernel base release asset for image.yml.
5. Verify a test PR end to end, including a fork PR and the merge queue.

Not verifiable before Actions is on: real job durations; that commit statuses created with the workflow token
carry integration id 15368 (if not, drop `integration_id` for `codex-review` in the ruleset); that a Codex comment
edit fires `issue_comment: edited` for the fork-PR path; the `sha_pinning_required` and fork-approval API fields
on this organisation's plan; that image.yml builds natively on ubuntu-24.04-arm (Docker, rustup present).

## Codex review gate

Codex (the chatgpt-codex-connector app) keeps one summary comment per PR with
`<!-- codex-review-state:v1 {json} -->`. `codex_gate.py` reads the bot's newest marker and sets the commit status
`codex-review` on the PR head:

- success: `code_review.commit_sha` is the head, `status` is `completed`, `has_unresolved_findings` is `false`;
- pending: no marker yet, a review of an older commit, a review still running;
- failure: unresolved findings, or a marker that does not parse.

A new push makes the status pending until Codex reviews the new head (automatic, or comment `@codex review`). In the
merge queue the group commit gets success: the PR could only enter the queue with a passing `codex-review`.

Outage fallback: when Codex is down, an admin merges through the ruleset bypass and states the reason in the PR.

## Agent identity

The maintainer cannot approve his own PRs, so agents work under their own identity: a GitHub App
`yottabyte-oos-agent` (preferred over a machine user: no seat, scoped, short-lived tokens).

1. Organisation settings > Developer settings > GitHub Apps > New. Name `yottabyte-oos-agent`, no webhook.
   Repository permissions: Contents read/write, Pull requests read/write, Issues read/write, Workflows read/write,
   Checks, Statuses and Actions read, Metadata read. No Administration. Install on `oos` only.
2. Do not give the app bypass in the ruleset. Its PRs pass the same checks; risk paths still need maintainer approval.
3. With the Workflows permission the app can push changes to `.github/workflows/`; they are CODEOWNERS paths and
   still need the maintainer's approval.
4. Agents get an installation token per session (`gh` with a token from the app's private key, kept in the local
   secret store, never in the repository or in Actions). Commits carry the DCO sign-off of the person responsible
   for the agent run; author is the app's noreply identity.
5. PRs from `yottabyte-oos-agent[bot]` get the `agent` label automatically (labeler.yml); pr-meta warns when author
   and label disagree.

## Optional second reviewer

For risk-path PRs the maintainer may also ask the installed Claude GitHub app for a review by mentioning it in a PR
comment. This is not a required check and needs no secret in any PR workflow; the app runs on its own installation.

## Versioning

- SemVer tags `vX.Y.Z` on main, annotated, starting at `v0.2.0` for the hard fork.
- The version lives in the crate manifests (0.2.0 in core, dsp, fpgad, hal, ui and xtask). A release is one
  `chore(release): vX.Y.Z` PR that sets all six, then the tag. `release.yml` refuses a tag that differs.
- Binaries already report it: `oos-ui` uses `CARGO_PKG_VERSION` in `*IDN?`, the web UI and mDNS.
- Proposed, not implemented (risk path, needs its own task and a unit test): the update package build id stays
  `YYYYMMDD-HHMMSS kernel <rel>` (the downgrade check compares it as text, date first). Add the version as a
  separate signed package field (`version`) next to `unit` in `oos-update pack`, shown by `oos-update verify` and
  the update page, after checking that older `oos-update` builds ignore unknown fields.

## Text for CONTRIBUTING/AGENTS

### Quick start

```sh
git clone https://github.com/yottabytesolutions/oos && cd oos
tools/bootstrap.sh                 # installs the pinned toolchain from rust-toolchain.toml and the host tools
git config core.hooksPath .githooks
git config pull.rebase true
cargo xtask check --fast           # before every commit (the pre-commit hook runs it)
cargo xtask check                  # before opening a PR
```

### Branches, commits, pull requests

- One issue, one branch, one PR. Branch: `<type>/<ref>-<slug>`, `type` one of feat, fix, perf, refactor, docs,
  test, ci, build, chore; `ref` the issue number or `t` plus the four-digit task id. Examples:
  `fix/123-trigger-holdoff`, `refactor/t0075-normal-trigger`.
- PR title and every commit subject: Conventional Commits, `type(scope): summary`, at most 72 characters. Each
  commit lands on main as it is and becomes a changelog line; no fixup or WIP commits.
- Every commit carries `Signed-off-by:` (`git commit -s`) matching the author (DCO, ADR 0003). Use your GitHub
  noreply address: other email addresses are rejected by the privacy check.
- Commits and PR texts carry no tool or session details: no session links, no `Agent:` or AI `Co-authored-by:`
  trailers.
- Keep PRs small: about 400 changed lines without goldens and fixtures. Larger refactors go as stacked PRs, each
  reviewable on its own.
- Link the issue (`Closes #NNN`) or the task file in the PR text. Fill in the template.
- History is linear: rebase on main (`git pull --rebase`), force-push your own branch with `--force-with-lease`.
  PRs merge through the merge queue by rebase.

### Review gate

A PR merges when all required checks pass: fast, clippy, test, python, deny, pr-meta, privacy and codex-review.
Codex reviews automatically when the PR opens; after new commits comment `@codex review`. Fix or answer every
finding and resolve every conversation; `codex-review` passes only for a completed review of the current head with
no unresolved findings. Paths in CODEOWNERS also need the maintainer's approval.

### Risk tiers

- High: hardware register writes (hal/src/dho, fpgad), calibration, update packages, signing, kexec, boot,
  kernel, rootfs, the pipeline itself. Label `risk/hardware`, code owner review, `tools/plan-matrix.sh` before and
  after in the PR, and a run on a real unit before merge (`needs-unit` until then).
- Medium: behaviour visible on the scope (UI, SCPI, DSP results). Golden pictures and tests; a unit run when the
  change touches acquisition timing.
- Low: host-only tooling, docs, tests.

### Unit testing

Say in the PR whether it ran on a unit, which model, which build and what was exercised. Host-only is a valid
answer for medium and low risk. Never claim a unit test that did not happen.

### Agents

- Work only on issues labelled `agent-ready`, one PR per issue, within the issue's acceptance criteria and its out
  of scope list.
- Agents push and open PRs only as `yottabyte-oos-agent[bot]`, never with a maintainer's credentials.
- Never touch high-risk paths unless the issue carries `risk/hardware` and says so.
- Run `cargo xtask check --fast` before every commit and `cargo xtask check` before the PR; report what ran.
- A flaky test is ignored only with `#[ignore = "flaky: #NNN reason"]` and an open `flaky` issue.
- Answer Codex findings by fixing them or by a reply with the reason; never resolve a thread without either.
