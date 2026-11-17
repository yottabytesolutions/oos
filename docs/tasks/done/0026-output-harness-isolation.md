---
id: 26
title: "Isolate build outputs and host harness runs"
kind: fix
status: done
release: 0.2.0
owner: codex-b
depends: []
---

# Isolate build outputs and host harness runs

## Goal

Prevent concurrent builds and tests from consuming or deleting each other's resources.

## Plan

Review round 1: preserve startup causes and child stderr, hash only the coordinator's
agreed build inputs without HEAD, test readiness publication and invalid build values,
recheck signing inputs after packaging, narrow harness error handling and keep Mesa's
cache outside Cargo's cleanable target. Add offline regressions, update OUTPUTS.md and
draft target/COMMIT_MSG.txt. Remain uncommitted; coordinator runs sockets and full gate.

Reserved paths: tools/, boot/*.sh target-dir handling, ui/tests/ harness helpers except M1's scripted-batch files (ui/src/ui/script.rs, ui/src/ui/batch.rs, ui/tests/golden*); xtask/src/coverage.rs only (G1 owns the rest of xtask/); gpu/build.sh; gpu/test-host.sh; ui/src/net6.rs; ui/src/http.rs; ui/src/scpi.rs; ui/src/vxi11.rs; ui/src/main.rs only minimal bind/port/readiness plumbing (shared with M1).

Continuation reservations: boot/build-gel.py; fpgad/tests/explicit_signing.rs;
.github/workflows/image.yml (image artifact paths must follow the selected output directory);
ui/build.sh; tools/perf-guard.sh; README.md; docs/update.md; docs/install-usb.md;
gpu/README.md; ui/README.md; boot/card-install.py and tools/image-manifest.py usage examples;
hal/src/update/inflate.rs image-test example. These are existing consumers or instructions
for the relocated outputs. New regression tests: tools/tests/test_image_outputs.py.
Do not edit tools/privacy/, .github/workflows/privacy.yml, docs/network.md or docs/fork/
(task 0024), or G1/M1's other held paths. Keep simulator changes shared with task 0024 minimal.

Before parallel migration, define one target-dir contract for Cargo, coverage, GPU, image and deploy tools.
Verify artifact revisions before consumption. Bind host harnesses to loopback with dynamic ports and per-run
temporary directories. Include VXI-11. Replace sleeps with a readiness handshake identifying the child and build.
Audit items: 10, 11.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: final round-1 sandbox checks all exited 0. Python unittest discovery:
  test_output_harness.py 8 passed, test_image_outputs.py 6 passed, test*lease*.py
  46 passed. Cargo tests used --offline --locked and this worktree's target:
  oos-ui net6::tests::host_ with the socket-dependent host_listeners test explicitly
  skipped, 2 passed in each of debug/release (559 filtered); oos-fpgad all targets,
  28 passed in each profile, including all 16 signing tests; xtask coverage::,
  2 passed in each profile (39 filtered). Workspace all-target Clippy for host and
  aarch64-unknown-linux-musl exited 0, with zero deduplicated finding increases
  against xtask/data/clippy.txt. Python/shell syntax and git diff --check passed.
  Final fast gate exited 0, exact closing line:
  `partial gate passed (not done): 5 steps in 18 s`.
  Full gate, measured coverage and tools/tests/test_host_sockets.py remain delegated
  to the coordinator outside the sandbox under the standing handoff. Actual Git
  commits, Docker/Mesa compilation, GPU/image/VM runs and hardware were not exercised.
  Task remains active for renewed Claude review and coordinator validation.
- 2026-11-15: Claude review round 1, fix-first response, no P1. P2-1: startup errors
  now retain the VXI-11/readiness cause, print it and exit 1. The launcher copies the
  child's bounded stderr tail into HostError before deleting its run directory.
  P2-2: HEAD is absent from receipts; OUTPUTS.md defines the exact path/mode/content
  input set and exclusions, including docs, Markdown, workflow files, draft messages.
  The tracked fonts stay inputs (embedded by include_bytes!). Missing tracked paths are absent inputs so committing a
  deletion preserves the hash too. Ignored prerequisites, features and RUSTFLAGS
  remain explicitly outside receipt provenance. Offline fixtures verify task edits,
  simulated HEAD/index changes and source edits; no temporary Git commit was made.
  P2-3: Rust tests call host_ready in separate child environments, verify the published
  record and reject non-hex and empty builds. A separate child test verifies startup
  cause text and exit 1. No environment mutation, sockets or extra dependencies.
  All P3 fixes applied: empty identity rejected; parent-echo/identity verification
  documented; signing re-verifies image and build id after pack; missing-receipt
  diagnostic requests rebuilding and signing assertions use it; dead serve argument
  removed; SCPI logs its address value; coverage has no test-only fallback; device-test
  catches LeaseError/HostError and obtains reports through Host.results_dir(). Mesa
  cache and tarball stay in rootfs/out outside the selected target. A fixture runs
  actual Cargo clean on its own target and verifies cache reuse. Draft message moved
  to target/COMMIT_MSG.txt. Work remains uncommitted and active for renewed review
  and coordinator validation. No push, merge, device access or hardware verification.
- 2026-11-15: review regression evidence: before the fixes, output/readiness tests
  exited 1 with two assertion failures (excluded inputs changed revision; startup
  lost stderr) and one missing-receipt error. First review fast gate exited 1:
  `gate FAILED in 17 s: size limits`; main.rs grew 1043 -> 1048 and main grew
  95 -> 99. Moved host startup failure handling to net6, retaining the minimal main
  plumbing and preserving all ratchets. Corrected fast gate exited 0:
  `partial gate passed (not done): 5 steps in 12 s`. Final checks recorded separately.
- 2026-11-15: implementation prepared for coordinator validation on t0026-harness-isolation,
  still uncommitted, base 1d5fa1b (`git log -1`). GEL now consumes the selected image and
  ARM updater paths, verifies both receipts, publishes atomically and stamps the result.
  Signing verifies the image and saved build-id receipts and stamps the signed package;
  GEL simulation verifies its input too. Image CI uses the same paths, fetches locked
  dependencies before offline builds and keeps its manifest inside the selected output.
  The three signing fixture failures are fixed with real output helpers and an isolated,
  fixed-revision Git stand-in. Fixtures select a target path containing spaces.
  Existing UI build/performance helpers now resolve the same target before changing
  directory; UI script shots and storage use a unique run directory. Updated existing
  build/install instructions and usage examples. All continuation files are listed in
  Plan; COMMIT_MSG.txt lists all 39 deliverable files and the proposed change message.
  Task remains active pending independent Claude review and the coordinator's full gate,
  coverage and socket validation outside this sandbox. Docker/GPU/image/VM runs and
  hardware remain unverified. No commit, push, merge, lease or device access. No held
  G1/M1/task-0024 files were edited; simulator hunks shared with task 0024 stay minimal.
- 2026-11-15: final continuation checks: `cargo test --locked -p oos-fpgad` and the
  same command with `--release` both exited 0: 27 passed, 0 failed, 0 ignored, 0 filtered
  across the package's targets. `python3 -I -m unittest discover -s tools/tests -p
  test_output_harness.py` passed 5; `test_image_outputs.py` passed 5; `test*lease*.py`
  passed 46, all exit 0. Image tests cover default/relative/absolute targets, legacy-path
  decoys, missing receipts, replaced inputs, UI build receipts and performance-helper
  target resolution. Running the GEL regressions against the original builder produced
  four expected assertion failures and no setup errors; the updated builder passes.
  Host and ARM all-target workspace Clippy both exited 0; deduplicated findings have
  zero increases against xtask/data/clippy.txt. Formatting, changed shell/Python syntax
  and `git diff --check` passed. Final fast gate exited 0, exact closing line:
  `partial gate passed (not done): 5 steps in 8 s`.
- 2026-11-15: performance-helper validation with relative `CARGO_TARGET_DIR=target`:
  the first `tools/perf-guard.sh` run exited 101, failing
  `drawpipe::tests::perf_guard_deposit_worker_per_frame` at ratio 25.42 (UI: 6 passed,
  1 failed), while release compilation overlapped the run. After builds finished, a
  second complete run exited 0 (UI: 7 passed), exact closing line:
  `perf guards: all passed`. Both results are retained in target/t0026-perf.log and
  target/t0026-perf-retry.log. The timing failure's cause is not proven; drawpipe code
  and limits were not changed. The full gate/coverage was not repeated, following the
  existing sandbox handoff. Its earlier failed steps and exit code remain recorded below.
- 2026-11-15: resumed with the coordinator's accepted expansion to boot/build-gel.py and
  fpgad/tests/explicit_signing.rs. Standing authorization covers further strictly needed,
  unheld files listed here. Added .github/workflows/image.yml for its legacy image output
  consumers. Reproduced the three signing fixture failures: targeted debug test exited
  101, 1 passed and 3 failed because the fixtures lack tools/output.sh. No commit or device access.
- 2026-11-15: implemented the expanded claim, then stopped at the next reservation boundary.
  Remaining required files: boot/build-gel.py (hardcoded boot/out and target paths, no
  path options) and fpgad/tests/explicit_signing.rs (copies only the signing shell script
  into a fixture and expects boot/out). Neither file was edited. Reservation requests
  are pending. GEL packaging cannot yet consume the new image output; three existing
  signing-script tests fail because their fixture lacks tools/output.sh. Both fpgad
  debug and release exited 101: 23 passed, 3 failed across its targets. Task is not done.
- 2026-11-15: output contract is documented in tools/OUTPUTS.md and implemented by
  tools/output.py and output.sh. Cargo receipts bind HEAD, current source contents and
  artifact hashes; consumers reject missing or stale receipts. Coverage uses the shared
  target resolver. GPU bundles, ARM hardware helpers and image/signing paths follow the
  selected worktree-local target directory. Deploy keeps its lease check before other
  transport. Kernel/rootfs prerequisite-cache provenance remains a follow-up outside
  this claim; those builders were not changed.
  Host HTTP/MCP, SCPI and VXI-11 use loopback and port 0; GETPORT advertises the selected
  core port. The launcher uses private storage and an atomic readiness record with PID,
  artifact hash and all ports, and terminates/reaps the child before cleanup. Simulators
  use unique temporary directories. main.rs changes only its five service-plumbing lines.
- 2026-11-15: sandbox verification: fmt and shell/Python syntax passed. Fast gate exit 0,
  exact closing line: `partial gate passed (not done): 5 steps in 19 s`.
  Strict workspace Clippy with `-D warnings` exited 101 on existing HAL/DSP findings;
  strict xtask Clippy passed. Normal host and ARM all-target Clippy both exited 0, with
  no finding increases against xtask/data/clippy.txt (deduplicated across targets).
  Five new offline output/readiness regression tests and all 46 lease tests passed.
  Debug and release each passed 1074 tests in DSP/HAL/UI/xtask, with 40 existing ignores
  and 23 explicitly filtered tests. The filters are 22 socket-dependent tests (including
  the two new host-listener tests) and xtask's temporary-commit test. The new Python
  two-host socket/MCP/VXI-11 test was not run because socket binds are blocked.
  Full gate and coverage are deferred to the coordinator outside the sandbox, per the
  continuation instruction. Docker/GPU/image/VM runs and hardware were not exercised.
  No ratchets, golden files or M1 script/batch files changed. No commit, review or merge.
- 2026-11-15: continuation time log: implementation and checks approximately 09:03-09:19
  UTC, plus initial inspection not separately timed. Debug/release and static checks
  overlapped implementation and each other. COMMIT_MSG.txt contains the proposed change
  message and file list; no Git commit was attempted.

- 2026-11-15: coordinator accepted the expanded reservation above and authorized resuming
  implementation on t0026-harness-isolation. Sockets and temporary Git commits are blocked
  in this sandbox; coordinator will run the full gate outside it. No commit or device access.

- 2026-11-15: offline full gate exited 1, with the exact closing line:
  `gate FAILED in 869 s: tests (debug), tests (release), coverage`.
  Command: `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$PWD/target" cargo xtask check`.
  Passed: fmt, size limits, architecture, mutable statics, unsafe, clippy and perf guards.
  Debug and release both failed in hal (329 passed, 1 failed, 8 ignored), ui (508 passed,
  19 failed, 29 ignored), and xtask (39 passed, 1 failed). HAL wlan::tests::join_commands and
  UI socket tests reported PermissionDenied / Operation not permitted. The xtask failure was
  base::tests::the_base_is_the_parent_on_the_branch_and_a_lost_file_fails while creating a
  temporary Git commit. Coverage stopped at the same HAL failure (329 passed, 1 failed,
  8 ignored); no complete coverage result. No tests or ratchets changed to bypass restrictions.
  Time log: 08:44-09:00 UTC, approximately 16 minutes elapsed, including 869 seconds running
  the gate. Inspection, blocker documentation and evidence updates overlapped that run;
  no implementation work. COMMIT_MSG.txt contains the proposed documentation commit message.
  No commit, review, merge, device use or hardware verification. Task remains blocked.
- 2026-11-15: codex-b stopped implementation at the reservation boundary, as instructed by the owner.
  Branch t0026-harness-isolation, base 1d5fa1b, initially clean. No implementation files changed.
  Required claim additions: xtask/src/coverage.rs (hardcoded target/xtask-cov), gpu/build.sh and
  gpu/test-host.sh (fixed output and binary paths), ui/src/main.rs, ui/src/net6.rs, ui/src/http.rs,
  ui/src/scpi.rs and ui/src/vxi11.rs (host listener binding, selected ports and readiness).
  The MCP endpoint shares the HTTP listener. Host serve currently opens VXI-11 with fixed portmapper
  ports and listeners on unspecified addresses; tools-only changes cannot establish the requested contract.
  M1's script, batch and golden paths remain untouched. No device or external network access.
  Follow-up: coordinator must accept the additional reservations before implementation resumes;
  independent Claude review remains required before merge. Gate evidence is recorded above.
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-b on branch t0026-harness-isolation by the coordinator, base overhaul.
- 2026-11-15: done. Merged via integ-4 after cross-vendor review (Claude) and a passing full gate.
