# 0010: Size limits with a ratchet

Status: accepted. 2026-11-14.

## Context

47 functions exceed 100 lines. `ui.rs` has 10,059 lines, `hw.rs` 3,437, `model.rs` 2,152. Large units hide the
dependencies that broke the picture, and they are where agents make line-order mistakes: an agent edits what fits in
its context, and a 10k-line file does not.

## Decision

- Hard limits for new code, counted by `cargo xtask check` (auxout, phase 1):
  - 400 physical lines per file under `src/`, minus items marked `#[cfg(test)]` or `#[test]`;
  - 60 physical lines per function, comments included, test functions exempt (clippy `too_many_lines` also covers
    tests);
  - 12 fields per struct, named or tuple;
  - a type's inherent `impl` blocks in at most 3 files of its crate (`Ui` is in 33 today).
- Existing violations are listed in an allowlist with their current size. An entry may only shrink or disappear. The
  checker fails if a listed item grows or a new one appears.
- The same limits and checker are used for auxout's crates.

The 12-field and 60-line limits stay hard (user decision). Gate step G1 (auxout): count only code lines for
the function limit (no comments, no blank lines), so that documenting a function never pushes it over.

## Consequences

- Splitting is part of each migration step. The allowlist shows progress.
- No exemptions by kind. A record with more than 12 fields groups them into sub-structs (`BatchMeta` holds an
  `AcqSnapshot`). The only exceptions are allowlist entries, which can only shrink.
- A type that outgrows the limit because it holds variants splits into an enum of per-variant structs
  (`TriggerKind::Edge(EdgeCfg)`), not into an allowlist entry.
