---
id: 57
title: Repair image tier simulation prerequisites
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Repair the SSH, firewall and WLAN image checks after the per-tool rootfs environment migration.
Catch removed builder roles and missing simulation opt-ins in the default Python tests.

## Plan

Reserved paths: `docs/tasks/0057-image-tier-repair.md`, `tools/check_inventory.py`,
`tools/wlan-sim.sh`, `tools/ssh-sim.sh`, `tools/tests/test_check_inventory_discovery.py`,
`tools/tests/test_image_sim_contracts.py`, `target/COMMIT_MSG.txt`.

Review tier: tooling. The owner's explicit assignment accepts this claim and releases the
task-0055 inventory paths for this repair. No commits, pushes or hardware access.

1. Audit callers of removed environment roles. Prepare only the per-tool roles needed by
   SSH and firewall simulations, and cover both selections against the real role lock.
2. Trace the WLAN test environment across host, container and guest boundaries. Set the
   simulation opt-in where the test executes, with a Docker-free regression test.
3. Run failing-before and passing-after focused tests, the default Python runner, the fast
   gate and image checks as permitted by the sandbox. Record exact results and remaining
   image-suite limits. Leave one commit-message section for the coordinator.

Round 2, parent `f23f981`: trace the missing-firewall SSH failure through the service,
simulation and tooling history. Pin the service's refusal exit code, state and absence of
a listener in a Docker-free regression; correct the simulation's expectation without
relaxing its success checks. Keep image-suite's missing-kernel diagnostic unchanged and
verify the supplied source checksum. Run focused tests and the fast gate. Docker execution
and independent review remain with the coordinator; no commits or hardware access.

## Status

- 2026-11-15: round 2 ready for coordinator review on the uncommitted tree over `f23f981`.
  `cargo xtask check --fast` prints exactly
  `partial gate passed (not done): 8 steps in 20 s`, exit 0. Focused simulation contracts
  pass all 4 tests, inventory discovery all 15 and image-output tests all 6, each exit 0.
  Script syntax passes with `script-syntax: 62 Python, 62 shell scripts`, exit 0.
  Logs and exit files: `target/t0057-round2/`; gate command logs:
  `target/check-logs/44598-1794783243768048000/`. Docker SSH/firewall simulations and the
  real image suite were not run in this sandbox; the coordinator must rerun them on the
  Docker host. Default full gate and hardware verification were not selected. No hardware
  access, commits or pushes. Three tracked files are intentionally modified; caches and
  required evidence are retained. Cross-vendor review and integration remain with the
  coordinator. `git diff --check`, changed-file privacy and message privacy pass, exit 0.
  `target/COMMIT_MSG.txt` contains one section for this round.
- 2026-11-15: round 2 history identifies the expectation mismatch in `57096f6`:
  its hardened `run()` changes an ignored command failure into a simulation failure.
  `oos-ssh` has been unchanged since `94c69d8`, including exit 1 after recording
  `off: firewall not loaded`. This is a configured service failing its required firewall,
  so reporting failure is correct. `oos-firewall` last changed in `8b51036` for IPv6 and
  logging. Task 25 builder roles, task 55 dependency identities and task 56 explicit Cargo
  target/log context do not change this service contract; the guest writes its own config,
  and Docker does not forward the host's SSH/config overrides. Network isolation still
  permits the loopback sessions shown passing in the coordinator's supplied log.
  History evidence: `target/t0057-round2/history.log`.
- 2026-11-15: round 2 adds a shared exact-status assertion: ordinary calls still require
  exit 0; the missing-firewall case requires exit 1 plus the original state and no-listener
  assertions. No production service or firewall changes. The Docker-free regression fails
  before the fix in two subcases: rejecting the real exit 1 and accepting exit 0. After the
  fix it also rejects exit 2, a wrong state, a listener and unexpected ordinary-start errors.
  A real-service host fixture pins the off/no-ACL/missing-firewall statuses and confirms
  the refused start does not prepare credentials. Before/after logs and exits are retained.
- 2026-11-15: supplied kernel tarball matches `kernel/base.sha256`. An isolated missing-source
  CLI check confirms the exact existing diagnostic and that no builders launch; image-suite
  source is byte-identical to the parent. Evidence: `target/t0057-round2/kernel-source.log`,
  exit 0. No kernel or image-suite fix is needed.
- 2026-11-15: round 2 claimed by codex on `t0057-image-tier`, parent `f23f981`, with a
  clean initial working tree. Owner assignment extends the claim to `tools/ssh-sim.sh`;
  no other active task holds it. Plan recorded before code. The coordinator's host log
  confirms firewall-sim passes and SSH fails only on the missing-firewall start result.
  The supplied kernel source is present; image-suite source and diagnostics stay unchanged.
- 2026-11-15: implementation is ready for coordinator handoff on the uncommitted working
  tree at the recorded parent. No commits, pushes or hardware access; hardware is not
  verified. Task remains active pending cross-vendor tooling review and integration.
  Coordinator next action: review this candidate, provide the pinned kernel source and
  Docker/socket access, and rerun the default and image gates through real suite completion.
  `target/COMMIT_MSG.txt` contains one section. The five changed/new source and task files
  are intentional; required evidence and worktree build caches are retained. Temporary
  test fixtures are cleaned. `git diff --check`, changed-file privacy and message privacy
  checks pass, exit 0.
- 2026-11-15: `cargo xtask check --image` completes, exit 1, printing exactly
  `gate FAILED in 885 s: doctor, tests-debug, tests-release, coverage, python-tests, gpu-host, ssh-sim, firewall-sim, image-suite`.
  Passed IDs: fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures, agent-rules, doctor-base, clippy, perf, script-syntax,
  arm64-release-link and image-required. Script syntax checks 62 Python and 62 shell
  scripts. ARM release receipts and static ELF checks pass for all three shipped binaries.
  Doctor cannot use Docker; HAL/UI debug and release tests fail on socket permission
  denial; coverage stops at HAL's `wlan::tests::join_commands` with the same denial.
  Python discovery repeats the single host-socket error. GPU, SSH, firewall and image-suite
  report `missing prerequisite results: ["doctor"]` and do not execute their runners.
  Complete output and exit: `target/t0057-evidence/image-gate.{log,exit}`. Complete per-command
  logs and `steps.log`: `target/check-logs/63418-1794780510966196000/`.
- 2026-11-15: `cargo xtask check --fast` prints
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. All eight fast checks pass.
  `python3 -I tools/check_inventory.py python-tests` exits 1: 131 tools tests execute,
  130 pass and the existing host socket test errors because TCP startup is denied with
  `Operation not permitted`. Both new regressions pass in that default discovery run.
  The runner stops there; separately executing its remaining roots passes all 69 privacy
  tests and all 8 UI-tool tests, exit 0. Complete logs and exit files are in
  `target/t0057-evidence/`.
- 2026-11-15: direct SSH and firewall runners both exit 1 at `prepare initramfs`, before
  rootfs preparation, because Docker access is denied. A Docker server probe confirms
  socket permission denial. Direct `python3 -I tools/image_suite.py` exits 1 before any
  builders: this worktree lacks the checksum-pinned kernel source. Consequently real
  image assembly, validation and guest simulations through suite completion remain
  unverified here. The Docker-free orchestration test reaches the end of the suite with
  mocked builders, validation and all four simulation commands; this is contract evidence,
  not a real image-suite pass. Direct `sh tools/wlan-sim.sh` also exits 1 at Docker
  resolution, before builds or guest startup (`target/t0057-evidence/wlan-sim.{log,exit}`).
- 2026-11-15: focused regressions fail before the repair: inventory discovery exits 1 with
  three failures (preparation order and removed-role checks for SSH/firewall); WLAN exits 1
  with two failing subcases (unset and inherited `0`). After the repair, all 15 inventory
  discovery tests and the WLAN contract test pass, exit 0. Logs are retained under
  `target/t0057-evidence/{before,after}-{inventory,wlan}.log`.
- 2026-11-15: SSH prepares `initramfs`, `rootfs-nft`, `rootfs-dropbear` and
  `rootfs-sftp-server`; firewall prepares only `initramfs` and `rootfs-nft`. Caller audit
  found no other removed-role requests: builders, receipt identities and profile-driven
  bootstrap use current roles. Audit logs: `target/t0057-evidence/role-callers.log` and
  `target/t0057-evidence/removed-role-audit.log`. WLAN now sets its opt-in at the guest test
  invocation, so host environment/context scrubbing cannot lose it across the QEMU boot.
- 2026-11-15: codex claims the owner-assigned repair on `t0057-image-tier` at `9dba339`.
  Initial working tree is clean. Plan recorded before code. The shared preparer requests
  removed role `rootfs`; the WLAN guest init invokes the ignored Rust test without its
  required `OOS_WLAN_SIM=1`. Coordinator owns cross-vendor tooling review and integration.
