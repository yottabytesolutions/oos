# 0006: A batch is the unit of drawing

Status: proposed. 2026-11-14.

## Context

Shares of one batch drawn by different workers reached different pictures. Partial batches gave brightness steps and
flicker. Batches from old settings were drawn after a change.

## Decision

- A batch carries its `BatchMeta`: `gen`, `epoch`, the acquisition snapshot and the trigger settings. Its frames
  convert from it, never from `Settings`.
- Its shares are published together as one `BatchUnit`. The accumulator adds whole units only.
- Minimum persistence: a window of whole batches, at most 3 batches or 67 ms (user decision, option C).
  Persistence: decay in u32 16.16. STOP freezes.
- A unit of another mapping key is counted as stale and dropped at merge. A unit of an older `gen` under the same
  mapping key is a valid picture.

## Consequences

- No partial pictures, measurable by the PARTIAL and STALE counters from draw step 0.
- Latency after a change is bounded by one batch plus the preview of the last record.
