# Tasks

Use `NNNN-slug.md`. Choose the next free id across this folder and `done/`.
Ids are zero-padded to four digits in filenames and branch names. Never reuse or delete an id.
One task tracks one feature, fix or chore. It may span several commits and branches.

## Format

Front matter has exactly these keys:

```yaml
---
id: 7
title: Golden pictures
kind: feature
status: planned
release: 0.2.0
owner: none
depends: []
---
```

- `id`: integer equal to the filename number.
- `title`: short text.
- `kind`: `feature`, `fix`, `chore` or `docs`.
- `status`: `proposed`, `planned`, `active`, `blocked`, `done` or `dropped`.
- `release`: a version in [releases.md](../releases.md), or `none`.
- `owner`: agent or person, or `none`. An active task needs an owner.
- `depends`: list of existing task ids. No cycles. A done task depends only on done tasks.

The body has `## Goal`, `## Plan` and `## Status`.
Write the Plan before code. Use dated Status lines, newest first. Link to specifications instead of copying them.
Keep input material in `notes/`. Notes are not tasks.

## Claims

Claim before work. Set `status: active` and `owner`. Name each branch `<type>/t<NNNN>-<slug>`
([ADR 0026](../adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)).
In Plan, write a `Reserved paths:` line listing exact files or directory prefixes, or `none`.
Record the contributor/session, branch and base commit in a dated Status line.
Reserve `ui/src/ui.rs`, `ui/src/board/dho/hw.rs` and `ui/src/model.rs` explicitly when touched.
Two active claims must not reserve overlapping paths, including a file inside a reserved directory.

The integration branch is the claim authority. Across machines, the first claim merged wins.
Before editing, obtain an accepted claim there. Serialize release edits and id allocation there too.
The planned `cargo xtask tasks new <slug>` and `cargo xtask tasks claim <id>` use an exclusive `mkdir` lock in
Git's common directory across worktrees on one machine, then commit the claim on the integration branch.
Task front matter records the accepted claim. It is not the lock.
Until [task 0040](0040-task-commands.md) lands, the coordinator serializes these steps manually.
Automatic overlap and schema checks are pending that task.

## Lifecycle

A proposal becomes planned when accepted. Claim planned work to make it active.
Move active work to blocked with a dated reason and the needed next action. Resume it as active after resolution.
A handoff records the new owner, branch, remaining work and reserved paths before work resumes.
A task may be dropped with a dated reason. Keep its file and id.

Finish only after the required checks and review. In the same merge, set `status: done`, add a dated Status line
and move the file to `done/` with its filename unchanged. Keep it there permanently. Use a new task for follow-up work.
Fold lasting facts into permanent docs. Keep dependencies valid across `done/` and this folder.

Release task lists come from task files. Do not duplicate them in [releases.md](../releases.md).
The planned `cargo xtask tasks notes <version>` reads done tasks from `done/` for `CHANGELOG.md`.
