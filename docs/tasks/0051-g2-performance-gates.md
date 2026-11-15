---
id: 51
title: "G2: performance gates"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: []
---

# G2: performance gates

## Goal

Gate the performance budgets before M3c, as [migration G2](../architecture/migration.md#g2-performance-gates) specifies.

## Plan

Reserved paths: none. Reserve paths when claimed.

Tail-aware perf guards (p99 over repeated runs against a reference path, beside the best-of-rounds ratios); a counting-allocator harness for the hot paths asserting zero allocations per frame after warm-up (reuse M1's ui/tests/soak.rs allocator); budget rows from the architecture's table that the host can measure. Unit rows go to the next unit round.

## Status

- 2026-11-15: seeded by the coordinator.
