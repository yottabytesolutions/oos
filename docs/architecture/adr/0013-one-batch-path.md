# 0013: Frames mode becomes the batch path

Status: accepted. 2026-11-14 (team-lead decision). Amended 2026-11-16: removal of the frames path superseded, see
Amendment.

## Context

Pictures are composed two ways. Batch mode (`acq.pipeline = batch`, the unit's setting) draws shares in the deposit
pipe and merges them. Frames mode (`acq.pipeline = frames`, upstream's default, the host and the no-buffer fallback)
draws each queued frame on the UI thread through `draw_frame` (route D in [current data flow](../drawing.md#current-data-flow-dho804-32041d0)). The
drawing plan kept frames mode as the default to stay close to upstream on `dho804`. On `overhaul` the branch diverges
anyway, and two composition paths are what the overhaul removes.

## Decision

- There is one composition: whole `BatchUnit`s into the accumulator (adr/0005, adr/0006).
- Frames mode produces the same `BatchUnit`. The UI thread deposits it itself when no worker pipe exists, one unit
  per step: the step's queued frames of one mapping key form one unit. Where the hardware delivers a single frame in
  a step, the unit has one frame.
- The window's bounds stay N frames and 67 ms of age. The 3-batch cap (the user's option C) also stays; with one unit
  per step it never binds before the age cap.
- `acq.pipeline` selects only who deposits (the workers or the UI thread), not how the picture is composed.
- The pipe has two outputs to main, in every mode: a `BatchUnit` on the fast path and a `RecordBatch` (decoded
  frames) off it (`../picture.md`, Route per batch). Off the fast path the step's records are drawn as one
  pseudo-batch through the same accumulator.
- This supersedes "frames mode stays the default" in [drawing implementation plan](../drawing.md#implementation-plan) for `overhaul`.

## Consequences

- Route D and its `draw_frame` composition are deleted with M9 (`../migration.md`).
- Frames and batch mode give the same picture for the same frames. M9 tests this.
- One unit per frame would cost 2 full-map passes per frame at merge: 48 frames per step is about 2.5 GB/s. One unit
  per step avoids it.
- The M9 gate has pass/fail perf-guard ratios against today's `draw_frame`, on the host and in frames mode: a
  one-frame unit with full `BatchMeta`, and a 48-frame step.
- Merging back to upstream gets harder in this area. Accepted in [Why](../README.md#why).

## Amendment (2026-11-16, owner decision in task 0066)

The upstream merge brought upstream's Standard draw path: the frames path on several cores (`fast_draw_all`, row
bands, upstream e52cd60c and 11067c59), upstream's default, chosen with Acquire > Draw path. Both paths stay. The
removal of route D's composition in M9 and of the old paths in M12 is superseded: M12 measures both paths on the
unit (picture, perf-guard ratios, unit round 5) and the owner decides which stays, or keeps both behind the
setting. Until then the frames path is not deleted, and M9 builds `BatchUnit` alongside it.
