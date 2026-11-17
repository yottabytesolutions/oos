---
id: 34
title: "Validate hostile inputs and unsafe paths"
kind: feature
status: active
release: 0.3.0
owner: codex
depends: []
---

# Validate hostile inputs and unsafe paths

## Goal

Find parser and memory errors that line coverage misses.

## Plan

Reserved paths for the assigned parser-fuzzing slice: `fuzz/`, `Cargo.toml`,
`ui/src/main.rs`, `ui/src/store.rs`,
`ui/src/http.rs`, `ui/src/scpi.rs` (constructor only), `ui/src/scpi/message.rs`,
`xtask/data/ui/size.txt`, `.github/workflows/main.yml`, `docs/development.md`, this task.

1. Add an excluded cargo-fuzz crate and minimal parser hooks with small seeds from host test fixtures.
2. Exercise every target for 60 seconds; fix reproducible parser crashes with regression tests.
3. Add the bounded nightly/changed-fuzz PR job, document usage, run fast and focused checks,
   and prepare the commit message for the runner.
4. Preserve all targets' failing fuzz inputs as CI artifacts, including after a failed run.

Add bounded fuzz targets for SCPI, HTTP, setup, update, package and archive parsing with synthetic corpora.
Run supported host unsafe paths under Miri and GPU C tests under sanitizers. Use a named required or scheduled tier
and retain reproducible failures. Audit item: 24.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-17: PR 80 review fix preserves `fuzz/artifacts/` after every fuzz job run,
  including failures; clean runs without artifacts are ignored. Actionlint and the focused
  workflow assertion passed; the assertion failed before the fix. `cargo xtask check --fast`:
  `partial gate passed (not done): 10 steps in 18 s`, exit 0. The 26 improved DSP size entries
  are unrelated to this fix and were left unchanged. No crates changed; production delta: 0.
  Host only; CI upload verification and integration remain with the coordinator.

- 2026-11-17: completed the eight-target parser harness, 13 small fixture seeds, pinned nightly,
  scheduled/changed-fuzz PR job and development instructions. All targets passed their 60-second
  AddressSanitizer runs (61 seconds observed each): SCPI 32,307 inputs; ISF 152,891; WFM 170,532;
  AG 169,779; setup 37,472; update 937,315; HTTP 157,964; mDNS 169,260. No crashes, timeouts or
  round-trip failures found; no parser behaviour fixes needed. Generated corpora stayed in scratch.
  Host debug tests for `oos-ui` and `oos-hal`: 1,930 passed, zero failed. `cargo xtask check --fast`:
  `partial gate passed (not done): 9 steps in 24 s`, exit 0. `cargo xtask ratchet` tightened only
  the changed SCPI size entry; unrelated DSP improvements were left alone. Production delta:
  +3 lines (`cargo xtask loc --compare HEAD`). `cargo xtask step clippy`: exit 0,
  `partial gate passed (not done): 1 steps in 4 s`. Fuzz formatting, workflow actionlint and
  `git diff --check` passed. Combined release run exited 101: unchanged HAL test
  `update::tests::busy_follows_the_lock` failed its final unlocked assertion (375 passed,
  one failed, eight ignored). An isolated diagnostic run passed; the combined failure remains open.
  Coordinator next action: investigate that lock-test failure outside this parser-fuzzing slice.
  `cargo test --locked --release -p oos-ui` passed, exit 0. Complete validation logs are
  retained in `target/fuzz-validation/`. Independent review remains pending.
  No hardware access; the coordinator owns integration and the full gate.

- 2026-11-17: owner-assigned parser-fuzzing slice on `test/72-fuzz-parsers`, base `a3d731c4`.
  No publication, GitHub access or unit access. The remaining Miri/archive audit scope stays pending.

- 2026-11-14: seeded from task 0001. Not yet claimed.
