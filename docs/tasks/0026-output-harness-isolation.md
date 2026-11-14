---
id: 26
title: "Isolate build outputs and host harness runs"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Isolate build outputs and host harness runs

## Goal

Prevent concurrent builds and tests from consuming or deleting each other's resources.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration, define one target-dir contract for Cargo, coverage, GPU, image and deploy tools.
Verify artifact revisions before consumption. Bind host harnesses to loopback with dynamic ports and per-run
temporary directories. Include VXI-11. Replace sleeps with a readiness handshake identifying the child and build.
Audit items: 10, 11.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
