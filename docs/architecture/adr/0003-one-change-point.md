# 0003: One change-detection point with requests

Status: accepted. 2026-11-14.

## Context

Three mechanisms decide what reacts to a change: `settings_changed`/`mark_changed` (39 calls), `dirty_map = true`
(16 places) and `hw_stamp` bumps in `ui.rs`, `ui/bode.rs` and `ui/autoset.rs`. A write that skips one leaves stale
state: old frames in the picture, statistics over two settings, segments from before a change.

## Decision

- Once per step, in the change stage: apply commands (outbox, store, remote, input), `normalize`, then
  `ChangeSet = classify(previous, current) | requests`.
- `classify` is one function per group and destructures every group without `..`. A new field does not compile
  until it has a class or is marked `Unrelated`. The same destructure builds `MappingKey` from the `mapping` fields.
- Internal writers (SINGLE or segments done to STOP, autoset and Bode steps, `auto_step`) do not write `Settings`.
  They put commands and requests into an outbox that the next step's change stage applies first.
- Requests cover events with no settings difference: CLEAR, RUN and SINGLE edges, Create mask, and
  `Request::Epoch(token)` from an autoset or Bode stage. Stages return requests as values.
- `Generations` is private to `change`. `classify` calls `new_gen` on `acq` and raises `stats_gen` on `stats`;
  request resolution calls `new_epoch` (`epoch` replaces `hw_stamp`).
- An autoset or Bode stage gets its epoch by token: it returns `Request::Epoch(token)`, the next step's
  `ChangeSet::epochs` carries `(token, epoch)`, and the stage accepts only records of that epoch. Conditions
  (compress):
  1. The settings commands a stage emits with its token are applied in the same change step, before the epoch, so
     `AcqIntent` carries the new `gen` and `epoch` together.
  2. An epoch-only change is an acquisition boundary: the thread drops a batch armed ahead and queued frames of the
     older epoch.
  3. A token without an epoch accepts nothing. SINGLE arm, CLEAR and RUN after STOP are seen in the change stage
     itself and get their epoch in the same step; they never go through a later stage.
  Tokens are unique per request; several requests in one step may share one epoch.
- UI state that changes what is acquired or drawn is derived each step as a value, `UiAcq`, and diffed against the
  previous step's value. It is not stored.
- The change stage produces a board-neutral `AcqIntent`, which carries the pipe's `route` tag; the thread echoes it
  into every `BatchMeta`. The current mapping goes to the pipe in its own cell, `DepositView`.
  `app/acq_client` maps the intent to the board's `AcqWant` through the pure `Board::want`.
- Run state is handled by transition. A mapping change in STOP redraws from the last record.

Details: ../README.md, [Change detection](../README.md#change-detection). The review that shaped this is in
the drawing amendment (`../drawing.md`, amendment section).

### Why tokens, not a synchronous epoch writer

An earlier draft had `new_epoch` return the value at once, so autoset could store it in the same call
(`ui/autoset.rs`, as today with `hw_stamp`). Autoset and Bode run in `views`, after `change`. A synchronous writer
needs `&mut` on the change stage from a later stage, which breaks adr/0004. The token costs one step (33 ms) before the
stage's acquisition starts; the stages already wait several batches. Review: upstreamarch HIGH 1, agreed with compress.

## Consequences

- Consumers subscribe to classes, not to call sites. Forgetting a mark is no longer possible.
- The three mechanisms are deleted at the end of migration steps M2 to M4 (`../migration.md`).
- Each class has a host test: a change of each field in the class resets exactly its consumers.
