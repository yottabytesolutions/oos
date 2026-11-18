# Extensions use static registered modules

- Number: 0028
- Category: code
- State: target: registry coverage follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Adding a decoder, measurement or analyser page used to require edits to menus, labels, setups, SCPI, feature lists and
several match statements. Each list could omit an extension independently. Static registration replaces that repeated
wiring without introducing runtime module loading into a static binary.

## Decision

Each extension is a type implementing its kind's trait and one static registry entry. No parallel list enumerates the
same extensions. Stored protocol keys, measurement numbers and analyser feature ids never change or get reused after
release. Release images build every registered module; decoder Cargo features are development aids only. There is no
runtime loading.

Check: existing registry-walking uniqueness tests; review. Pinning released ids, complete measurement registration and
checking release feature completeness are target follow-ups.

## Alternatives

Separate lists for menus, SCPI and setups: rejected because an extension can be omitted from one list. Runtime module
loading: rejected because OOS ships as one static binary. Stand-alone analyser page modules: not adopted; registration
covers wiring while page state and menu rows follow the existing UI model.

## Consequences

Existing decoder/analyser registries are in force. The Protocol split across dsp/core/ui remains migration work, not a
second registry.
