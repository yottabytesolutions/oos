---
id: 41
title: "Land the overhaul guardrails baseline"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: [23]
---

# Land the overhaul guardrails baseline

## Goal

Complete the original guardrails phase before migration work starts.

## Plan

Reserved paths: none. Reserve paths when claimed.

Integrate the guardrails branch. Keep rustfmt setup and per-crate mechanical formatting separate from behavior
changes. Enforce the lint set, size ratchets, coverage floors, architecture boundaries and debug/release tests.
Provide cargo xtask check, hooks, CI and docs/development.md. Include ARM64 and performance checks in the required
inventory. Follow the [formatting ADR](../architecture/adr/0009-formatting-and-lint.md) and
[size ADR](../architecture/adr/0010-size-limits.md). This task preserves the former overhaul phase 1.

## Status

- 2026-11-14: guardrails work is on a separate branch. This worktree starts before that merge; no xtask or hook exists here.
