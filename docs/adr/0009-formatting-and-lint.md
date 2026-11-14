# 0009: Formatting and lint are enforced

Status: proposed. 2026-11-14.

## Context

No formatter or linter is enforced. Style differs by file and author, and review time goes to it.

## Decision

- `rustfmt.toml` at the root, `cargo fmt --check` in the gate. One mechanical format commit per crate, nothing else
  in those commits.
- Clippy at `-D warnings` for all targets, with `clippy::pedantic` selectively: `cast_lossless`, `needless_pass_by_value`,
  `redundant_closure_for_method_calls`, `semicolon_if_nothing_returned`, plus `too_many_lines` (threshold 60) and
  `too_many_arguments` (threshold 7). `unwrap_used` and `expect_used` are denied outside tests.
- Exceptions are `#[allow]` on the item with a reason comment. No crate-wide allows beyond those in the root
  `Cargo.toml` `[workspace.lints]`.

## Consequences

- The branch diverges from upstream in formatting. Accepted in `overhaul.md`.
- Phase 1 owns the exact lint set. This ADR is updated when it changes.
