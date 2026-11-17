# ADR 0032: Lints and limits are per crate and ratchet down only

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

ADR 0017 planned Clippy with `-D warnings`, three crate-wide allows (`neg_cmp_op_on_partial_ord`,
`needless_range_loop`, `too_many_arguments`) and a one-time clean-up; it rejected a ratchet. This tree took the other
road ([architecture/0009](../../architecture/adr/0009-formatting-and-lint.md), [architecture/0010](../../architecture/adr/0010-size-limits.md)):
a stricter lint set in each crate's `[lints]` table, enforced by counts in `xtask/data/` that may only shrink. On
2026-11-17 the Clippy ratchet holds 1172 findings (core 46, dsp 265, fpgad 19, hal 174, ui 668, xtask 0) and has
shrunk with every integration.

A clean-up to zero before enforcing would block the restructuring for weeks. A ratchet lets every change leave the
code no worse and most changes leave it better.

## Decision

- Each crate's `Cargo.toml` holds its `[lints.rust]` and `[lints.clippy]` tables. The set is the one in
  architecture/0009 and [docs/development.md](../../development.md) (What each check enforces). Lints are warnings to
  rustc and Clippy, so an ordinary build runs; `xtask` itself denies them.
- The gate's `clippy` step runs Clippy per crate on the host and for `aarch64-unknown-linux-musl` and counts
  findings per crate, file and lint. The counts may not exceed `xtask/data/<crate>/clippy.txt`.
- Every ratchet file (`clippy.txt`, `size.txt`, `statics.txt`, `unsafe.txt`, `allow.txt`, `known.txt`,
  `test-time.txt`) only shrinks, and coverage floors only rise, compared with their version at the merge base with
  `main`. A branch never edits them; the integrator runs `cargo xtask ratchet` after every integration to write
  the improvements back.
- An exception is `#[allow]` or `#[expect]` on the item with the reason next to it. No crate-wide allows.
- There is no widening path. The one exception that may come is the introduction of a stricter lint set: a later
  phase adds it with its own record, which defines one authorised baseline commit that writes the new counts. That
  record supersedes this point; nothing else widens a ratchet.
- A new toolchain that brings new findings is fixed in its own commit before the toolchain pin moves.

## Alternatives

- **ADR 0017's `-D warnings` after a clean-up:** stops the restructuring until the count is zero, and its crate-wide
  allows hide the lints this tree wants to see.
- **Warnings shown, not counted:** the count only grows.
- **One workspace lint table now:** planned with the strict set; until then the per-crate tables differ only where a
  crate needs it (`missing_debug_implementations` in `hal` and `dsp`).

## Consequences

- A new finding fails the gate; fixing an old one in the same file makes room. Counts are per file, not per place
  (docs/development.md, Known limits).
- The ratchet files tell how much is left to fix per crate and file.
- Moves of code keep their allowance only through `xtask/data/moves.txt`.

## References

`xtask/src/clippy.rs`, `xtask/src/base.rs`, `xtask/src/steps/ratchets.rs`, `xtask/data/`, `clippy.toml`,
[docs/development.md](../../development.md) (Ratchets), architecture/0009, architecture/0010, ADR 0017
(superseded), ADR 0031.
