# 0013: Measure both draw paths at M12 and keep one

Status: accepted. 2026-11-17. Incorporates the retired ADR 0004.

## Context

Standard draws queued frames on the UI thread using row bands across cores. Batch lends read-out buffers to deposit
workers and merges their maps. Host tests show the same picture for the same frames.

On a DHO924S (2026-11-15, 100 kHz square wave on CH1, Auto trigger), Batch drew more acquired frames but refreshed at
18.2 fps with four channels at 10 us/div, against Standard's 26.1 fps. The cause is not yet known. Standard remains
the default; Acquire > Draw path, `:SYSTem:PIPeline` or `acq.pipeline = batch` selects Batch.

## Decision

- M9 builds whole `BatchUnit`s into the accumulator (architecture/0005 and architecture/0006) alongside Standard.
  Frames of one mapping key queued in a step form one unit; a single-frame step forms a one-frame unit.
- Keep both paths until M12 measures picture equality, performance ratios and unit round 5 on the scope. Select the
  winning path from that evidence and delete the losing path. Do not delete Standard before the cause of Batch's
  drop to about 18 fps with four channels at 10 us/div is found.
- The planned single composition consumes `BatchUnit`s whether workers or the UI thread deposit them. The pipe
  also supplies `RecordBatch` off the fast path; those records use the same accumulator as one pseudo-batch.
- Keep the window bounds of N frames and 67 ms, and the 3-batch cap. With one unit per step the age cap binds first.

## Consequences

Removal of route D's `draw_frame` composition waits for the M12 decision; it is not an M9 deletion. Measure both
one-frame units with full `BatchMeta` and 48-frame steps: one unit per frame would require two full-map passes per
frame at merge, about 2.5 GB/s at 48 frames per step. Hardware performance is not inferred from host equality.

## References

[Drawing](../drawing.md), [picture](../picture.md), [migration](../migration.md),
[ADR 0030](../../adr/process/0030-this-tree-is-a-hard-fork-of-oos.md).
