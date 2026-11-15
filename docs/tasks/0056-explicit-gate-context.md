---
id: 56
title: Pass explicit gate execution context
kind: chore
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Remove the process environment writes in `xtask/src/run.rs::start_logs` while preserving target
isolation and complete logs for every gate subprocess. Follow-up to item 4 of task 0055.

## Plan

Reserved paths: `xtask/src/run.rs`, `xtask/src/run/`, `xtask/src/check.rs`,
`xtask/src/inventory.rs`, `xtask/src/inventory/tests.rs`, `xtask/src/steps/`,
`xtask/src/base.rs`, `xtask/src/base/tests.rs`, `xtask/src/store.rs`,
`xtask/src/workspace.rs`, `xtask/src/fmt.rs`, `xtask/src/clippy.rs`,
`xtask/src/coverage.rs`, `xtask/src/testrun.rs`, `xtask/src/target.rs`,
`xtask/src/doctor/`, `xtask/src/agent_rules.rs`, `xtask/src/agent_rules/tests.rs`,
`xtask/src/size/mod.rs` (test caller only), `xtask/src/tighten.rs`, `xtask/src/testrepo.rs`,
`docs/tasks/0056-explicit-gate-context.md`.

The owner's explicit assignment accepts this task and releases the doctor boundary after task 25.
Review tier: tooling. Keep the change limited to explicit subprocess context and regression tests.
No commits, pushes or hardware access. Prepare `target/COMMIT_MSG.txt` for the coordinator.

Pass an immutable execution context containing the selected Cargo target and log directory from
the gate dispatcher through the inventory runners to command capture. Preserve environment overrides
for coverage's separate target. Cover formatting, Cargo metadata, Git base checks, doctor probes,
Python/image runners and command-start failures. Do not replace process globals with hidden
thread-local state. Keep ordinary doctor/ratchet invocations usable without a gate log session.

Test concurrent contexts with different targets and log directories, inherited environment remaining
unchanged, complete output retention and explicit coverage target overrides. Run focused xtask tests
and the fast gate; the coordinator supplies independent review and the integrated full gate.

## Status

- 2026-11-15: dependent reconciliation passes the dependency (5), build/profile/review (31)
  and output/image-output (15) Python tests and all 115 xtask debug tests, exit 0.
  `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 21 s`, exit 0. Full gate not selected.
  Evidence and an application patch against `adf0c41` are retained with task 0055 under
  `target/t0055-reconciliation/`. The original dependent worktree was read-only for this
  session. Coordinator owns patch application, renewed cross-vendor review and full gate.

- 2026-11-15: reconcile the dependent task 0055 item 6 with task 25 on `overhaul`
  at `b10b629`, preserving the explicit execution-context implementation at `adf0c41`.
  The original dependent worktree is outside the session writable roots, so reconciliation
  is prepared as a patch in a separate validation checkout. Doctor pin and test changes
  merge without conflicts; context arguments remain explicit. Focused checks and fast
  gate results follow above. No commit, push or hardware access.

- 2026-11-15: author implementation is ready for one tooling review round. Candidate is the
  uncommitted working tree on the recorded parent; task remains active pending coordinator
  cross-vendor review and batch integration. `target/COMMIT_MSG.txt` is prepared. All remaining
  tracked changes and the new `xtask/src/run/tests.rs` are intentional task changes. Build caches
  and required evidence are retained; test fixtures and temporary editing scripts are cleaned.
  No commit, push, full gate, hardware access or hardware verification was performed.
- 2026-11-15: `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. All eight fast IDs pass;
  doctor, clippy, Rust suites, performance, coverage, script/Python and image checks remain
  not selected by this partial gate. Complete output and exit files are in
  `target/t0056-evidence/`; 99 command/step log files are retained in
  `target/check-logs/68582-1794756497215047000/`, including the Git agent-rule inventory.
  `cargo test --locked -p xtask --release` passes all 115 tests, exit 0. Existing inventory,
  doctor, ratchet lifecycle and source-size tests stay green in debug and release.
  `git diff --check` passes. This is an execution-context refactor, so a failing-before bug
  regression and new golden pictures are not applicable. Integrated full-gate evidence remains
  the coordinator's next action after independent review.

- 2026-11-15: implementation passes an immutable context from the dispatcher through all
  command-producing runners and helpers. `Command::env` sets the selected target and log
  directory for each child; explicit coverage overrides still apply last. The Git agent-rule
  inventory now uses byte-preserving shared capture so its environment and logs also follow
  the context. Standalone doctor/ratchet calls use a default context without a log session.
  No process environment setters remain in `xtask/src/`; no step selection, prerequisites,
  flags, ratchets, application behavior or golden pictures changed.
- 2026-11-15: `cargo test --locked -p xtask` passes 115 tests, exit 0. New tests cover
  concurrent inventory contexts, shell/image forwarding, unchanged parent/inherited values,
  target overrides, full stdout/stderr, nonzero exits, start failures, selector failures,
  raw inventory bytes and formatting/metadata/Git log retention. Running the focused
  `run::tests::` filter with both inherited variables set passes 10 tests, exit 0
  (includes the existing `testrun` count test matched by that substring).

- 2026-11-15: codex claims the owner-assigned work on `t0056-gate-context`, based on
  `1ce2bc9` (`t0055-followups`). The initial worktree is clean. Plan and reserved paths
  recorded before code. Fast gate and focused tests only; coordinator owns cross-vendor
  review, integration and the full gate. No commits authorized.

- 2026-11-15: proposed by codex-c from task 0055 under the owner's roughly two-hour item limit.
  The runner API spans the held `xtask/src/doctor*` paths. No implementation or claim; coordinator
  owns acceptance, scheduling and the task-25 handoff. Id 56 is unused in the local integration
  branch inventory; this proposal has not been merged into the claim authority.
