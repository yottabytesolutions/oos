---
id: 58
title: Fix gate context review follow-ups
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Address the task 0056 tooling review follow-ups for repository toolchain resolution,
isolated Python execution, retained gate logs and independent test fixtures.

## Plan

Reserved paths: `xtask/src/run.rs`, `xtask/src/run/tests.rs`, `xtask/src/target.rs`,
`xtask/src/coverage.rs`, `xtask/src/steps/mod.rs`,
`docs/tasks/0058-gate-followups.md`, `target/COMMIT_MSG.txt`.

The owner's assignment accepts this follow-up claim for the listed task 0056 paths.
Review tier: tooling. The coordinator owns cross-vendor review and batch integration.

1. Pass the repository root into availability and installed-target probes and add
   `-I` to coverage's Python output selector.
2. Exercise a gate path with a fixture context and verify retained logs. Replace the
   real-repository helper test with a temporary Git/Cargo fixture. Make fixture cleanup
   report errors during unwinding without a second panic.
3. Run `cargo test --locked -p xtask` in debug and `cargo xtask check --fast`.
   Record results and prepare one section in `target/COMMIT_MSG.txt`. No full gate or commits.

## Status

- 2026-11-16: author handoff is ready for the coordinator's cross-vendor tooling
  review on the uncommitted working tree at the recorded base. Task stays active
  pending review and batch integration. `target/COMMIT_MSG.txt` contains one section.
  The five modified source files and this new task file are intentional; build caches
  and evidence remain. Temporary test fixtures are removed. No commits, publication,
  full gate or hardware access; hardware behavior is not verified.
- 2026-11-16: `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. All eight selected IDs
  pass: fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Doctor, clippy, Rust debug/release suites,
  performance, coverage, script/Python and image checks are not selected by the
  partial gate; debug xtask tests were run separately as recorded below.
  Complete gate output and exit code are in `target/t0058-evidence/fast-gate.log`
  and `.exit`; per-command logs are in
  `target/check-logs/61349-1794785023682358000/`. `git diff --check` passes.
- 2026-11-16: all five follow-ups implemented. Availability and installed-target probes
  receive the repository root through their gate callers; coverage selects output with
  isolated Python. The coverage-path fixture verifies isolated mode, child context and
  retained logs. Formatting, metadata and protected-base assertions now use a temporary
  Git/Cargo repository. Cleanup preserves the original panic when removal fails.
  `cargo test --locked -p xtask` passes all 121 debug tests, exit 0; complete output and
  exit code are retained in `target/t0058-evidence/xtask-debug.log` and `.exit`.
- 2026-11-16: the coverage-selector regression fails before the `-I` fix, exit 101,
  at its isolated-mode assertion, and passes after the fix. An initial probe test
  relying on malformed fixture toolchain configuration failed because Cargo inherits
  `RUSTUP_TOOLCHAIN`; it was replaced with a directory-forwarding check independent
  of installed tools and overrides. Final tests cover explicit probe directories and
  preservation of the original panic on cleanup failure.
- 2026-11-16: codex claims the owner-assigned tooling follow-up on
  `t0058-gate-followups`, based on `overhaul` at `8f47241`. Initial worktree is clean;
  plan and reserved paths recorded before implementation. No hardware access.
