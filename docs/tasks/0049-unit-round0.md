---
id: 49
title: "Unit round 0 for overhaul"
kind: chore
status: active
release: 0.2.0
owner: unitlab
depends: []
---

# Unit round 0 for overhaul

## Goal

Hardware evidence that overhaul runs on the unit before the migration steps add risk.

## Plan

Reserved paths: none (no source changes). Unit: scope lease with cap reboot.

Build a signed image of overhaul eaa5590, install it as a trial through /update, run the round 0 checks
(boot, panic=10, watchdog kicker, frame-rate counters against the dho804 baseline, roll and STOP with bounded RSS,
:ACQuire:MDEPth? in roll and normal, web UI, sustained SCPI), then reboot without confirming so the unit returns
to the dho804 build. No panic or watchdog-kill tests in this round. Results:
private RE notes docs/logs/oos_unit_round0_20261115.md. M1b adds its own round 0 checks later.

## Status

- 2026-11-15: seeded and claimed for unitlab by the coordinator. Owner go: standing authorization for autonomous
  unit work with restarts and trial installs (2026-11-15).
