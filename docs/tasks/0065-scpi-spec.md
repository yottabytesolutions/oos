---
id: 65
title: Legacy SCPI reference and state query schema
kind: docs
status: active
release: none
owner: codex
depends: [63]
---

# Legacy SCPI reference and state query schema

## Goal

Extract the legacy SCPI surface and state response for the M15 rebuild under
[adr/0017](../architecture/adr/0017-hybrid-after-m12.md). Review tier: standard.
Docs only; no hardware access, code changes or commits.

## Plan

Reserved paths: `docs/tasks/0065-scpi-spec.md`, `docs/spec/scpi.md`,
`docs/spec/state-query.md`, `target/COMMIT_MSG.txt`.

1. Read the parser, delegated handlers, transport adapters and response producers.
2. Write one requirement row per accepted command/query path, including aliases,
   arguments, replies, mapping, effects and source references; document cross-cutting
   behaviour and a host differential-oracle session contract.
3. Extract every state field with its producer and units; propose versioning and
   append-only stability without changing the wire format.
4. Apply the coordinator review decisions: complete transport procedures, error and
   Command/Query contracts, STEP and trigger MODE dispositions, and transitional
   state fields with global-regex compatibility fixtures.
5. Audit the tables against source and run `cargo xtask check --fast`; record the
   actual result and prepare one commit-message section for coordinator review.

## Status

- 2026-11-16: review corrections applied on dd8df94 with documentation changes
  uncommitted. Added nine VXI-11 procedure rows with dispositions and D6/D7
  proposals, the MCP screenshot transport, row/token count explanation and a
  Command/Query mapping column for all 162 headers (variant names due at M14).
  Added scpi-320's complete error map; recorded the *CLS? non-empty-queue defect
  as D4 with the unconditional target reply/error rule. D2 now selects the
  trigger type using Rigol tokens; D5 accepts STEP. Included all three optional
  clarifications. State shadow fields 83..101 and 112 are transitional through
  M2.3e, excluded from version 1 at M15; append-only stability starts there.
  State-query-203 preserves the three supplied global patterns verbatim plus
  12 format-derived patterns without claiming access to private consumer code.
- 2026-11-16: `cargo xtask check --fast` on dd8df94 plus the review corrections:
  exit 0, exactly `partial gate passed (not done): 8 steps in 22 s`.
  All selected steps passed: `ok fmt`, `ok size`, `ok architecture`,
  `ok mutable-statics`, `ok unsafe`, `ok method-exceptions`,
  `ok known-failures`, `ok agent-rules`; no failed steps. Complete logs:
  `target/check-logs/3104-1794820743818914000/` (including `steps.log`).
  The full/default and image-only steps were not selected, as in the earlier
  fast run below. A documentation audit passed unique requirement IDs, all
  Markdown table widths, 162 ordered typed mapping cells, complete non-overlapping
  row error coverage and source anchor bounds. All 15 regex patterns preserved
  first-match/find-all captures across 32 synthetic CPU/GPU, settle, identity and
  transitional shapes; version 1 fixtures omit shadow fields. Audit output:
  `target/t0065-spec-review-audit.log`. `git diff --check` passed. Replaced
  `target/COMMIT_MSG.txt` with one section. No executable producer/consumer tests,
  code or hardware changes. Coordinator owns the P1/P2 verification review and
  full batch gate; task remains active. No commits, pushes or merges.

- 2026-11-16: coordinator assigned a review correction round on `t0065-scpi-spec`,
  HEAD dd8df94 (docs only); initial worktree clean. Same reserved paths and
  docs-only scope. Plan extended before edits; apply the supplied P1/P2 decisions
  and optional clarifications, run the fast gate and replace the single commit
  message draft. No commits or hardware access.

- 2026-11-16: extraction and source audit complete; prepared for coordinator's
  standard cross-vendor review. `docs/spec/scpi.md` has 153 canonical/alias header
  rows, nine query-effect alias rows and all 40 measurement value tokens;
  every row cites source. Open-ended accepted syntax is recorded as predicates.
  Proposed differences: existing empty-reply fix 0137955, actual trigger MODE
  reporting and the ambiguous TVM abbreviation. No drops proposed.
  `docs/spec/state-query.md` covers 130 field/tuple positions, current optional
  blocks, all loss/change/shadow/acquisition counters, and proposed version 1
  with append-only stability. No code or hardware changes. The corpus contract
  is specified; client session recordings and unitlab source are not available
  here and were not fabricated. Prepared one section in `target/COMMIT_MSG.txt`.
- 2026-11-16: `cargo xtask check --fast` on base c6f1f98 with the documentation
  additions uncommitted: exit 0, exactly
  `partial gate passed (not done): 8 steps in 22 s`.
  Selected results: `ok fmt`, `ok size`, `ok architecture`,
  `ok mutable-statics`, `ok unsafe`, `ok method-exceptions`,
  `ok known-failures`, `ok agent-rules`. Complete gate logs:
  `target/check-logs/9528-1794819623734700000/` (`steps.log` records exit=0).
  Not selected: doctor-base, doctor, clippy, tests-debug, tests-release, perf,
  coverage, script-syntax, python-tests, arm64-release-link, gpu-host, ssh-sim,
  firewall-sim, image-required, image-suite. No new executable tests for this
  source-extraction task. The subsequent documentation audit checked all parser
  `kw` tokens, unique requirement IDs, table column counts, source anchors and
  whitespace. Final review and full batch gate remain with the coordinator;
  task stays active. No commits, pushes or merges.

- 2026-11-16: Codex claimed the owner-assigned task on `t0065-scpi-spec`, base
  `overhaul` c6f1f98. Initial worktree clean; no overlapping specification claim
  found. Plan written before specification edits. Cross-vendor standard review
  and full integration gate remain the coordinator's next actions; no commits.
