---
id: 80
title: Restore SCPI measurement row removal and overflow
kind: fix
status: active
release: none
owner: codex
depends: []
---

## Goal

Port the fork's `:MEASure:CLEar ITEM<n>` and SCPI evict-first-row behavior onto upstream's measurement
implementation, as directed by the owner for task 0066 question 3. Review tier: host. Prepare two independently
reviewable commits: indexed clearing first, SCPI eviction second. The menu retains upstream PR 8's refusal at capacity.

## Plan

Branch: `t0080-meas-clear`, base `3e316143`.
Reserved paths: `docs/tasks/0080-measure-clear-item.md`, `ui/src/scpi.rs`, `ui/src/scpi/actions.rs`,
`ui/src/scpi/vernier_tests/measurement_list_tests.rs`, `ui/src/scpi/vernier_tests/measurement_eviction_tests.rs`,
`ui/src/ui/tools.rs`, `docs/scpi.md`, `docs/spec/scpi.md`.

1. Keep indexed clearing in its own source, documentation and test changes, reusing the Selected-page removal helper.
2. Keep SCPI evict-first-row behavior in a second change, retaining capacity, source validation and duplicate handling.
   Remote scripts cannot see the menu's refusal toast. Remove the standalone PNG references and golden-tool edits;
   `meas_list` assertions already check the rows used by the screen and SCPI.
3. Run focused debug/release tests, the gate's exact host/ARM clippy step with zero new findings, and
   `cargo xtask check --fast`. Prepare two plain commit messages in `target/COMMIT_MSG.txt`, separated by `-----`,
   and record exact staging boundaries. Leave all changes uncommitted for the owner.

## Status

- 2026-11-16: Review round ready for owner staging, intentionally uncommitted. Fast gate printed
  `partial gate passed (not done): 8 steps in 34 s`, exit 0 (`target/t0080/review-round/fast.log`; complete logs
  `target/check-logs/22171-1794851178484914000/`). `git diff --check` passed. Two messages are in
  `target/COMMIT_MSG.txt`; exact file/hunk ownership and staging commands are in
  `target/t0080/review-round/STAGING.txt`. The ordered clear-item and evict-first-row patches reproduce the
  final working files exactly in scratch; the real index is untouched. Owner next action: commit the two
  changes separately. Coordinator acceptance and the integration batch full gate remain outstanding.

- 2026-11-16: Review-round verification: 9 measurement and 8 SCPI helper tests passed in each of debug and
  release, all four commands exit 0 (`target/t0080/review-round/{tests,helpers}-{debug,release}.log`). All 11 exact
  gate clippy commands passed with zero new/raised findings; UI counts match the previous review exactly, with
  no findings in either new test module (`target/t0080/review-round/clippy-step.log`).

- 2026-11-16: Addressing the owner's review: semantics correct, no blocking findings. Split clear-item and
  evict-first-row tests into separate modules, removed both PNG references and their helper, restored the golden
  tooling, and retained upstream's menu refusal. Focused tests, exact clippy step and fast gate passed.
  No commits, pushes or hardware access. Owner next action: commit the two changes separately after verification.
- 2026-11-16: Previous host evidence before this review round: 9 measurement and 8 SCPI helper tests passed in
  debug/release; all 11 gate clippy commands passed with zero new/raised findings. Fast gate printed
  `partial gate passed (not done): 8 steps in 179 s`, exit 0 (`target/t0080/fast-final-with-golden.log`).
  Golden captures and updater tests from that round are superseded by the owner's instruction to drop them.
- 2026-11-16: Failing-before evidence: four initial SCPI regressions failed on the unchanged implementation
  (`target/t0080/tests-before.log`, exit 101). Indexed removal uses one-based screen order, rejects invalid rows
  with -224 without mutation, and shares the statistics reset. SCPI eviction removes the first screen row
  (standard, then pair, then catalog), preserving upstream's 1024-row limit and source support.
- 2026-11-16: Claimed by Codex under the owner's explicit assignment. Read task 0066, delivery/review rules and the
  SCPI reference/spec. Fork behavior and tests are in `12a8a3c7`. Task stays active until coordinator acceptance
  and the integration batch's full gate.
