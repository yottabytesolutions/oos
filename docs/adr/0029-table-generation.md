# One table can generate per-field code

- Number: 0029
- Category: code
- State: in force
- Date: 2026-11-18

## Context

The former no-generation rule left about 2,500 lines of per-field patch handling in core/src/change (#47). Setup codecs
and SCPI tables repeat the same shape, making omitted or inconsistent fields likely. The syntax gate cannot inspect
macro expansion, but behaviour tests can exercise every generated row.

## Decision

A single declarative macro_rules table or const descriptor array can generate repeated per-field handling. No procedural
macros or build scripts generate this code. The table remains the source; generated code is not edited. Every row has
behaviour tests for the operations it generates. Size and lint rules apply to the generator and table as written.
Generation does not exempt handwritten logic from limits.

Check: row coverage tests and normal size/lint gate steps; review checks whether the table replaces duplication.

## Alternatives

Handwritten per-field code: rejected because hundreds of repeated patterns obscure omissions. Bring M13/M14 forward:
rejected because cleanup would wait for the largest settings migration. A procedural macro crate: rejected because a
declarative table needs neither that power nor another build unit.

## Consequences

Count generated tables as written for production LOC. Member classification and mapping semantics remain explicit.
