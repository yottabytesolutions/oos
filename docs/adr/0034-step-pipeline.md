# The step is a fixed pipeline of stages

- Number: 0034
- Category: code
- State: target: M16 (F7 supplies picture state)
- Date: 2026-11-18

## Context

Ui::step was a 289-line loop over a 130-field Ui in the original audit. Correctness depended on side effects between
distant lines, and moving a line had broken the picture more than once. Named stages make that ordering and each state
owner explicit instead of relying on incidental statement order.

## Decision

A full step calls intake, change, acquire, records, accumulate, measure, views, compose, present, reply in that order.
Each stage owns its state and exchanges typed values. Session holds only its stages and their channels.
Settings readers use the single snapshot taken by that step's change stage. Picture conversion and placement
instead take BatchMeta and Projection, never live Settings or Model, as specified by the
[frame pipeline](../architecture/frames.md). F7 supplies the picture-state part of M16b.

Check: `architecture` layer rules per stage and order/snapshot tests required by M16.

## Alternatives

Keep one large step and rely on statement order: rejected because distant side effects already broke drawing. Stages
sharing mutable Ui: rejected because named functions alone would not establish ownership or typed contracts.

## Consequences

Migrating Ui into Session moves state and wiring across stages; order and snapshot tests must guard each move.
