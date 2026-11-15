---
id: 48
title: "M1b: safe start and exit"
kind: feature
status: active
release: 0.2.0
owner: codex-b
depends: [45]
---

# M1b: safe start and exit

## Goal

A scope that cannot loop: restart limits, `--safe`, durable markers, bounded shutdown and kernel panic recovery.

## Plan

Reserved paths: rootfs/initramfs/etc/rcS, rootfs/initramfs/bin/oos-crash-save, rootfs/initramfs/bin/oos-watchdog,
rootfs/board/dho/etc/oos-board, hal/tests/rootfs_selfheal.rs, ui/src/main.rs (startup and shutdown only), new
modules under ui/src/ for markers and shutdown, their tests. Extend when claimed.

Specification: [M1b in the migration](../architecture/migration.md#m1b-safe-start-and-exit), adr/0015, and
[Safe start and exit](../architecture/README.md#safe-start-and-exit). Builds on the self-heal forward-port (task 0045:
panic=10, watchdog kicker, oos-crash-save). Write the plan here first (what 0045 already covers, what remains,
failing-before tests), then implement. Unit checks belong to round 0 and run under a scope lease.

## Status

- 2026-11-15: seeded and claimed for codex-b by the coordinator; the M1b prerequisite (roll fix forward-port bb7dd9b)
  is merged.
