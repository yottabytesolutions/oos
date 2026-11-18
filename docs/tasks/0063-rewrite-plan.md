---
id: 63
title: "Hybrid plan: migrate through M12, rebuild menus and SCPI spec-first"
kind: docs
status: active
release: none
owner: claude
depends: []
---

# Hybrid plan: migrate through M12, rebuild menus and SCPI spec-first

## Goal

Record the coordinator's hybrid decision as ADR 0038 (proposed): keep the strangler migration through M12 with
M2.3d and M2.3e, fold an owned `AcqRequest` into M5, and after M12 rebuild the menus and the SCPI parser from a
specification while settings, commands and views migrate. Add the soft freeze and scope the spec work. Review tier:
architecture.

## Plan

Reserved paths: `docs/adr/0038-menus-and-parser.md`, `docs/adr/README.md`,
`docs/architecture/spec-gaps.md`, `docs/architecture/migration.md` (M2.3 regression note, M5 `AcqRequest`, M15 and
M17 pointers), `docs/process/delivery.md` (Soft freeze), this task, `target/COMMIT_MSG.txt`.

1. ADR 0038: context, the hybrid decision, consequences, rejected alternatives (full rebuild with the reviewer's
   numbers and the task 0044 evidence; pure strangler for menus and remote).
2. `migration.md`: the deep-memory step regression note under M2.3 with task 0062 and the smoke log path; the owned
   `AcqRequest` in M5; one-line pointers to ADR 0038 in M15 and M17.
3. `delivery.md`: Soft freeze.
4. `spec-gaps.md`: scoped to menus, overview, SCPI, the setup keys M13 needs and the `:SYSTem:STATe?` schema.
5. No code, no commits, no hardware access. One section in `target/COMMIT_MSG.txt`.

Follow-ups after approval: task files for the five spec documents; `extending.md` notes for menu items and SCPI
commands once the rebuilds are accepted.

## Status

- 2026-11-16: reworked to the hybrid after the independent architecture review rejected the full-rebuild draft.
  Removed `rebuild.md` and the A/N/X/L sequence; reverted the supersession and freeze edits in `AGENTS.md`,
  architecture `README.md`, `integration.md` and `migration.md` (which also removes the broken Rebuild column in the
  Fable performance table). The ADR is renamed to `0017-hybrid-after-m12.md`. `releases.md` unchanged: 0.2.0 stays
  at M12. Regression figures cited from the M2.3b smoke log, not copied as evidence. Uncommitted; waiting for
  architecture review and the owner's approval of ADR 0038.
- 2026-11-16: first draft (full rebuild on a kept hardware adapter) on `t0063-rewrite-plan` from `overhaul`
  dca35ed, uncommitted. Rejected by the independent architecture review and the coordinator.
