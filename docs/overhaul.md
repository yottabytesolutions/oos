# Overhaul: target architecture, guardrails, migration

Branch `overhaul`, started 2026-11-14 from the private fork. `dho804` stays frozen at 32041d0, the build running on
the unit (20261114-184352), so the scope keeps working while this is in progress.

## Motivation

A day of fixing display bugs on `dho804` showed the same causes again and again: frames reaching the picture by six
routes with two weight models, batches split across pictures, frames from old settings drawn after a change. Two
independent reviews traced this to structure, not to individual bugs:

- `ui` has no single owner for settings changes. `Model` has 178 public fields, about 103 direct writes and a
  string-keyed setter for SCPI and HTTP. Three separate mechanisms decide what must react to a change. Any write that
  skips one leaves stale state, in the picture and also in measurement statistics, history, segments and decodes.
- `Ui` is a 130-field object over 28 files. `Ui::step` is a 289-line loop whose correctness depends on line order.
  The acquisition thread is one closure of about 890 lines in the fork.
- 47 functions exceed 100 lines; `ui/src/ui.rs` is 8.5k lines upstream. No formatter or linter is enforced.

The crate split (hal, dsp, ui, fpgad, gpu), the `Board` port and the pure, tested `dsp` and `hal` are sound and stay.
The overhaul fixes the inside of `ui` and adds guardrails so the structure cannot erode again.

Consequence, accepted: formatting and restructuring make this branch diverge far from upstream. Cherry-picks between
the two get harder. Upstream PRs from this branch become self-contained proposals, not small patches.

## Goals

1. A fast, responsive UI without hiccups or flicker (acceptance numbers in `fork/draw-architecture.md` section 4).
2. A documented target architecture that others can extend: where new code goes, how a feature crosses the layers.
3. Rules that tools enforce, so humans and agents cannot violate the architecture by accident.
4. Idiomatic Rust, small files, small functions, clear abstractions, good test coverage.

## Plan

Phase 1, guardrails (tooling, no behaviour change):
- `rustfmt.toml`, then one mechanical `cargo fmt` commit per crate, nothing else in those commits.
- Clippy at `-D warnings` with a fixed lint set, including size and complexity lints.
- `tools/check/`: one gate script that every human and agent runs before a commit: fmt check, clippy, tests in
  release and debug, arm64 check, size limits, architecture boundary checks, coverage floor.
- Size limits with a ratchet: new files and functions must stay under the limits; existing oversized ones are listed
  in an allowlist that can only shrink.
- Architecture boundary tests: allowed dependency directions between crates and between `ui` modules.
- Pre-commit hook and CI job that run the gate.

Phase 2, target architecture (documentation, reviewed before code moves):
- `docs/architecture.md` rewritten as the target: layers, ports, ownership, threads, data flow from FPGA to pixels and
  from a knob/SCPI/HTTP to settings, change detection, the accumulator, the record and measurement path.
- Extension guides: how to add a measurement, an SCPI command, a menu item, a setting, a trigger type, a board.
- ADRs for the decisions: one owner per state, one change-detection point, one picture owner, batch as the unit.
- `AGENTS.md` and `CLAUDE.md` at the repo root: the rules, the gate, what is forbidden, where to read first.

Phase 3, migration (strangler, one small step per merge, each through the gate and reviewed):
- Change-detection point and settings ownership (replaces the three mechanisms).
- Acquisition thread split into small units, then the drawing rework from `fork/draw-architecture.md`.
- Break up `Ui` into owned components (input, menus, views, accumulator, interfaces) behind the documented ports.
- Unit rounds on the scope after the steps the drawing plan lists, then the full stress round.

## Status

- 2026-11-14: branch created from the drawing design (0fbda78), its final decision (e08839e), draw step 0 counters
  (3692085, two review fixes open) and the change-detection amendment (7c7816b). `dho804` reset to 32041d0.
