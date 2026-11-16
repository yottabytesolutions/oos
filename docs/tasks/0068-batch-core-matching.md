---
id: 68
title: Match ADC cores on the batch path
kind: fix
status: active
release: none
owner: codex
depends: [66]
---

# Match ADC cores on the batch path

## Goal

Apply Standard's software ADC core matching to every Batch frame, with deterministic learning in record order and
no matcher locks or allocations per warmed frame. Follow task 0066 decision d and the existing matcher semantics.
Review tier: Unit. Hardware is not verified by this host-only task.

## Plan

Reserved paths: `ui/src/board/dho/cores.rs`, `ui/src/board/dho/matching.rs`,
`ui/src/board/dho/matching/`, `ui/src/board/dho/mod.rs`, `ui/src/board/dho/hw.rs` explicitly,
`ui/src/board/dho/hw/batch.rs`, `ui/src/board/dho/hw/batch/tests.rs`,
`ui/src/drawpipe.rs`, `ui/src/drawpipe/feed.rs`, `ui/src/drawpipe/decoder.rs`,
`ui/src/board/scripted.rs`, `ui/src/board/dho/scripted.rs`,
`ui/src/ui/script/host/hot_paths.rs`, `ui/tests/snapshots/hot_paths.rs`, `docs/architecture/README.md`, `tools/perf-budgets.py`,
`tools/tests/test_perf_budgets.py`, this task and `target/COMMIT_MSG.txt`.
The owner's direct task assignment authorizes this follow-up claim. `ui/src/ui.rs` belongs to another lane.

1. Separate ordered matcher learning from immutable correction application. Read strided DMA samples directly;
   reuse the matcher window and decoded frame capacities.
2. Prepare corrections in acquisition order before dispatch, including the newest frame. Pass indexed snapshots
   with the batch decoder, and bypass raw-code drawing for matched records. Preserve the unmatched paths.
3. Test Standard/Batch sample equality, corrected neighbours, out-of-order worker execution, skipped formats and
   warmed allocation counts. Run focused debug/release tests, performance guards, the gate's exact clippy command
   and the fast gate. Preserve complete logs and exit codes.
4. Review follow-up: decide buffer acceptance before Batch learning; exclude stale frame 0 from learning while
   drawing it with the prior pattern. Exercise the acquisition dispatch with and without a replacement buffer.
   Measure full 4 MB learning and derive a timing guard using the existing 4x A72 estimate. Apply the three
   requested readability fixes, then repeat focused debug/release, perf guards, exact clippy and the fast gate.
5. Record the review handoff and prepare an imperative commit message. Leave all changes uncommitted.

## Status

- 2026-11-16: Review follow-up ready. All requested corrections and optional readability fixes applied;
  candidate remains uncommitted on `t0068-batch-core-match`, parent `15efe011`. Acquisition regressions exercise
  the production dispatch, including two dropped neighbours when no replacement buffer exists. Both fail
  with the reviewed defects restored and pass with the fix. Requested final checks all exit 0:
  focused debug and release each 28 passed, 3 ignored timing guards (logs `target/review-focused-debug.log`,
  `target/review-focused-release-final.log`); exact host/ARM64 UI clippy has 0 compile errors and 0 new/grown
  findings (`target/review-clippy-summary.log`); fast gate prints
  `partial gate passed (not done): 8 steps in 46 s` (`target/review-fast-gate.log`, detailed run
  `target/check-logs/94741-1794852072545254000/`). No failed final selected checks.
  Full performance guard exits 0 with exact output `11 timing budgets and 6 zero-allocation budgets passed`
  and `perf guards: all passed (142 s)`; command `CARGO_TARGET_DIR="$PWD/target" tools/perf-guard.sh`.
  Complete logs `target/review-perf-check.log`, `target/perf-guard.log`, `target/perf-guard-step.log`.
  Final full 4 MB learning best / p99 CPU: 1 ch 0.951 / 1.035 ms, 2 ch 1.072 / 1.205 ms,
  4 ch 1.063 / 1.162 ms; estimated A72 p99 4.141 / 4.821 / 4.647 ms, unverified on hardware.
  The 9 ms projected budget derived below passes for every case. Both matching allocation probes report
  `64 0`. Python budget contract: 8 passed, exit 0 (`target/review-perf-budget-tests.log`).
  `git diff --check` passes; `target/review-files.sha256` pins the final changed files and
  `target/COMMIT_MSG.txt` has the plain imperative subject `Match ADC cores on accepted batch frames`.
  All working-tree changes are intentional task 0068 source, tests and evidence documentation.
  No commits, staging, pushes, merges, ratchet edits or hardware access. Task stays active for the
  coordinator's follow-up Unit review and integrated full gate; full completion is not claimed.

- 2026-11-16: Final-source focused debug: 28 passed, 3 ignored timing guards, 1216 filtered out, exit 0
  (`target/review-focused-debug.log`). Exact UI clippy host and ARM64 commands both exit 0:
  `cargo clippy --locked --all-targets --message-format=json --quiet`, plus
  `--target aarch64-unknown-linux-musl`, run from `ui`. Gate-equivalent primary-span deduplication reports
  `0 compile errors, 0 new/grown findings against the ratchet` (249 existing keys, 735 total).
  Logs `target/review-clippy-{host,arm64}.{jsonl,stderr}`, comparison `target/review-clippy-summary.log`.
  Initial clippy identified four style findings in the new test helper; fixed without lint changes.
  Final fast gate exits 0: `partial gate passed (not done): 8 steps in 46 s`;
  complete log `target/review-fast-gate.log`. Release and full performance verification continue.

- 2026-11-16: Full 4 MB `BatchMatch::learn` timing guard, 101 alternating rounds of three calls, passes initially
  (exit 0, `target/review-learning-perf.log`). Best / p99 CPU: 1 ch 0.956 / 1.192 ms;
  2 ch 1.074 / 1.175 ms; 4 ch 1.084 / 1.415 ms. Estimated A72 p99: 4.769 / 4.700 / 5.660 ms,
  using the existing perf plan's unverified 4x factor. New projected budget: 9 ms/4 MB, derived from worst
  observed p99 1.415 ms x 4 x 1.5 headroom, rounded up. The guard checks this projection plus Batch/Standard
  best <= 1.0 and p99 <= 1.5; hardware timing remains unverified. Matrix covers exactly 4,000,000 bytes per case.
  Reinstating both reviewed defects in live dispatch makes both acquisition regressions fail (0 passed,
  2 failed, exit 101, `target/review-acquisition-negative.log`); restored the fix before final verification.

- 2026-11-16: Acquisition dispatch now decides replacement-buffer acceptance before preparation. Only drawable
  records teach; stale frame 0 uses the prior pattern and does not change pattern weight or misfit state.
  Added exact reservations `ui/src/board/dho/hw/batch.rs` and `ui/src/board/dho/hw/batch/tests.rs` for the live
  dispatch extraction and host regressions. Tests exercise worker submission, newest publication and buffer
  exhaustion with `frame0_drop` both ways, cold and warmed matching. Optional doc/import/index fixes applied.
  Initial fast gate exits 0: `partial gate passed (not done): 8 steps in 33 s`, complete log
  `target/review-fast-gate-initial.log`, detailed run `target/check-logs/16242-1794851767854968000/`.
  Budget-parser tests: 8 passed, exit 0 (`target/review-perf-budget-tests.log`). Release timing build in progress.

- 2026-11-16: Owner supplied Unit review follow-up: learning must follow drawing acceptance and exclude stale
  frame 0. Review-ready status below is superseded while implementing and verifying these corrections.
  No commits requested; retain the active claim and existing reserved paths.

- 2026-11-16: Ready for independent cross-vendor Unit review. Final fast gate exits 0 with exact summary
  `partial gate passed (not done): 8 steps in 33 s`; complete log `target/fast-gate.log`, detailed run
  `target/check-logs/75444-1794850997385900000/`. All selected steps pass; not-selected checks remain as printed.
  Prior architecture/mutable-statics and performance-registry failures are resolved with evidence below.
  Candidate is the uncommitted working tree on parent `15efe011`, branch `t0068-batch-core-match`;
  `target/review-files.sha256` pins the changed files. Commit message prepared in `target/COMMIT_MSG.txt`.
  `git diff --check` passes. No commits, staging, pushes, merges or hardware access. `ui/src/ui.rs` and all
  ratchet files unchanged. Task remains active for the coordinator's Unit review, integrated full gate and
  any required hardware smoke under the owner's task-specific release.

- 2026-11-16: Final performance guard run exits 0. Exact final output:
  `10 timing budgets and 6 zero-allocation budgets passed`;
  `perf guards: all passed (52 s)`. Command `CARGO_TARGET_DIR="$PWD/target" tools/perf-guard.sh`.
  Complete logs: `target/perf-check.log`, `target/perf-guard.log`, `target/perf-guard-step.log`.
  The release census confirms `core_matching_learn_and_decode 64 0` and `core_matching_worker_decode 64 0`.
  All eight UI timing guards pass, including the two ignored timing tests in the focused suites.
  Final fast gate rerunning after the Python registry/test changes; Rust sources remain the debug/release/clippy candidate.

- 2026-11-16: Initial `tools/perf-guard.sh` exits 1: all HAL and UI deterministic/timing tests passed (8 UI timing
  guards), but the final checker reports `perf budgets: invalid allocation observation`. Its fixed required-probe
  set lacked the two new matching observations. Added both to `tools/perf-budgets.py` and documented the six probes;
  no timing/allocation threshold changed. New Python test proves each matching observation is required.
  `python3 -m unittest tools.tests.test_perf_budgets`: 7 passed, exit 0 (`target/perf-budget-tests.log`).
  Reserved paths expanded for that necessary registry change and test. Initial complete logs retained as
  `target/perf-check-initial.log`, `target/perf-guard-initial.log`, `target/perf-guard-step-initial.log`.

- 2026-11-16: Focused release suite passes (26 passed, 2 ignored timing guards, 1215 filtered out), exit 0:
  `cargo test --locked --release -p oos-ui --bin oos-ui -- board::dho::cores board::dho::matching drawpipe`.
  Complete log `target/focused-release.log`. Full performance guards now running serially after builds/checks.

- 2026-11-16: Final-source fast gate exits 0, exact summary:
  `partial gate passed (not done): 8 steps in 37 s`. Complete output `target/fast-gate.log`;
  detailed run `target/check-logs/` (the directory printed at the start of that log). All eight selected steps pass;
  clippy/tests/perf/coverage and host/image checks are not selected by the fast gate. Earlier failed architecture
  and mutable-statics run is retained in its check-logs directory. No ratchets, lint limits or architecture rules
  changed. `target/COMMIT_MSG.txt` contains a plain imperative subject under 72 characters, a blank line and a why body.

- 2026-11-16: Focused debug suite passes (26 passed, 2 ignored timing guards, 1216 filtered out):
  `cargo test --locked -p oos-ui --bin oos-ui -- board::dho::cores board::dho::matching drawpipe`, exit 0,
  complete log `target/focused-debug.log`. Coverage includes six channel masks, header/no header, short frames,
  changing lengths through the estimator cap, reversed concurrent decoding, frozen in-flight snapshots,
  rejected/relearned patterns, clipping, incomplete interleave groups, excluded Peak/pairs and actual worker pictures.
  Replacing only the worker decoder with the previous uncorrected decode made the neighbour regression fail
  (0 passed, 1 failed, exit 101); restored the implementation afterward. Log `target/core-negative-control.log`.
  The first negative-control attempt hit a missing module qualifier in the extracted test helper; corrected it
  before collecting the behavioural failure above.
- 2026-11-16: Allocation census passes in debug (exit 0, `target/allocation-debug.log`): both matching preparation
  plus decode and immutable worker decode report `ALLOC_BUDGET ... 64 0` after eight warm-up iterations.
  `cargo test --locked -p oos-ui --test soak hot_paths_allocate_nothing_after_warmup -- --nocapture`.
  Gate-exact UI clippy commands from the ui directory both exit 0:
  `cargo clippy --locked --all-targets --message-format=json --quiet`, and the same with
  `--target aarch64-unknown-linux-musl`. Gate-equivalent primary-span deduplication across host/ARM64 finds
  0 compile errors and 0 new/grown findings against `xtask/data/ui/clippy.txt` (249 existing keys, 735 total).
  Logs `target/clippy-{host,arm64}.{jsonl,stderr}`, comparison `target/clippy-summary.log`.

- 2026-11-16: Ordered learning and indexed immutable correction snapshots implemented. Standard retains its original
  f64 word-to-volts result; raw-code drawing is bypassed only for matched Normal, uncompressed batches. The newest
  record is prepared after its neighbours. Window and decoded vectors reuse capacity. Snapshot bound recorded in
  architecture state ownership: 192 B/frame, at most 1000 frames/batch and three buffers (576 kB).
  Initial fast gate failed architecture (allocation-probe re-export crossed the board boundary) and mutable-statics
  (the moved submission helper renamed the existing IDS owner). Both fixed using port wrappers and retaining
  `Pipe::feed_batch`; no guardrail edits. Initial test compilation also needed the fixture depth converted to i128;
  a zero-worker fixture was corrected to use real workers because only a returned strong batch reference retains
  a helper-only submission. Sample equality, neighbour and worker-picture tests pass; verification continues.

- 2026-11-16: Codex claimed branch `t0068-batch-core-match` at `15efe011` under the owner's direct assignment.
  Plan written before code. Read delivery, task 0066, architecture and core-offset/self-cal 14.x contracts.
  Independent cross-vendor Unit review and the coordinator's integrated full gate remain required.
