---
id: 35
title: "Count acquired roll frames"
kind: fix
status: done
release: 0.2.0
owner: codex
depends: []
---

# Count acquired roll frames

## Goal

Include roll captures in the acquired counter.

## Plan

Reserved paths: `ui/src/board/dho/hw.rs` (hot file), `ui/src/board/dho/mod.rs`,
`ui/src/board/dho/roll.rs`, `ui/src/ui.rs` (hot file), `ui/src/ui/tests/roll.rs`,
`docs/tasks/0035-roll-counter.md`, `COMMIT_MSG.txt`.

Forward-port plan: adapt the regression to the fixture Host and supplied Switches, extract roll control and
publication without changing behavior, format the UI, then run the full offline gate in this worktree.
Leave changes uncommitted for coordinator review and amendment.

Add a failing-before test for the roll branch in ui/src/board/dho/hw.rs. Move the counter update onto the roll
path that currently continues before the add. Coordinate with the existing branch instead of duplicating work.

## Status

- 2026-11-15: closed by task 0055 after confirming the forward-port `bb7dd9b` already
  increments `acquired` in `ui/src/board/dho/roll.rs::publish_roll`. Existing regression
  `ui::tests::roll::tests::roll_single_bounds_snapshots_holds_and_rearms` in
  `ui/src/ui/tests/roll.rs` checks 927 publications, no acquisitions while held, and two
  fresh acquisitions on resume/rearm. Focused debug execution on task 0055's unchanged roll
  source passes in debug and release: 1 passed each, exit 0. No additional roll edits are needed or made. Closure was
  explicitly assigned by the owner; task 0055 remains active for review and batch integration.

- 2026-11-15: final dirty-tree full gate on `fae3382`, with offline Cargo and the target directory
  inside this worktree, exited 1. Exact summary:
  `gate FAILED in 562 s: tests (debug), tests (release), perf guards, coverage`.
  Formatting, sizes, architecture, mutable statics, unsafe and clippy pass. Debug and release
  each report HAL 329 passed / 1 failed / 8 ignored, UI 509 / 19 / 29, xtask 39 / 1 / 0.
  Test failures are local socket permission denials and xtask's temporary-Git-commit failure;
  coverage stops at the same HAL WLAN socket denial. The deposit-worker timing guard fails
  at ratio 7.87 (limit 1.5); an isolated full perf-guard rerun exits 101 at ratio 1.89,
  with 6 UI guards passed and 1 failed. No deposit-worker code changed in this port.
  Full output and exit evidence are retained in the private handoff logs. Updated COMMIT_MSG.txt
  preserves the original why and records port adaptations and verification. No device used;
  unit verification of the source fix is reported by the owner. Changes remain uncommitted
  for coordinator review and amendment; status stays active because the full gate has not passed.

- 2026-11-15: initial full offline gate exited 1: `gate FAILED in 580 s: clippy, tests (debug),
  tests (release), coverage`. Fixed the missing Linux import and the test-only boundary marker.
  A subsequent run exposed a moved legacy `unnecessary_map_or` lint and was interrupted (exit 130);
  used equivalent `is_none_or` without changing the scan policy. Offline Linux-target clippy now
  exits 0 with no findings in the two new files. Final full gate restarted on the corrected source.

- 2026-11-15: the focused roll regression and board boundary check both pass (2 passed, exit 0).
  The legacy boundary scanner needs an explicit test-only block in the extracted regression file;
  supplied it without changing the scanner. The full suites also report permission-denied sockets
  and an xtask temporary-Git-commit failure in this sandbox. Final full gate pending.

- 2026-11-15: port repair prepared. Regression uses fixture Host and its Switches; roll helpers and
  scan publication are extracted, with the regression under `ui/src/ui/tests/roll.rs`. Size checks
  pass: `hw.rs` 2510 (limit 2511), `spawn` 902 (limit 903), `ui.rs` unchanged outside tests.
  Focused fixed regression: 1 passed, exit 0. Restoring pre-fix snapshot publication makes the
  ported regression fail at 1000000 queued bytes in 4 snapshots, exit 101. Full gate in progress.
  A first full-gate run caught a missing Linux-only Shared import during extraction; restored it
  before the final rerun. No ratchet files changed.

- 2026-11-15: active, codex. Owner handed off the forward-port repair on `fwd/roll-queue`,
  base `fae3382` (textual port of `96088af` onto `1d5fa1b`). The branch name is owner-assigned.
  No device access, network, commit or push; the coordinator retains amendment and review.

- 2026-11-14: branch `fix/roll-queue` is in progress, as reported by the owner. This task records that work;
  do not claim or modify the branch without a handoff. Matrix evidence: 2026-11-14.
