---
id: 95
title: Faster CI pipeline with fewer external actions
kind: chore
status: active
release: 0.2.0
owner: agent
depends: []
---

## Goal

Shorter wall time for the required checks of a pull request and for the nightly gate, on free GitHub-hosted runners,
without a required check that can pass unseen. Fewer third-party actions. A working image job.

## Plan

Reserved paths: `.github/workflows/pr.yml`, `.github/workflows/main.yml`, `.github/workflows/release.yml`,
`.github/workflows/image.yml`, `.github/workflows/privacy.yml`, `tools/ci/changes.py`, `tools/ci/run_summary.py`,
`tools/ci/kernel-base.sh`, `tools/ci/pr_meta.py`, `tools/ci/apply-repo-settings.sh`, `tools/tests/test_changes.py`,
`tools/tests/test_run_summary.py`, `tools/tests/test_pr_meta.py`, `docs/development.md`, `kernel/README.md`.

- Path filter (`tools/ci/changes.py`, tests): documents, GitHub settings and CI scripts skip the Rust jobs; every
  required job still reports. Merge-queue groups with the PR head's tree skip checks that passed there.
- Release tests per PR without LTO; LTO release tests, coverage and doctor nightly. Debug info line tables only.
- Caches saved only by the push to main, which builds tests without running them.
- Nightly gate as parallel `cargo xtask step` jobs.
- cargo-deny and git-cliff through taiki-e/install-action; allow list without their actions.
- pr-meta: only `ubuntu-24.04`, `ubuntu-24.04-arm`, `ubuntu-latest`.
- Kernel base as the pre-release `kernel-base-6.12.115` (`tools/ci/kernel-base.sh`).
- Job and step durations in the run summary; before and after timings in the PR.

- Hooks: only the fast tier before a push; child commands of xtask never see the hook's `GIT_*` variables.

Not done, with reasons: cargo-nextest (171 s instead of 252 s for the warm debug tests locally, but both test jobs
hung in CI; follow-up #23); mold (the pinned toolchain already links with rust-lld on x86-64 Linux, and `RUSTFLAGS` would
replace the scope target's flags in `.cargo/config.toml`); splitting clippy into host and aarch64 jobs (the clippy
ratchet counts a finding seen on both targets once; split runs would need an xtask change of the ratchet).

### Round 2, issue #141

Reserved paths (round 2): `Cargo.toml`, `xtask/src/`, `tools/perf-guard.sh`,
`tools/tests/test_image_outputs.py`, `.github/workflows/pr.yml`, `.github/workflows/main.yml`,
`tools/tests/test_changes.py`, `docs/development.md`, this task file.

1. Measure cold and warm local `cargo xtask check` and `check --fast`, including command wall time.
   Use existing local CI evidence only; this worker may not access GitHub or run `gh`.
2. Share source measurements within one gate run and format the whole workspace in one Cargo call.
   Use line-table debug info for host builds. Split lints from standalone decoder checks in CI and retain
   workspace artifacts in the existing Rust caches. Prepare performance binaries concurrently in a separate
   target subdirectory, then execute the original guards alone. Preserve failures, test profiles, partitions,
   doctests, coverage floors and required CI check names. Divide test concurrency between debug, release and
   coverage in a full local run, respecting an explicit nextest override and leaving single-step CI defaults alone.
3. Run focused debug/release tests and the fast gate, repeat timings and record the comparison for the coordinator.

### PR #148 gate repair

Reserved paths: `tools/build-tools.json`, `.github/workflows/pr.yml`, `.github/workflows/main.yml`,
`xtask/src/doctor/tests.rs`, `tools/tests/test_build_tools.py`, `ui/src/drawpipe.rs`,
`ui/src/drawpipe/host.rs`, `ui/tests/golden/README.md`, `docs/development.md`, this task file.

1. Run `doctor-base` and `tests-release` against an isolated snapshot of cached `origin/main` before edits.
2. Align the nextest pin across bootstrap, doctor and CI; diagnose the soak allocation lifetime and add a
   deterministic regression without increasing the allowance unless the measured bound is wrong.
3. Run focused crate tests, the fast gate and the full gate. Treat previous unpaired local timings as inconclusive
   under shared host load. CI job and step timings require coordinator-supplied evidence under this job's network rules.

### PR #148 critical-path repair

Reserved paths: `.github/workflows/pr.yml`, `xtask/src/testrun.rs`, `xtask/src/stack_ci/tests.rs`,
`docs/development.md`, this task file. Test fixtures may extend `xtask/src/testrun/` if needed.

1. Inspect the actual per-test hash partitions and local test durations to identify the slow shard's work.
2. Distribute the expensive tests explicitly while hashing the remaining tests; retain complete, disjoint coverage,
   doctests once, both profiles and the aggregate required `test` check. Run register plans alongside clippy and
   decoders without splitting host/ARM lint deduplication.
3. Run partition regression tests, debug/release xtask tests and the fast gate. Record supplied CI baseline evidence;
   the coordinator supplies after timings and full CI acceptance under this worker's network/Git restrictions.

### Round 3, issue #141 follow-up

Reserved paths: `.github/workflows/pr.yml`, `xtask/src/steps/decoders/`,
`xtask/src/steps/decoders.rs`, `xtask/src/testrun.rs`, `xtask/src/run.rs`,
`xtask/src/run/tests.rs`, `xtask/src/stack_ci/tests.rs`, `docs/development.md`, this task file.

1. Split the 15 standalone decoder configurations across independent CI runners, retaining each configuration's
   all-target, no-default-feature check and the aggregate `clippy` result. Preserve the host/ARM lint ratchet.
2. Inspect nextest durations and selections for debug shard 2/4. Spread its expensive tests across the available
   runners without changing tests, profiles, doctest coverage or the aggregate `test` result.
3. Run focused xtask tests in debug and test-release, all changed shards, the fast gate and the default full gate.
   Record the supplied run 44196789417 baseline. The coordinator supplies the PR's after-run timings because this
   worker has build-only network access and may not use GitHub or `gh`. Target: PR wall time at most 220 s.

## Status

- 2026-11-18: Round 3 default full gate exited 1: `gate FAILED in 507 s: perf`.
  Final fast gate exited 0: `partial gate passed (not done): 11 steps in 3 s`.
  All other steps passed: debug/release tests (including 222 xtask tests in each profile and 1513/1512 UI tests),
  coverage, host/ARM clippy, all 15 decoder configurations, Python and all 65 register plans. Performance
  preparation passed in 438.4 s. The unchanged `ui::tests::shadow::perf_guard_intake_change_shadow_budget`
  failed its picture p99 assertion: 883959 ns against the calibrated host limit of 750000 ns; the other 23 UI
  performance tests passed. The guard and its threshold were left unchanged, and it was not retried for a green
  result. This failure requires coordinator investigation before full-gate acceptance.
  Changed files have no lowered ratchet counts. Production line delta is 0; Rust tooling grows by 23 lines.
  Host only; no hardware accessed. Task remains active for full-gate acceptance, independent review and the
  PR's CI after timings verifying the 220-second target. Complete local evidence is in `target/pipeline-speed3/`
  and the full gate's `target/check-logs/` directory.

- 2026-11-18: All three decoder groups passed through `cargo xtask step decoders` (exit 0 each),
  checking five configurations each: `partial gate passed (not done): 1 steps in 60 s`, then 64 s and 88 s.
  Through-Cargo wall times were 59.90, 64.15 and 88.41 s; these sequential local checks are not CI after timings.
  The unpartitioned default full gate is running. Production lines remain 108344 (delta 0); Rust tooling grows
  by 23 lines. The final fast gate so far passed all 11 steps in 2 s, with no lowered counts in changed files.

- 2026-11-18: All five debug partitions passed through `cargo xtask step tests-debug` (exit 0 each):
  `partial gate passed (not done): 1 steps in 31 s`, then 32 s, 39 s, 28 s and 20 s.
  Through-Cargo wall times were 31.48, 32.06, 39.52, 28.05 and 20.29 s. The dedicated shards skip
  unrelated crate builds; doctests still run only in shard 1. These sequential local runs are not CI after timings.

- 2026-11-18: Round 3 nextest JSON verifies complete, disjoint coverage of all 1513 runnable UI tests:
  511, 480, 515, 1 and 6 per shard. Baseline debug hash shard 2/3 (CI 2/4) passed 481 UI tests in 31.796 s
  locally. Its two dominant tests were second-pane layout (30.246 s) and large-monitor random input (27.638 s);
  the next slowest was 8.063 s. The layout test now runs alone in shard 4/5, with golden tests in 5/5.
  These local measurements identify the imbalance; CI per-test evidence is still requested.
  Decoder groups select configurations round robin, including core-only, with five configurations each today.
  Debug and test-release xtask tests pass (222 each), and the initial fast gate exited 0:
  `partial gate passed (not done): 11 steps in 2 s`. All partitioned steps and the full gate remain in progress.

- 2026-11-18: Round 3 assigned on `ci/141-pipeline-speed-3`, base `2e2af564`. Decoder checks currently
  execute 15 feature configurations serially; the four test shards reserve only the golden binary explicitly.
  Local baseline shard 2 is running; CI per-test logs and after-run timings requested from the coordinator.

- 2026-11-18: Critical-path repair full gate exited 0: `gate passed (default): 22 steps in 429 s`.
  All 220 xtask tests passed in both profiles, UI debug/release passed 1491/1490 tests, and coverage, host/ARM
  clippy, every decoder configuration, Python, all 65 register plans and performance guards passed. Changed
  files have no lowered ratchet counts. Production line delta remains 0; Rust tooling grows by 9 lines.
  Evidence is retained in `target/pipeline-critical-path/` and the full gate's `target/check-logs/` directory.
  Host only; no hardware accessed. The task remains active for coordinator publication, remote synchronization,
  independent review and CI after timings demonstrating the under-300-second PR target. No CI improvement is claimed.

- 2026-11-18: All four debug shards pass through `cargo xtask step tests-debug` (exit 0 each):
  `partial gate passed (not done): 1 steps in 26 s`, then 34 s, 40 s and 21 s. Through-Cargo wall times were
  26.49, 34.20, 40.73 and 22.44 s. These are sequential local runs, not CI after measurements.
  The golden shard reports six passed tests and skips the five unrelated crate builds. The other shards jointly
  run all remaining tests, and doctests still run only with the first shard. The default full gate is running.

- 2026-11-17: Actual nextest JSON confirms old hash shard 2/3 contains `compare_scripted_pictures`,
  `second_pane_fits_with_everything_else` and large-monitor random input. In an unsharded local debug run these
  took 32.292, 34.576 and 31.273 s; the trigger corpus took 44.649 s in another hash shard. These local durations
  identify expensive tests, not CI timings. The debug composed soak skips its long work by default.
  The new four UI selections cover all 1491 runnable tests exactly once (501, 474, 510, 6), verified from actual
  nextest lists. Two partition regressions fail before the fix (exit 101); xtask test-release passes all 220 tests.
  The fast gate exits 0: `partial gate passed (not done): 11 steps in 2 s`. Changed source has no lowered ratchet
  counts. Production line delta is 0; Rust tooling grows by 9 lines. All four xtask debug shards are being exercised.

- 2026-11-17: Critical-path repair assigned on `ci/141-pipeline-speed`, runner-provided revision `9e1ea3d6`.
  Cached branch comparison is two commits ahead and one behind, not a simple behind state. No fetch/reset performed
  under the worker's no-Git-writes and build-only network policy. The runner owns ref synchronization and publication.
  Existing test partitions already hash individual tests; decoder checks already have a separate parallel matrix row.

- 2026-11-17: PR #148 repair full gate exited 0: `gate passed (default): 22 steps in 526 s`.
  UI tests passed in debug (1493) and test-release (1492), including the unchanged five-mode release soak;
  xtask passed 218 tests in each profile. Coverage, host/ARM clippy, all decoder configurations, Python,
  performance guards and all 65 register plans passed. No lowered ratchet counts were reported for changed source files.
  Production line delta is 0. This is host-only validation, not a local speed comparison: sampled shared-host
  load averages varied from 33 to 190 on 16 cores. The documentation fast check exited 0:
  `partial gate passed (not done): 11 steps in 2 s`.
  CI critical-path/job/step timing evidence, fresh remote-ref confirmation and independent review remain
  coordinator actions. The timing table explicitly leaves the CI measurements open; the task remains active.

- 2026-11-17: The destructor-barrier regression failed on both the unchanged baseline batch and the old branch
  (exit 101), then passed in test-release with the lease last (exit 0). The test uses barriers, not timing.
  The first fast gate reported `gate FAILED in 2 s: fmt`; `cargo fmt --all` corrected the fixture layout.
  The repeated fast gate exited 0: `partial gate passed (not done): 11 steps in 8 s`. The full gate is running.
  `cargo xtask loc --json --compare HEAD` reports 108269 production lines, delta 0. Complete baseline, regression
  and gate logs remain in `target/pipeline-speed-fix/`; the isolated baseline build was removed after its logs
  were preserved. Hardware was not accessed.

- 2026-11-17: Baseline `cargo xtask step doctor-base tests-release` exited 1:
  `gate FAILED in 282 s: doctor-base`. All baseline release crate tests passed on this invocation, including
  the five-mode soak. The old branch also passed one five-mode invocation, so that test is intermittent.
  The destructor-barrier regression fails deterministically with the old batch field order, proving buffer return
  precedes decoder cleanup. Move the lease last so the observer no longer races those captured allocations;
  the existing soak bound and trend checks stay unchanged. Aligned nextest 0.9.151 across the tool pin and
  both installing workflows; all 23 focused build-tool tests passed, including the CI/bootstrap pin contract.

- 2026-11-17: PR #148 repair started on the runner-provided branch. Cached `origin/main` is `04fc6ddc`;
  the isolated baseline reproduces `doctor-base` failure (expected nextest 0.9.150, installed 0.9.151).
  Baseline release tests are running. Fetch/reset and GitHub timing reads remain coordinator actions under
  the job's no-Git-writes and build-only-network rules.

- 2026-11-17: Final warm run: 321.27 s, exit 1, `gate FAILED in 321 s: doctor-base, tests-release`.
  Doctor requires nextest 0.9.150 but this host has 0.9.151. The unchanged release UI test
  `scripted_modes_reach_a_plateau` failed in pairs mode with an 816-byte transient increase against its
  752-byte block allowance. Debug tests, coverage, Python, clippy, decoders, register plans and perf passed.
  These failures and independent review remain open for the coordinator; no UI assertion was changed.
- 2026-11-17: Focused validation passed: 218 xtask tests in both debug and test-release, 14 change-filter tests,
  nine output/guard tests, shellcheck for 40 host scripts, and the final fast gate (11 checks).
  No ratchet count fell in changed files. Production lines remain 107590 (delta 0); tooling grows by 59 Rust
  and 15 shell lines. Host only, no unit access. The required CI names and their checks remain intact.
- 2026-11-17: Cold after run: 1007.50 s, exit 1, `gate FAILED in 1007 s: doctor-base, python-tests`.
  The Python failure was the old parent-target expectation, corrected using the existing output fixture.
  All Rust tests and perf passed; no production code changed after this run. Perf preparation overlapped the
  other checks (719.4 s); the final guards took 81.7 s. Cold wall time did not improve in this sample.
- 2026-11-17: Baseline cold: 919.90 s, exit 1, `gate FAILED in 920 s: doctor-base`.
  Baseline warm: 406.97 s, exit 1, `gate FAILED in 406 s: doctor-base, tests-debug, tests-release`.
  The warm baseline failed UI process-group isolation in both profiles and the release observer allocation census.
- 2026-11-17: Assigned round 2 on `ci/141-pipeline-speed`, base `72dc61f8`. CI measurements and cache hit rates
  await the coordinator: this worker may not use GitHub or gh. CLI and complete child-command logs are retained
  in `target/pipeline-speed-evidence/`; preserve that directory before retiring the worktree.

### Round 2 PR timing table (issue #141)

| Metric, seconds | Before | After |
|---|---:|---:|
| CI PR wall time (recent successful runs) | 340-500 | coordinator measurement needed |
| CI slowest job (run 44192358769) | 449 | coordinator measurement needed |
| Local gate, cold (unpaired, inconclusive) | 919.90 | 1007.50 |
| Local gate, warm (unpaired, inconclusive) | 406.97 | 321.27 |
| Fast gate through Cargo, cold | 10.18 | 37.00 |
| Fast gate through Cargo, initial warm | 3.02 | 13.89 |
| Fast gate through Cargo, final warm | unavailable | 9.70 |
| Paired warm fast executables, trial 1 | 18.52 | 11.86 |
| Paired warm fast executables, trial 2 | 20.10 | 6.25 |

Cold means a fresh workspace target apart from xtask and its dependencies built by the preceding fast run;
the toolchain and Cargo registry were already populated. Gate commands are `cargo xtask check`, not `gate`.
The paired trials ran the saved original and changed executable sequentially on identical current inputs,
excluding Cargo startup; every fast check passed. Host load averages ranged around 132-245 on 16 cores during
after measurements. The unpaired wall times do not establish a cold improvement, and the full-run failures
above limit the warm comparison. The original local runs shared the host with six concurrent jobs; there is no
matched-load cold pair, so both the cold and warm local comparisons are inconclusive. No CI wall-time improvement
or restore hit rate has been measured here. The coordinator supplied the baseline below; PR #148 after timings
remain an acceptance gap. This repair job's build-only network policy prohibits retrieving them or publishing
the changes. Do not infer CI speed from local gate durations.

### Coordinator CI baseline for the critical-path repair

Successful `pr.yml` run 44192358769 on `refactor/138-m15a-netio`; recent successful runs took 340-500 s overall.
Setup took about 12 s and cache restoration about 3 s. Target: PR wall time under 300 s.

| Job / step, seconds | Before | PR #148 after |
|---|---:|---:|
| test (debug 1/3), job | 199 | coordinator measurement needed |
| test (debug 2/3), job | 449 | coordinator measurement needed |
| Tests (debug 2/3), step | 421 | coordinator measurement needed |
| test (debug 3/3), job | 163 | coordinator measurement needed |
| clippy, job | 314 | coordinator measurement needed |
| Clippy ratchet, standalone decoders and register plan matrix, step | 292 | coordinator measurement needed |
| python, job | 70 | coordinator measurement needed |
| fast, job | 45 | coordinator measurement needed |
| deny, job | 14 | coordinator measurement needed |

After publication, record all four debug shards (4/4 is golden only), all three clippy matrix rows and the aggregate
`test` and `clippy` checks, along with the run id and PR wall time. Required check names remain unchanged.

- 2026-11-17: nextest dropped from this branch after two CI runs hung (#23). Codex finding on API read timeouts fixed.
- 2026-11-17: branch `ci/t0095-faster-pipeline` from main 9072ea2. Pre-release `kernel-base-6.12.115` published,
  asset sha256 matches `kernel/base.sha256`.

### Round 3 CI timing table (issue #141)

Baseline run 44196789417, supplied in the brief after PR #148. Target: PR wall time at most 220 s.
After values require the coordinator to publish this change and supply its own PR run's job JSON and test log.
This worker may not access GitHub, run `gh`, push or create a PR. No CI speed improvement is claimed yet.

| Metric, seconds | Before | Round 3 after |
|---|---:|---:|
| PR wall time | ~303 | coordinator measurement needed |
| clippy (decoders) | 276 | replaced by three groups |
| clippy (decoders 1/3) | n/a | coordinator measurement needed |
| clippy (decoders 2/3) | n/a | coordinator measurement needed |
| clippy (decoders 3/3) | n/a | coordinator measurement needed |
| clippy (lints) | 110 | coordinator measurement needed |
| clippy (plans) | 42 | coordinator measurement needed |
| test (debug 1/4), now 1/5 | 180 | coordinator measurement needed |
| test (debug 2/4), now 2/5 | 250 | coordinator measurement needed |
| test (debug 3/4), now 3/5 | 147 | coordinator measurement needed |
| test (debug 4/5), layout alone | n/a | coordinator measurement needed |
| test (debug 4/4), now 5/5 golden | 107 | coordinator measurement needed |
| fast | 56 | coordinator measurement needed |
| python | 45 | coordinator measurement needed |

Record the after run id, aggregate `clippy` and `test` conclusions, all group durations and full-run wall time here
before accepting the 220-second target. CI per-test durations should confirm or revise the local layout-test split.
