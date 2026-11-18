---
id: 55
title: Review follow-ups
kind: chore
status: active
release: 0.2.0
owner: codex-c
depends: []
---

> Historical unit-image references below are superseded by ADR 0015; use generic images and device state.

## Goal

Resolve the six bounded review follow-ups assigned to this tooling/standard worker job.

## Plan

Branch: `t0055-followups`, based on `overhaul` at `18ac5c7`.
Reserved paths: `docs/tasks/0055-review-followups.md`, `docs/tasks/0035-roll-counter.md`,
`docs/tasks/done/0035-roll-counter.md`, `ui/tools/golden_run.py`, `ui/tools/test_golden.py`,
`tools/check_inventory.py`, `tools/tests/test_check_inventory_discovery.py`,
`tools/tests/test_dependency_inputs.py`,
`docs/tasks/0056-explicit-gate-context.md`, `rootfs/initramfs/bin/oos-ui-exits`,
`hal/tests/safe_start/mod.rs`, `hal/tests/safe_start/lock.rs`,
`tools/output.py`, `rootfs/deps.sh`, `boot/unit-hook.sh`, `target/COMMIT_MSG.txt`,
`docs/development.md`, `docs/fork/deps.md`, `xtask/src/inventory.rs` (runner description only).
Documentation and the inventory description are included to keep the declared discovery roots current.

1. Check the roll forward-port and its existing acquired-counter regression. Close 0035 if covered.
   The roll implementation and regression are read-only in this task.
2. Separate debug, release and coverage golden run directories; test pruning isolation.
3. Report skip reasons and reject Python tests outside the declared discovery roots.
4. Pass the gate target and log directory explicitly instead of mutating process environment.
   Assess the shared runner API and respect the held doctor paths.
5. Identify ledger-lock owners by PID and proc starttime, preserving atomic claims and safe reaping.
   Add host regressions for reused PIDs and live owners using POSIX shell primitives.
6. Share the native-dependency input identity between receipts and dependency stamps, including
   deps.sh, builder, build environment and selected deps.lock fields. Test deps.sh-only invalidation.

Reconcile the reviewed item 6 with task 25 by merging `overhaul`, preserving per-tool rootfs
environment identities and the stub-able `tools/build-env.sh` seam. Share the stamp description
with scoped receipt identities, update native builder fixtures to capture `identity ARTIFACT`,
and run both dependency and build/output regressions. If Git metadata is sandbox-blocked,
apply the file merge and retain the merge path list. Prepare a patch for the dependent 0056
worktree if its location cannot be written, preserving its uncommitted implementation.

Run focused checks and `cargo xtask check --fast`. Keep complete logs below target and prepare
`target/COMMIT_MSG.txt` with a section and file list per item. No commits, pushes or hardware access.
Defer work exceeding roughly two hours to a separate proposed task for coordinator allocation.
Independent cross-vendor review and the integrated full gate remain with the coordinator.

## Status

- 2026-11-15: task 25 reconciliation passes focused dependency (5 tests), build/profile/review
  (31 tests) and output/image-output (15 tests) suites, all exit 0. New regressions cover
  selective per-tool environment invalidation, all seven native tools using the shell seam,
  and dependency-like artifact names outside rootfs retaining whole-tree identities.
  `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. Full gate not selected.
  Complete logs, exit files and merge/reconciliation path lists are under
  `target/t0055-reconciliation/`; draft reconciliation is in `target/COMMIT_MSG.txt`.
- 2026-11-15: dependency receipts now capture and validate the same scoped source identity
  as their builders. The three manifest-based tools hash the exact `dep_stamp_text` bytes,
  including helper hash and per-tool environment identity. Other native tools retain their
  builder/environment identities. Both paths query `tools/build-env.sh`; URL-only and
  unrelated Rust edits remain current. Native fixtures capture `identity ARTIFACT` before
  building. Automatic-merge code fails the original four dependency tests; resolving only
  the shell seam still reproduces the reported source-identity mismatch.
- 2026-11-15: dependent 0056 advanced from uncommitted work to `adf0c41` during this
  session. Its original worktree remains untouched. A separate validation checkout based
  on that commit preserves all explicit-context changes and reconciles the overlapping
  doctor files without conflicts. The same 51 Python tests and all 115 xtask debug tests
  pass, exit 0. Its fast gate reports
  `partial gate passed (not done): 8 steps in 21 s`, exit 0. A complete application patch
  and separate draft message are retained under `target/t0055-reconciliation/`.
  Git metadata for 0055 and write access to the original 0056 directory remain sandbox
  limitations. No branch update, commit, push or hardware access was performed by this job.
  Coordinator owns patch application, renewed cross-vendor review and integrated full gate.

- 2026-11-15: owner requests reconciliation of reviewed `1ce2bc9` with `overhaul` at
  `b10b629`, then the dependent 0056 implementation. Git merge cannot lock ORIG_HEAD in
  the shared Git metadata. File-level merge is authorized; the only textual conflict is
  `rootfs/deps.sh`. Branch and index remain unchanged. Fast gate only; no commit or push.
  Additional reconciliation paths: `tools/OUTPUTS.md`, `tools/tests/test_build_review.py`;
  task 25 changes are carried as merge inputs and listed under target evidence.

- 2026-11-15: final source-input digest is `99dbedd750720aa7a705e3af4e6467fb0df78dc81a1deca376e8decdc3f683d9`
  (`tools/output.py revision`), also retained in `target/t0055-evidence/candidate-inputs.sha256`.
  All intended changes remain unstaged and uncommitted for the coordinator. The 0035 move is
  intentional; 0056 is a proposal with no accepted claim. Build caches and required evidence remain.
- 2026-11-15: implementation ready for coordinator review; task stays active pending independent
  cross-vendor review and the integration batch's full gate. No commits, pushes or hardware access.
  Candidate is the uncommitted tree over `18ac5c7`; no held implementation paths were edited.
- 2026-11-15, item 1: resolved by existing forward-port `bb7dd9b`; moved 0035 to `done/` with
  the acquired-counter regression cited. Focused debug and release tests both pass (1 passed each, exit 0). The roll implementation and test were read only.
- 2026-11-15, item 2: implemented separate `golden/debug`, `golden/release`,
  `golden/coverage-debug` and `golden/coverage-release` namespaces. Comparator suite passes
  (8 tests, exit 0). Restoring the shared namespace fails the profile-isolation regression.
  No golden references or drawing behavior changed.
- 2026-11-15, item 3: implemented explicit reporting of every skip reason, with failure if a
  skipped count lacks reasons. Git-listed tests outside `tools/tests`, `tools/privacy` and
  `ui/tools` fail before the UI build; all three roots execute in separate interpreters.
  Discovery suite passes (14 tests, exit 0); parent skip reporting fails the reason assertion,
  and the parent lacks the new test-root validation. Lasting facts updated in development docs
  and the inventory prerequisite description. No allowlist was added; reasons remain visible.
- 2026-11-15, item 4: deferred, with the separate proposed task `0056-explicit-gate-context.md`.
  Estimated 2-4 hours including runner-wide regression evidence and the held doctor boundary.
  Coordinator owns proposal acceptance and the task-25 handoff. Environment writes remain as before.
- 2026-11-15, item 5: implemented atomic `PID.starttime` directory claims. The parser extracts
  Linux stat field 22 after the final comm delimiter using POSIX shell primitives. Reapers
  preserve live matching identities and remove only the stale exact claim. Unreadable identity
  fails closed; legacy PID-only claims stay protected while live until boot clears temporary state.
  Safe-start suite passes in debug and release (29 tests each, exit 0), including new identity
  publication, reused-live-PID and live-owner protection tests plus existing kill/commit cases.
  The parent shell times out on the same stale-identity fixture; the fixed shell commits, exit 0.
  Host fixtures emulate proc stat on non-Linux; BusyBox runtime and hardware were not exercised.
- 2026-11-15, item 6: implemented native receipts from the exact `dep_stamp_text` description
  used by unit-hook: builder hash, deps.sh hash, rootfs environment identity and selected lock
  name/version/checksum fields. Helper-only edits rebuild nft, dropbearmulti and sftp-server.
  Dependency suite passes (4 tests), output harness (9 tests), unit-hook identity (1 test), all
  exit 0. Restoring the parent helper makes the helper-only regression fail. Missing builder or
  dependency data fails closed. Native receipt schema values invalidate old receipts once.
  Lasting facts updated in dependency docs; unrelated-source and URL-only edits leave both current.
- 2026-11-15: final `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 20 s`, exit 0. Earlier run also passed in 21 s.
  Script syntax passes: 58 Python, 62 shell scripts. The isolated `ui/tools` selection counts
  8 tests; the real Git test-root inventory passes. Full gate, clippy, coverage, perf and image
  tiers were not selected.
  Complete focused, reversion, syntax and gate logs/exit files are under `target/t0055-evidence/`;
  the gate also retains per-command logs under `target/check-logs/`.
- 2026-11-15: privacy audit: `scan.py --tree` exits 1 with 151 findings, all in untouched
  paths. Applying the same scanner/policy to all changed/new paths gives 0 findings, exit 0.
  No unrelated privacy sources or allowlists changed. Commit-message scan passes (0 findings, exit 0).


- 2026-11-15: claimed by codex-c on `t0055-followups`, base `18ac5c7`, under the owner's
  explicit assignment. Tooling/standard tier; fast gate only; no commits or hardware access.
- 2026-11-15, item 1: inspection finds `publish_roll` increments acquired and
  `roll_single_bounds_snapshots_holds_and_rearms` asserts 927 publications and two fresh
  acquisitions after resume/rearm. Focused execution pending before closing 0035.
- 2026-11-15, item 2: planned; debug and coverage currently share the debug pruning namespace.
- 2026-11-15, item 3: planned; skips expose counts only, and discovery omits unclassified test roots.
- 2026-11-15, item 4: deferred to proposed task 0056 (estimated 2-4 hours including regression
  evidence). Explicit context must cross inventory runners, nested formatting/metadata/Git/coverage
  calls, and `doctor/mod.rs` plus `doctor/pins.rs`, which task 25 holds. A local run.rs-only change
  would lose command logs or target propagation; process-global or thread-local substitutes would
  retain hidden state. No runner-context source changed or reserved. Coordinator must allocate/accept the
  proposal and serialize the doctor handoff before implementation.
- 2026-11-15, item 5: planned; current PID-only claims cannot distinguish process reuse.
- 2026-11-15, item 6: planned; `boot/unit-hook.sh` consumes stamps from `rootfs/deps.sh`;
  those stamps currently omit the helper hash that the receipt's source revision includes.
