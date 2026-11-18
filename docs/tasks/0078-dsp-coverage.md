---
id: 78
title: Restore upstream DSP analysis coverage
kind: chore
status: active
release: 0.2.0
owner: dsp-coverage-workers
depends: []
---

## Goal

Restore DSP coverage with independent behavioral tests without lowering the floor.

## Plan

### Part b

Reserved paths: `dsp/src/linecode.rs`, `dsp/src/linecode/`, `dsp/src/cphy.rs`,
`dsp/src/cphy/`, `dsp/src/power.rs`, `dsp/src/power/`; this part's task and
`target/COMMIT_MSG.txt` sections only.

1. Measure baseline with `cargo llvm-cov -p oos-dsp --summary-only`.
2. Test known line codes, C-PHY electrical states and analytical power signals,
   including invalid input and boundary behavior. Keep upstream bugs unchanged,
   with ignored regression tests and concrete failing inputs below.
3. Remeasure covered lines, run focused debug/release tests, the gate's exact DSP
   clippy command and `cargo xtask check --fast`. Hand off without commits.

### Review fixes

Reserved paths: `dsp/src/power.rs`, `dsp/src/linecode/behavior.rs`,
`dsp/src/cphy/behavior.rs`, `dsp/src/power/tests/edge.rs`, `dsp/tests/motor.rs`,
`dsp/tests/dpt.rs`, this task file and `target/COMMIT_MSG.txt`.

1. Measure current DSP coverage before edits and check upstream Conv documentation
   and UI labels. Preserve production behavior and record any additional bug.
2. Apply the module-layout, MLT-3 threshold and optional assertion review fixes.
   Keep active convolution boundary coverage separate from the ignored bug case.
3. Compare coverage, run debug/release DSP tests, the exact gate clippy step and
   the fast gate. Record evidence and one commit-message section. No commits.

## Status

- 2026-11-16 (task 0084): upstream fixed three of the recorded bugs (Hall start on an invalid code ad6d4b45, MLT-3
  67c9f7a9, a clock longer than the rail 79491fbd); `hall_recovers_when_record_starts_with_invalid_code`,
  `mlt3_decodes_a_three_level_all_ones_signal` and `longer_clock_cannot_index_past_the_rail` run again and pass.
  Convolution and D2A extrema stay ignored.
- 2026-11-16: Final `cargo xtask check --fast` passed, exit 0, exactly:
  `partial gate passed (not done): 8 steps in 198 s`. All eight selected steps
  passed: fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Size reported one existing DSP pending
  upstream rebaseline under the retired upstream-rebaseline procedure (Git history); no ratchet was edited. Complete output:
  `target/task0078-review/fast-gate.log`; command logs:
  `target/check-logs/60654-1794847160757186000/`.
  `git diff --check` passed. Seven tracked files intentionally remain modified;
  `target/COMMIT_MSG.txt` contains one consolidated message. No commits, pushes,
  merges or hardware access. Full integration gate and cross-vendor verification
  remain coordinator work; task stays active.
- 2026-11-16: Final DSP debug and release runs each passed, exit 0:
  315 passed, zero failed, seven ignored (five task bugs, two pre-existing).
  Commands: `cargo test --locked -p oos-dsp` and the same with `--release`.
  Evidence: `target/task0078-review/tests-{debug,release}-final.{log,exit}`.
- 2026-11-16: The exact `steps::clippy` implementation passed, exit 0,
  reporting `ok   clippy (14.6 s)`: all six host and five ARM runs plus
  source-location deduplication and protected-base ratchet comparisons, zero
  new/grown findings or compiler errors. DSP stays at 115 entries, 262 total.
  A scratch xtask copy restricted dispatch to clippy and redirected root to
  this checkout; the step and comparison implementations were unchanged.
  Evidence: `target/task0078-review/clippy-step-final.{log,exit}`, referenced
  complete command logs, `clippy-driver-source-hashes.json` and driver notes.
  The scratch source copy was removed after use; no gate sources were edited.
- 2026-11-16: Review fixes applied without changing production algorithms:
  restored the power module's upstream layout; MLT-3 uses threshold 0.5;
  convolution now has the contract-based ignored regression below. Optional
  fixes derive decimation count from n/1500 and excluded gaps, check clipped
  travel/time as 3 rpm, select trigger sample 5 away from the midpoint, test
  double-pulse zoom edge containment and exact 19.1..19.9 switch crossings.
  The C-PHY regions literal is removed; independent partition invariants check
  payload capacity, unique flip masks and exact coverage of all 16-bit words.
- 2026-11-16: Final `cargo llvm-cov -p oos-dsp --summary-only` passed, exit 0:
  12,830/13,508 lines (94.98%), at least the baseline 12,848/13,527 (94.98%)
  at full precision. Motor improves from 475/482 to 479/482 covered lines.
  C-PHY production coverage stays 251/252. Functions: 1,577/1,626 (96.99%);
  regions: 27,977/29,234 (95.70%). The added ignored Conv case and removal of
  the C-PHY literal change the test-source denominator. Floor and ratchets
  are unchanged. Evidence: `target/task0078-review/coverage-final.log`.
- 2026-11-16: Initial debug/release and after-coverage runs exited 101 because
  the first trigger edit still rounded to the midpoint. Moving to sample 5
  resolved that assertion. The first successful after-coverage run was 94.81%
  because removing the literal stopped exercising the C-PHY const constructor
  at runtime. Partition invariants and leading-gap/default motor expectations
  restore the line coverage requirement. Initial logs are retained alongside
  final logs in `target/task0078-review/`.
- 2026-11-16: Codex accepted the owner-assigned review fixes on `t0078-review`,
  base `c59b71d5` (from `git log -1`). Scope and reserved paths are above.
  Baseline coverage passed, exit 0: 12,848/13,527 lines (94.98%),
  27,987/29,242 regions (95.71%), 1,575/1,624 functions (96.98%).
  Evidence: `target/task0078-review/coverage-before.log`. No hardware access.

- 2026-11-16: Part a is ready for cross-vendor review, uncommitted as requested. `cargo xtask check --fast` exited 0 and printed `partial gate passed (not done): 8 steps in 132 s`; all eight selected steps passed (fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, agent-rules). Full integration gate and part b remain coordinator work; task stays active.
- 2026-11-16: Final focused commands `cargo test --locked -p oos-dsp --test motor --test dpt` and the same with `--release` exited 0: motor 12 passed/1 ignored, double pulse 10 passed, each suite 0.00 s in both profiles. Gate-exact DSP clippy commands from `dsp/`: `cargo clippy --locked --all-targets --message-format=json --quiet` and the same with `--target aarch64-unknown-linux-musl`, both exit 0. Combined findings deduplicated exactly as `xtask/src/clippy.rs`: 262 existing findings, zero new against `xtask/data/dsp/clippy.txt`, no compiler errors.
- 2026-11-16: Final `cargo llvm-cov -p oos-dsp --summary-only --locked` exited 0: `motor.rs` 475/482 covered lines (98.55%, +175); `dpt.rs` 416/431 (96.52%, +69). Total assigned-file gain: 244 lines, exceeding the 200-line target. Crate 12,368/13,240 (93.41%); restoring the crate floor still needs part b. Coverage floor and ratchets unchanged. Before/after JSON and full logs, focused test logs, clippy outputs/comparison and the expected failing Hall reproduction are preserved in `target/task0078-part-a/`; the fast gate also retains `target/check-logs/93955-1794845377885935000/`. Handoff text is in `target/COMMIT_MSG.txt`.
- 2026-11-16: Added 22 active integration tests and one ignored Hall regression, with analytical Clarke/Park vectors, signed angular motion, encoder index/trends, Hall faults, mechanical power, double-pulse crossings, slopes, recovery charge/energy, dead time, polarity/time scaling, clipping and missing-data cases. Production files are unchanged. Focused debug tests pass; both suites report 0.00 s.
- 2026-11-16: Baseline `cargo llvm-cov -p oos-dsp --summary-only --locked` passed (exit 0): `motor.rs` 300/482 covered lines (62.24%, 182 uncovered), `dpt.rs` 347/431 (80.51%, 84 uncovered); crate 12,122/13,240 (91.56%). Initial after measurement passed (exit 0): motor 475/482 (98.55%), double pulse 416/431 (96.52%), 244 newly covered assigned-file lines.
- 2026-11-16: Owner assigned part a to Codex on `t0078-dsp-tests-a`, base `3e316143` (from `git log -1`); accepted scope is motor and double-pulse tests only. Baseline running. No hardware access or commits requested. Cross-vendor review and the integration full gate remain coordinator work.

## Upstream bugs

- `dsp/src/motor.rs:363` and `dsp/src/motor.rs:378`: when the initial Hall code is invalid, `hall_step(prev, cur)` always returns `None` and `prev` never advances to a valid code. The decoder remains stuck for the entire record.
- Failing input: Hall codes `[0, 4, 6, 2, 3, 1, 5, 4]` with A/B/C as bits 2/1/0, `dt = 1.0 / 6.0`, `t0 = 0.0`. The six valid transitions after code 4 should yield 360 degrees of forward travel; actual angles all remain NaN and speed is absent.
- Regression: `hall_recovers_when_record_starts_with_invalid_code` in `dsp/tests/motor.rs`, marked `#[ignore = "bug: Hall decoding never recovers from an invalid initial code"]`. Explicit reproduction: `cargo test --locked -p oos-dsp --test motor hall_recovers_when_record_starts_with_invalid_code -- --ignored`, exit 101 with `NaN != 0`. No upstream fix included.

- Convolution: upstream `main` at `15b819e2` calls this convolution in
  `dsp/src/linecode.rs` and `docs/reference-analysis.md`; `ui/src/math.rs` labels
  it `Conv` and `Conv(A, B)`. Current `ui/src/math.rs` routes `MathOp::Conv` to
  the same DSP function. For `a = [1, 2, 3]`, `h = [1, 2, 4]`, centred
  convolution is `[4, 11, 14]`, but actual output is `[10, 17, 8]` because the
  kernel is not flipped. Regression:
  `linecode::behavior::convolution_centers_and_flips_the_kernel`, ignored with
  `bug: conv correlates (kernel not flipped)`. Active missing-sample and
  empty-input/kernel tests remain separate. Contract evidence:
  `target/task0078-review/upstream-conv-contract.txt`.
- MLT-3: a 40-one signal encoded as repeated `0, +1, 0, -1` levels, 16 samples
  per cell, with `dt = 1`, rate `1/16`, threshold `0.5`, hysteresis `0.1`,
  returns second-byte value `0x33` instead of `0xff`. Regression:
  `linecode::behavior::mlt3_decodes_a_three_level_all_ones_signal`. The threshold
  now matches the active MLT-3 test and leaves the repair strategy open.
- D2A: input `[2, 3, 4]` should slice to its actual extrema `[2, 4, 4]`, but
  yields `[4, 4, 4]` because the extrema fold starts with invented 0/1 rails.
  Regression: `linecode::behavior::d2a_uses_the_actual_trace_extrema`.
- Rail: an 8-sample 5 V rail and 64-sample clock alternating every four samples
  cause a panic at slice start 12 beyond length 8. A longer clock must not
  index past the rail. Regression:
  `power::tests::rail::longer_clock_cannot_index_past_the_rail`.
- 2026-11-16 reproduction: each of the five named ignored tests fails with
  exit 101 using `cargo test --locked -p oos-dsp <test-name> -- --ignored`.
  Commands, actual failures and exit map are preserved in
  `target/task0078-review/bug-*.log` and `bug-exits.json`.
