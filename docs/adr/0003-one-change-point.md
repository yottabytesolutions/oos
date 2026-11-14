# 0003: One change-detection point with requests

Status: proposed. 2026-11-14.

## Context

Three mechanisms decide what reacts to a change: `settings_changed`/`mark_changed` (39 calls), `dirty_map = true`
(16 places) and `hw_stamp` bumps in `ui.rs`, `ui/bode.rs` and `ui/autoset.rs`. A write that skips one leaves stale
state: old frames in the picture, statistics over two settings, segments from before a change.

## Decision

- Once per step: `ChangeSet = classify(previous, current) | requests`.
- `classify` destructures every group without `..`. A new field does not compile until it has a class or is marked
  `Unrelated`.
- Requests cover events with no settings difference: CLEAR, RUN and SINGLE edges, Create mask, a retried autoset or
  Bode stage.
- `Generations::new_gen` is the only writer of generations, `hw_stamp` included, and returns the new value at once.
- UI state that changes what is acquired or drawn sits in `UiAcq` and is part of the diff.
- Run state is handled by transition. A mapping change in STOP redraws from the last record.

Details: architecture.md, [Change detection](../architecture.md#change-detection). The review that shaped this is in
the drawing amendment (`fork/draw-architecture.md`, amendment section).

## Consequences

- Consumers subscribe to classes, not to call sites. Forgetting a mark is no longer possible.
- The three mechanisms are deleted at the end of migration steps 1a to 1c.
- Each class has a host test: a change of each field in the class resets exactly its consumers.
