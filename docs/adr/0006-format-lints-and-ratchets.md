# Format, limits, unsafe and ratchets protect the code

- Number: 0006
- Category: policy
- State: target: gate tightening follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The original size audit found 47 functions above 100 lines and files of 10,059, 3,437 and 2,152 lines. On 2026-11-17,
the Clippy ratchet still held 1,172 findings. Moving tightening to an integrator stopped working once the merge queue
replaced that role: improvements left unwritten allowed later changes to reuse the old slack (#46).

## Decision

Use the pinned stable toolchain in `rust-toolchain.toml` and root `rustfmt.toml`. Each crate declares its lints in
Cargo.toml; docs/development.md lists them. Normal builds warn; the gate counts findings. Limits are 400 lines per file,
60 code lines per function, 12 fields per struct and inherent impls in at most 3 files. File lines exclude tests and
comment-only lines, including doc comments, but count blank lines. Function lines contain tokens; blank lines and
comments do not count. Tests are exempt from syntax size limits.

Unsafe is confined to the paths in `xtask/data/architecture.txt`; each unsafe block or function states its invariant in
a SAFETY comment. `oos-core` and `oos-dsp` contain no unsafe code. Item-level `allow` or `expect` attributes carry a
reason. No crate-wide allows.

Existing excess lives in `xtask/data/<crate>/*.txt`. Counts only shrink and coverage floors only rise. The author
commits `cargo xtask ratchet` when a count falls; nobody edits its output by hand. The gate rejects loosening and must
reject uncommitted tightening. A stricter lint set needs an owner-approved record and one baseline commit.

Check: `fmt`, `clippy`, `size`, `architecture`, `unsafe`, `mutable-statics`, `method-exceptions`, `test-time` and
`coverage`; review checks SAFETY and exception reasons. Rejecting unused ratchet slack is a target follow-up, not an
existing check.

## Alternatives

A scheduled job: rejected because slack remains until it runs and its tightening needs another review. Let the merge
queue tighten: rejected because it would create unreviewed commits. Require zero warnings first: rejected because 1,172
existing findings would block incremental cleanup.

## Consequences

All rules except rejection of unused slack are in force. Two branches lowering the same entry must rerun ratchet
after rebasing rather than merge generated counts by hand.
