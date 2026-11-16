---
id: 67
title: Close watchdog and early boot safety gaps
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Check the three upstream review findings against overhaul and fix applicable watchdog
handoff, early panic restart and card alias bypasses. Review tier: unit.

## Plan

Reserved paths: `docs/tasks/0067-watchdog-boot-safety.md`,
`rootfs/initramfs/bin/oos-watchdog`, `rootfs/initramfs/bin/oos-oneshot`,
`rootfs/initramfs/etc/rcS`, `rootfs/board/dho/etc/oos-board`,
`kernel/dts/rk3399-rigol-dho900.dts`, `hal/src/update/kexec/watchdog/`, `hal/src/update/kexec.rs`,
`hal/src/update/card/`, `hal/src/update/card.rs`,
`hal/tests/rootfs_watchdog.rs`, `hal/tests/rootfs_boot_safety.rs`,
`fpgad/src/bin/oos-update.rs`, `fpgad/src/bin/oos_update/args.rs`,
`fpgad/tests/unit_identity/mod.rs`, `docs/boot.md`, `docs/update.md`,
`docs/architecture/README.md`, `docs/architecture/system.md`, `target/COMMIT_MSG.txt`.

The owner's direct assignment accepts this claim on branch `t0067-watchdog-safety`
from overhaul, superseding earlier task 0054/0060 reservations for these fixes.
Coordinator owns cross-vendor review, the full integration gate and unit acceptance.
No commits or hardware access.

1. Trace all three findings with source locations and record applies/does-not-apply.
2. Add failing-before host regressions for backoff handoff and returned kexec,
   panic restart ordering and failed boot restoration, and CLI card aliases.
3. Make backoff interruptible and require a bounded, request-specific supervisor
   acknowledgement before opening the watchdog. Enable panic restart only after
   confirmed stock selection. Exempt only regular card files from hardware checks;
   resolve aliases and compare device numbers in HAL helpers to respect CLI size.
4. Run focused debug/release tests, the gate's exact clippy step and
   `cargo xtask check --fast`. Retain complete logs in ignored `target/`, record
   limitations here and prepare one section in `docs/architecture/README.md`, `docs/architecture/system.md`, `target/COMMIT_MSG.txt`.

## Status

- 2026-11-16: final shell syntax checks for all three changed scripts,
  `git diff --check`, commit-message privacy and changed-file privacy checks pass,
  exit 0. Whole-tree privacy audit exits 1 with 150 findings, all on unchanged
  paths; no out-of-scope cleanup was attempted. Complete logs/exits remain under
  review-round evidence. HEAD remains `37438021`; eleven tracked paths contain
  intentional uncommitted work. No temporary snapshot/runner sources or binaries
  remain, and all test/check processes have finished.
- 2026-11-16: review-round fixes are ready for coordinator review as an
  uncommitted diff on parent `37438021`. The trap kills the global backoff
  sleeper, a post-launch flag check covers signals before its pid is recorded,
  and reaping clears child pids. The acknowledgement reset precedes removal.
  rcS warns when automatic restarts stay off; restoration requests direct
  read-back. gel-install now uses the shared checked opener, resolving the
  opened device number through sysfs to a whole disk on the exact SD controller
  component, independent of stock/OOS node names. Regular test files retain
  their exemptions. The updater remains below its unchanged size ratchet.
  No commits, pushes or hardware access; task remains active. All tracked changes
  are intentional. `target/COMMIT_MSG.txt` contains one proposed commit section.
- 2026-11-16: final focused host debug/release tests pass, exit 0:
  `cargo test --locked -p oos-hal --test rootfs_watchdog --test rootfs_boot_safety
  --test rootfs_selfheal` (6/4/23 tests), `cargo test --locked -p oos-hal --lib
  update:: -- --test-threads=1` (98 passed, two existing ignored), and
  `cargo test --locked -p oos-fpgad --bin oos-update --test explicit_signing`
  (4/25 tests), plus their `--release` equivalents. The first parallel update
  run failed the existing `busy_follows_the_lock` assertion after dropping its
  lock (97 passed, two ignored); the complete serial rerun passes. An initial
  acknowledgement test incorrectly required an intermediate killed kicker to
  print its startup line; its corrected assertion requires resumed supervision.
  Complete logs and exit files are in `target/t0067-evidence/review-round/`:
  `rootfs-debug-final`, `cli-debug-final`, `update-debug-serial`,
  `rootfs-release`, `update-release`, `cli-release`; initial failures remain.
- 2026-11-16: final exact `steps::clippy` passes, exit 0: six host crates,
  five ARM crates, unchanged ratchets (hal 93/177, dsp 66/151, core 3/46,
  ui 129/406, fpgad 7/20, xtask 0/0 entries/total). Reproducer and complete
  log/exit: `clippy-step.py`, `clippy-step-final.log` and `.exit` in the
  review-round evidence directory. Per-command logs:
  `target/check-logs/18919-1794826264509972000/`.
  Final `cargo xtask check --fast`, exit 0, reports exactly
  `partial gate passed (not done): 8 steps in 23 s`.
  Complete log/exit: `fast-gate-final.log` and `.exit`; per-command logs:
  `target/check-logs/20345-1794826267247906000/`.
  Full gate, coverage, performance and image checks were not selected.
- 2026-11-16: final tests against isolated parent-source snapshots reproduce
  two watchdog failures (USR1 after the check/before wait, second request during
  acknowledgement removal), three boot failures (missing warning, missing direct
  read request, mismatched read-back enabling restarts) and the gel-install
  device-alias refusal regression. All three test processes exit 101 as expected;
  the reproduction runner exits 0. `parent-regressions.py`, `parent-watchdog`,
  `parent-boot_safety` and `parent-aliases` logs/exits are in review-round evidence.
  The logging-only case passes on the parent under the host shell and remains a
  regression guard. The CLI parent snapshot links current HAL; the old
  gel-install bypasses that checked opener. Snapshot trees and runner binaries
  were removed after execution.
- 2026-11-16: BusyBox ash execution remains unverified: Docker access is denied
  by the session sandbox (both inventory and an Alpine ash launch); no host
  BusyBox executable is installed. The available cached image binary is ARM Linux
  ELF, so it cannot execute directly on this host. `busybox-runner.log` and `.exit`
  record the failed launch. Coordinator next action: run the final boot/watchdog
  suites with `OOS_TEST_SHELL` pointing at an executable BusyBox ash in a Linux
  environment with Python 3. The boot fixture now selects that shell for both
  rcS and oos-oneshot. Hardware, real block-device access and direct media
  behavior remain unverified; no unit access was attempted.
- 2026-11-16: rootfs selects distro `busybox-static` (container recipe and build
  copy), with no custom BusyBox config. The
  [distro static config](https://sources.debian.org/src/busybox/1:1.37.0-6/debian/config/pkg/static/)
  enables `CONFIG_FEATURE_DD_IBS_OBS`, which supplies `iflag=direct` as well as
  the existing `conv=fsync`. Inspection of the cached Ubuntu 1:1.37.0-7ubuntu1
  image binary confirms the compiled dd `iflag`/`direct` keywords.
  `busybox-direct-support.txt` records the binary hash and source/config evidence.
  BusyBox itself can retry without O_DIRECT on EINVAL; this change adds no cached
  fallback, and actual media behavior still needs unit verification.
- 2026-11-16: review round claimed by Codex on the existing branch, parent
  `37438021`. Plan: close the check/wait and acknowledgement-reset windows,
  clear reaped child pids, report disabled boot restarts, require direct boot
  read-back if supported by the image's BusyBox, and share checked card opening
  with gel-install across stock and OOS device names. Add deterministic signal,
  boot-warning/read-back and card-identity regressions; prove failures on parent
  snapshots, then run focused debug/release tests, exact clippy and the fast gate.
  Existing reserved paths cover this round. No commits or hardware access.
- 2026-11-16: author handoff ready on the uncommitted working tree based on
  `c6f1f98d`. Task remains active pending coordinator cross-vendor unit review,
  the full batch gate and unit acceptance. No commits, pushes or hardware access.
  Hardware timing, BusyBox ash execution and real block-device alias behavior
  remain unverified on the unit. Host scripts ran under POSIX sh; watchdog and
  boot simulations accept `OOS_TEST_SHELL` for the image's ash runner. All tracked
  changes and the three new task/source/test files are intentional. Evidence and
  active build caches remain in ignored `target/`; disposable snapshot trees,
  temporary runner sources/binaries and scratch scripts were removed.
  `target/COMMIT_MSG.txt` contains one proposed commit section.
- 2026-11-16: final `cargo xtask check --fast` reports exactly
  `partial gate passed (not done): 8 steps in 22 s`, exit 0. Selected steps fmt,
  size, architecture, mutable-statics, unsafe, method-exceptions, known-failures
  and agent-rules all pass. Complete output/exit: `target/t0067-evidence/fast-gate.log`
  and `.exit`; command logs: `target/check-logs/67955-1794824385520006000/`.
  Doctor, full Rust suites, performance, coverage, script inventory and image
  checks were not selected. Shell syntax was separately checked with `/bin/sh -n`
  for each changed init script (exit 0); `python3 -I tools/tests/test_watchdog_dt.py`
  passed three tests (exit 0). `git diff --check` passes.
- 2026-11-16: final exact `steps::clippy` passes, exit 0, covering six host crates,
  five ARM crates and the unchanged clippy ratchets: hal 93 entries/177 total,
  dsp 66/151, core 3/46, ui 129/406, fpgad 7/20, xtask 0/0. No new findings.
  Reproduction script `target/t0067-evidence/clippy-step.py` compiles the actual
  xtask modules and invokes the actual step with `run::start_logs`, without a
  product CLI or tracked test hook. Complete output/exit: `clippy-step.log` and
  `.exit` in that directory; per-command logs:
  `target/check-logs/95017-1794824469663298000/`. An initial scratch runner selected
  a syn artifact without xtask's features; it was corrected to use Cargo's actual
  artifact list before this successful final run. No gate or ratchet was changed.
- 2026-11-16: focused debug/release checks pass, exit 0 in both modes:
  `cargo test --locked -p oos-hal --lib update::` (96 passed, two existing ignored),
  `cargo test --locked -p oos-fpgad --bin oos-update --test explicit_signing`
  (four binary tests, 25 CLI tests), and `cargo test --locked -p oos-hal
  --test rootfs_watchdog --test rootfs_boot_safety --test rootfs_selfheal`
  (three, two and 23 tests). Release equivalents add `--release`. After the last
  acknowledgement cases and failed-fsync case, final debug/release reruns of
  `cargo test --locked -p oos-hal --lib update::kexec::` (24 passed) and
  `cargo test --locked -p oos-hal --test rootfs_boot_safety` (two passed) also pass.
  Full logs and per-command exits are under `target/t0067-evidence/`, named
  `update-*`, `cli-*`, `rootfs-*`, `kexec-*` and `boot-*`.
- 2026-11-16: final regression sources fail on isolated baseline snapshots without
  changing the working tree: watchdog backoff has two failures; early panic and
  restoration ordering have two failures; the device-alias CLI test fails. Each
  test process exits 101, as expected. Reproduction:
  `target/t0067-evidence/parent-regressions.py`; retained `parent-watchdog.log`,
  `parent-boot_safety.log`, `parent-aliases.log` and matching `.exit` files. The CLI
  snapshot links the current HAL, whose unit comparison contract is unchanged;
  the baseline CLI bypasses the new card path helper. The original test-first
  failures are also retained as `watchdog-before`, `boot-before`, `aliases-before`.
- 2026-11-16: implemented interruptible backoff and a requester-pid acknowledgement
  after kicker reaping, with a shared two-second acknowledgement/open deadline.
  Missing, wrong, stale or unreadable acknowledgements cannot acquire the device;
  cancellation closes before resuming and resets backoff. Boot arguments and kexec
  command lines now force `panic=0`; rcS enables a 10 s panic restart and starts
  userspace watchdog supervision only after confirmed stock selection. One-shot
  write/fsync/read-back failures and unknown magic return failure. CLI simulation,
  extra-key and direct-install exemptions derive from the opened regular file;
  HAL canonicalizes aliases, compares block device numbers and checks the SD
  controller before access. The updater shrinks eight physical lines and passes
  its unchanged size ratchet. Permanent boot/update/architecture descriptions
  reflect the safety fix; no ownership or crate boundary changes were introduced.
- 2026-11-16: all three findings APPLY on the baseline `c6f1f98d`:
  1. `rootfs/initramfs/bin/oos-watchdog:98` sleeps in the foreground, while
     `hal/src/update/kexec/watchdog/device.rs:87` accepts a free device without
     supervisor acknowledgement. Its restore at line 111 only removes the
     request, leaving a supervisor still sleeping in backoff. Fixed handshake:
     `rootfs/initramfs/bin/oos-watchdog:76` and
     `hal/src/update/kexec/watchdog/device.rs:89`.
  2. `rootfs/board/dho/etc/oos-board:8` and
     `kernel/dts/rk3399-rigol-dho900.dts:55` set `panic=10` before rcS. The unit's
     verified loader switch (`docs/boot.md:62`) does not consume OOSK; only
     `rootfs/initramfs/bin/oos-oneshot:29` restores KRNL, called by rcS at line 10.
     Before that, neither trial count nor rollback runs. Thus repeated early
     panics can reset into the same recovery forever on this loader, including
     a one-shot selection or a persist re-arm. `oos-boot-oos-again` invokes checked
     arm; persist re-arm is after the health verdict in the updater watcher,
     while install arms only after read-back (`hal/src/update/mod.rs:363`,
     `hal/src/update/mod.rs:564`). No boot-loader fallback counter bounds those
     early resets. This is a source/doc trace, deliberately not reproduced on
     the unit. Fixed rcS ordering: `rootfs/initramfs/etc/rcS:12`. An early cold
     panic now halts; manual power cycles can still repeat the bad image and
     require a card reader. An inherited kexec watchdog can still reset into the
     installed fallback; the trial image has not been written yet.
  3. `fpgad/src/bin/oos_update/args.rs:49` selects a simulated running unit for
     every nonliteral card path; `fpgad/src/bin/oos-update.rs:275` applies test
     exemptions the same way, and line 113 skips the controller check for aliases.
     Unit comparison still runs, but can compare against a caller-selected
     simulated unit. Fixed file-type/device-number checks live in
     `hal/src/update/card/path.rs:12` and `hal/src/update/card/path.rs:18`.
     CLI regressions use device aliases to a host character-device stand-in and
     regular card-file aliases, without touching a real card.

- 2026-11-16: Codex claimed the owner-assigned task on `t0067-watchdog-safety`,
  base `c6f1f98d` (overhaul). Initial working tree clean; hooks already configured.
  Plan recorded before implementation. Hardware behavior will remain unverified.
