# Menus are data and the SCPI parser comes from one table

- Number: 0038
- Category: code
- State: target: M5, M15, M17
- Date: 2026-11-18

## Context

The menu audit counted about 40 menus and 319 items in an 11,233-line ui.rs; the SCPI parser had about 258 tokens
without a complete reference table. The fresh-core spike measured kernels 3.5-24 times slower and estimated 74-132 days
to feature parity. A full rebuild was estimated at 150-300 adjusted host days versus 70-90 to migrate to M12.

## Decision

Menus are data: label, kind, value reader, command, choices and visibility. One renderer draws all menus; active-choice
highlight is a property of the model. One specification table maps SCPI text to Command and Query; the command reference
is generated from or checked against it. Board takes an owned board-neutral AcqRequest, never a borrow of the settings
model.

Check: every-row parser tests and a differential run against the legacy parser until deletion; menu model/highlight and
Board fake tests.

## Alternatives

Full rebuild: rejected because the spike was slower and delayed the first shipping goal until one switch-over. Pure
strangler for menus/parser: rejected because it would encode incidental legacy behaviour with a bridge per menu or
command family. Settings, commands and views keep their specified incremental migration.

## Consequences

The migration strategy, soft freeze and estimates live in migration.md and process/delivery.md. Settings and views
continue incremental migration.
