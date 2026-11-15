---
id: 45
title: "Forward-port the self-heal hotfix (panic restart, watchdog, crash archive)"
kind: fix
status: done
release: 0.2.0
owner: selfheal
depends: []
---

# Forward-port the self-heal hotfix (panic restart, watchdog, crash archive)

## Goal

Bring the `dho804` hotfix `fix/self-heal` (merged as abd00ca) to `overhaul` through the hotfix lane
([migration](../architecture/migration.md), Hotfix lane): the unit restarts by itself after a kernel panic or a hard
hang, and the panic text is archived on `/oos`. Where ADR 0015 ([safe start](../architecture/adr/0015-safe-start.md),
its "Kernel panics" part) is more specific than the hotfix, the scripts follow the design. The rest of M1b (the
abnormal-exit counter, the markers and safe mode in `oos-ui`) stays planned under M1b.

## Plan

Reserved paths: `rootfs/board/dho/etc/oos-board`, `rootfs/initramfs/etc/rcS`, `rootfs/initramfs/bin/oos-watchdog`,
`rootfs/initramfs/bin/oos-crash-save`, `hal/tests/rootfs_selfheal.rs`, `docs/boot.md`, the self-heal subsection of
`docs/architecture/system.md`, this file.

- `panic=10 softlockup_panic=1` in `OOS_BOOTARGS`.
- `oos-watchdog` as on `dho804`: busybox `watchdog -F -T 30 -t 5`, started by rcS after `oos-oneshot`, supervised
  with backoff, the `before open:` diagnostic line.
- `oos-crash-save` to the design: a 1 MiB budget for `/oos/crash`, 4 MiB of `/oos` kept free, the dmesg record whole
  and the console's last 64 KiB, archives named by the records (time, id, checksum) so an interrupted boot repeats
  without a second archive, durable order archive, then clear pstore. The counter step between the two belongs to
  M1b. The reason (panic or oops) is reported for M1b to count. The boot is identified by `boot_id`, not uptime.
- Host tests (failing before): a fake pstore and a fake `/oos` with a 16 MiB `df`; archive once, repeat after an
  interruption without a second archive, the budget and the reserve hold, console records alone archive nothing, a
  failed archive keeps pstore; the watchdog wrapper restarts an exiting kicker with growing delays.
- `docs/boot.md` section; the two unit findings in `docs/architecture/system.md`.
- Full gate on the final commit.

## Status

- 2026-11-15: done. Merged into overhaul with the 2026-11-15 batch (379cd84, Codex review confirmed); the counter and safe mode stay in M1b.

- 2026-11-15: Codex confirm of 5a580c5: the build metadata copy is now compared with its source before the commit
  (as boot_id); test with a corrupting cp for boot_id and build, which fails when the comparison is removed.
- 2026-11-15: Codex review of 66d1d4c, six findings fixed. The retry path checks an existing archive against
  pstore and fsyncs it and `/oos/crash` before clearing (a mismatch is set aside and redone); at most one partial;
  only Panic/Oops dmesg records start an archive; boot_id and build are checked and read back; fault injection in the
  host tests (PATH shims for cp, mv, rm, sync that fail or corrupt on a given call), 15 tests, each removal of an
  fsync or of the read-back fails at least one; `docs/boot.md` says the unit results are from abd00ca.
- 2026-11-15: rebased onto overhaul 3a1db82 as task 0045. Both deviations below accepted by the coordinator. The
  watchdog test now waits for its log lines (up to 60 s) and checks order and the lower bound of the backoff only, so a
  loaded host cannot fail it. Next: full gate, review.
- 2026-11-15: ported. Scripts, rcS, bootargs, `docs/boot.md`, the unit facts in `docs/architecture/system.md`, host
  tests `hal/tests/rootfs_selfheal.rs` (7, failing before: the scripts did not exist). Deviation from the design text:
  old archives are pruned after the new one is committed, not before, so a failed write never costs an old record;
  the 1 MiB budget can be exceeded by one archive (at most about 200 KiB: a 128 KiB dmesg record and 64 KiB of
  console) until that prune. The archive key uses md5sum (busybox has no cksum). Left for M1b: the abnormal-exit
  counter between archive and clear, the `oos-ui.running` dedup, safe start. Next: full gate, review.
- 2026-11-15: claim accepted on overhaul by the coordinator for selfheal, branch `t0045-selfheal-fwd` (first drafted
  as 0044, which overhaul had already given to the new-core spike). The hotfix tests passed on the unit on
  `dho804`: a warm reboot clears the watchdog; a sysrq panic came back in 39 s with the record saved; a killed kicker
  reset the unit after 41 s, back after 56 s.
