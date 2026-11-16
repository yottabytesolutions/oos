---
id: 77
title: Recover calibration saves after a stale backup link
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Remove a stale calibration backup temporary before linking so a save can recover after an interrupted backup.
Distinguish a published core-offset overlay from an unsuccessful save, and clarify the hardware mask source wording.
Review tier: host, as assigned. No commits or hardware access.

## Plan

Reserved paths: `docs/tasks/0077-calfile-stale-backup-link.md`, `hal/src/dho/calfile.rs`,
`hal/src/dho/calfile/tests.rs`, `ui/src/board/dho/hw.rs`, `hal/src/dho/auxout.rs`, `target/COMMIT_MSG.txt`.

1. Add a host regression that pre-creates the backup hard link and checks successful publication, backup contents
   and temporary cleanup. Run it against the unchanged implementation and retain the failing result.
2. Remove the stale backup temporary before linking. Correct overlay save diagnostics for the published-directory-
   sync outcome with a host regression, and use the assigned auxiliary-output wording.
3. Run focused debug and release tests, the gate's exact clippy step with no new findings, and
   `cargo xtask check --fast`. Retain complete logs and exit codes in `target/`.
4. Prepare one commit-message section in `target/COMMIT_MSG.txt` and hand off the uncommitted change for
   cross-vendor review. The coordinator owns the full integration gate and task completion.

## Status

- 2026-11-16: Ready for cross-vendor host review, intentionally uncommitted. Exact clippy on untouched HEAD
  `11a8b2b9` also exited 1 on the same baseline ratchet failures. Comparison of all eleven retained host/ARM
  command outputs, using the gate's per-member filtering and location deduplication, found identical counts:
  471 lint keys / 1243 findings before and after, zero new or increased keys. Reproduce with
  `python3 target/0077-run-clippy.py` and `python3 target/0077-compare-clippy.py`; complete baseline log:
  `target/0077-baseline-clippy.log`, comparison: `target/0077-clippy-comparison.log`, baseline command logs:
  `target/0077-baseline/target/check-logs/68601-1794844671691091000/`. No guardrail changes or baseline repairs.
  `target/COMMIT_MSG.txt` contains one section ending in `Task: 0077`. Changed-file and commit-message privacy
  checks found zero findings; the optional whole-tree audit exited 1 with 142 findings outside changed paths.
  Privacy evidence: `target/0077-privacy.log`. Full integration gate and independent review remain outstanding.
- 2026-11-16: Focused debug and release suites all exited 0 in both profiles: HAL calfile 12 passed / 1 private
  calibration test ignored, HAL selfcal 10 passed, UI overlay diagnostic 1 passed, UI self-cal reload 1 passed.
  Commands: `cargo test --locked -p oos-hal --lib dho::calfile::`, same for `dho::selfcal::`, and
  `cargo test --locked -p oos-ui --lib overlay_save_error_distinguishes_published_sync_failure`, same for
  `audit0073_selfcal_reloads`; repeat with `--release`. Complete logs:
  `target/0077-{debug,release}-{hal-calfile,hal-selfcal,ui-overlay,ui-selfcal}.log`.
  Fast gate exited 0 and printed `partial gate passed (not done): 8 steps in 79 s`; all eight selected steps
  passed, with existing upstream pending-rebaseline entries reported. Complete log: `target/0077-fast-gate.log`;
  command logs: `target/check-logs/28437-1794844579299266000/`. Full gate and independent review remain for
  integration; no hardware used or verified.
- 2026-11-16: Exact `steps::clippy` host/ARM run completed but exited 1 on existing ratchet findings across DSP,
  core and UI. Complete log: `target/0077-clippy.log`; command logs:
  `target/check-logs/86136-1794844534046857000/`. Running the same step against an untouched HEAD snapshot to
  distinguish task findings from the baseline. The ignored harness calls unchanged gate modules, and no xtask
  source or ratchet was edited. Initial harness setup refused the unpruned copied lockfile with exit 101;
  only the scratch lockfile was reduced to its locked dependency closure before the actual step.
- 2026-11-16: Reproduced the stale-link failure on the unchanged production code: `cargo test --locked -p oos-hal
  --lib stale_backup_link_does_not_block_save` exited 101, one failed test, `AlreadyExists` / OS error 17.
  Complete log: `target/0077-red-stale-backup.log`. Added stale temporary cleanup, typed overlay save diagnostics
  with a host regression, and the assigned mask-source wording. Focused verification is next.
- 2026-11-16: Claimed by Codex under the direct task assignment on branch `t0077-calfile-stale-link`, base
  `11a8b2b9`. Clean checkout confirmed before work. This narrow follow-up supersedes task 0073's historical
  reservations for the assigned paths. Plan recorded before code; hardware not used or verified.
