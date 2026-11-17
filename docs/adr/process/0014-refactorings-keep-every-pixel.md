# ADR 0014: A refactoring keeps every pixel and every register write

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

A refactoring must show that its screen, hardware writes and computed values stay the same. Host comparisons catch
regressions before slow unit rounds. This record also incorporates architecture/0011.

## Decision

- **Screen:** golden-picture tests in the gate are the screen proof. They compose pictures from the Demo source and
  fake board across display modes and time bases, comparing with committed references in `ui/tests/golden/` using
  a per-pixel tolerance and a budget of differing pixels. A changed reference needs a reason in its commit.
  `gpu/test-host.sh` compares CPU and GPU output.
- **Layout:** retain `ui/tools/layout-check.sh record` before a change and `check` after it until the goldens
  provably cover its scripts and all three text sizes. Then remove the manual check. Its temporary baseline is
  local (`target/layout-base`); scripts use fixed inputs and clocks, an empty storage directory and named menus.
- **Analog path:** register sequences must stay unchanged unless the change intends otherwise. The host
  comparison runs in the `plan-matrix` Rust gate step against `xtask/data/plan-matrix.txt`, with standard defaults
  and synthetic calibration. An intended change updates that baseline with `cargo xtask plan-matrix --update`
  and explains the reviewed diff in the pull request. `tools/plan-matrix.sh` remains for calibrated comparisons
  by the device tools.
- **Values:** compare moved computations bit for bit over the same inputs.

## Consequences

Intentional screen or register changes explain and review their baseline changes. Golden references need care when
fonts or LUTs change. Host equality does not prove timing on the unit; `tools/device-test.py` checks frame rate and
load on a scope with its owner's authorization.

## References

[local-validation.md](../../local-validation.md), [CONTRIBUTING.md](../../../CONTRIBUTING.md).
