---
id: 22
title: "Define migration integration and independent review"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: []
---

# Define migration integration and independent review

## Goal

Publish integration order, permitted parallel areas and review responsibility before parallel migration.

## Plan

Reserved paths: none. Reserve paths when claimed.

Serialize hot-file and layout steps. Resolve conflicts with affected owners. Merge reviewed branches without rebasing.
Rerun the gate on the integrated result. Add CODEOWNERS per area and a PR template with task, boundaries,
failing-before evidence, golden changes and hardware evidence. Require an independent adversarial review
by a different agent or vendor before merge. Keep the owner accountable for acceptance.
Audit items: 5, 6, 20.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
