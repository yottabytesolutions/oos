---
id: 61
title: Count acquisition lifecycle operations
kind: feature
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Expose worker-side acquisition lifecycle counts in `:SYSTem:STATe?` so M2.3b/c
unit rounds can observe hardware restarts independently of log lines.

## Plan

Reserved paths: `docs/tasks/0061-acquisition-counters.md`,
`ui/src/board/counters.rs`, `ui/src/board/dho/lifecycle.rs`,
`ui/src/board/dho/lifecycle/tests.rs`, `ui/src/board/dho/mod.rs`,
`ui/src/board/dho/hw.rs`, `ui/src/board/dho/roll.rs`, `ui/src/board/dho/tests.rs`,
`ui/src/board/mod.rs`, `ui/src/drawpipe.rs`, `ui/src/ui/change.rs`,
`ui/src/ui/tests/epochs.rs`, `target/COMMIT_MSG.txt`.

The owner's assignment accepts this claim and assigns standard review. The M2
task also touches `hw.rs`; keep its edits to small operation-site substitutions.
The coordinator owns cross-vendor review, the full batch gate and integration.
No commits or hardware access.

1. Add eight monotonic atomic counters to the existing process metrics storage.
   Count calls to segmented start, early segment stop, roll/scan stream stop/start,
   batch prearm/disarm, full configure and SINGLE hardware arm. Failed attempts
   count too; reads, completed segment stops and reuse of an armed batch do not.
   Store a fixed 64 bytes, with no new static or hot-path allocation.
2. Share counted operation helpers between the worker and fake hardware tests.
   Preserve hardware operations and error propagation; expose space-separated
   `name=value` fields beside the existing stale counters through the board port.
3. Exercise exact event deltas, idle/reuse cases, failures and SCPI formatting
   with the existing fake bus/segmented engine. Keep size ratchets unchanged.
4. Run focused debug/release tests, the exact xtask clippy step and
   `cargo xtask check --fast`. Retain logs and write one commit-message section.

## Status

- 2026-11-16: author handoff is ready on the uncommitted working tree based on
  `9d8df2d`. Task stays active pending coordinator cross-vendor standard review,
  full batch gate and integration. `target/COMMIT_MSG.txt` contains one section.
  All remaining source/task changes are intentional; build caches and evidence
  are retained. Temporary clippy runner source/binary are removed. No commits,
  pushes or hardware access. Hardware behavior remains unverified.
- 2026-11-16: focused release checks pass, exit 0:
  `cargo test --locked -p oos-ui --bin oos-ui --release board::` (67 passed,
  two existing manual reports ignored), and the same command with
  `ui::tests::epochs::` (12 passed). Complete logs and exit codes:
  `target/t0061-evidence/release-board.*` and `release-epochs.*`.
  `git diff --check` passes. The full gate, performance and coverage checks are
  left to the coordinator's integration batch as required by delivery.
- 2026-11-16: final `cargo xtask check --fast` exits 0 and reports
  `partial gate passed (not done): 8 steps in 22 s`. All selected steps pass:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. No ratchet edits. Complete output and exit:
  `target/t0061-evidence/fast-gate.log` and `.exit`; per-command logs:
  `target/check-logs/74073-1794806502474453000/`. The initial architecture failure
  is retained in `fast-gate-initial.log` and `.exit` in the evidence directory.
- 2026-11-16: the exact `steps::clippy` implementation passes, exit 0, including
  all six host crates, five ARM crates and unchanged ratchets: hal 93 entries/177
  total, dsp 66/151, core 3/46, ui 129/406, fpgad 7/20, xtask 0/0. No new findings.
  Reproduction script/output/exit: `target/t0061-evidence/clippy-step.py`,
  `clippy-step.log`, `clippy-step.exit`; per-command logs:
  `target/check-logs/63737-1794806446547215000/`. The temporary runner uses the
  actual xtask modules and is removed after execution; no product command added.
- 2026-11-16: final focused debug tests pass, exit 0:
  `cargo test --locked -p oos-ui --bin oos-ui board::` (67 passed, two existing
  manual reports ignored), and the same command with `ui::tests::epochs::`
  (12 passed). Exact eight-counter deltas cover real fake-bus operations,
  repeated STOP/rearm polls, hardware settings versus identity-only boundaries,
  SINGLE reuse, errors and lifetime saturation. SCPI exercises the public query
  and verifies adjacent `name=value` fields. Logs and exit codes:
  `target/t0061-evidence/debug-board.*` and `debug-epochs.*`.
- 2026-11-16: implemented counted operation helpers and SCPI fields, with small
  substitutions at the existing `hw.rs` sites. Focused debug DHO tests pass
  (58 passed, one existing manual report ignored). The first fast gate reports
  FAIL architecture: a DHO-specific storage type escaped through the board
  re-export. Moved that storage type to `ui/src/board/counters.rs` inside the port;
  worker operations remain in DHO. The same run passes size with unchanged
  ratchets. Final checks rerun after this correction are recorded above.
- 2026-11-16: codex claims the owner-assigned task on `t0061-acq-counters`, based
  on `overhaul` at `9d8df2d`. Initial worktree is clean. Plan recorded before code;
  `hw.rs` overlap acknowledged by the assignment. Hardware remains unverified.

- 2026-11-16 review (no P1/P2). Reading notes for unit rounds: in scan mode each new sweep counts `roll_start`, but its
  end (`roll::scan_done`) is not a counted stop, so `roll_stop` and `roll_start` diverge by design. A SINGLE with a force
  request whose prearm fails counts `single_arm` twice (failed attempt plus the real arm), per the failed-attempts rule.
  Review P3s applied: saturating `AtomicU64::update` instead of a discarded `try_update` Result.
