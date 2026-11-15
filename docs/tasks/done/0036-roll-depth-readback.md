---
id: 36
title: "Decide memory depth readback in roll"
kind: chore
status: done
release: 0.2.0
owner: codex-c
depends: []
---

# Decide memory depth readback in roll

## Goal

Decide whether :ACQuire:MDEPth? in roll reports the roll record or the menu depth.

## Plan

Reserved paths: `docs/tasks/0036-roll-depth-readback.md`, `docs/protocols/roll-mode.md`,
`ui/src/scpi/vernier_tests/depth_tests.rs`, `ui/src/scpi.rs` (test module declaration only), `target/COMMIT_MSG.txt`.

1. Compare the recorded matrix summary, roll protocol evidence, existing readbacks and remote consumers.
2. Document the compatibility contract in the roll protocol. Retain existing behavior and pin it with hermetic
   Host query tests for roll, normal acquisition, initial fallback and web MCP JSON. No drawing change.
3. Run focused debug/release tests and available static checks. Leave full gate, cross-vendor review and unit
   acceptance to the coordinator. Do not commit or access hardware.

### Decision

Keep `:ACQuire:MDEPth?` as the latest acquired record's points per channel, shared with the top bar and System
information. In roll this is the currently populated rolling buffer, including a partial buffer; normal acquisition
uses the frame's acquisition depth, even when its display data is compressed. With no usable hardware frame, keep
the model estimate. The Acquire menu remains the requested setting (Auto or fixed depth, with an existing cap
annotation), not a query of the rolling buffer. Web MCP JSON wraps the same SCPI response; there is no separate
web depth field in this checkout. See [the contract and evidence limits](../protocols/roll-mode.md#10-depth-readback-compatibility).

Reason: the matrix summary recorded when this task was seeded says the roll record was reported. The existing
normal-query decision in `docs/fork/zoomout.md` deliberately aligns SCPI with actual acquisition metadata and the
top bar. Reporting the menu setting in roll would instead advertise unavailable points. Remote tools already
consume numeric rate/depth answers; their coverage does not establish stock roll parity. No raw roll matrix or
stock roll query capture, or programming-guide wording for this query, was found in this checkout. Do not treat
the matrix summary as proof of stock Rigol behavior. Hardware behavior beyond the recorded evidence is unit acceptance.

## Status

- 2026-11-15: final-layout verification complete, uncommitted for coordinator review. With `CARGO_NET_OFFLINE=true`,
  `cargo test --locked -p oos-ui depth_tests -- --test-threads=1` and the same command with `--release` each exit 0:
  5 passed, 0 failed, 0 ignored. `cargo clippy --locked -p oos-ui --all-targets` exits 0 with existing warnings and
  no findings in the new test file. Final fast-gate result remains `partial gate passed (not done): 5 steps in 19 s`.
  Logs and the proposed commit message are in `target/depth-{debug,release,clippy,fast-gate}.log` and
  `target/COMMIT_MSG.txt`. No production behavior changed, no hardware used, no commit made. Status stays active:
  coordinator must run the full gate, obtain independent Claude review and handle unit acceptance before completion.
- 2026-11-15: final fast gate exits 0: `partial gate passed (not done): 5 steps in 19 s`.
  Formatting, sizes, architecture, mutable statics and unsafe pass. `git diff --check` passes.
  Final-layout debug tests: 5 passed, 0 failed, exit 0. Release passed 5/5 before test-module relocation;
  the final-layout release rerun is in progress. Shared-file edit is exactly two test-only lines inside
  `scpi.rs::vernier_tests`; no production code, HTTP code, held paths or ratchet files changed.
- 2026-11-15: decision documented in roll protocol section 10. Added five hermetic Host tests: partial/full roll
  and STOP, compressed normal metadata and STOP, roll-to-normal retained-frame semantics, initial/empty-frame
  fallback, and web MCP JSON. No listener, device or OS service is opened. Focused debug run: 5 passed, 0 failed,
  exit 0. These pin retained behavior, so a failing-before regression is not applicable.
  Initial test run was 3 passed / 2 failed due to exact floating-point comparison in the fixture; corrected the
  assertion tolerance. Initial fast gate: `gate FAILED in 12 s: size limits` because the test cfg attribute counted
  as one production line. Wiring moved inside the existing test-only module without growing production sizes.
  Next fast gate: `gate FAILED in 15 s: fmt`; fixed import ordering in the new test file. Final fast gate and
  release verification pending. Host clippy exits 0 with existing warnings and no findings in the new test file.
- 2026-11-15: evidence audit found only the seeded roll matrix summary, not raw matrix results or stock SCPI roll
  captures. The stock reconstruction distinguishes memory settings from the stream but gives no roll depth-query
  response. No depth-query programming-guide wording found in `docs/`. Exact stock parity and further hardware
  behavior are listed as unit acceptance, not inferred. Coordinator retains full gate and cross-vendor review.
- 2026-11-15: claimed by codex on owner-assigned branch `t0036-roll-depth`, base `5a94e7f`.
  Owner supplied this task and overlap constraints. Plan and compatibility decision recorded before tests.
  Existing behavior implements the selected contract; no production behavior or golden pictures need changing.
  Only planned shared-file hunk is the test module declaration in `ui/src/scpi.rs`; no HTTP edits.
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0036-roll-depth by the coordinator, base overhaul.
- 2026-11-15: done. Merged via integ-4. Stock Rigol parity remains unit acceptance (see docs/protocols/roll-mode.md section 10).
