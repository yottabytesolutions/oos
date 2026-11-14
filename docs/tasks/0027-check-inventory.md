---
id: 27
title: "Define and enforce the full check inventory"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: []
---

# Define and enforce the full check inventory

## Goal

Make full gate completion mean the same thing locally, in CI and in review.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration, inventory required GPU host tests, script syntax, simulations and ARM64 musl release
linking of shipped binaries. Run each through cargo xtask check or a named CI job required for done.
Require image assembly validation for boot, kernel, rootfs and build changes. Make missing prerequisites visible.
Audit items: 14, 15.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
