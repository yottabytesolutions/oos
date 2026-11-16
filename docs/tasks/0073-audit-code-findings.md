---
id: 73
title: Fix verified documentation audit code findings
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Verify the code findings from task 0072's four audit reports and fix confirmed host-testable defects.
Review tier: host, as assigned. No commits, publication or hardware access.

## Plan

Reserved paths: `docs/tasks/0073-audit-code-findings.md`, `hal/src/dho/selfcal.rs`,
`hal/src/dho/selfcal/`, `hal/src/dho/adc.rs`, `hal/src/dho/adc/`, `hal/src/dho/calfile.rs`,
`hal/src/dho/boot/cal.rs`, `hal/src/dho/coretrim.rs`, `hal/src/dho/boot/io.rs`, `hal/src/dho/boot/regs.rs`,
`hal/src/dho/roll.rs`, `hal/src/dho/hz.rs`, `hal/src/dho/auxout.rs`, `ui/src/trig.rs`,
`ui/src/ui/selfcal.rs`, `ui/src/ui/selfcal/`, `ui/src/ui/autoset.rs`,
`ui/src/board/dho/hw.rs`, `fpgad/src/main.rs`, `fpgad/src/boot_guard.rs`, `target/COMMIT_MSG.txt`.

1. Check each finding against source and task 0072's verified reports. Record accepted and rejected findings.
2. Add failing-before host regressions for vertical payload growth, ADC location fallback, I2C data direction,
   calibration save staging, model channel selection and the BOOT module guard. Extract small helpers where
   required to stay within existing size ratchets.
3. Fix confirmed defects without changing calibration layouts. Stage the full calibration set before publication,
   retaining existing backup and fsync behavior. Correct the listed text and annotate both unit-check holds only.
4. Run focused debug tests, the gate's exact clippy step, and `cargo xtask check --fast`. Preserve logs under
   `target/`, record exit codes, and prepare one commit-message section ending in `Task: 0073`.
5. Review round: unify ADC acceptance after read-only backup CRC checks; retain a live path during atomic
   replacement, exhaust rollback and report durability/cleanup errors. Reject malformed vertical groups and
   exercise the actual two-channel UI adjustment/start path. Record the unit holds and I2C follow-up.
6. Round 3: preserve a typed published-but-directory-fsync-failed outcome through self-cal save; reload live
   tables and report a warning for that outcome. Link the current file to a temporary backup, sync the directory,
   then replace `.bak`. Add failing-before host regressions and rerun focused tests, exact clippy and fast gate.
   Additionally reserve `hal/src/dho/calfile/` for extracted tests; the existing selfcal directory claim covers
   `hal/src/dho/selfcal/files.rs`.

## Status

- 2026-11-16: Round 3 ready for verification, intentionally uncommitted. Focused debug and release suites
  all exited 0 in both profiles: calfile 11 passed / 1 private test ignored, HAL selfcal 10 passed, UI self-cal
  completion 1 passed. Commands: `cargo test --locked -p oos-hal --lib dho::calfile::`, same for
  `dho::selfcal::`, and `cargo test --locked -p oos-ui --lib audit0073_selfcal_reloads`; repeat with `--release`.
  Logs: `target/0073-round3-{debug,release}-{hal-calfile,hal-selfcal,ui-selfcal}.log`.
  Exact gate clippy passed all registered host/ARM crates and lint ratchets, exit 0:
  `python3 target/0073-round3-run-clippy.py`, output `target/0073-round3-clippy.log`, command logs
  `target/check-logs/62076-1794832897427725000/`. Temporary xtask harness restored byte-for-byte; no xtask
  or ratchet diff. Fast gate exited 0 and printed `partial gate passed (not done): 8 steps in 23 s`;
  all eight selected steps passed, full-only steps not selected. Output `target/0073-round3-fast.log`,
  command logs `target/check-logs/65231-1794832911269934000/`. `git diff --check` passed.
  Updated `target/COMMIT_MSG.txt` as one section ending in `Task: 0073`; removed temporary source snapshots,
  preserving test/gate evidence and build caches. Remaining intentional source/docs changes belong to 0073.
  No commits, publication or unit access. Full integration gate and cross-vendor verification remain
  coordinator work; task stays active.

- 2026-11-16: First round 3 exact clippy run exited 101: the Linux-only fpgad self-cal caller still expected
  a String error from `selfcal::save`. Add the required display conversion at that CLI boundary, preserving
  the typed result through the UI reload decision. Existing claim already reserves `fpgad/src/main.rs`.
  Output: `target/0073-round3-clippy-first.log`; command logs:
  `target/check-logs/51718-1794832850221776000/`. Temporary xtask harness restored byte-for-byte.

- 2026-11-16: Round 3 regressions failed before behavior fixes with neutral typed/error and callback seams:
  HAL 8 passed / 2 failed / 1 private test ignored, UI 1 failed; both exit 101.
  Logs: `target/0073-round3-before-{hal,ui}.log`. The failures cover the distinct post-publication sync outcome,
  preserving the previous backup on a hard-link failure, and reloading published results at the host-testable
  self-cal completion seam. Tests extracted into `hal/src/dho/calfile/tests.rs` to preserve file limits.
  A save failure still skips reload; a published sync failure now reloads AFE/ADC tables, reapplies the trim
  overlay, resets trim tracking and reports a warning. Backups now link to `.bak.new`, fsync the directory,
  then rename over `.bak`; failed backup replacement cleans the temporary link.

- 2026-11-16: Round 3 accepted on the existing `t0073-audit-code` claim. Limit changes to the three verification
  findings and required host seams/tests. Preserve earlier uncommitted task work. No commits or unit access.

- 2026-11-16: Review round ready for verification, intentionally uncommitted. Final fast gate exited 0 and
  printed `partial gate passed (not done): 8 steps in 23 s`; all eight selected steps passed, full-only steps
  not selected. Output: `target/0073-review-fast-final.log`; command logs:
  `target/check-logs/66681-1794832343621754000/`. Exact gate clippy passed for all registered host/ARM crates
  and lint ratchets, exit 0: `python3 target/0073-run-clippy.py`, `target/0073-review-clippy.log`, commands in
  `target/check-logs/48740-1794832310726863000/`. Temporary xtask harness restored byte-for-byte; no xtask or
  ratchet diff. `git diff --check` passed. Updated `target/COMMIT_MSG.txt` as one section ending in `Task: 0073`.
  Removed the two synthetic failing-before scratch directories, preserving build caches and evidence.
  All remaining source/docs changes belong to this task. No commits, publication, backup writes or unit access.
  Full integration gate and cross-vendor verification of this round remain coordinator work; task stays active.

- 2026-11-16: Focused debug and release suites all exited 0: ADC 5 passed, cold boot 35 passed / 2 private tests
  ignored, calfile 8 passed / 1 private test ignored, selfcal 10 passed, selfcal UI 1 passed, fpgad binary 10
  passed, in each profile. Commands: `cargo test --locked -p oos-hal --lib <filter>` for `dho::adc::`,
  `dho::boot::`, `dho::calfile::`, `dho::selfcal::`; `cargo test --locked -p oos-ui --lib ui::selfcal::`;
  `cargo test --locked -p oos-fpgad --bin oos-fpgad`; repeat with `--release`.
  Logs: `target/0073-review-{debug,release}-{hal-adc,hal-boot,hal-calfile,hal-selfcal,ui-selfcal}.log` and
  `target/0073-review-{debug,release}-fpgad-final.log`. The fpgad debug integration suite also passed 25/25,
  `target/0073-review-debug-fpgad.log`. Real calibration backups were checked read-only, never used as fixtures.


- 2026-11-16: First review-round fast gate exited 1 and printed `gate FAILED in 24 s: size`:
  `fpgad: fn fpgad/src/main.rs selfcal grew: 229 -> 230 (ratchet)`. All seven other selected steps passed.
  The propagated Result made rustfmt wrap the CLI call onto another code line; use captured error formatting
  on that same call to fit the existing limit without changing semantics or ratchets. Full output:
  `target/0073-review-fast.log`; command logs `target/check-logs/35258-1794832246241225000/`.


- 2026-11-16: First review-round exact clippy run exited 101: the ARM pass found the Linux-only fpgad vertical
  save caller still passing the new Result as a payload. Propagated that error before any save. This caller was
  already reserved. Complete output: `target/0073-review-clippy-first.log`; underlying command logs:
  `target/check-logs/7620-1794832109282505000/`. The temporary xtask harness was restored byte-for-byte.

- 2026-11-16: Review regressions failed before fixes: HAL 5 failures / 4 passes, UI 1 failure, both exit 101.
  Neutral filesystem/model injection seams and the vertical Result signature retained the old behavior for that
  run. Logs: `target/0073-review-before-{hal,ui}.log`. After fixes HAL 9/9 and UI 1/1 pass, both exit 0:
  `target/0073-review-after-{hal,ui}.log`. Atomic replacement now links the current file to `.bak` while the
  live path remains readable; rollback attempts all earlier entries and combines failures. Directory fsync
  errors propagate, and unexpected temporary cleanup errors are logged. Rollback restores current good files,
  not the previous `.bak` generation. Vertical bases require whole nonempty groups. UI regression adjusts and
  starts the actual UI with DHO802 (2) and DHO804 (4) channel counts, including every individual channel.

- 2026-11-16: Unit holds retained without changing either algorithm; stable code citations use task 0073 by
  number, as other source cites task 0045, rather than a path that changes on completion.
  Core-offset hold: the audit reports about 0.6 LSB per register step versus the code's assumed 16 codes per
  step, and no settle wait between offset writes and measurement. Neither slope nor required settle time is
  verified in this task. Measure both on the unit before changing the divisor or adding a wait.
  Scan-threshold hold: `hal/src/dho/roll.rs` enables 0x4058 at a `tdiv/10` threshold; the reported stock rule
  uses `tdiv/100`. Threshold value and direction still need comparison on the unit, not verified here.
  Follow-up owned by the coordinator for routing: `ui/src/trig.rs` 10-bit I2C reads treat the first data byte
  after `Sr + 11110xxR` as the low address byte. Real 10-bit reads send no second address byte there.
  This review round deliberately records that framing defect without changing its code; add a repeated-start
  10-bit read regression when taking the follow-up.

- 2026-11-16: Review round accepted on the existing claim; additionally reserve `hal/src/dho/boot/cal.rs`.
  Read-only backup validation (paths relative to the private backup root, no contents copied):

  | File | Header CRC | Payload CRC | Strict ADC acceptance |
  |---|---|---|---|
  | `20261108/cal_adc.hex` | PASS | PASS | PASS |
  | `20261113/rigol_data/cal_adc.hex` | PASS | PASS | PASS |
  | `20261113-fallback/rigol_data/cal_adc.hex` | PASS | PASS | PASS |
  | `20260804/rigol_data/cal_adc.hex` | PASS | PASS | PASS |
  | `20260804/pre-upgrade/rigol_data/cal_adc.hex` | PASS | PASS | PASS |

  Use the strict header + payload CRC rule for ADC in both loaders. No unit access or commits authorized.

- 2026-11-16: Final `cargo xtask check --fast` exited 0 and printed
  `partial gate passed (not done): 8 steps in 23 s`. All eight selected steps passed; full-only checks
  remain not selected as requested. Output: `target/0073-fast-final.log`; command logs:
  `target/check-logs/75268-1794831519372094000/`. `git diff --check` passed. Prepared one section in
  `target/COMMIT_MSG.txt` with `Task: 0073`. Removed the two scratch directories left by failing-before
  tests; retained build caches and evidence. Only task-related source/docs remain intentionally uncommitted.

- 2026-11-16: Implementation ready for independent host review, with intentional uncommitted source and task
  changes. All reported findings accepted, none rejected. Legacy vertical payloads grow to the existing
  16-block layout, preserving untouched fallback channels; runtime ADC load checks size per location;
  software I2C Data applies Direction for Any and 1..5 bytes. Self-cal saves resolve every base and fsync all
  temporary containers before renames, keep `.bak` files, and restore earlier replacements on rename failure.
  Separate file renames still cannot guarantee set atomicity across power loss. All/individual channel choices
  follow the model; BOOT checks both modules and preserves the documented `--force` bypass.
  Corrected all nine requested text sites (including both PLL messages and the unconfirmed upper 0x105c bits).
  Core-offset slope/settle behavior and scan 0x4058 threshold are unchanged, annotated with this task's path.
  No hardware access or validation, no commits. Cross-vendor review and the full integration gate remain
  coordinator work; task stays active for that handoff.
- 2026-11-16: Exact clippy runner passed, exit 0, for every registered host/ARM crate and all lint ratchets:
  `python3 target/0073-run-clippy.py`, commands retained under
  `target/check-logs/18024-1794831463916818000/`. Temporary harness restored byte-for-byte, no xtask diff.
  ADC test iteration lint fixed without ratchet changes. Final HAL regressions passed 4/4, exit 0:
  `cargo test --locked -p oos-hal --lib audit0073`, `target/0073-after-hal-final.log`.

- 2026-11-16: Focused debug suites passed: ADC 5, selfcal 9, calfile 5 (one explicit private-fixture test
  ignored), serial matcher 4, selfcal UI 1, fpgad 10. Commands and output are in
  `target/0073-focused-{hal-adc,hal-selfcal,hal-calfile,ui-serial,ui-selfcal,fpgad}.log`; all exit 0.
  Fast rerun printed `partial gate passed (not done): 8 steps in 23 s`, exit 0;
  log `target/0073-fast-final.log`. No ratchet files changed.
  The exact `xtask::steps::clippy` runner (host and ARM for all registered crates, including ratchet comparison)
  was invoked through a temporary test harness, then the harness was removed. First run exited 101 for one
  new `needless_range_loop` in the ADC regression. Changed it to enumerate the paths; successful rerun recorded above.
  Output: `target/0073-clippy.log`, underlying command logs in its printed check-logs directory.

- 2026-11-16: All six behavioral findings confirmed; no finding rejected. New regressions failed before
  fixes: HAL 3/3 (slice panic, bad-size fallback, mixed save), UI 2/2 (direction, two-channel All),
  fpgad 1/1 (PCIe host alone). Logs: `target/0073-before-{hal,ui,fpgad}.log`, each exit 101.
  Neutral extraction supplied explicit filesystem/model/module inputs for deterministic host tests.
  After fixes the six regressions and an additional late-rename rollback test pass (HAL 4, UI 2, fpgad 1).
  The first fast gate exited 1: fmt, architecture and agent-rules passed; size, mutable-statics, unsafe,
  method-exceptions and known-failures rejected the test module's explicit path attribute. Removed it in
  favor of the default module path; successful rerun recorded above. Complete output: `target/0073-fast.log`.

- 2026-11-16: Owner's direct assignment accepts this new claim on `t0073-audit-code`, base `71f50eb8`
  from overhaul. Read AGENTS, delivery, review, integration, task format, development and architecture guidance.
  Task 0072 reports inspected from `t0072-doc-audit`. Existing active 0067/0071 source reservations do not overlap;
  commit-message output is worktree-local. Coordinator retains independent cross-vendor review and integration.
