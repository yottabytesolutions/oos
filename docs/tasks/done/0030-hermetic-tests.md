---
id: 30
title: "Make mandatory tests hermetic"
kind: fix
status: done
release: 0.2.0
owner: codex
depends: []
---

# Make mandatory tests hermetic

## Goal

Ensure passing mandatory tests exercise the same behavior without private calibration files.

## Plan

Reserved paths: `hal/src/dho/`, `hal/src/update/tests.rs`, `hal/src/vendorkeys.rs`,
`hal/testdata/`, test configuration and optional-tool checks in `ui/src/` (including hot file `ui/src/ui.rs`),
`ui/src/board/dho/hw.rs` (hot file: test configuration defaults),
`tools/generate-test-cal.py`, `docs/tasks/0030-hermetic-tests.md`,
`docs/tasks/notes/hermetic-tests.md`, `docs/tasks/notes/hermetic-tests-results.json`,
`COMMIT_MSG.txt`, `.gitattributes`, `ui/src/ui/host.rs`, `tools/perf-guard.sh`,
`ui/src/scpi.rs`, `dsp/tests/analysis.rs`, and manual opt-in checks across `ui/src/` and `hal/tests/`.

1. Audit test bodies and their production entry points across hal, dsp, ui and fpgad.
2. Generate fake calibration containers and independent model expectations; replace hidden skips.
3. Isolate private and external-tool validation with explicit ignores; normalize test clock/configuration.
4. Run locked offline debug and release suites, preserve counts and identify sandbox socket failures.

With the migration, use synthetic calibration fixtures. Keep private-calibration validation in an explicit suite.
Normalize configuration, clock and storage inputs. Use locked dependency resolution and report skipped scenarios
and zero-test selections. Audit item: 21.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: done. merged into overhaul with the 2026-11-15 batch (integ-2 44a82d9, full gate passed).

- 2026-11-15: integrated on integ-2. The comparator mapping under test (`hw::mask_map_cell`) stays one
  process-wide cell: `Map::default()` until `Dho::spawn` sets it from `BoardSwitches::mask_map`; tests that change
  it hold `mask_map_test_lock`. Its owner for state shared by the UI and the acquisition thread is migration M5
  (typed channels replace `Shared`).

- 2026-11-15: review corrections prepared, owner codex. All eight final offline debug/release
  runs completed. HAL: 340 passed, 1 socket failure, 10 ignored per profile. UI: 500 passed,
  19 socket failures, 29 ignored per profile. DSP: 168 passed, 1 ignored. Fpgad: 12 passed.
  Cargo exits are 101 for HAL/UI and 0 for DSP/fpgad. Per-target summaries and the exact
  socket failure list are in [the report](../notes/hermetic-tests.md). Production UI check,
  fixture reproducibility, 18 explicit-input checks and the 13 manual PNG outputs pass.
  Strict missing-input checks fail as intended. Changes and COMMIT_MSG.txt remain at
  the worktree root for the coordinator. No commit or push. Status stays active because
  integration review and the unavailable xtask gate remain with the coordinator.

- 2026-11-15: restored four private assertions with strict OOS_CAL_DIR opt-in, removed all
  production cfg!(test) branches, and supplied UI clock and host inputs explicitly. Removed
  the two tautological tests. Generated the large vertical fixture in memory. Restored the
  13 manual pictures and added reasons and required inputs to bare ignores. Generator
  reproducibility, explicit missing-input failures and the production UI check pass.

- 2026-11-15: active, codex. Owner assigned review corrections on `t0030-hermetic-tests`,
  base `aae6c0e`. Plan: restore private assertions, inject host inputs at the UI boundary,
  generate the large fixture in tests, restore manual pictures, require explicit diagnostic
  opt-ins, then rerun all eight offline suites. No network, device, commit or push.

- 2026-11-14: done, owner codex. Synthetic calibration and mandatory assertions implemented; all eight
  offline, locked debug/release runs completed. DSP and fpgad pass. HAL has one sandbox Unix-socket failure
  and UI has 19 sandbox socket failures in each profile, unchanged for that restriction. Exact counts,
  ignored/zero-test outcomes, audit inventory and deviations are in
  [the report](../notes/hermetic-tests.md) and [structured results](../notes/hermetic-tests-results.json).
  Generator reproducibility and hostile-environment checks pass. The private opt-in fails visibly when
  unconfigured. Per owner instruction, changes remain uncommitted, with `COMMIT_MSG.txt` at the root;
  the coordinator retains integration/review and task archival. The xtask gate is absent from this snapshot.

- 2026-11-14: converted calibration-dependent assertions to generated fixtures and made external-tool
  and private-file suites explicitly ignored. Audited indirect configuration, wall clock and system-info reads.

- 2026-11-14: codex accepted the owner-assigned task on `t0030-hermetic-tests`, base `0a4ba5a`.
  Work is confined to this worktree. The owner requests no Git metadata writes, commits, pushes or device access.

- 2026-11-14: seeded from task 0001. Not yet claimed.
