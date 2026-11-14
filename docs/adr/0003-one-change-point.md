# 0003: One change-detection point with requests

Status: proposed. 2026-11-14.

## Context

Three mechanisms decide what reacts to a change: `settings_changed`/`mark_changed` (39 calls), `dirty_map = true`
(16 places) and `hw_stamp` bumps in `ui.rs`, `ui/bode.rs` and `ui/autoset.rs`. A write that skips one leaves stale
state: old frames in the picture, statistics over two settings, segments from before a change.

## Decision

- Once per step, in the change stage: apply commands, `normalize`, then
  `ChangeSet = classify(previous, current) | requests`.
- `classify` is one function per group and destructures every group without `..`. A new field does not compile until it has a class or is marked
  `Unrelated`.
- Requests cover events with no settings difference: CLEAR, RUN and SINGLE edges, Create mask, and
  `Request::Epoch(token)` from an autoset or Bode stage. Stages return requests as values.
- `Generations` is private to `change`. `classify` calls `new_gen` on `acq`; request resolution calls `new_epoch`
  (`epoch` replaces `hw_stamp`).
- An autoset or Bode stage gets its epoch by token: it returns `Request::Epoch(token)`, the next step's
  `ChangeSet::epochs` carries `(token, epoch)`, and the stage accepts only records of that epoch.
- UI state that changes what is acquired or drawn is derived each step as a value, `UiAcq`, and diffed against the
  previous step's value. It is not stored.
- The change stage produces a board-neutral `AcqIntent`. `app/acq_client` maps it to the board's `AcqWant`.
- Run state is handled by transition. A mapping change in STOP redraws from the last record.

Details: architecture.md, [Change detection](../architecture.md#change-detection). The review that shaped this is in
the drawing amendment (`fork/draw-architecture.md`, amendment section).

### Why tokens, not a synchronous epoch writer

An earlier draft had `new_epoch` return the value at once, so autoset could store it in the same call
(`ui/autoset.rs`, as today with `hw_stamp`). Autoset and Bode run in `views`, after `change`. A synchronous writer
needs `&mut` on the change stage from a later stage, which breaks adr/0004. The token costs one step (33 ms) before the
stage's acquisition starts; the stages already wait several batches. Review: upstreamarch HIGH 1, agreed with compress.

## Consequences

- Consumers subscribe to classes, not to call sites. Forgetting a mark is no longer possible.
- The three mechanisms are deleted at the end of migration steps M2 to M4 (`overhaul-migration.md`).
- Each class has a host test: a change of each field in the class resets exactly its consumers.
