---
id: 60
title: Report updater undo failures and isolate toolchain probes
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Report failed kexec undo operations while continuing cleanup, resolve toolchain probes
from the repository pin, isolate fixture Git configuration, and clarify watchdog DT coverage.

## Plan

Reserved paths: `docs/tasks/0060-small-followups.md`, `fpgad/src/bin/oos-update.rs`,
`hal/src/update/kexec.rs`, `hal/src/update/kexec/legacy.rs`,
`hal/src/update/kexec/undo.rs`, `xtask/src/run.rs`, `xtask/src/run/tests.rs`,
`xtask/src/run/tests/probe_tests.rs`, `xtask/src/target.rs`, `xtask/src/testrepo.rs`,
`tools/tests/test_watchdog_dt.py`, `xtask/data/moves.txt`, `target/COMMIT_MSG.txt`.

The owner's assignment accepts this follow-up claim for the listed paths, superseding
the earlier task 0054/0056/0058 reservations for this work. Review tiers: tooling for
xtask and scripts; standard for fpgad, as assigned. The coordinator owns cross-vendor
review and batch integration. No commits or hardware access.

1. Add host-tested HAL diagnostic helpers for best-effort undo operations, including
   nonzero command status. Reuse legacy stderr/kernel logging and keep the updater's
   file size from growing.
2. Clear inherited `RUSTUP_TOOLCHAIN` only in probe children, retaining context/logs.
   Test pin selection and unchanged parent environment in isolated child processes.
   Disable global/system configuration in fixture Git calls.
3. Remove the impossible watchdog label reference and exercise the valid node forms.
4. Run focused debug/release tests, the exact xtask clippy step with its ratchets,
   and `cargo xtask check --fast`. Retain complete logs and prepare one section in
   `target/COMMIT_MSG.txt`; leave review/full gate to the coordinator.

## Status

- 2026-11-16: author handoff is ready on the uncommitted working tree based on
  `adb72ff`. Task stays active pending coordinator cross-vendor review (tooling for
  xtask/scripts, assigned standard tier for fpgad) and full batch gate/integration.
  `target/COMMIT_MSG.txt` contains one section. All source/task/move changes are
  intentional. Build caches and evidence are retained; disposable fixtures and
  the temporary clippy driver source/binary have been removed. No commits, pushes,
  full gate or hardware access; hardware behavior is not verified.
- 2026-11-16: `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. All selected steps
  pass: fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Clippy and focused debug/release checks passed
  separately. Doctor, full Rust suites, performance, coverage, script inventory
  and image checks remain not selected. Complete output/exit code:
  `target/t0060-evidence/fast-gate.log` and `.exit`; command logs:
  `target/check-logs/72266-1794801668149946000/`. `git diff --check` passes.
- 2026-11-16: the exact `steps::clippy` implementation passes, exit 0, including
  all six host crates, all five ARM crates and the unchanged clippy ratchets:
  hal 93 entries/177 total, dsp 66/151, core 3/46, ui 129/406, fpgad 7/20,
  xtask 0/0. Zero new findings. A temporary runner compiled the actual xtask
  modules and invoked the step with `run::start_logs`; it added no product CLI
  or tracked test hook. Reproduction script, complete step output/exit code:
  `target/t0060-evidence/clippy-step.py`, `clippy-step.log`, `clippy-step.exit`.
  Per-command logs: `target/check-logs/69605-1794801649644439000/`.
- 2026-11-16: release checks pass, exit 0: `cargo test --locked -p xtask --release`
  (123 tests), `cargo test --locked -p oos-hal --lib --release
  update::kexec::undo::tests::` (two tests), and `cargo test --locked -p oos-fpgad
  --release --bin oos-update` (four tests). Debug equivalents passed, as did
  `python3 -I tools/tests/test_watchdog_dt.py` (three tests). Complete logs and
  per-command exit codes are in `target/t0060-evidence/` with debug/release names.
- 2026-11-16: all four follow-ups implemented. The undo helper reuses the legacy
  stderr, kmsg and pmsg reporter; remount reports launch and nonzero-status failures.
  The updater file shrinks by four lines. Probe-only child commands remove the proxy
  override and retain context/logs. Fixture Git disables global/system configuration;
  fixture paths include test thread names to prevent parallel collisions. Watchdog
  tests cover valid node/path forms without the nonexistent label reference.
  Debug checks pass: 123 xtask tests, two HAL undo tests, four updater tests and
  three watchdog DT tests. Complete logs/exit codes: `target/t0060-evidence/`.
- 2026-11-16: the two new tooling regressions fail against the parent runner,
  target probe and fixture Git helper (exit 101): inherited invalid toolchain breaks
  the Rust probe; malformed host Git configuration breaks fixture init. Their final
  fixed versions pass in the full xtask debug suite. Failing-before evidence:
  `target/t0060-evidence/regressions-before.log` and `.exit`.
- 2026-11-16: codex claims the owner-assigned task on `t0060-small-followups`, based
  on `overhaul` at `adb72ff`. Initial worktree is clean; plan recorded before code.
  No hardware access or verification.
