# 0001: One owner for settings

Status: proposed. 2026-11-14.

## Context

`Model` (`ui/src/model.rs:900`) has 178 public fields. About 103 places write them directly, and SCPI and HTTP use a
string-keyed setter. Nothing sees all writes, so nothing can say reliably what changed.

## Decision

- `Settings` is one owned struct of groups: channels, timebase, trigger, acquire, display, measure, analysis,
  interfaces.
- Each group lives in its own module under `settings/`, with private fields, accessors, invariants, a command enum
  and `apply`.
- The change stage is the only writer (architecture.md, [Settings and commands](../architecture.md#settings-and-commands)).
- One struct, not one per area: change detection needs one consistent snapshot per step, and a whole-settings clone
  is cheap (a few kB).

## Consequences

- A new setting touches one group module. `classify` does not compile until the field has a class.
- Cross-group effects (a scale change clamping a trigger level) are explicit in `apply` through `Context`.
- The string setter disappears. `store` keeps the `key = value` names for setup files.
- The boundary checker forbids `pub` fields in `settings/` and writes to `Settings` outside the change stage.
