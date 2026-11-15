---
id: 26
title: "Isolate build outputs and host harness runs"
kind: fix
status: active
release: 0.2.0
owner: codex-b
depends: []
---

# Isolate build outputs and host harness runs

## Goal

Prevent concurrent builds and tests from consuming or deleting each other's resources.

## Plan

Reserved paths: tools/, boot/*.sh target-dir handling, ui/tests/ harness helpers except M1's scripted-batch files (ui/src/ui/script.rs, ui/src/ui/batch.rs, ui/tests/golden*).

Before parallel migration, define one target-dir contract for Cargo, coverage, GPU, image and deploy tools.
Verify artifact revisions before consumption. Bind host harnesses to loopback with dynamic ports and per-run
temporary directories. Include VXI-11. Replace sleeps with a readiness handshake identifying the child and build.
Audit items: 10, 11.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-b on branch t0026-harness-isolation by the coordinator, base overhaul.
