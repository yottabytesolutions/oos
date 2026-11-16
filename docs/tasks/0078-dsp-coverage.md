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

## Status

- 2026-11-16: Final `cargo xtask check --fast` passed (exit 0), reporting
- 2026-11-16: Final full crate tests passed in debug and release (exit 0 each):
- 2026-11-16: Added 42 behavioral tests (39 passing, three ignored upstream
- 2026-11-16: Ignored regressions confirmed with
- 2026-11-16: Intermediate fast checks failed first on forbidden `#[path]`
- 2026-11-16: Codex author claimed the owner-assigned part on
- 2026-11-16: Part a is ready for cross-vendor review, uncommitted as requested. `cargo xtask check --fast` exited 0 and printed `partial gate passed (not done): 8 steps in 132 s`; all eight selected steps passed (fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, agent-rules). Full integration gate and part b remain coordinator work; task stays active.
- 2026-11-16: Final focused commands `cargo test --locked -p oos-dsp --test motor --test dpt` and the same with `--release` exited 0: motor 12 passed/1 ignored, double pulse 10 passed, each suite 0.00 s in both profiles. Gate-exact DSP clippy commands from `dsp/`: `cargo clippy --locked --all-targets --message-format=json --quiet` and the same with `--target aarch64-unknown-linux-musl`, both exit 0. Combined findings deduplicated exactly as `xtask/src/clippy.rs`: 262 existing findings, zero new against `xtask/data/dsp/clippy.txt`, no compiler errors.
- 2026-11-16: Final `cargo llvm-cov -p oos-dsp --summary-only --locked` exited 0: `motor.rs` 475/482 covered lines (98.55%, +175); `dpt.rs` 416/431 (96.52%, +69). Total assigned-file gain: 244 lines, exceeding the 200-line target. Crate 12,368/13,240 (93.41%); restoring the crate floor still needs part b. Coverage floor and ratchets unchanged. Before/after JSON and full logs, focused test logs, clippy outputs/comparison and the expected failing Hall reproduction are preserved in `target/task0078-part-a/`; the fast gate also retains `target/check-logs/93955-1794845377885935000/`. Handoff text is in `target/COMMIT_MSG.txt`.
- 2026-11-16: Added 22 active integration tests and one ignored Hall regression, with analytical Clarke/Park vectors, signed angular motion, encoder index/trends, Hall faults, mechanical power, double-pulse crossings, slopes, recovery charge/energy, dead time, polarity/time scaling, clipping and missing-data cases. Production files are unchanged. Focused debug tests pass; both suites report 0.00 s.
- 2026-11-16: Baseline `cargo llvm-cov -p oos-dsp --summary-only --locked` passed (exit 0): `motor.rs` 300/482 covered lines (62.24%, 182 uncovered), `dpt.rs` 347/431 (80.51%, 84 uncovered); crate 12,122/13,240 (91.56%). Initial after measurement passed (exit 0): motor 475/482 (98.55%), double pulse 416/431 (96.52%), 244 newly covered assigned-file lines.
- 2026-11-16: Owner assigned part a to Codex on `t0078-dsp-tests-a`, base `3e316143` (from `git log -1`); accepted scope is motor and double-pulse tests only. Baseline running. No hardware access or commits requested. Cross-vendor review and the integration full gate remain coordinator work.
- `dsp/src/motor.rs:363` and `dsp/src/motor.rs:378`: when the initial Hall code is invalid, `hall_step(prev, cur)` always returns `None` and `prev` never advances to a valid code. The decoder remains stuck for the entire record.
- Failing input: Hall codes `[0, 4, 6, 2, 3, 1, 5, 4]` with A/B/C as bits 2/1/0, `dt = 1.0 / 6.0`, `t0 = 0.0`. The six valid transitions after code 4 should yield 360 degrees of forward travel; actual angles all remain NaN and speed is absent.
- Regression: `hall_recovers_when_record_starts_with_invalid_code` in `dsp/tests/motor.rs`, marked `#[ignore = "bug: Hall decoding never recovers from an invalid initial code"]`. Explicit reproduction: `cargo test --locked -p oos-dsp --test motor hall_recovers_when_record_starts_with_invalid_code -- --ignored`, exit 101 with `NaN != 0`. No upstream fix included.
