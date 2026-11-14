---
id: 31
title: "Provide bounded and actionable gate feedback"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: []
---

# Provide bounded and actionable gate feedback

## Goal

Give agents fast feedback and enough evidence to reproduce each gate result.

## Plan

Reserved paths: none. Reserve paths when claimed.

With the migration, add versioned JSON with check id, status, command, location, duration, baseline and fix hint.
Retain complete logs and bound subprocess runtimes. Print checked revision, dirty state and executed checks.
Measure fast/full cold and warm budgets. Add focused runs that print what remains required for done.
Audit items: 22, 23.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
