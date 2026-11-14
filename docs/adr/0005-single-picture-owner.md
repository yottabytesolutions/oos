# 0005: One owner of the picture

Status: proposed. 2026-11-14.

## Context

Frames reach the phosphor by six routes (A to F) with two weight models (`fork/draw-architecture.md`, section 2).
Brightness and flicker differ by route, and fixes to one route miss the others.

## Decision

- The `picture` accumulator is the only writer of the phosphor maps, weights and window.
- Every route delivers `BatchUnit`s or records to it. One weight per column per frame.
- `compose` reads the maps by `&`. The GPU path uses the same weights (`vmax = W`).

## Consequences

- Brightness is defined in one place and tested with golden pictures.
- Deposit kernels stay where they are fastest (A53 workers, the helper). Only the accumulation has one owner.
- Details and acceptance: `architecture/picture.md`, `fork/draw-architecture.md`.
