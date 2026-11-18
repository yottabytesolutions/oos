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

## Status

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
