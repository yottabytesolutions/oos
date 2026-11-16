---
id: 59
title: Make lease guard signal cleanup deterministic
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Find and fix the intermittent SIGINT failure in the offline guard cleanup test
without weakening proof that each catchable signal runs cleanup.

## Plan

Reserved paths: `docs/tasks/0059-lease-signal-flake.md`,
`tools/tests/test_scope_lease_review.py`, `tools/tests/test_scope_lease.py`,
`tools/scope_lease.py`, `target/COMMIT_MSG.txt`.

The owner's assignment accepts this new claim. Review tier: tooling. The
coordinator owns cross-vendor review and batch integration. Committed as 5a29c83; review P3s (portable builtin in the test, interrupt message) applied in the follow-up commit.

1. Run the existing signal test 200 times under CPU load and retain complete logs.
   Inspect signal delivery, inherited dispositions and guard acquisition ordering.
2. Make signal delivery deterministic and close any guard trap installation window.
   Add regressions that fail before the fix and prove cleanup for every signal.
3. Repeat 200 loaded runs, execute `python-tests` and `cargo xtask check --fast`,
   and prepare one section in `target/COMMIT_MSG.txt` with evidence and limitations.

## Status

- 2026-11-16: author handoff prepared on the uncommitted working tree at base
  `7be056e`. Three modified Python sources and this task file are intentional.
  `target/COMMIT_MSG.txt` contains one section. Obsolete intermediate regression
  logs and temporary source fixtures are removed; final evidence and build caches
  remain. No commits, push, merge, full gate or hardware access. Task stays active
  for the coordinator's cross-vendor tooling review, unrestricted Python inventory
  rerun and full-gate batch integration.
- 2026-11-16: `python3 -I tools/check_inventory.py python-tests` exits 1. UI build
  succeeds; tools/tests runs 139 tests in 43.421 s, with 138 passing and one error:
  `test_host_sockets.HostSockets.test_two_runs_use_their_own_children_ports_and_storage`.
  Its child cannot bind host listeners in the restricted sandbox (errno 1,
  Operation not permitted). An independent loopback bind probe gets the same error.
  All lease tests pass. Full output/exit: `target/t0059-evidence/python-tests.log`
  and `.exit`; probe: `socket-probe.log`. The inventory stops at that root, so the
  remaining roots were run explicitly with `python3 -I -m unittest discover -s
  tools/privacy -p 'test_*.py' -v` (69 pass) and the same command with `-s ui/tools`
  (8 pass), both exit 0. Complete logs/exit codes: `privacy-tests.log`, `.exit`,
  `ui-tools-tests.log` and `.exit` in the evidence directory. Required next action:
  coordinator reruns the unchanged inventory outside the restricted sandbox.
- 2026-11-16: `cargo xtask check --fast` reports
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. All selected steps pass:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Full output and exit code:
  `target/t0059-evidence/fast-gate.log` and `.exit`; command logs:
  `target/check-logs/52615-1794793612759104000/`. Doctor, clippy, Rust debug/release,
  performance, coverage, script-syntax and image checks are not selected. The
  Python inventory is run separately. No full gate or hardware verification.
- 2026-11-16: post-fix loaded run passes 200/200 iterations, exit 0, under eight
  parallel `yes` processes (106.22 s). Each iteration runs the original named test
  plus both new regressions: 600 test executions and 3,200 signal subcases total,
  including inherited SIGINT-ignore and interrupted successful/busy acquisition.
  Evidence: `python3 -I target/t0059-evidence/stress.py after`, `after.log` and
  `after.exit`. All load children are terminated and reaped.
- 2026-11-16: running the unchanged parent test and guard with SIGINT inherited as
  ignored reproduces the reported `AssertionError: LeaseError not raised` exactly,
  exit 1. Evidence: `target/t0059-evidence/ignored-parent.log` and `.exit`.
  This confirms an environmental failure mode, not the disposition at the earlier
  full-gate failure. Temporary parent-source fixtures are removed after each run.
- 2026-11-16: guard now records signals before acquisition, arms EXIT cleanup only
  after successful mkdir and then honors any pending signal. Interrupted contention
  leaves the other holder's guard intact. The fake resets catchable dispositions in
  a fresh Python child before exec, avoiding thread-unsafe preexec hooks. The signal
  test waits for trap termination instead of racing EOF; its existing five-second
  executor timeout remains an error, never accepted as LeaseError.
- 2026-11-16: final regression tests against parent guard and fake sources fail,
  exit 1: all four signals after successful mkdir strand the guard, and inherited
  SIGINT-ignore times out. Existing busy guards survive all four signals. Evidence:
  `target/t0059-evidence/regressions-parent.log` and `.exit`. The original body sends
  its signal after trap installation, so the acquisition gap is a separate guard
  defect. Delivery versus immediate EOF and inherited ignore are test weaknesses;
  the original intermittent gate event itself has not been reproduced.
- 2026-11-16: original test passes 200/200 serial invocations under eight parallel
  `yes` CPU load processes (12.78 s), exit 0. Complete log, runner and exit status:
  `target/t0059-evidence/before.log`, `stress.py` and `before.exit`. Load children
  are terminated and reaped in the runner's finally block.
- 2026-11-16: codex claims the owner-assigned task on `t0059-lease-signal-flake`,
  based on `overhaul` at `7be056e`. Initial worktree is clean. Plan and reserved
  paths recorded before implementation. No hardware access is authorized or needed.
