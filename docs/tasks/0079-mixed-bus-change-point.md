---
id: 79
title: Route mixed-bus acquisition through the change point
kind: fix
status: active
release: 0.2.0
owner: codex
depends: [3, 66]
---

## Goal

Apply the owner's task 0066 decision: only the change point publishes `Shared.want`.
Preserve upstream mixed live/reference Bus candidates, with classified generations and
normal STOP/SINGLE transitions. Review tier: unit. Host verification only.

## Plan

1. Audit direct writers and pin failing regressions for publication, classification,
   generation, mixed-bus results and STOP/SINGLE.
2. Derive the Edge/Auto candidate request at the change point from accepted settings;
   classify its effective policy and remove the record-stage settings write. Reject
   stale candidate identities. Add an architecture ownership guard using the gate's
   existing syntax inspection.
3. Run focused debug/release tests, timing guards, the exact gate clippy step and
   `cargo xtask check --fast`. Retain logs and one draft commit-message section.

Reserved paths: `ui/src/ui/bus_candidate.rs`, `ui/src/ui/bus_candidate/`,
`ui/src/ui/change.rs`, `ui/src/ui/change/acquisition.rs`,
`ui/src/ui/change/legacy_keys.rs`, `core/src/change/values.rs`,
`xtask/src/arch/mod.rs`, `xtask/src/arch/shared_want.rs`,
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.
No commits, publication or hardware access. Cross-vendor unit review and the
coordinator's full gate remain integration requirements.

## Status

- 2026-11-16: Implementation ready for unit-tier review, left uncommitted as requested.
  Final focused tests exit 0 in debug (75 passed, 1 pre-existing ignored) and
  release (74 passed, 1 pre-existing ignored); the extra debug test is the existing
  should-panic writer-entry assertion. All six new mixed-bus regressions pass in
  both profiles. Core change tests pass 25/25 in each profile; all xtask debug
  tests pass 139/139 and the new architecture controls pass in release too.
  `sh tools/perf-guard.sh` exits 0: `10 timing budgets and 4 zero-allocation budgets
  passed`, `perf guards: all passed (53 s)`. Final fast gate exits 0:
  `partial gate passed (not done): 8 steps in 206 s`. Exact gate clippy exits 0
  with zero new/grown findings (all 11 host/ARM invocations exit 0).
  Complete commands, logs, exits and final source hashes are in
  `target/t0079-verification.json`; final sources match that evidence.
  `target/COMMIT_MSG.txt` contains one section. Removed the scratch dispatch build
  directory, retaining dispatch sources/provenance, test evidence and normal build
  caches. No staging, commits, ratchet edits, hardware access or publication.
  Intentional remaining changes are this task, architecture documentation, the
  acquisition projection/filter, derived policy classification and ownership tests.
  Next: cross-vendor unit reviewer; rerun the socket case on a socket-capable
  runner, then coordinator full gate and leased unit smoke before integration.

- 2026-11-16: Final-source debug selection passes 75 tests, 0 failed, 1 pre-existing
  ignored; `target/t0079-focused-debug-final.log`. Core change tests pass 25/25;
  architecture guard controls pass 1/1. Exact unchanged gate `steps::clippy` passes
  all 11 host/ARM invocations and protected-base comparisons, zero new/grown
  findings: `ok   clippy (45.0 s)`; `target/t0079-clippy-final.log` links complete
  command logs, and `target/t0079-clippy-dispatch.json` records scratch dispatch
  provenance. Initial fast gate exits 0: `partial gate passed (not done): 8 steps
  in 238 s`; `target/t0079-fast-initial.log`. Final fast gate and release/timing
  verification are running. Writer grep evidence: `target/t0079-writer-audit.txt`.

- 2026-11-16: Three new parent-compatible regressions fail on the original production
  code (exit 101, 0 passed / 3 failed): publication before candidate intake,
  an unclassified late decoder-threshold rewrite, and old-generation input with
  a current epoch. Complete log: `target/t0079-failing-before.log`.
  After the fix, focused debug passes 74 tests (one pre-existing ignored test),
  including mixed Bus matching/FORCE/STOP/rearm and the outbox/epoch/shadow suites.
  The initial full mixed-bus selection passes 8 tests and fails only its socket
  SCPI test because the sandbox cannot bind its listener; the final focused
  selection excludes that single socket case, with no source ignore added.
  Further tests pin policy classification, transition effects and the mixed result.
  Acquisition publication and legacy observation now project the same accepted
  Edge/Auto request; the mixed policy is classified ACQ|STATS and candidate intake
  checks both generation and epoch. Added the architecture syntax fence and its
  positive/negative controls. No hardware verification.

- 2026-11-16: Owner-assigned new claim on `t0079-mixed-bus-change`, base `3e316143`;
  clean start. Audit found one production bypass in `acquire_mixed_bus_record`;
  the remaining direct acquisition-want assignments are board test fixtures.
  Plan written before implementation. Existing M2 reservations are the parent
  specification; this task owns the mixed-bus correction assigned by the owner.
