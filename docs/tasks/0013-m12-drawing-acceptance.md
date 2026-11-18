---
id: 13
title: "M12: acceptance and removal (D9)"
kind: chore
status: active
release: 0.2.0
owner: codex
depends: [12]
---

# M12: acceptance and removal (D9)

## Goal

Complete [migration M12](../architecture/migration.md#m12-acceptance-and-removal-d9). The migration document is the specification.

## Plan

Reserved paths: `ui/src/ui.rs` (test registration only),
`ui/src/ui/draw_routes.rs`, `ui/src/ui/draw_routes/`,
`ui/src/system.rs`, `ui/src/system/test_allocator.rs`, `ui/tests/support/counting.rs`, `ui/tests/soak.rs`,
`ui/src/board/scripted.rs`, `ui/src/board/dho/scripted.rs`,
`ui/src/board/dho/scripted/evidence.rs`, `tools/perf-guard.sh`, `docs/architecture/drawing.md`, this task.

M12a (#157): compare identical captured batches through both UI routes, characterize shared
record-only modes and weights, measure host route build/allocations, and isolate the four-channel
10 us/div cost. Publish a recommendation with explicit hardware/prerequisite limits. No route deletion
or production behavior change; owner selection and M12b remain separate.

Follow [migration M12](../architecture/migration.md#m12-acceptance-and-removal-d9), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

Issue #184 reserves `ui/src/drawpipe.rs`, `ui/src/drawpipe/worker.rs`, `ui/src/drawpipe/budget_tests.rs`,
`ui/src/ui.rs`, `ui/src/ui/batch_merge.rs`, `ui/src/ui/batch_merge/`, `ui/src/ui/lifecycle.rs`,
`docs/architecture/README.md`, `docs/architecture/drawing.md`, `CHANGELOG.md` and lowered UI size ratchets.
Bound the merge's pixel-sample work by combining excess shares per producer, then use the existing capacity-one
Worker for picture assembly. Preserve the existing picture-equivalence tests. Add separate host merge/UI timings
at four channels in Normal acquisition at 1 us/div and 10 us/div. Coordinator owns review and the unit round.

PR #186 unit follow-up also reserves `ui/src/drawpipe/reduction.rs`, `ui/src/drawpipe/identity_tests.rs`,
`ui/src/framecost.rs`, `ui/src/perfguard/draw.rs`, `ui/src/perfguard/ring.rs`, `ui/src/perfguard.rs`,
`ui/src/ui/step_draw.rs`, `ui/src/ui/tests/change_picture_contracts.rs`,
`tools/tests/fixtures/perfmap/state.txt`, `tools/tests/fixtures/perfmap/README.md`,
`docs/spec/state-query.md` and generated lowered ratchets.
Measure serial and channel-parallel reduction on the same four-channel host rows. Reuse bounded Workers and
placement, retain frame/pixel equivalence, allocate only enabled accumulator maps, reject empty completions,
and expose worker elapsed time plus frames per completed picture. Validate UI debug/optimized suites and fast gate.

## Status

- 2026-11-18: PR #186 unit A/B follow-up ready for coordinator review and another DHO804 round. Channel-parallel
  reduction reuses bounded Workers and little-core placement, without copying producer pixel buffers. Empty
  completions preserve the visible picture and clear intent; live jobs require available maps. Replacement jobs
  skip the visible-map copy. STATe and perf counters expose worker p50/p90/p99, per-step new-frame quantiles
  including zeros, and cumulative worker microseconds. The real-handler fixture was regenerated and all 17
  perfmap parser tests pass. Enabled-only maps reduce the reviewed branch's steady payload bound by up to
  37.30/24.87 MiB at 1/2 channels. A delayed-consumer test bounds each producer to three reused accumulators.
  The full +45-50 MB RSS delta over main is not attributed: extra pixel ownership accounts for at most 19.89 MiB
  compared with two full-channel accumulators per producer; a matched unit allocation/RSS census remains open.
  Final isolated shipping-release merge p50/p90/p99: 1 us/div serial 0.349/0.385/0.446 ms, parallel
  0.164/0.182/0.202 ms; 10 us/div serial 0.341/0.366/0.386 ms, parallel 0.151/0.164/0.172 ms.
  UI handoff p99 0.005/0.003 ms and whole-step p99 0.631/5.840 ms. Both perf guards exit 0.
  Full UI debug/optimized commands exit 0: 1,606/1,605 unit tests, plus 6 golden, 15 script, 8 socket and 14
  soak/support tests per profile. Warmed allocations remain zero. Fast gate prints
  `partial gate passed (not done): 11 steps in 5 s`; Clippy prints
  `partial gate passed (not done): 1 steps in 38 s`. Initial size slack and stale schema-fixture failures were
  corrected without relaxing limits; generated ratchets retain only lowered entries in changed files.
  Production delta +138 UI lines, zero elsewhere (`cargo xtask loc --compare HEAD`). Complete final logs:
  `target/batchmerge-{fast,clippy,debug,optimized,perf,parser}.log`. No Git writes or hardware access.
  Unit expectations are in drawing.md: worker p90 <16.5 ms, p99 <33.3 ms, handoff p90 <=0.1 ms, zero drops
  at 10 us/div and no increase over main's matched 17-33/s at 1 us/div. These are unverified targets. Coordinator
  owns Git preparation, cross-vendor review, required CI and unit timing/drop/empty-picture/RSS acceptance.

- 2026-11-18: PR #186 check fix splits hardware-average fallback assertions from the blending/restart test.
  This removes the new Clippy function-size finding without changing production code or ratchet allowances.
  Host UI debug and optimized suites exited 0, including both hardware-average tests. Clippy printed
  `partial gate passed (not done): 1 steps in 29 s`; fast gate printed
  `partial gate passed (not done): 11 steps in 5 s`. Production delta +0 in every crate.
  Fetch was blocked by the sandbox; HEAD matches the cached branch ref. The supplied public check logs could
  not be retrieved. Coordinator must inspect the remote review failure and rerun CI; no review threads were supplied.
  No hardware access.

- 2026-11-18: Issue #184 host implementation ready for coordinator review and unit verification. One combined
  share per producer bounds reduction to 8,691,200 pixel samples at the unit's four-channel geometry, without
  losing frame weights/counts. Live publication uses the existing bounded Worker with recycled snapshot storage.
  Production delta: +247 UI lines, zero in other crates (`cargo xtask loc --compare HEAD`).
  Final full UI debug/optimized suites exited 0: 1,579/1,578 unit tests, plus 6 golden, 15 script, 8 socket and
  14 soak/support tests per profile. The unchanged picture comparisons and warmed allocation guard pass.
  Final focused optimized run including both performance probes: 7 passed. Isolated worker merge p99
  0.400/0.472 ms and complete UI-step p99 0.803/6.532 ms at 1/10 us/div. Shipping-release merge guard passed too.
  One concurrent timing rerun exceeded the UI-step ceiling; the final isolated rerun above passed.
  Initial size/test-clock findings and four warmed snapshot allocations per step were fixed without loosening
  limits. Generated ratchets retain only lowered entries in the changed drawpipe/UI files.
  Fast gate: `partial gate passed (not done): 11 steps in 6 s`. Clippy step: `gate FAILED in 8 s: clippy`, with
  three pre-existing findings only: cognitive complexity and too many lines in `ui/src/ui/menu/simulation/tests.rs`,
  and cognitive complexity in `ui/src/ui/menu/tests.rs`. Both files match HEAD byte-for-byte and remain untouched.
  No hardware access. Coordinator owns those existing findings, cross-vendor review, full integration gate and the
  DHO804 unit round. Expected unit targets are in drawing.md; no measured unit improvement is claimed.

- 2026-11-18: Issue #184 assigned on `perf/184-batch-merge`. Plan above recorded before final validation.
  One combined share per producer preserves all pixel weights and frame counts; the live UI polls a capacity-one
  picture worker. Headless runs wait for completion events. Host tests and performance evidence are in progress.

- 2026-11-18: PR #161 review fix: both M12a fixture types now use `Route::dispatch` for acquisition preparation,
  route selection, frame delivery, the buffer-return barrier and UI stepping. The production-plan fixture implements
  the existing batch interface and retains its boxed buffer. The one/full-step matrix now checks two consecutive
  steps, counted frames and newest sequence numbers. Focused host route tests: 5 passed, 2 ignored per test target.
  Full single-threaded UI debug and optimized suites passed: 1,515/1,514 unit tests, plus 6 golden, 15 script,
  8 socket and 14 soak/support tests in each profile. The optimized route timing probe passed all 42 rows per test
  target. Host Clippy passed with no findings in the changed files; fast gate: `partial gate passed (not done):
  11 steps in 2 s`. No lowered ratchets reported. Production delta +0; host only.
- 2026-11-18: M12a host evidence is in [drawing](../architecture/drawing.md#m12a-host-draw-route-evidence-2026-11-18-157).
  Five deterministic route tests cover the requested inputs/transitions, including actual software segment completion
  and selection of an older history record. Live-pair weights differ; shared STOP/deep-read contract gaps are explicit.
  Final release evidence probes: 2 passed; 42 route timing/allocation rows plus the paired deposit profile. Four-channel plan:
  312.5 MSa/s, 35,160 samples/channel, k=1, 14 frames/read. Worker decode/deposit costs are identified; the historical
  unit 18.2 fps cause is not established. No local production fix, route deletion or owner selection.
- 2026-11-18: `cargo xtask loc --compare HEAD`: production delta +0 in every crate and total. Initial fast-gate
  module-path failures were corrected using default module registration; final `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps in 4 s`. No lowered ratchets reported. Debug UI crate tests passed.
  Initial optimized UI run passed unit/golden/script/socket suites but failed the existing process-wide observer
  census (prepare_keys=4, stage=1 unexpected allocations); the isolated rerun passed with all six phases at zero.
  Final `RUST_TEST_THREADS=1 cargo test -p oos-ui --profile test-release` passed: 1,506 unit tests (58 ignored),
  6 golden tests (1 ignored), 15 host scripts, 8 host sockets and 14 soak/support tests. The earlier concurrent
  observer-census failure remains recorded; no unrelated test change or ignore was added. Final focused release
  route tests: 5 passed, 2 timing probes ignored. No hardware tests claimed.
- 2026-11-18: Validation limits: host only; no hardware access. Coordinator still owns independent review, full gate,
  M2..M11 prerequisite acceptance, matched unit profile/load/soak, owner selection and M12b. M12 remains active.

- 2026-11-18: M12a evidence assigned on `test/157-m12a-evidence`, base `e846b2cf`.
  Host-only worker brief; runner owns git writes and coordinator owns review/full gate/unit acceptance.

- 2026-11-14: seeded from task 0001. Not yet claimed.
