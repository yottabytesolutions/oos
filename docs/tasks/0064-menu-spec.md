---
id: 64
title: Menu surface and setup overview specification
kind: docs
status: active
release: 0.3.0
owner: codex
depends: []
---

# Menu surface and setup overview specification

## Goal

Specify the legacy menu surface and the owner's active-choice and overview requirements for M17 under
[ADR 0038](../adr/0038-menus-and-parser.md). Review tier: standard. Docs only.

## Plan

Reserved paths: `docs/tasks/0064-menu-spec.md`, `docs/spec/menus.md`, `docs/spec/overview.md`,
`target/COMMIT_MSG.txt`.

1. Read the architecture, process rules and specification gaps; inventory every menu constructor and item handler.
2. Write ordered menu paths and a requirement catalogue with kinds, bindings, value domains, conditions, current
   highlighting and keep/change/drop decisions, each verified against source locations.
3. Specify active-choice presentation and touch/key/knob navigation. Specify the 1024x600 setup overview using
   the same value readers, menu links and explicit owner questions.
4. Cross-check inventory coverage and citations; run `cargo xtask check --fast`. Prepare one commit-message
   section. Leave changes uncommitted for the coordinator's cross-vendor standard review and owner approval.

## Status

- 2026-11-16: merged-base `cargo xtask check --fast` returned exit 0:
  `partial gate passed (not done): 8 steps in 203 s`. Passed fmt, size, architecture, mutable-statics, unsafe,
  method-exceptions, known-failures and agent-rules; no selected step failed. Complete logs:
  `target/check-logs/39558-1794849265424170000/`, including `steps.log` with exit=0.
  Doctor, clippy, debug/release tests, perf, coverage, script/Python checks and image suites were not selected.
  No full gate or hardware evidence is claimed. One section is prepared in `target/COMMIT_MSG.txt`.
  Temporary extraction files were removed; required gate logs and active build caches retained.
  Intentional uncommitted files are the two specifications and this task (untracked on the merged base); the
  ignored commit-message file is also prepared. HEAD remains c59b71d5; no code changes or commits.
  Ready for coordinator-owned independent Claude standard review, owner decisions and integration full gate;
  task stays active until that process completes. Earlier pre-merge gate results below do not cover this draft.


- 2026-11-16: re-extracted merged c59b71d5 against e9f7c30b. Preserved every earlier requirement id.
  Current source has 48 MenuId variants (10 added) and 456 Item variants (130 added, 5 removed).
  Menu tables: 961 rows, **395 kept / 140 changed / 420 added / 6 dropped** versus the earlier draft.
  Inventory subset: 901 rows, **354 kept / 121 changed / 420 added / 6 dropped**; remaining 60 shared/order/domain
  rows: 41 kept / 19 changed. Counts describe extraction deltas, separately from H1/N1 keep/change/drop dispositions.
  Retained six tombstones: old Add item settings link, DecFdRate/DecArate/DecTick/DecNibbles module replacements,
  and FullScreen replaced by Density/expand. Ten new menu sections and fourteen auxiliary surface sections
  have fresh ids; existing section ids remain. No menu variant disappeared.
- 2026-11-16: merged additions and changed contracts now cover Cards/Compact, source/context popups, zoom
  H-/H+/V-/V+/Box/Reset/Close, Single/Live, SIM/Advanced, feature/decoder availability, Bus, Magnetic/Inverter/Dpt,
  draw path/core matching/aliasing, storage destination/fast-frame, mask actions, monitor view, telnet, source
  references, math families and measurement catalogue/configuration/paging. Enumerated 29 production protocols
  and every production decoder option, plus 160 measurement definitions (60 standard, 17 pair, 83 module).
  Recorded 50 ps display versus 1 ns ADC floor and exact engineering-ladder quirks, preserving behaviour by default.
  Actual HEAD has SIM under Main Analysis and Load file under Reference, not top buttons.
- 2026-11-16: updated Overview to the current top controls and both bottom-bar layouts, preserving their state
  on return from the proposed standalone page. Added live-overlay/record/SIM ownership, selected versus trigger
  and measurement sources, density/format/monitor, catalogue totals/overrides/gate, module decoders and new views.
  Owner questions remain explicit at both document ends. Audit preserved all earlier ids, found no duplicate
  definitions, validated all Markdown table widths and checked 1,981 current menu plus 64 overview source
  locations for existing files/line bounds; manual review checked bindings, labels, conditions and registry order.
  All 48 menu and 456 item names are covered. Gate execution and review handoff follow.


- 2026-11-16: owner reassigned the accepted task to Codex on `t0064-menu-spec-v2`, merged base c59b71d5.
  Restored the three documents from e9f7c30b as the starting point. Reserved paths and review tier unchanged.
  Plan: re-extract this tree, preserve unchanged ids, annotate changed/removed rows, add new ids and dispositions,
  refresh overview for both bar layouts, audit counts/citations, run the fast gate and prepare one message section.
  No commits, code changes or hardware access. Earlier evidence below applies only to the pre-merge draft.

- 2026-11-16: final fast-gate run returned exit 0:
  `partial gate passed (not done): 8 steps in 23 s`. Same eight selected steps passed; complete logs:
  `target/check-logs/56724-1794821082694438000/`, including `steps.log`. Source/citation audit passed after
  corrections. Prepared one section in `target/COMMIT_MSG.txt`; removed temporary extraction scripts/data while
  retaining gate logs and build caches. Intentional remaining changes are this task and the two spec documents,
  all uncommitted. Standard cross-vendor review, owner decisions and coordinator integration remain pending.

- 2026-11-16: source cross-check accounts for all 38 MenuId variants and all 331 Item variants at the base,
  represented by 481 inventory rows (indexed expansions and repeated paths included). Every inventory row has
  a requirement id, source evidence, binding/domain, conditions, current highlight and disposition. Checked unique
  table ids and all 1,474 menu plus 72 overview source locations. Exact conditional trigger/decoder ordering,
  serial relations, keypad order and I2C address clamping were checked against their handlers. Nothing is dropped.
- 2026-11-16: drafted `docs/spec/menus.md` and `docs/spec/overview.md`. H1 covers active choices, inline submenu
  summaries, picker identity and effective view indication; N1 proposes complete Back/Close paths. Overview covers
  all requested categories with a 1024x600 layout budget, pure menu links and coherent requested/effective state.
  Owner questions end both documents. Review tier remains standard; coordinator must arrange independent Claude
  review and owner approval. Task remains active for that handoff and the integration full gate.
- 2026-11-16: first `cargo xtask check --fast` returned exit 0:
  `partial gate passed (not done): 8 steps in 22 s`. All selected steps passed: fmt, size, architecture,
  mutable-statics, unsafe, method-exceptions, known-failures, agent-rules. Complete logs:
  `target/check-logs/18130-1794820848045991000/` (including `steps.log`). The command does not select clippy,
  debug/release tests, perf, coverage, host or image checks. No full-gate or hardware evidence is claimed.

- 2026-11-16: owner assigned task 0064 to Codex on `t0064-menu-spec`, based on `overhaul` c6f1f98.
  Claim limited to the reserved documentation paths above. Plan recorded before specification edits.
  No code changes, commits or hardware access authorized or performed. Independent review is coordinator-owned.
