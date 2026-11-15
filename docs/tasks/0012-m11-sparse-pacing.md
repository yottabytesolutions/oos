---
id: 12
title: "M11: sparse-event pacing (D8)"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: [11]
---

# M11: sparse-event pacing (D8)

## Goal

Complete [migration M11](../architecture/migration.md#m11-sparse-event-pacing-d8). The migration document is the specification.

## Plan

Reserved paths: none. Reserve paths when claimed.

Follow [migration M11](../architecture/migration.md#m11-sparse-event-pacing-d8), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

## Status

- 2026-11-15: Task 0037 handoff, host-only reproduction at `1fe7150`, unchanged `ui/src/pacing.rs`.
  A disposable Rust probe copied the current `Fill` constants/implementation and the existing `run` helper's
  synthetic-Instant sequence: 30 Hz steps, initial start 2 ms after the phase origin, constant rest 24 ms,
  worker readiness 5 ms (plus the existing 3 ms margin), next start 0.5 ms after each read, 60 batches.
  After 20 warm-up batches, fills `[20..30]` were `[9, 9, 9, 9, 9, 8, 9, 9, 9, 9]` ms and reads were
  `[725, 758.5, 792, 825.5, 859, 891.5, 925, 958.5, 992, 1025.5]` ms. Their gaps were
  `[33.5, 33.5, 33.5, 33.5, 32.5, 33.5, 33.5, 33.5, 33.5]` ms. The strict assertion
  `reads[20..].windows(2).all(|w| w[1] - w[0] <= 1000.0f32 / 30.0)` failed, exit 101.
  M11 owns `sparse_rest_24ms_respects_period`: resolve deadline/whole-millisecond rounding precision, then pin
  multiple start phases, bounded rest jitter and ready-before-step deadlines, retaining startup/no-phase and
  impossible-rest cases. Do not add tolerance solely to pass. This is not AUTO-timeout or hardware evidence;
  task 0037 changes no pacing code or tests. No unit used.
- 2026-11-14: seeded from task 0001. Not yet claimed.
