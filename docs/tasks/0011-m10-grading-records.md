---
id: 11
title: "M10: grading and records together (D6)"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: [10]
---

# M10: grading and records together (D6)

## Goal

Complete [migration M10](../architecture/migration.md#m10-grading-and-records-together-d6). The migration document is the specification.

## Plan

Reserved paths: none. Reserve paths when claimed.

Follow [migration M10](../architecture/migration.md#m10-grading-and-records-together-d6), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

M1 review follow-up: the legacy `drawpipe::Pipe::set_view` refreshes its cloned model only when the mapping
key changes, but that key excludes `meas` and `meas2`. Refresh measurement selection independently of mapping
changes. Add a regression that enables, disables and changes cheap measurements between batches with an
unchanged mapping, checking folds and counts against records. Until this is fixed, M1 scripted-batch captures
require both measurement lists empty and reject re-enablement. Remove that constraint only with the regression.

## Status

- 2026-11-15: M1 review round 1 recorded the cached measurement-selection follow-up above. Implementation
  remains planned and unclaimed; M1 enforces measurements off for its scripted batches.

- 2026-11-14: seeded from task 0001. Not yet claimed.
