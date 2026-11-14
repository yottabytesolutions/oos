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
- The change stage is the only writer. It holds `Settings` privately and hands out `&Settings`; group setters are
  `pub(in crate::change)` ([Settings and commands](../architecture.md#settings-and-commands)).
- Cross-group constraints have one model: `Settings::normalize()`, run by the change stage after the step's commands
  and before `classify`. Group `apply` checks only its own group, with read-only `Context`. Today's per-step rewrites
  (`clamp_offsets`, `fit_platform`) move into `normalize`, so they are no longer a second writer outside `classify`.
- One struct, not one per area: change detection needs one consistent snapshot per step, and a whole-settings clone
  is cheap (a few kB).

## Consequences

- A new setting touches one group module. `classify` does not compile until the field has a class.
- Cross-group effects (a scale change clamping a trigger level) live in one place, `normalize`, which is idempotent
  and tested as such. A value it changes is classified like any other (`fit_platform` switching a channel off is `acq`).
- The string setter disappears. `store` keeps the `key = value` names for setup files.
- The compiler enforces single ownership through visibility. No checker rule is needed for it.
