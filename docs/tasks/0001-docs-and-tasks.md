---
id: 1
title: "Architecture docs in one folder, and a task and release tracker"
kind: chore
status: active
release: 0.2.0
owner: codex-b
depends: []
---

# Architecture docs in one folder, and a task and release tracker

## Goal

- Every architecture document and decision lives under `docs/architecture/`. One place to read, one place to change.
- A plain-text task system in the repo: one file per feature, fix or chore, each assigned to a release. Agents and
  humans see what is planned, active and done without an external tool. The gate keeps it consistent.

## Plan

Reserved paths: `AGENTS.md`, `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `docs/`,
`ui/src/drawpipe.rs`, `ui/src/pacing.rs`, `GEMINI.md`, `.github/copilot-instructions.md`,
`.cursor/rules/oos.mdc`, `CONVENTIONS.md`.

Current scope: implement the documentation on `t0001-docs-tasks` from `overhaul-agents`, as instructed by the owner.
The original sequencing below is retained as design input. Task 0040 carries the deferred xtask commands and checks.
No network, push or hardware access. Do not edit xtask, Cargo files or lint and formatting configuration.

Starts after `overhaul-guardrails` and `overhaul-agents` are merged into `overhaul` (both touch these docs and
`xtask`). One branch `docs-tasks` from that tip, one worktree, one target dir.

### Architecture folder (`git mv` only, then link fixes, in separate commits)

| From | To |
|---|---|
| `docs/architecture.md` | `docs/architecture/README.md` |
| `docs/architecture/{acquisition,picture,records,system}.md` | unchanged |
| `docs/adr/` | `docs/architecture/adr/` |
| `docs/extending.md` | `docs/architecture/extending.md` |
| `docs/overhaul-migration.md` | `docs/architecture/migration.md` |
| `docs/fork/draw-architecture.md` | `docs/architecture/drawing.md` |
| `docs/overhaul.md` | content folds into task files (below), file deleted |

`docs/development.md` (the gate) stays where it is: it is process, not architecture. Fix every relative link and
anchor in the repo (AGENTS.md, CLAUDE.md, README, docs, code comments, `xtask/data`). No content changes in the move
commits.

### Task files

- `docs/tasks/NNNN-slug.md`, `NNNN` = next free id, zero-padded. Front matter, exactly these keys:
  - `id` (integer, equals the file number), `title`;
  - `kind`: `feature` | `fix` | `chore` | `docs`;
  - `status`: `proposed` | `planned` | `active` | `blocked` | `done` | `dropped`;
  - `release`: a version from `docs/releases.md`, or `none`;
  - `owner`: agent or person, or `none`;
  - `depends`: list of task ids.
- Body: `## Goal`, `## Plan` (before code), `## Status

- 2026-11-14: review fixes (docsreview, Claude): restored the measured motivation and goals in the architecture
  README, fixed a stale link, wired the prerequisite tasks into M2 and M4, Cursor rule front matter. The moves,
  link fixes and content share one commit (589ccb8): an accepted exception to the separate-moves rule, because
  the worker could not commit and git detects every move as a rename. Section 4 (task commands, link check) is
  task 0040, after the guardrails merge.
- 2026-11-14: 589ccb8 committed by the coordinator from codex-b's work (the worker sandbox cannot write git
  metadata). Checks: 42 task schemas and dependencies, 304 relative links and anchors, shims, privacy.
- 2026-11-14: claimed by codex-b on t0001-docs-tasks, base b6c7683.
- 2026-11-14: plan written by the coordinator, audit folded in.
