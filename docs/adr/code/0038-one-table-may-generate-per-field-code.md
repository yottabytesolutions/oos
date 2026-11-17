# ADR 0038: One declarative table may generate per-field code when tests cover every item

- Status: accepted
- Date: 2026-11-17
- Category: code (touches process: the gate)

## Context

The migration plan ([migration.md](../../architecture/migration.md), M2) wrote `classify` and the per-field change
code by hand: "No macro generates them (macro bodies are a blind spot of the gate)". The gate reads unexpanded code
([development.md](../../development.md), Known limits), so a size or lint rule does not see what a macro produces.

The cost of that rule is now measured: `core/src/change` holds about 2500 lines of per-field patch code, the same
few lines repeated per field (#47). The setup-file codec (M13, M14) and the SCPI table (M15) have the same shape.
Repetition is where a field gets forgotten or handled differently from its neighbours.

The gate's blind spot is about structure. Behaviour is still seen when tests run every generated item.

Decided by the owner on 2026-11-17 (#47, #50).

## Decision

- Code that repeats per field may be generated from one declarative table: a `macro_rules!` invocation that lists
  the fields, or a data table (a `const` array of descriptors) that generic code walks.
- Tests cover every generated item: a test walks the same table, so a new row is tested without a new test. A row
  the tests do not reach is a review finding.
- The table is the single place a field is listed. The generator stays small and readable on its own; the size and
  lint rules apply to it as written.
- No procedural macros and no build scripts for this. Both hide the generator in another crate or step.

## Alternatives

- **Hand-written code per field (the old rule):** the gate sees every line, and a reviewer reads the same pattern
  hundreds of times.
- **Bring the settings groups of M13 and M14 forward:** shrinks the code too, but blocks the cleanup on the largest
  step of the migration.
- **A procedural macro crate:** more power than a table needs, and a second crate to build and review.

## Consequences

- `core/src/change`, the setup-file codec and the SCPI table can shrink to a table plus one generator each.
- The size limit counts the table, not the expansion. Coverage counts the expansion, so a row without a test shows.
- A compile error inside a macro expansion is harder to read than one in plain code. Keep generators simple.
- Worth a second look if a generator grows its own logic per row: then the table has become a language.

## References

[migration.md](../../architecture/migration.md) (M2), [development.md](../../development.md) (Known limits),
ADR 0039, issues #47 and #50.
