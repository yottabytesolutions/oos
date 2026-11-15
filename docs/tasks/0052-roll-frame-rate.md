---
id: 52
title: "Roll at slow time bases redraws below 1 frame/s"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Roll at slow time bases redraws below 1 frame/s

## Goal

Roll at 200 ms/div redraws at about 30 frames/s with a bounded step time.

## Plan

Reserved paths: none. Reserve paths when claimed.

Unit round 0 (2026-11-15) measured 0.6 frames/s at 200 ms/div with the step time growing from 1.0 to 1.3 s, on overhaul and on dho804 alike: pre-existing. Reproduce on the host with the scripted batch harness (roll route), find where the step time goes and why it grows, fix with a failing-before test, confirm on the unit.

## Status

- 2026-11-15: seeded by the coordinator.
