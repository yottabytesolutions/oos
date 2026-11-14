---
id: 21
title: "Lease scope access and separate capabilities"
kind: feature
status: done
release: 0.2.0
owner: codex-b
depends: []
---

# Lease scope access and separate capabilities

## Goal

Require an exclusive scope lease before host tools access the shared unit running
OOS. Stock adb access still needs the owner's release of our shared unit.

## Plan

Reserved paths: `tools/scope_lease.py`, `tools/scope-lease`, `tools/tests/`,
`tools/device-test.py`, `tools/device-deploy.sh`, `tools/device-survey.sh`,
`tools/platform-probe.sh`, `tools/README.md`, `AGENTS.md`,
`docs/tasks/0021-scope-lease.md`, `docs/tasks/done/0021-scope-lease.md`,
`COMMIT_MSG.txt`.

1. Add a stdlib SSH lease library and CLI with atomic acquire and guarded updates.
2. Enforce host selection and capability checks in device tools.
3. Add offline fake-SSH tests for races, quarantine, identity and permissions.
4. Document usage, run host checks and prepare the coordinator commit message.
5. Apply the accepted review: remove stock adb lease checks, retry busy guards and
   heartbeats, add explicit stale guard breaks, use device time and require tokens.
6. Add wiring metadata, policy clarifications and offline regression tests.
   Audit item 8, the privacy scanner, is deferred to task 0024.

Before parallel migration work, add an atomic directory on the device with holder, task, build and heartbeat.
OOS host tools must refuse access without the lease. Stock adb tools use no SSH lease.
Separate observe, test, install and reboot capabilities.
Install and reboot need the owner's go for the task. Quarantine expired leases; never transfer automatically.
Make tools/device-deploy.sh owner-only. Remove default device addresses from tools.
Audit items: 3, 4. Item 8, the privacy scanner, is deferred to task 0024.
No hardware access is authorized by this task file.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: finish the assigned clock and reboot review fix locally.
  Store the boot ID and use uptime for heartbeat age. Keep wall time for start
  and log text. A matching holder, task and token with install or reboot
  capability can rebind after reboot. Log before updating. Other holders need
  an explicit break. Missing info reports `quarantined lease: incomplete info`
  before any sed. Document installs and reboots under a lease.
  `python3 -I -m unittest discover -s tools/tests` exits 0:

  ```text
  Ran 46 tests in 17.333s

  OK
  ```

  `python3 -I -m py_compile tools/scope_lease.py tools/scope-lease tools/device-test.py tools/tests/test_scope_lease.py tools/tests/test_scope_lease_review.py tools/tests/test_scope_lease_tools.py tools/tests/test_scope_lease_boot.py`
  exits 0 with no output.
  `for script in tools/device-deploy.sh tools/device-survey.sh tools/platform-probe.sh; do sh -n "$script" || exit "$?"; done`
  exits 0 with no output. Each script is checked separately.
  `git diff --check` exits 0 with no output.
  The new boot regression module against the original implementation in a
  scratch copy exits 1, as expected. Six tests run and expose the defects.
  The library stays under 400 lines. Leave the changes and `COMMIT_MSG.txt`
  uncommitted. No network, device access or push. Full gate unverified.

- 2026-11-15: start the assigned clock and reboot review fix on
  `t0021-scope-lease`, base `38c3f0a`. Reuse the accepted reserved paths.
  Plan: record boot ID and uptime, authorize and log reboot rebinds, reject
  missing info clearly, add offline regressions and document the lifecycle.
  Leave changes and `COMMIT_MSG.txt` uncommitted. No network or device access.

- 2026-11-15: final review-fix validation:
  `python3 -I -m unittest discover -s tools/tests` exits 0 and prints:

  ```text
  ....................................
  ----------------------------------------------------------------------
  Ran 40 tests in 10.862s

  OK
  ```

  `python3 -I -m py_compile tools/scope_lease.py tools/scope-lease tools/device-test.py tools/tests/test_scope_lease.py tools/tests/test_scope_lease_review.py tools/tests/test_scope_lease_tools.py`
  exits 0 with no output.
  `for script in tools/device-deploy.sh tools/device-survey.sh tools/platform-probe.sh; do sh -n "$script" || exit "$?"; done`
  exits 0 with no output. Each script is checked separately.
  `git diff --check` exits 0 with no output.
  Three regressions for tokens, device clock skew and busy-guard renewal also
  fail against the original tip in an isolated scratch copy. That run exits 1
  as expected. The revised implementation passes all three in the full suite.
  File and function size checks pass. No em-dashes were added.
  Leave the changes uncommitted with `COMMIT_MSG.txt` at the worktree root.
  No network or device access. No push. Task stays in `done/` as requested.
  The full gate remains unverified. This checkout has no xtask, and this session
  forbids commits. No hardware behavior was verified on the unit.

- 2026-11-15: apply the accepted review decisions, including the optional fixes.
  Stock adb tools use no SSH lease. Survey reminds users about the owner's release.
  Guard contention retries for 3.1 seconds. Signal traps clean up owned guards.
  Explicit guard break requires age over 60 seconds and logs before removal.
  Device time drives timestamps and age checks. Tokens are required for checks,
  renew and release. Only acquire prints the token. Optional wiring is bounded
  and quoted. Heartbeats retry until expiry minus a 75 second margin.
  Document SSH setup, the owner's go, destructive-command limits and no automatic
  unit recovery. Add offline regressions. Split tool integration tests to keep
  each file under 400 lines. Audit item 8 remains deferred to task 0024.
- 2026-11-15: start the owner-assigned review fixes on `t0021-scope-lease`,
  base `dbf596b`. Reuse the accepted reserved paths. Keep this task in `done/`
  as requested. No network, device access, pushes or commits are authorized.

- 2026-11-14: codex-b completes the requested implementation handoff. Status is done
  as requested by the owner. The task is in `done/` for the merge. No commit
  was made. `COMMIT_MSG.txt` is ready for the coordinator. No network or hardware access.
  The full gate on a final commit remains unverified because commits are forbidden
  for this session and this checkout has no xtask.
- 2026-11-14: final offline validation:
  `python3 -I -m unittest discover -s tools/tests` exits 0 and prints:

  ```text
  ......................
  ----------------------------------------------------------------------
  Ran 22 tests in 3.079s

  OK
  ```

  `python3 -I -m py_compile tools/scope_lease.py tools/scope-lease tools/device-test.py tools/tests/test_scope_lease.py`
  exits 0 with no output.
  `sh -n tools/device-deploy.sh tools/device-survey.sh tools/platform-probe.sh`
  exits 0 with no output. `git diff --check` exits 0 with no output.
  `cargo xtask check` exits 101 and prints:

  ```text
  error: no such command: `xtask`

  help: a command with a similar name exists: `test`

  help: view all installed commands with `cargo --list`
  help: find a package to install `xtask` with `cargo search cargo-xtask`
  ```

- 2026-11-14: document the lease policy and usage. Deploy is owner-only. The Python
  harness renews in the background and stops IO after renewal failure. Shell tools
  check capability before transport. Tests run the real shell protocol locally and
  stub or block all network transports used by wrapper tests.
- 2026-11-14: implement atomic lease acquire, identity and capability checks, explicit
  go recording, quarantine, compare-before-update and break logging. Use the existing
  `/etc/oos-build` rather than a new `/oos` version file. It is the build source used
  by the updater and avoids recording the SCPI serial number. Add a short metadata
  guard and unique lease token so stale renew, release and break cannot affect a
  replacement lease. Also gate platform probe with observe because it accesses the
  shared unit. All device addresses now come from `--host` or `OOS_SCOPE`.

- 2026-11-14: codex-b starts the owner-assigned task on `t0021-scope-lease`,
  base `589ccb8838bedb4dda824deb84aaf5c0bc3e2105`. The assignment authorizes the
  reserved paths above. No network, hardware access, git metadata writes or commits.

- 2026-11-14: seeded from task 0001. Not yet claimed.
