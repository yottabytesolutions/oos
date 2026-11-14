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
  settings, the saved setup renamed to `oos-ui.setup.bad`, a toast.
- Both setup files (`/tmp/oos-ui.setup` for a same-boot restart, `/oos/oos-ui.setup` for a power-on) are written only
  with settings that ran `stable_s` (60 s) since their last change without an abnormal exit. Setup files, markers and
  the counter are written durably: temporary file, `fsync`, `rename`, `fsync` of the directory. A clean power-off writes the current settings.
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
