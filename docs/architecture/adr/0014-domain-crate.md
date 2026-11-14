# 0014: The domain is its own crate, oos-core

Status: accepted. 2026-11-14 (team-lead decision, from archreview).

## Context

The first draft placed the domain as modules inside `oos-ui` and enforced its direction with a rules file. A rules
checker over module paths has gaps (overlapping prefixes such as `picture::window` against `picture`, `super::` paths,
macro bodies), and the shared checker (auxout) works on top-level modules only.

## Decision

- A workspace crate `oos-core` holds the domain:
  - settings, their commands and `normalize`;
  - the change stage: `classify`, `Generations`, `ChangeSet`, `MappingKey`, `AcqIntent`, `PipeView`;
  - the acquisition data types: `BatchMeta`, `Frame`, `MeasRead`, `Record`, and the LA record type that `Frame`
    carries (today `ui::la::LaRec`, `board/mod.rs` `Frame::la`), which moves with it;
  - the picture model: window bounds, decay, weights;
  - `Config` with a pure `Config::parse`; `oos-ui` does the reading.
- `oos-core` depends on `oos-dsp` and std only. `oos-ui` depends on it.
- Ownership inside the domain is enforced by visibility. The rules file covers only what Cargo and visibility cannot:
  adapter no-cross, stages against adapters, the statics allowlist and size limits.

## Consequences

- Cargo enforces the direction. A domain module that names `oos_hal`, `libc` or `board` does not compile.
- The domain's tests run on the host without the UI crate.
- Types the acquisition thread and the UI share (`BatchMeta`, `Frame`) live in `oos-core`, so `board/dho` depends on
  `oos-core` too, which is the right direction.
- The crate split lands with M2, so its rules hold from the first migration step.
- The module checker resolves `super::` paths against the file path, so the gap named above does not reopen inside
  `oos-ui`.
