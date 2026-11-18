# One accumulator owns the picture and adds whole batches

- Number: 0035
- Category: code
- State: target: M9 and host Batch-default alignment in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The drawing audit found six frame-to-phosphor routes with two weight models; brightness and flicker differed by route.
Worker shares of one batch could reach different pictures, and batches from old settings could appear after a change.
Whole-unit publication and conversion from batch metadata address those specific failures. Verified on the unit:
DHO924S, 2026-11-15, four-channel Auto at 10 us/div refreshed at 18.2 fps on Batch versus 26.1 on Standard,
although Batch drew more acquired frames. That result requires M12 measurement before deleting either path.

## Decision

Only the picture accumulator writes phosphor maps, weights and the window. Compose reads by reference. A batch carries
BatchMeta: generation, epoch, acquisition snapshot and trigger settings. Convert from it, never from current Settings.
Publish all shares together as BatchUnit; accumulate only whole units. Minimum persistence is a window of whole batches,
at most 3 batches or 67 ms. Decay uses u32 16.16; STOP freezes. Drop and count units with another mapping key. An older
generation with the same key remains valid.

Batch is the chosen default. The image already selects Batch; host/model and invalid-value fallbacks still select
Frames until the separate default-alignment change. M12 measures both paths and removes the loser.

Check: golden pictures, batch conversion tests; PARTIAL and STALE must be zero in the unit round.

## Alternatives

Publish worker shares independently: rejected because partial batches cause brightness steps and flicker. Convert with
current settings: rejected because queued frames belong to their own acquisition snapshot. Keep both drawing paths
indefinitely: rejected; M12 measures them on the unit and deletes the loser.

## Consequences

Existing counters are in force. Whole-batch ownership and window acceptance are M9 targets. Batch is today’s image
default; selecting the surviving draw path remains M12.
