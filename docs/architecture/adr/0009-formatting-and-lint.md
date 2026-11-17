# 0009: Formatting and lint are enforced

Status: accepted. 2026-11-14. Amended 2026-11-17: enforcement and the ratchet model are
[ADR 0032](../../adr/process/0032-lints-and-limits-ratchet-down-only.md).

## Context

No formatter or linter is enforced. Style differs by file and author, and review time goes to it.

## Decision

- `rustfmt.toml` at the root (120 columns, max heuristics), `cargo fmt --check` in the gate. One mechanical format
  commit per crate, nothing else in those commits.
- The lint set is the one the gate implements (`../../development.md`, What each check enforces), identical in every crate:
  `clippy::all`, plus `too_many_lines` (60 code lines, `clippy.toml`), `cognitive_complexity`, `too_many_arguments`,
  `type_complexity`, `large_enum_variant` and `unwrap_used` outside tests. `expect` is allowed with a reason that
  states the invariant. rustc: the `unused` group, and `missing_debug_implementations` in `hal` and `dsp`.
- Lints are warnings to rustc and clippy, so an ordinary build runs. The gate turns findings into failures through
  the ratchet (`xtask/data/clippy.txt`), which only shrinks. `xtask` itself denies all of them.
- Exceptions are `#[allow]` on the item with a reason comment. No crate-wide allows.

## Consequences

- The tree diverges from upstream in formatting. Since the hard fork
  ([ADR 0030](../../adr/process/0030-this-tree-is-a-hard-fork-of-oos.md)) that has no cost.
- The exact lint set changes only through a record. The planned strict set comes with its own record that amends
  ADR 0032 (one authorised baseline commit).
