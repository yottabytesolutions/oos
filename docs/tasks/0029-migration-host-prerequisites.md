---
id: 29
title: "Enforce golden and acquisition harness prerequisites"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: [2, 4]
---

# Enforce golden and acquisition harness prerequisites

## Goal

Have golden pictures before drawing migrations and a scripted acquisition device before parallel acquisition work.

## Plan

Reserved paths: none. Reserve paths when claimed.

Use M1 for deterministic Demo and scripted-batch baselines and gate comparisons. Define tolerances and an explicit
reference update command, with expected, actual and difference pictures for review. Use M3.0 for the scripted Device
and Board contract. Cover STOP/SINGLE, mid-fill reconfiguration, roll/scan, stale generations, queue pressure
and errors. These tasks retain the specifications. Audit items: 12, 13.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
