---
id: 83
title: Retain complete Peak record batches
kind: fix
status: active
release: 0.2.0
owner: codex
depends: [82]
---

## Goal

Fix S8's Peak record loss, including the 720-record batch that overflowed by
464 records in task 0082. Keep bounded memory, Normal behavior and the G2
step/performance budgets. Review tier: unit.

## Plan

Reserved paths: `docs/tasks/0083-peak-record-overflow.md`,
`ui/src/board/dho/publication.rs`, `ui/src/board/dho/tests.rs`,
`ui/src/board/dho/hw.rs` explicitly (batch bound and producer pacing only),
`ui/src/board/dho/peak_tests.rs`, `ui/src/board/dho/mod.rs` (test wiring and batch constants),
`ui/src/board/scripted.rs` (test-only producer seam),
`ui/src/board/mod.rs` (queue-limit documentation and shared limit export),
`docs/architecture/README.md` (current Peak queue contract),
`ui/src/ui/tests/epochs.rs` (child module declaration),
`ui/src/ui/tests/epochs/peak.rs`,
`target/t0083-evidence/`, `target/COMMIT_MSG.txt`.
The owner's explicit new-task assignment authorizes this claim and the task
0082 test handoff. Round 2 also reserves `ui/src/board/record.rs`
(shared record-step limit),
`ui/src/board/dho/replay.rs` and `ui/src/board/dho/hw/single_fake.rs`
(test callers of applied-mode publication), and `ui/src/ui.rs` for replacing
its record-limit literal with the shared constant required by the owner. Do not edit another worktree.

1. Read task 0082 S8 and trace batch planning, publication and UI consumption.
   Add a failing-before test for complete, ordered delivery of 720 Peak records.
2. Fit one complete Peak batch in a bounded queue, sharing one UI
   count-budget step (48 records) with the Peak planner and publication. Pause
   the next batch read until the queue drains, after control/STOP handling. Keep the sample bound and newest-frame
   exception. Do not change Normal or the UI's 48-record/15 ms limits.
   A larger queue alone cannot absorb sustained bursts; consuming a whole
   batch on main conflicts with the step budget. One batch plus producer
   backpressure bounds retained work without increasing per-step processing.
3. Test repeated batches, the maximum batch, sample bounds, identities and
   Normal behavior. Run focused debug/release tests, perf guards, the gate's
   exact clippy step and `cargo xtask check --fast`, preserving complete logs
   and command exits under `target/t0083-evidence/`.
4. Record evidence and limits here and prepare an imperative commit message
   in `target/COMMIT_MSG.txt`. No commits. Coordinator owns cross-vendor unit
   review, full integration gate and hardware validation.

## Status

- 2026-11-16: ported onto integ-25 (98b71997) as branch `t0083b-peak-on-integ25`.
  Task 68's `batch::Readout` delivery passes the applied mode to `push_frames`;
  the every-frame core correction is unchanged. `record_decode` replaces the
  inline take/stale block and uses integ-24's timed acquisition lock. Held-frame
  publication stays in `publication::publish_held`. Fast gate passes. Debug and
  release: `board::` 100 passed/3 ignored, `ui::tests::epochs::` 15/2,
  `drawpipe` 18/2, HAL 454/10, all exit 0. Release perf guards with
  `OOS_PERF_TESTS=1`: 2 passed, exit 0. Full gate, review and unit validation
  remain with the coordinator.

- 2026-11-16: round 2 author handoff ready, all four review items addressed;
  task remains active for coordinator-arranged cross-vendor unit review, full
  integration gate and hardware validation. Peak planner/publication cap is
  48, derived from the UI's single shared record-step limit. Queue-empty
  admission stays before the next read and after controls/STOP. The 720-record
  repro delivers all 720 in order across actual bounded batches, with zero
  overflow, stale loss or duplicates. Normal planning/overflow and 32 Mi-sample
  behavior remain covered and unchanged. `push_frames` uses the applied mode,
  including the asynchronous Normal delivery; tests cover both request races.
  Peak waits over one second use existing hardware status/logging with
  `event=peak_backpressure`, duration, queue count and identity, at most once
  per second. Host tests cover threshold, repeated suppression, queue/mode/
  identity reset, and callback lock release. No new dependency or telemetry
  static. Controls, calibration and release retain the same loop ordering.
- 2026-11-16: final explicit release Peak run passes 18 tests, zero failures
  or ignores, exit 0, including both perf guards. The new maximum-batch guard
  has a two-step ceiling and observes batch=48, steps=1 with both deterministic
  and live-budget hosts. Before the fix it failed with batch=1000, steps=21,
  exit 101. This guards publication-to-display progression, not change intake.
  The unchanged 0.5 ms intake/change guard also passes: paired p99 6042 ns,
  256 samples. Full output/exit: `round2-release-peak-guards.log`/`.exit`.
  Final focused debug totals: UI 139 passed/4 ignored (89 board, 15 epochs,
  35 outbox), HAL 253 passed/5 ignored (247 acquisition, 4 allocation and 2
  early-trigger guards). Every scoped command exits 0. The broader socket
  failures below remain reported as failures, not counted as passing evidence.
- 2026-11-16: lag interpretation: count-budget drain improves from 21 to one
  step for a maximum batch; the perf guard allows at most two host steps.
  The existing 15 ms cutoff may yield earlier on the unit, so one/two is not
  a hardware guarantee. The conservative queue-induced ceiling improves from
  1000 to 48 progressing steps if only one record fits per step, for an
  unchanged identity with no STOP or trigger filtering. Wall-clock lag,
  hardware trigger dead time and full-step timing remain unverified. No
  hardware access. No review item rejected; no perf guard weakened and no
  changes to `tools/perf-guard.sh`. Earlier size/clippy rejections below were
  fixed without ratchet or lint changes. `target/COMMIT_MSG.txt` has one section,
  plain imperative subject first, and `Task: 0083`. Final source hashes verify
  against `round2-source-sha256.json`; `git diff --check` passes. No commits,
  pushes, merges, credential or hook changes; base stays `9f04a63a`.

- 2026-11-16: final fast gate exits 0, printing exactly
  `partial gate passed (not done): 8 steps in 32 s`. All eight steps pass:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Every file meets its existing size ratchet.
  Exact clippy also exits 0 with zero new findings across all six host and five
  ARM crates: hal 93 entries/177 total, dsp 115/262, core 3/46, ui 249/733,
  fpgad 7/20, xtask 0/0. Final board and epoch debug reruns pass with 89/15
  passes and 2/2 intentional ignores. Outbox (35) and HAL acquisition (253)
  evidence covers unchanged relevant host code. Complete final logs/exits:
  `round2-fast-gate`, `round2-clippy-step`, `round2-debug-board`,
  `round2-debug-epochs` under `target/t0083-evidence/`. Final Rust source hashes
  are in `round2-source-sha256.json`. Explicit release Peak guards still
  compiling; no commits, pushes, hardware, ratchet, dependency or hook changes.

- 2026-11-16: exact clippy passes after callback correction, exit 0, zero new
  findings (host six crates plus ARM five crates). The following fast gate
  rejects only size: `hw.rs` 2548 -> 2549, `gate FAILED in 33 s: size`.
  Evidence preserved in `round2-fast-gate-before-import-cleanup.log`/`.exit`.
  Removed the new constant import and qualified its two uses, bringing the
  file back within its ratchet without removing documentation or changing
  behavior. Rechecking final board/epoch tests, clippy, fast gate and release
  Peak guards; no limit, ratchet or performance guard changes.

- 2026-11-16: HAL debug acquisition checks pass: `--lib dho::` 247 passed,
  five intentional ignores; `alloc_batch` four passed, `early_trigger_end` two
  passed, all exit 0. Complete logs/exits: `round2-debug-hal-{dho,guards}`.
  The extra unfiltered HAL run exits 101: 372 passed, one failed, eight ignored
  (381 total), 219.31 s. The sole failure is `wlan::tests::join_commands` at
  `UnixDatagram::bind`, denied by the sandbox (`Operation not permitted`).
  Full evidence: `round2-debug-hal.log` and `.exit`. No networking changes.
  Exact clippy rejected one new `needless_borrows_for_generic_args` in the ARM
  worker's status callback (`round2-clippy-step-initial.log`/`.exit`, exit 1).
  Removed that unnecessary borrow; no lint or ratchet exceptions. Rerunning
  exact clippy and the remaining final checks on the corrected source.

- 2026-11-16: additional unfiltered debug UI command
  `cargo test --locked -p oos-ui -- --test-threads=1` exits 101: 1179 passed,
  35 failed, 44 ignored (1258 total), in 346.08 s. Every failure requires a
  local TCP/UDP listener or connection unavailable in this sandbox: direct
  binds return `PermissionDenied`/`Operation not permitted`, other tests fail
  when the listener has no port. Includes network-backed SCPI, Bode and bus
  candidate cases. No task-specific regression in this run. Complete result:
  `target/t0083-evidence/round2-debug-ui.log` and `.exit`. This broad run is
  not a pass; keep its failed result visible alongside focused-suite evidence.

- 2026-11-16: round 2 focused debug UI suites pass, exit 0 each: board 89
  passed/2 ignored, epochs 15 passed/2 explicit perf guards ignored, outbox 35
  passed/0 ignored. Total 139 passed, four intentional ignores. Complete logs
  and command exits: `target/t0083-evidence/round2-debug-{board,epochs,outbox}.{log,exit}`.
  Peak-only debug with explicit lag guard previously passed 17/17; one added
  identity-timer regression is now covered by the 89 board passes. HAL, exact
  clippy, final fast gate and explicit release guards are pending. The broader
  debug UI run additionally encounters sandbox-denied socket binds outside
  this task; preserve its complete result rather than changing those tests.
  Round 1 evidence below is historical; round 2 supersedes its batch limit,
  source hashes, test totals and claim that `ui/src/ui.rs` equals HEAD.

- 2026-11-16: round 2 lag negative control fails on unchanged round 1
  production code: maximum planned batch 1000, publication-to-display 21 steps,
  two-step guard fails, exit 101. `round2-lag-negative-control.log` and `.exit`
  preserve complete evidence. With the cap at 48, that same test observes one
  step with both headless and live time-budget hosts. Hardware timing remains
  unverified; 15 ms cuts can require up to 48 progressing steps for 48 records.
  Initial trial checks rejected a stale test's single-burst assumption and
  file growth (hw 2548 -> 2553, board/mod 530 -> 533). Constants now live in
  existing smaller modules; no ratchet edits. Trial logs/exits are retained.
  A subsequent trial build exposed a misplaced cfg on the moved import, now
  corrected. Final debug suites and gate results will supersede these trials.

- 2026-11-16: round 2 review fixes active on the existing uncommitted diff.
  Plan: derive the Peak batch cap from a shared per-step record limit, retain
  queue-empty admission, add a failing-before publication-to-display step guard,
  and rate-limit structured waits longer than one second through worker status.
  Pass applied mode to publication. Adapt the 720-record repro to bounded actual
  batches with zero loss; keep Normal and 32 Mi-sample behavior. The owner's
  shared-bound request authorizes the minimal `ui/src/ui.rs` constant use above.
  Run debug UI/HAL suites with full counts, the new guard, exact clippy step and
  fast gate. No commits or hardware access; cross-vendor review stays pending.

- 2026-11-16: author handoff ready, uncommitted on base `9f04a63a`; task
  remains active for coordinator-arranged cross-vendor unit review, full batch
  gate and hardware validation. `ui/src/ui.rs` equals HEAD. No commits,
  pushes, hardware access, ratchet, dependency, hook or credential changes.
  `target/COMMIT_MSG.txt` contains a plain imperative subject and why body.
  Final changed Rust hashes: `target/t0083-evidence/source-sha256.json`.
  `git diff --check` passes. Full gate, coverage, image qualification and unit
  behavior/timing remain unverified; the partial gate is not a done verdict.
- 2026-11-16: final `cargo xtask check --fast` passes, exit 0, printing
  `partial gate passed (not done): 8 steps in 32 s`. fmt, size, architecture,
  mutable-statics, unsafe, method-exceptions, known-failures and agent-rules
  all pass. Complete output/exit: `target/t0083-evidence/fast-gate.log` and
  `.exit`. The earlier pre-timing-test pass (55 s) remains in
  `fast-gate-before-peak-timing.log` and `.exit`; no failed gate steps.
- 2026-11-16: final exact `steps::clippy` passes, exit 0, with zero new
  findings across all six host crates and five ARM crates. Printed counts:
  hal 93 entries/177 total, dsp 115/262, core 3/46, ui 249/733, fpgad 7/20,
  xtask 0/0. Runner `target/t0083-evidence/clippy-step.py` compiles unchanged
  xtask modules and calls the gate's actual function, with Cargo JSON-selected
  dependency artifacts. Complete output/exit: `clippy-step.log` and `.exit`;
  command logs: `target/check-logs/59905-1794852412476155000/`. No gate or
  ratchet edits. Earlier clippy evidence remains in
  `clippy-step-before-peak-timing.log` and `.exit`.
- 2026-11-16: focused debug/release suites pass, all commands exit 0.
  Each uses `cargo test --locked -p oos-ui --bin oos-ui FILTER -- --test-threads=1`,
  with `--release` for release. `board::`: 85 passed, two existing manual
  reports ignored, in both profiles. `ui::tests::epochs::`: 15 passed and the
  explicit timing guard ignored in both functional profiles; that guard passes
  in perf-guard execution below. `ui::tests::outbox::`: 35 debug and 34 release
  passes (one debug-only panic invariant). Total: 135 debug and 134 release
  passes, three ignores per profile. Epoch suites reran after the final timing
  test addition; board/control evidence reuses unchanged relevant source.
  Complete logs/exits: `target/t0083-evidence/debug-{board,epochs-final,outbox}.{log,exit}`
  and `release-{board,epochs-final,outbox}.{log,exit}`. Reproduction driver:
  `target/t0083-evidence/run-checks.py`.
- 2026-11-16: `sh tools/perf-guard.sh` passes, exit 0, printing
  `10 timing budgets and 4 zero-allocation budgets passed` and
  `perf guards: all passed (283 s)`. All deterministic HAL guards, paired
  best/p99 ratios, allocation guards and nine UI timing guards pass. The added
  `perf_guard_peak_intake_change_budget` repeatedly supplies 1000-record Peak
  batches through real publication and UI steps: paired intake/change p99 is
  10917 ns (10.917 us), below 500000 ns, over 256 retained observations.
  Complete output/exit: `target/t0083-evidence/perf-guards.log` and `.exit`;
  primitive observations also remain in `target/perf-guard.log`. Actual
  hardware full-step p50/p99, acquisition latency and trigger dead time are
  not measured by these host guards.
- 2026-11-16: the deterministic UI comparison matches a 720-record burst
  against 15 lossless 48-record batches: identical persistence maps (including
  an early distinct transient), statistics and all 200 retained history
  entries. Both consume 720 records; existing statistics count 15 measurement
  updates, one per step. This fixes transport loss, without changing the
  existing measurement cadence, bounded history or drawing algorithm. No
  golden reference changes. The live-budget counterpart yields across at
  least 15 steps, at most 48 records each, with zero drops and no duplicates.
- 2026-11-16: board regressions cover three consecutive 1000-frame batches
  with ordered delivery and no loss, admission blocked until the last queued
  frame is consumed, the existing count/sample bounds and oversized-newest
  exception, exact old-identity rejection, Normal's original 464-overflow
  behavior, and the Normal-to-Peak decode-cap transition. Planner coverage
  enumerates every time base/depth with one through four enabled channels:
  multi-frame reads fit the queue's sample bound and no batch exceeds 1000.
  Multi-frame read-out remains about 4 MB encoded, at most about 8 MB of f32
  samples, below the unchanged 128 MiB queue sample cap. Single oversized
  records retain the existing hardware/memory-plan bound. No new pending
  batch storage or growable producer backlog is introduced.
- 2026-11-16: limits: producer backpressure can introduce trigger dead time;
  it preserves already-read current-identity captures, not continuous capture
  of every physical trigger. Existing epoch/configuration rejection,
  STOP/RUN clearing and deliberate hardware frame-zero rejection remain.
  Control and STOP handling precede the new ordinary-batch wait; prearmed
  hardware is not disarmed merely to wait for the queue. The bounded 2 ms
  poll follows the current legacy worker's waiting model; event-driven
  acquisition remains the M3c migration. Hardware behavior is unverified.
- 2026-11-16: the strengthened S8 test fails against unchanged production code
  with exactly 464 overflows, exit 101 (`negative-control.log` and `.exit`).
  Initial fixed Peak suite passes (9 tests, exit 0), including 720 ordered
  records and the real UI record/picture/measurement/history path across steps.
  Additional planner/identity checks are being validated. An initial test-build
  failure (test module visibility) is retained in `debug-peak.log`, exit 101;
  corrected output is in `debug-peak-retry.log`, exit 0.
- 2026-11-16: implementation keeps one bounded Peak batch (1000 frames maximum,
  existing 32 Mi-sample cap), then polls the queue on the acquisition worker
  after controls/STOP and before the next batch read. Each wait releases the
  lock before sleeping 2 ms. A prearmed batch stays in its bounded FPGA ring.
  Peak decode also ignores an inherited Normal `take` cap, closing a transition
  race before publication. Normal decode, overflow and production pace retain
  their previous behavior. No changes to `ui/src/ui.rs`, step limits or ratchets.
- 2026-11-16: accepted the owner-assigned task on `t0083-peak-record-overflow`,
  clean base `9f04a63a` (task 0082). Read repository architecture, acquisition,
  records, development, delivery and review/integration contracts. This scope
  is the oscilloscope repository, so the OptiSource-only operating model does
  not apply. Plan recorded before implementation. Hardware unverified.
