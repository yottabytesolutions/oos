# One accumulator owns the picture and accepts whole deposits

- Number: 0035
- Category: code
- State: target: F0-F7 (F4 = M12b; F7 supplies M16b picture state)
- Date: 2026-11-18

## Context

The drawing audit found six record-to-phosphor routes with two weight models. Worker shares could reach different
pictures, and old settings could reappear after a change. Summing full worker maps adds traffic proportional
to worker count. The [frame design](../architecture/frames.md) replaces those maps with column differences.

## Decision

Only the accumulator writes Picture maps, weights and window; consumers read &Picture. Use one shared
placement/difference kernel and publish whole-batch Deposit values. The canonical
[frame specification](../architecture/frames.md) defines the types, projection identity, independent record taps,
capacity, weights, window lifetime and publication. Source back-pressure follows ADR 0036.

Check: the specification's [tests](../architecture/picture.md#tests) and
[M12 acceptance](../architecture/frames.md#m12-acceptance), implemented through
[F0-F7](../architecture/migration.md#f0-f7-frame-pipeline). Existing counters do not establish target completion.

## Alternatives

Publish shares separately: rejected because partial batches change brightness and flicker. Worker-map reduction:
rejected because its traffic grows with worker count. Convert from live settings: rejected because queued
records carry older acquisition snapshots. Three-ring-read window cap: rejected because several reads per picture
can omit records. Read and drop under load: rejected because it wastes IO and hides capture-rate loss.

## Consequences

Standard remains the equality oracle until authorized unit acceptance permits its deletion. The
[migration plan](../architecture/migration.md#f0-f7-frame-pipeline) owns sequencing, files, acceptance and risk.
Target performance remains unverified on hardware.
