---
id: 30
title: "Make mandatory tests hermetic"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Make mandatory tests hermetic

## Goal

Ensure passing mandatory tests exercise the same behavior without private calibration files.

## Plan

Reserved paths: none. Reserve paths when claimed.

With the migration, use synthetic calibration fixtures. Keep private-calibration validation in an explicit suite.
Normalize configuration, clock and storage inputs. Use locked dependency resolution and report skipped scenarios
and zero-test selections. Audit item: 21.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
