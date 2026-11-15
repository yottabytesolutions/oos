---
id: 36
title: "Decide memory depth readback in roll"
kind: chore
status: active
release: 0.2.0
owner: codex-c
depends: []
---

# Decide memory depth readback in roll

## Goal

Decide whether :ACQuire:MDEPth? in roll reports the roll record or the menu depth.

## Plan

Reserved paths: none. Reserve paths when claimed.

Record the compatibility decision and its reason. Add query tests for roll and normal acquisition.
Current matrix evidence reports the roll record. Hardware behavior beyond that is not yet verified here.

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0036-roll-depth by the coordinator, base overhaul.
