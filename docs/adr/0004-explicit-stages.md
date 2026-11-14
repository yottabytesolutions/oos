# 0004: The step is a fixed pipeline of stages with typed I/O

Status: proposed. 2026-11-14.

## Context

`Ui::step` is a 289-line loop over a 130-field `Ui`. Its correctness depends on line order and on side effects
between distant lines. Moving one line has broken the picture more than once.

## Decision

- `Session::step` calls fixed stages in order: intake, change, acquire, records, accumulate, measure, views,
  compose, present, reply.
- Each stage owns its state and takes and returns typed values. No stage reads another stage's state except through
  those values.
- `Session` creates and wires the stages and holds no domain state. It is not a new god object.

## Consequences

- Each stage is testable alone with fakes.
- The data each stage depends on is visible in its signature. Reordering that breaks a dependency fails to compile.
- Some data is passed explicitly that today is read from `self`. That is the point.
