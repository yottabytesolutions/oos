---
id: 22
title: "Define migration integration and independent review"
kind: chore
status: done
release: 0.2.0
owner: codex-c
depends: []
---

# Define migration integration and independent review

## Goal

Publish integration order, permitted parallel areas and review responsibility before parallel migration.

## Plan

Reserved paths: `docs/process/integration.md`, `docs/process/review.md`, `.github/pull_request_template.md`,
`.github/CODEOWNERS`, `docs/tasks/0022-integration-review.md`, `AGENTS_PATCH.txt`, `COMMIT_MSG.txt`.

Derive merge order and parallel work from the migration steps and architecture boundaries. Write the integration
and review protocols, PR evidence fields and placeholder area owners. Supply explicit AGENTS replacements and additions separately.
Check the documents and attempt the full gate offline. Leave the patch and commit message for the coordinator.

Serialize hot-file and layout steps. Resolve conflicts with affected owners. Merge reviewed branches without rebasing.
Rerun the gate on the integrated result. Add CODEOWNERS per area and a PR template with task, boundaries,
failing-before evidence, golden changes and hardware evidence. Require an independent adversarial review
by an agent from another vendor before merge. Keep the owner accountable for acceptance.
Audit items: 5, 6, 20.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: done. merged into overhaul with the 2026-11-15 batch; AGENTS.md patch applied by the coordinator.

- 2026-11-15: applied the follow-up. Local links, anchors, roles, style, patch lines and single-source checks passed.
  `git diff --check` passed. AGENTS.md is unchanged. No network, hardware access, commits or pushes.
  Offline full gate: `CARGO_NET_OFFLINE=true cargo xtask check`, tip `674d663` with uncommitted changes.
  Exit code 101. Complete output:

  ```text
  error: no such command: `xtask`

  help: a command with a similar name exists: `test`

  help: view all installed commands with `cargo --list`
  help: find a package to install `xtask` with `cargo search cargo-xtask`
  ```

  Task 0041 supplies the missing gate. Renewed independent review and a passing full gate remain required.
  Status stays active. The coordinator owns integration and task closure.
- 2026-11-15: resumed the accepted claim on `t0022-integration-review` for the owner's follow-up.
  Reserved paths are unchanged. Clarify roles and merge acceptance order.
  Keep the serial migration rule in integration.md and the area map in CODEOWNERS.
  Supply old and new lines in the root AGENTS patch. Leave changes uncommitted and offline.

- 2026-11-14: applied the coordinator's Claude review decisions. Unified role usage, evidence locations,
  migration references and serial crate creation. Commented all CODEOWNERS rules and filled the area map.
  Added the human-author review rule and conditional lease fields. Updated the root patch and commit message.
  Local links, anchors, whitespace, style, role references, task state and PR path checks passed.
  CODEOWNERS checks passed for commented rules, catch-all order, coverage and exact area-map parity.
  Confirmed the parent has the live-rule and premature-closure defects. AGENTS.md is unchanged.
  Hardware was not used or verified. No network, commits or pushes.
  Full gate attempt: `CARGO_NET_OFFLINE=true cargo xtask check`, tip `bde14d4` with these uncommitted changes.
  Exit code 101. Complete output:

  ```text
  error: no such command: `xtask`

  help: a command with a similar name exists: `test`

  help: view all installed commands with `cargo --list`
  help: find a package to install `xtask` with `cargo search cargo-xtask`
  ```

  Task 0041 supplies the missing gate. The coordinator must obtain renewed independent review and a passing
  full gate before merging. The coordinator sets done and moves this file in the integration merge.
- 2026-11-14: codex-c resumed the accepted claim on `t0022-integration-review`, tip `bde14d4`,
  to apply the coordinator's Claude review decisions. Reserved paths are unchanged. Keep status active until
  the integration merge. Work stays offline and uncommitted. Renewed review and the full gate remain required.

- 2026-11-14: documentation delivered by codex-c. Local link, anchor, style, placeholder-team and commit-message
  checks passed. Offline `cargo xtask check` exited 101: error: no such command: `xtask`.
  Task 0041 supplies the missing gate. Full-gate validation and independent Claude review remain for the coordinator
  before merge. The coordinator sets done and moves this file in the integration merge.
- 2026-11-14: wrote integration order, conditional parallel claims, adversarial review, PR evidence and area owners.
  Left five AGENTS additions in `AGENTS_PATCH.txt` and the proposed commit message in `COMMIT_MSG.txt`.
  AGENTS.md is unchanged. No network, hardware access, Git metadata writes, commits or pushes.
- 2026-11-14: codex-c assigned task 0022 on `t0022-integration-review`, base `0a4ba5a`.
  Reserved the paths above. The owner requested offline edits without commits or Git metadata writes.
- 2026-11-14: seeded from task 0001. Not yet claimed.
