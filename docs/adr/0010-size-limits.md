# 0010: Size limits with a ratchet

Status: proposed. 2026-11-14.

## Context

47 functions exceed 100 lines. `ui.rs` has 10,059 lines, `hw.rs` 3,437, `model.rs` 2,152. Large units hide the
dependencies that broke the picture, and they are where agents make line-order mistakes: an agent edits what fits in
its context, and a 10k-line file does not.

## Decision

- New code: at most 400 lines per file (test modules excluded), 60 lines per function, about 12 fields per struct.
- Existing violations are listed in an allowlist with their current size. An entry may only shrink or disappear. The
  checker fails if a listed item grows or a new one appears.
- The same limits and checker are used for auxout's crates.

## Consequences

- Splitting is part of each migration step. The allowlist shows progress.
- 12 fields is a guide, not a hard limit, for data records such as `BatchMeta`. Exceptions go on the allowlist with a
  reason.
