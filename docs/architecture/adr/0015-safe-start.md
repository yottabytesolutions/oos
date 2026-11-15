# 0015: Safe start breaks restart loops

Status: accepted. 2026-11-15 (team-lead decision, from fablearch B2).

## Context

rcS restarts `oos-ui` after every exit (`rootfs/initramfs/etc/rcS`, step 5), and `oos-ui` restores the saved setup
(`/oos/oos-ui.setup`) at every start. The release build has `panic = abort`. A setting that provokes a panic or an
OOM kill therefore restarts into the same setting, forever, and a power cycle does not help because the setup is on
the data partition. The roll OOM of 2026-11-14 was one such case.

## Decision

- An exit contract: exit 0 is clean; any other exit or a signal is abnormal.
- A marker on `/oos` (`oos-ui.running`, replaced by `oos-ui.stable` after `stable_s`) and the times of abnormal exits
  within the last hour, kept by the rcS loop, both surviving a power cycle.
- After 3 abnormal exits within an hour, or one exit with code 75 (memory), rcS starts `oos-ui --safe`: default
  settings, the first saved setup retained as `oos-ui.setup.bad`, a toast. Later quarantines use a timestamp;
  retain the first and the two newest later copies, at most three in total.
- Both setup files (`/tmp/oos-ui.setup` for a same-boot restart, `/oos/oos-ui.setup` for a power-on) are written only
  with settings that ran `stable_s` (60 s) since their last change without an abnormal exit. Setup files, markers and
  the counter are written durably: temporary file, `fsync`, `rename`, `fsync` of the directory. A clean power-off writes the current settings.
- Owner clarification, review round 1, 2026-11-15: storage failure must never stop the UI, including `--safe`.
  Log once, disable automatic restoration, marker changes and promotion for that process, and show
  `Persistence off: storage unavailable` on the waveform and System page. A failed setup save during a requested
  power-off is logged; teardown and the MCU command proceed with a clean exit. A temporary supervisor stop flag
  is attempted even when saving fails. Existing on-disk markers may be impossible to remove on failed storage.
- After a promotion failure, retry after 1, 2, 4 seconds, doubling up to 300 seconds. Report the first error
  only until a complete promotion succeeds. Candidate changes still reset their own 60 s stability interval.
- The supervisor supplies a fresh boot_id/process/sequence run identity before every UI invocation. An abnormal
  process exit counts that identity even if it failed before `Startup::device` or left an older marker intact.
- A stale `running` marker from the same kernel boot always counts as an abort. A marker from a previous
  boot counts only with crash evidence: a panic/oops pstore record durably archived and counted by
  `oos-crash-save` with that run receipt, or a positive watchdog reset indication exposed by the platform.
  With no evidence, treat it as a power cut and do not count it. Panic and marker receipts deduplicate one exit.
- `mode` reads and filters the ledger without rewriting or syncing it. Explicit writes prune expired entries.
  On a damaged ledger the supervisor selects safe once, moves the old file to a timestamped `.bad` name and
  creates a fresh ledger, retaining at most two damaged copies. Recovery revalidates under the writer lock
  and requires a positive parse failure; tool/IO failures and timeouts preserve healthy history. Explicit clear
  does not depend on awk. Writers serialize with a BusyBox-compatible mkdir lock and stale PID reclamation,
  including a counter that outlives its timed-out archive parent. A failed operation or timeout selects safe
  once. A trial still belongs to the update watcher.
- Run crash archiving under BusyBox `timeout -s KILL 20`; every ledger operation, including the archive's
  count, uses `timeout -s KILL 5`. Archive counter failure keeps pstore and signals the supervisor through a
  temporary failure flag. These are process timeouts; blocked kernel IO may still wait for the kernel.
- The RSS watchdog forces STOP and resets the buffer-holding stages at the hard limit, and exits with 75 if memory
  does not come back.
- Kernel panics restart the unit: `panic=10` on the OOS kernel command line (`OOS_BOOTARGS` in the board's boot
  config and the image's own boot config, never p4, p5 or U-Boot). Today there is no `panic=`, and
  `/proc/sys/kernel/panic` is 0, so a panic halts until the power cable is pulled (2026-11-15, after the INTx revert
  reboot). At the next boot, after the one-shot and trial bookkeeping, rcS archives the ramoops records
  (`/sys/fs/pstore`, kept across a warm reset) to `/oos/crash/` within a 1 MiB budget, durably and idempotently by
  record id, then counts the boot as one abnormal exit only if a dmesg record holds a panic or an oops, then clears
  pstore. It shows in `:SYSTem:STATe?` and on the System page. A panic before rcS still loops the same image, the
  known limitation of `../../update.md`.
- Off during an image's trial start; there the update watcher rolls the image back.
- A bounded shutdown order, and a GPU helper death that falls back to the CPU path instead of ending the process.

Details: ../README.md, [Safe start and exit](../README.md#safe-start-and-exit).

## Consequences

- A bad setting costs at most 3 restarts and leaves a file the user can inspect.
- A setting changed less than a minute before a power loss is not restored. Accepted.
- The stress round gets a restart-loop case: a setup that aborts at start must reach safe start within 3 restarts,
  and a later clean start must persist again.

## Reset-evidence availability (review round 1)

Repository evidence: `system.md` records the observed dw_wdt TOPs warning and `boot.md` records that a watchdog
reset leaves no panic dmesg record. `oos-watchdog` logs `/sys/class/watchdog/watchdog0/bootstatus`, but the
repository contains no captured positive reset-reason value. The TOPs warning is configuration information,
not reset evidence. The supervisor accepts bit `WDIOF_CARDRESET` (0x20) from that sysfs attribute if supplied;
missing, zero or unreadable status is no evidence. No free-text dmesg warning is guessed to mean a reset.

Source inspection of [upstream Linux 6.12 dw_wdt](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/watchdog/dw_wdt.c)
finds no bootstatus assignment or CARDRESET capability. The
[watchdog UAPI](https://raw.githubusercontent.com/torvalds/linux/v6.12/include/uapi/linux/watchdog.h) defines the
0x20 reset bit. This supports treating a watchdog reset reason as unavailable unless the running platform
actually reports it. The pinned Rockchip BSP source and kernel source archive were unavailable here; upstream
inspection does not verify the BSP. Live `/sys` and dmesg on RK3399 were not read: this job prohibits device
access. An RK3399-specific reset-reason interface remains unverified for coordinator round 0.
