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
[ADR 0038](../adr/0038-menus-and-parser.md). Review tier: standard.
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
5. Re-spec against merged c59b71d5: walk every parser/dispatch branch, preserve
   existing IDs, mark changed/removed contracts, add upstream paths with new IDs,
   compare the validator and state producer, and record counts/disagreements.
6. Audit the tables against source and run `cargo xtask check --fast`; record the
   actual result and prepare one commit-message section for coordinator review.

## Status


- 2026-11-16: merged-base re-spec on c59b71d5 prepared for coordinator review,
  docs uncommitted. `docs/spec/scpi.md`: 358 live header templates / 628 C/Q
  paths (270 both, 25 command, 63 query), nine removed side-effect alias
  tombstones, nine separate RPC rows. Kept all 153 inherited header IDs:
  119 retain their row contract and 34 are marked changed, with updated global
  error/framing rules. Added 205 rows scpi-500..704 (203 upstream additions
  and two prior-extraction omissions, CORes and channel POSition). Complete
  value tables: 70 single definitions / 60 measurement kinds, 17 pairs and
  46 eye figures. State schema: 140 diagnostic positions (130 retained plus
  ten hardware suffix fields), eight separate /ui-state JSON keys.
  Recorded fork/upstream/merge decisions, including task 0080's still-missing
  clear-item/drop-oldest behavior, source-aware RUN/SINGLE, 50 ps display ladder,
  strict waveform enums/paging, actual trigger type and bounded programs/OPC.
- 2026-11-16: validator cross-check: 251 literal header occurrences, 200 distinct
  spellings, 119 mapped inventory IDs; sole unmatched header REF:NOPE? is the
  intentional negative test. Eight findings V1..V8 in the spec: invalid-query
  error text versus earlier empty-invalid target; cancelled/oversized query
  no-reply exceptions to validator's broad comment; JSON versus diagnostic
  schema; client text/binary limits and whitespace/encoding coverage; absent
  selective-clear/full-list assertions; changed numeric/source/page contracts;
  OPC acknowledgement versus completion coverage; current statistics without
  history. No current parser-versus-validator contradiction established by
  static audit. Remaining target divergences: MODE protocol selection (D2),
  STEP (D5), TVM compatibility proposal and M15 gen/epoch waveform barrier.
  Successful empty replies/compound slots are fixed in this tree; the old
  *CLS? side effect and missing reply are gone. SYST:PIPeline? preference and
  STATe pipeline startup-switch diagnostic can disagree. Hardware suffix adds
  a global frames regex match; recorded as a version 0 wire addition.
- 2026-11-16: `cargo xtask check --fast`, c59b71d5 plus documentation changes:
  exit 0, exactly `partial gate passed (not done): 8 steps in 161 s`.
  All selected steps passed: `ok fmt`, `ok size`, `ok architecture`,
  `ok mutable-statics`, `ok unsafe`, `ok method-exceptions`,
  `ok known-failures`, `ok agent-rules`; no failed steps. Complete logs:
  `target/check-logs/16886-1794848842552098000/` (`steps.log` has exit=0).
  Default/full and image steps were not selected. Existing validator framing
  suite: `python3 -m unittest discover -s tools/tests -p test_scpi_validate.py -v`,
  exit 0, four tests passed. Static audit passed unique IDs, Markdown table
  widths, all source anchor bounds and complete non-overlapping inherited
  row error coverage. All 15 original regex definitions preserved; first-match
  and find-all captures remain stable after version append across 32 synthetic
  merged CPU/GPU, identity, settle, hardware-suffix and transitional shapes.
  Evidence: `target/t0065-respec-audit.log` and
  `target/t0065-validator-literals.json`. `git diff --check` passed. Replaced
  `target/COMMIT_MSG.txt` with one section. No running-server validator or
  hardware access; no source-code changes, commits, pushes or merges.
  Task remains active; coordinator owns cross-vendor review and full batch gate.

- 2026-11-16: owner assigned merged-base re-spec on `t0065-scpi-respec`,
  HEAD c59b71d5; initial worktree clean. Accepted task 0065 claim and reserved
  paths retained. Docs only, no hardware or commits. Plan extended before edits.

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
