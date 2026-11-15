---
id: 51
title: "G2: performance gates"
kind: feature
status: active
release: 0.2.0
owner: codex
depends: []
---

# G2: performance gates

## Goal

Gate the performance budgets before M3c, as [migration G2](../architecture/migration.md#g2-performance-gates) specifies.

## Plan

Reserved paths: `docs/tasks/0051-g2-performance-gates.md`, `tools/perf-guard.sh`,
`ui/src/system/perfguard.rs`, `ui/src/system.rs`, `hal/tests/perf_support/`, `tools/perf-budgets.py`, `tools/tests/test_perf_budgets.py`,
`ui/src/perfguard.rs`, timing-test call sites in `ui/src/fastdraw.rs`, `ui/src/sparse.rs`,
`ui/src/compress.rs`, `ui/src/framestats.rs`, `ui/src/drawpipe.rs`, `hal/tests/perf_ratio.rs`,
`ui/src/ui/script.rs`, `ui/src/ui/script/host.rs`, `ui/src/ui/script/host/hot_paths.rs`,
`ui/tests/support/mod.rs`, `ui/tests/soak.rs`, `ui/tests/support/counting.rs`, `ui/tests/hot_paths.rs`,
`xtask/data/architecture.txt` (new allocator authorization only), `docs/architecture/README.md`,
`ui/tests/golden/README.md`, `target/COMMIT_MSG.txt`.

1. Provide matching bounded interleaved reference/candidate timing samplers for HAL and UI. Keep best-of-rounds
   assertions, add exact nearest-rank p99 ratios and deterministic sampling/regression tests.
2. Extract M1's test allocator without changing its accounting. Exercise existing allocation-free hot kernels
   after warm-up in an isolated child; prove transient allocations and reallocations are rejected.
3. Read the architecture budget rows into a host-check manifest with explicit deferred rows. Add parser tests
   and guard wiring; preserve absolute unit budgets for the next unit round.
4. Run focused debug/release tests, the fast gate and timed perf guards, including a loaded-host run.
   Supply complete logs and a proposed commit message; do not commit, merge, push or access hardware.

Tail-aware perf guards (p99 over repeated runs against a reference path, beside the best-of-rounds ratios); a counting-allocator harness for the hot paths asserting zero allocations per frame after warm-up (reuse M1's ui/tests/soak.rs allocator); budget rows from the architecture's table that the host can measure. Unit rows go to the next unit round.

## Status

- 2026-11-15: Final-source loaded-host confirmation passed twice with 16 busy CPU processes, one per
  logical host CPU. `target/g2-perf-loaded-1.log`: `perf guards: all passed (89 s)` (89.15 s wall);
  `target/g2-perf-loaded-2.log`: `perf guards: all passed (98 s)` (97.58 s wall). Both exited 0,
  checked ten timing rows across 59 workloads and all four allocation rows. The load wrapper
  `target/g2-loaded.py` terminates and reaps every owned load process in `finally`; outer validation
  exited 0 (`target/g2-validation-summary.log`, `target/g2-loaded-summary.log`). Cached ordinary perf
  execution was 53.91 s. Measured worst ratios per row/run: `target/g2-budget-observations.csv`.
  Final source receipt remains `6e5bc9470890267386d53f80e172148a42fd26bafea13f6c12a5868083b8ce7b`.
  Implementation ready for tooling review; task stays active pending coordinator acceptance. No commits,
  push, merge or hardware access. Proposed two-part messages/file lists: `target/COMMIT_MSG.txt`.
  Open: independent Claude review, coordinator full batch gate and its total duration, Linux host
  validation, future-stage harness extensions, and absolute unit budgets in their next unit rounds.
- 2026-11-15: Reproduction commands for focused suites: `cargo test --offline --locked -p oos-ui --test
  hot_paths -- --nocapture`; `cargo test --offline --locked -p oos-ui --bin oos-ui perfguard::`;
  `cargo test --offline --locked -p oos-hal --test perf_ratio`; `cargo test --offline --locked -p
  oos-ui --test soak retained_growth_and_freed_transients_are_distinguished -- --nocapture`.
  Repeat sampler and soak commands with `--release`; release hot-path execution is wired into
  `sh tools/perf-guard.sh`. `cargo clippy --offline --locked -p oos-ui -p oos-hal --all-targets
  --message-format=json` exited 0; `python3 -m unittest discover -s tools/tests -p test_perf_budgets.py
  -v` passed six tests. `sh -n tools/perf-guard.sh` and `git diff --check` passed.

- 2026-11-15: Final-source fast gate: `partial gate passed (not done): 8 steps in 21 s`, exit 0,
  `target/g2-fast-final.log`. Final cached perf guard: `10 timing budgets and 4 zero-allocation budgets
  passed`; `perf guards: all passed (54 s)`, exit 0, `target/g2-perf-final.log` (53.91 s wall;
  UI timing 32.69 s). Worker delivery uses the fast gate; default full-gate duration against the
  approximately twenty-minute budget is still the coordinator's integration measurement.
- 2026-11-15: Focused final-source debug/release checks all exited 0; `target/g2-focused-summary.log`
  records durations and `target/g2-<name>.log` the complete outputs. Allocation debug suite: 5 passed
  (4.07 s including build); release suite passed in the perf guard. Both UI sampler suites: 4 passed
  (debug 11.58 s, release 79.03 s including build); both HAL sampler suites: 4 passed, 2 deliberately
  ignored timing tests (debug 0.62 s, release 4.77 s). M1 retained-growth/transient allocator probe:
  1 passed in each profile (debug 0.82 s, release 55.69 s including build). Commands use
  `cargo test --offline --locked`, package/target/profile/name selections in the logs' summary script
  `target/g2-focused.py`. Budget parser: 6 passed (`target/g2-budget-tests.log`).
  Source receipt for this uncommitted candidate: `6e5bc9470890267386d53f80e172148a42fd26bafea13f6c12a5868083b8ce7b`.

- 2026-11-15: Preliminary CPU-tail guard run passed all ten timing rows and four allocation rows in
  236 s including cold release compilation (`target/g2-perf-cpu.log`); UI timing execution was 40.20 s
  and HAL timing 18.69 s. Subsequently bounded the worker benchmark to one recycled input buffer
  instead of 101 retained copies, keeping the tail sample count from multiplying fixture memory.
  Final-source focused tests and loaded-host confirmation are in progress. Clippy for UI/HAL all targets
  exited 0 in 9.40 s with no diagnostics in the new sampler/allocator/probe modules.

- 2026-11-15: Longer-run wall-clock p99 experiment also failed under host scheduling noise (worker
  ratio 4.182 > 2.25, sparse ratio 5.649 > 1.65), log `target/g2-perf-second.log`, exit 101.
  Retained the existing elapsed best-ratio gates; p99 work budgets now use POSIX process CPU including
  asynchronous workers. The test observer blocks instead of spinning while workers finish. The CPU
  adapter is test-only under `ui/src/system/perfguard.rs`, an existing permitted unsafe boundary;
  `ui/src/system.rs` is additionally reserved for its test-only declaration. No production timer changed.

- 2026-11-15: Implemented 101-round interleaved distributions and unchanged best-ratio limits, plus
  explicit p99 ratios with 50 % scheduling headroom. Added ten executable architecture host rows and
  four zero-allocation UI probes using the extracted M1 allocator. Additional strictly required paths:
  `ui/src/ui/script.rs`, `ui/src/ui/script/host.rs`, `ui/src/ui/script/host/hot_paths.rs`,
  `ui/tests/support/mod.rs`, `ui/tests/golden/README.md`, and the two crate-local sampler modules.
  Cross-crate path imports are forbidden by the source walker, so sampler identity is checked instead.
- 2026-11-15: Initial fast gate failed (`size`, `mutable-statics`, `unsafe`, `method-exceptions`,
  `known-failures`, exit 1) before checks could walk the HAL sampler because its module was
  placed below the integration target stem. Corrected it to the default target-root module directory.
  Next fast gate: `partial gate passed (not done): 8 steps in 22 s`, exit 0, log `target/g2-fast.log`.
  Debug allocation suite: 5 passed, child proved alloc/realloc/zeroed transients and four zero-allocation
  kernels. Six Python budget contract tests passed. First short-run timing experiment failed a p99
  under host scheduling noise (HAL f32 ratio 1.692 > 1.2); restored longer fixed runs for amortization.

- 2026-11-15: Codex claimed the assigned tooling worker job on `t0051-g2`, base `18ac5c7`.
  Claim and plan recorded before implementation. Held M2/settings, pacing/roll and doctor paths excluded.
  Default full integration gate and cross-vendor review remain coordinator work per delivery rules.

- 2026-11-15: seeded by the coordinator.
