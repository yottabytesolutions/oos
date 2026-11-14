---
id: 35
title: "Count acquired roll frames"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Count acquired roll frames

## Goal

Include roll captures in the acquired counter.

## Plan

Reserved paths: none. Reserve paths when claimed.

Add a failing-before test for the roll branch in ui/src/board/dho/hw.rs. Move the counter update onto the roll
path that currently continues before the add. Coordinate with the existing branch instead of duplicating work.

## Status

- 2026-11-14: branch `fix/roll-queue` is in progress, as reported by the owner. This task records that work;
  do not claim or modify the branch without a handoff. Matrix evidence: 2026-11-14.
