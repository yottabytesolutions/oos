# ADR 0039: rustfmt formats; lints and limits ratchet down; the author of a change tightens

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

Three records described how the code is held to a shape: ADR 0016 (rustfmt), ADR 0017 (Clippy as errors after a
clean-up, superseded by ADR 0032) and ADR 0032 (per-crate lints and ratchets). ADR 0016 is mostly the story of a
rollout that is done.

ADR 0032 said a branch never edits a ratchet file and the integrator runs `cargo xtask ratchet` after every
integration. Since the hard fork there is no integrator: pull requests land through the merge queue (ADR 0040), and
nobody writes the improvements back (#46). An improvement that is not written back leaves slack: a later change in
the same file may add a finding up to the old count.

On 2026-11-17 the Clippy ratchet holds 1172 findings (core 46, dsp 265, fpgad 19, hal 174, ui 668, xtask 0).

Ratchet ownership decided by the owner on 2026-11-17 (#46, #50).

## Decision

- **Formatting.** All Rust code is formatted by `cargo fmt` with the root [rustfmt.toml](../../../rustfmt.toml),
  stable options only (`max_width = 120`, `use_small_heuristics = "Max"`). The gate's `fmt` step runs
  `cargo fmt --check` per crate. A table whose layout carries meaning may be `#[rustfmt::skip]` with a comment that
  says why.
- **Lints.** Each crate's `Cargo.toml` holds its `[lints.rust]` and `[lints.clippy]` tables
  ([architecture/0009](../../architecture/adr/0009-formatting-and-lint.md)). Lints are warnings to rustc and Clippy,
  so an ordinary build runs. The gate runs Clippy per crate on the host and for `aarch64-unknown-linux-musl` and
  counts findings per crate, file and lint.
- **Ratchets.** The ratchet files in `xtask/data/` (`clippy.txt`, `size.txt`, `statics.txt`, `unsafe.txt`,
  `allow.txt`, `known.txt`, `test-time.txt`) only shrink and the coverage floors only rise, compared with their
  version at the merge base with `main`. The gate rejects any loosening.
- **The author tightens.** A pull request that lowers a count runs `cargo xtask ratchet` and commits the result in
  the same pull request: with `--move` when it lists moves in `xtask/data/moves.txt`, with `--coverage` when it
  raised coverage. The tool only tightens. Editing a ratchet file by hand is not allowed.
- **Exceptions** are `#[allow]` or `#[expect]` on the item with the reason next to it. No crate-wide allows.
- **No widening path.** A stricter lint set may come with its own record, which names one baseline commit that
  writes the new counts. Nothing else widens a ratchet.
- A new toolchain that brings new findings is fixed in its own commit before the toolchain pin moves.

## Alternatives

- **A scheduled job on `main` opens a ratchet pull request:** slack stays open until the job runs, and the job's
  pull request needs a review of its own.
- **The merge queue tightens:** the queue would write commits nobody reviewed.
- **ADR 0017's `-D warnings` after a clean-up:** blocks the cleanup until the count is zero.

## Consequences

- Layout is not a review topic. A review comment about layout means the settings are wrong; change this record.
- A new finding fails the gate; fixing an old one in the same file makes room. Counts are per file, not per place
  ([development.md](../../development.md), Known limits).
- Two pull requests that both lower the same file's count conflict on its ratchet line. The second rebases and
  runs `cargo xtask ratchet` again; it never merges the line by hand.
- The gate reports an entry that shrank but does not fail on it. Review asks for the ratchet commit. Worth a second
  look if slack keeps reaching `main`: then the gate fails on an untightened entry.

## References

[rustfmt.toml](../../../rustfmt.toml), `xtask/src/clippy.rs`, `xtask/src/base.rs`, `xtask/src/tighten.rs`,
`xtask/data/`, [development.md](../../development.md) (Ratchets, Moves), architecture/0009, architecture/0010,
ADR 0016, ADR 0017 and ADR 0032 (superseded), ADR 0031, issue #46.
