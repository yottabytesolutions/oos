---
id: 45
title: "Forward-port the self-heal hotfix (panic restart, watchdog, crash archive)"
kind: fix
status: active
release: 0.2.0
owner: selfheal
depends: []
---

# Forward-port the self-heal hotfix (panic restart, watchdog, crash archive)

## Goal

Bring the unit hotfix dho804 abd00ca (panic=10, hardware watchdog kicker, crash archive) into overhaul, adapted to
the safe-start design. Part of migration M1b; the abnormal-exit counter and safe mode stay with M1b.

## Plan

Reserved paths: `rootfs/board/dho/etc/oos-board`, `rootfs/initramfs/etc/rcS`, `rootfs/initramfs/bin/oos-watchdog`,
`rootfs/initramfs/bin/oos-crash-save`, `hal/tests/rootfs_selfheal.rs`, `docs/boot.md`, the self-heal subsection of
`docs/architecture/system.md`, this file.

## Status

- 2026-11-15: claim accepted on overhaul by the coordinator for selfheal, branch t0045-selfheal-fwd. The branch
  carries the full plan and status.
