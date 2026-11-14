# 0013: Frames mode becomes the batch path

Status: proposed. 2026-11-14 (team-lead decision).

## Context

Pictures are composed two ways. Batch mode (`acq.pipeline = batch`, the unit's setting) draws shares in the deposit
pipe and merges them. Frames mode (`acq.pipeline = frames`, upstream's default, the host and the no-buffer fallback)
draws each queued frame on the UI thread through `draw_frame` (route D in `fork/draw-architecture.md` section 1). The
drawing plan kept frames mode as the default to stay close to upstream on `dho804`. On `overhaul` the branch diverges
anyway, and two composition paths are what the overhaul removes.

## Decision

- There is one composition: whole `BatchUnit`s into the accumulator (adr/0005, adr/0006).
- Frames mode produces the same `BatchUnit`. The UI thread deposits it itself when no worker pipe exists. Where the
  hardware delivers single frames, a unit is a batch of one frame.
- `acq.pipeline` selects only who deposits (the workers or the UI thread), not how the picture is composed.
- The pipe has two outputs to main, in every mode: a `BatchUnit` on the fast path and a `RecordBatch` (decoded
  frames) off it (`architecture/picture.md`, Route per batch). Off the fast path the step's records are drawn as one
  pseudo-batch through the same accumulator.
- This supersedes "frames mode stays the default" in `fork/draw-architecture.md` section 5 for `overhaul`.

## Consequences

- Route D and its `draw_frame` composition are deleted with M9 (`overhaul-migration.md`).
- Frames and batch mode give the same picture for the same frames. M9 tests this.
- A batch of one frame carries the full `BatchMeta` and unit overhead. The per-frame cost is small against the
  deposit. The M9 gate has a pass/fail perf-guard ratio: a one-frame `BatchUnit` with full `BatchMeta` against
  today's `draw_frame`, on the host and in frames mode.
- Merging back to upstream gets harder in this area. Accepted in `overhaul.md`.
