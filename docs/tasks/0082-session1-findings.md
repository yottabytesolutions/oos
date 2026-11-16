---
id: 82
title: Diagnose session 1 acquisition findings
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Resolve round 5 session 1 findings c7, S8, 1.8 mask-source stale drops and
SIGTERM worker deadlines. Give each a code cause, fix or justified existing
behavior, and host regression evidence. Review tier: unit.

## Plan

Reserved paths: `docs/tasks/0082-session1-findings.md`,
`ui/src/board/dho/tests.rs`,
`ui/src/ui/tests/epochs.rs`, `ui/src/ui/tools/scpi_helpers_tests.rs`,
`ui/src/mdns.rs`, `ui/src/mdns/lifecycle.rs`, `docs/scpi.md`,
`target/COMMIT_MSG.txt`. Extend this list before editing further required files.

1. Trace code against the owner's round-log evidence. Distinguish intended
   identity rejection and bounded-queue loss from defects.
2. Add failing-before regression tests and fix clear causes. Preserve exact
   identity acceptance, bounded memory, captured metadata and shutdown budgets.
   Avoid `ui/src/ui.rs`, which another lane holds.
3. Run focused debug and release tests, the gate's exact clippy step with no
   new findings, and `cargo xtask check --fast`. Retain complete logs and exit
   codes under `target/t0082-evidence/`.
4. Record per-item file/line causes, outcomes and limits here; prepare an
   imperative commit message in `target/COMMIT_MSG.txt`. No commits, pushes,
   installation or hardware access. Coordinator owns independent cross-vendor
   unit review, full batch gate and integration.

## Status

- 2026-11-16: author handoff ready on the uncommitted working tree based on
  `15efe011`; task remains active for coordinator-arranged cross-vendor unit
  review and the full integration gate. `ui/src/ui.rs` is untouched. No
  ratchet, dependency, hook or credential changes; no commits, pushes or unit
  access. Hardware behavior, multicast/HTTP teardown, full gate, coverage,
  performance and image checks are not verified here. `target/COMMIT_MSG.txt`
  has one plain imperative subject (52 characters), a blank line and a why
  body. `git diff --check` passes. Final Rust hashes are retained in
  `target/t0082-evidence/source-sha256.json`.
- 2026-11-16: final focused debug and release suites all pass, exit 0.
  Commands use `cargo test --locked -p oos-ui --bin oos-ui FILTER` and the
  same command with `--release`, for each filter: `board::` (79 passed,
  two existing manual reports ignored), `ui::tests::epochs::` (13 passed),
  `ui::tools::scpi_helpers_tests::` (9 passed), `mdns::lifecycle::tests::`
  (1 passed), `system::shutdown::tests::` (5 passed). Thus 107 passes and two
  ignores per profile. Exact runner, complete outputs and individual exits:
  `target/t0082-evidence/run-focused.py`, `debug-{board,epochs,scpi,shutdown,shutdown-policy}.{log,exit}`
  and `release-{board,epochs,scpi,shutdown,shutdown-policy}.{log,exit}` in the
  same directory. All four items have direct regressions, with the existing
  shutdown budget/buffer-ownership suite also retained.
- 2026-11-16: shutdown regression negative control restores the original
  network-only poll behavior by replacing the wake descriptor with poll's
  ignored descriptor -1. The test fails as expected, exit 101 at 0.50 s,
  printing `shutdown deadline: 1 workers still own their buffers` twice.
  Script, complete output and exit: `target/t0082-evidence/shutdown-negative-control.py`,
  `shutdown-negative-control.log`, `shutdown-negative-control.exit`. The script
  restores the exact production source bytes in a finally block. This is a
  targeted behavior mutation, not a claimed test build of the parent revision.
  The first test attempt needed a UDP socket and failed under sandbox bind
  restrictions; `debug-shutdown-initial.*` retains that failure. The final
  regression uses the production poll helper and Unix sockets only, without
  multicast or TCP listeners.
- 2026-11-16: exact `steps::clippy` passes, exit 0, with unchanged ratchets
  across all six host crates and five ARM crates. Counts as printed: hal
  93 entries/177 total, dsp 115/262, core 3/46, ui 249/734, fpgad 7/20,
  xtask 0/0. No new findings. Reproduction runner and complete output:
  `target/t0082-evidence/clippy-step.py`, `clippy-step.log`,
  `clippy-step.exit`; command logs: `target/check-logs/88446-1794850822007018000/`.
  The runner compiles unchanged xtask modules and calls `steps::clippy` directly;
  no product gate command or ratchet edits. A runner-only dependency selection
  failure is retained in `clippy-runner-feature-failure.log`; the final runner
  selects exact Cargo JSON artifacts/features instead of filesystem timestamps.
- 2026-11-16: `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 33 s`. fmt, size, architecture,
  mutable-statics, unsafe, method-exceptions, known-failures and agent-rules
  all pass. Complete output/exit: `target/t0082-evidence/fast-gate.log` and
  `.exit`; command logs: `target/check-logs/71493-1794850752251702000/`.
  The initial run failed size because `mdns::run` grew 94 -> 97 lines. Moving
  its existing log flush into the lifecycle module fixes the finding without
  changing limits; initial output/exit retained in `fast-gate-initial.*`.
- 2026-11-16: d, fixed in host code. `ui/src/mdns.rs:1629` computes a wait of
  up to one second. Before the fix, that poll watched only network sockets, so
  `Ctl::Stop` could not interrupt it. `ui/src/system/shutdown.rs:21` supplies
  only 500 ms to remote teardown; `ui/src/system/device/remote.rs:128` waits
  for mDNS before asking HTTP to stop on that same deadline. This permits one
  expired mDNS join followed by an immediately expired HTTP join, explaining
  two generic warnings without an acquisition DMA fault. The log lacks worker
  names, so attribution of those exact two messages remains an inference.
  `ui/src/mdns/lifecycle.rs:133` now signals a nonblocking Unix datagram wakeup;
  `ui/src/mdns.rs:1362` polls it alongside network sockets. Pending controls
  drain before waiting again. No shutdown budget changes, unsafe-block count
  increase or new dependency. The test at `ui/src/mdns/lifecycle.rs:99`
  exercises the production poll with idle network inputs, real control/stop,
  and a simulated subsequent HTTP worker on the shared 500 ms deadline.
  Actual multicast/HTTP sockets and SIGTERM on hardware remain unverified.
- 2026-11-16: c, correct as is. `core/src/change/analysis3.rs:10` classifies
  mask source as ACQ/STATS/MASK, per the accepted D9 contract in architecture.
  A source change advances gen once without changing epoch. Producer rejection
  at `ui/src/board/dho/publication.rs:64` counts old in-flight frames;
  `ui/src/ui/change/acquisition.rs:57` removes queued old-generation frames.
  Tests at `ui/src/board/dho/tests.rs:172` and `ui/src/ui/tests/epochs.rs:286`
  pin 80 worker drops and 96 queued drops respectively, no overflow, unchanged
  epoch, and acceptance of current records without further drops on unchanged
  steps. About 170 drops is queue/in-flight occupancy, not a required exact
  count. The unit plan's zero-stale expectation is wrong for a live source
  boundary; enable/reference publication alone is a different operation.
- 2026-11-16: b, correct bounded best-effort behavior, with real record loss.
  `ui/src/ui/autoset.rs:191` selects Peak; `ui/src/ui.rs:3426` permits the fast
  route only for Normal. The record route sets take-all at `ui/src/ui.rs:2905`
  and consumes at most 48 records per step at line 2912. A hardware batch can
  hold up to 1000/ratio frames (`ui/src/board/dho/hw.rs:1327`); the worker then
  decodes all available frames when take is zero (line 2423). Publication
  bounds the queue to 256 frames or 32 Mi samples, always keeping the newest
  (`ui/src/board/dho/publication.rs:36`), and counts discarded oldest current
  records as overflow. Configuration count itself does not cause overflow;
  old identities are counted separately. Test at `ui/src/board/dho/tests.rs:139`
  demonstrates a 720-frame current batch causing 464 overflows even without
  a settings boundary, with every retained record and the newest capture
  still readable. Lost captures can remove transient/persistence contributions,
  per-record statistics and history entries. This is not lossless acquisition.
  Autoset uses the latest eligible stage record, and terminal clear discards
  intermediate persistence/statistics; overflow does not discard that newest
  capture or substitute an old-stage record. Unit timing and the precise
  observed +391..463 total have not been reproduced on hardware.
- 2026-11-16: a, correct as is under the documented SCPI fallback contract,
  with a deep-read limitation. `ui/src/board/dho/hw.rs:1315` reads large Normal
  records as compressed min/max pairs when a small raw screen window is not
  possible. `ui/src/ui.rs:2737` deliberately excludes such envelopes from native
  transfer and falls back to the 3600-point record. Selecting RAW only selects
  the retained backing data (`ui/src/scpi.rs:4262`), not a new FPGA read. STOP
  raw rereads require sample-consuming deep measurements (`ui/src/ui.rs:2655`,
  `ui/src/board/dho/hw.rs:1446`), available ring replay and the existing byte
  budget; they can retain only a window. `docs/scpi.md` already specified the
  native-or-processed fallback and now explains this case explicitly. Test at
  `ui/src/ui/tools/scpi_helpers_tests.rs:63` checks stopped NORM/RAW/MAX point
  counts, preamble times and page contents with deep envelopes and retained
  native samples. It also proves native transfer follows actual retained data,
  not the nominal 10M acquisition metadata. Full-depth acquisition via RAW
  selection is not implemented; unit c7 alignment remains inconclusive.
- 2026-11-16: code diagnosis: c7 follows the documented retained-native-or-screen
  fallback (`docs/scpi.md`); Normal deep compression alone does not retain native
  samples. S8 Peak uses the bounded full-record route (48 records per UI step),
  not the Normal-only fast path. Mask source is explicitly ACQ/STATS/MASK (D9),
  so its old-generation drops are required. mDNS waits on network sockets for
  up to one second, but remote teardown gives it only 500 ms and stops HTTP
  afterward with the same deadline. Add a control wakeup and regressions.
- 2026-11-16: codex accepts the owner-assigned new task on
  `t0082-session1-findings`, clean base `15efe011`. Read AGENTS, delivery,
  architecture/area contracts, review/integration rules and the specified
  round-log items. Plan recorded before implementation. Hardware unverified.
