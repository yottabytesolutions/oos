# Format, limits, unsafe and ratchets protect the code

- Number: 0006
- Category: policy
- State: accepted
- Date: 2026-11-18

## Context

The original size audit found 47 functions above 100 lines and files of 10,059, 3,437 and 2,152 lines. On 2026-11-17,
the Clippy ratchet still held 1,172 findings. Moving tightening to an integrator stopped working once the merge queue
replaced that role: improvements left unwritten allowed later changes to reuse the old slack (#46).

## Decision

Use the pinned stable toolchain in `rust-toolchain.toml` and root `rustfmt.toml`. Declare lints in
the root `[workspace.lints]`; members inherit them and docs/development.md lists them. Normal builds warn; the gate counts
findings. Limits are 400 lines per file, 60 code lines per function, 12 fields per struct and inherent impls in at
most 3 files. File lines exclude tests and
comment-only lines, including doc comments, but count blank lines. Function lines contain tokens; blank lines and
comments do not count. Tests are exempt from syntax size limits.

Unsafe is confined to the paths in `xtask/data/architecture.txt`; each unsafe block or function states its invariant in
a SAFETY comment. `oos-core` and `oos-dsp` contain no unsafe code. Item-level `allow` or `expect` attributes carry a
reason. No crate-wide allows.

Existing excess lives in `xtask/data/<crate>/*.txt`. Counts only shrink and coverage floors only rise. The author
commits `cargo xtask ratchet` when a count falls; nobody edits its output by hand. The gate rejects loosening and must
reject uncommitted tightening for files touched since the protected base, including local changes. Untouched files
keep their allowances. A stricter lint set needs an owner-approved record and one baseline commit.

Check: `fmt`, `clippy`, `size`, `architecture`, `unsafe`, `mutable-statics`, `method-exceptions`, `test-time`,
`naming` and `coverage`; review checks SAFETY, exception reasons and the reviewed Rust style rules.

Ratchet checks reject unused slack in touched files and
direct the author to run `cargo xtask ratchet`.

## Alternatives

A scheduled job: rejected because slack remains until it runs and its tightening needs another review. Let the merge
queue tighten: rejected because it would create unreviewed commits. Require zero warnings first: rejected because 1,172
existing findings would block incremental cleanup.

## Consequences

All rules are in force. Two branches lowering the same entry must rerun ratchet after rebasing rather than merge
generated counts by hand.

## Readability baseline, issue #198

The owner-approved issue #198 brief adds pedantic and selected idiom lints plus glossary-based declaration checks.
`cargo xtask ratchet --init-style` generates the first counts for these newly enabled rules on existing files.
`xtask/data/style-lints.txt` names the measured new lint kinds. Existing lint kinds keep their prior limits, and
new source files receive no initial allowance. The transition is available only while this marker is absent at
the protected base. Once integrated, normal shrink-only checks apply, including to the new naming kind.
This is the single baseline commit required above, not a general way to raise ratchets.

Numeric DSP/register casts, numeric literal grouping, exact float comparisons, blanket `must_use` suggestions and
broad const-API conversion are allowed with reasons in Cargo.toml. Other existing findings are frozen; zero-count
rules apply immediately. Per-lint measurements and decisions live in task 0097. No firmware behavior changes.
