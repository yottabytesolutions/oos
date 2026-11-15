---
id: 20
title: "Enforce the shared agent rules"
kind: chore
status: done
release: 0.2.0
owner: Codex
depends: [1]
---

# Enforce the shared agent rules

## Goal

Keep universal rules in AGENTS.md and each agent entry file a thin pointer.

## Plan

Reserved paths: `xtask/src/agent_rules.rs`, `xtask/src/agent_rules/`, `xtask/src/main.rs`,
`xtask/src/check.rs`, `xtask/src/steps/mod.rs` (adapter if needed), `CLAUDE.md`, `GEMINI.md`,
`.github/copilot-instructions.md`, `.cursor/rules/oos.mdc`, `.aider.conf.yml`, `docs/development.md`,
`docs/tasks/0020-agent-rules.md`. Branch: `t0020-agent-rules`.

Planning only on `plans-xtask`; implementation claims are still required. Implement in the serial order
**0043 (G1) -> 0020 -> 0025 -> 0027 -> M2**. Start `t0020-agent-rules` from `overhaul` after G1 merges;
finish review and integration before 0025 claims `xtask/` and `docs/development.md`.

### Design and files

1. Keep task 0001's universal policy in root `AGENTS.md`. Add `xtask/src/agent_rules.rs` with an explicit
   inventory of the client entry files and their permitted content. Compare complete normalized text, not a
   search for the word AGENTS: an extra rule before or after a valid pointer must fail. Normalize CRLF and the
   final newline only; reject missing files, unreadable files and symlinks. Resolve each relative Markdown link
   to root `AGENTS.md` and require that file to exist and be nonempty.
2. Cover `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md` and `.cursor/rules/oos.mdc`. Permit only the
   existing pointer sentence, with the correct relative link. Cursor additionally needs its exact loader metadata
   (`description: OOS repository rules`, `alwaysApply: true`). Claude alone may append the exact prose instruction
   to use the harness-provided `Claude-Session` trailer and session URL, per the round 1 coordinator decision.
   This exception permits no other policy or actual session identifier. Keep the checker templates about loading,
   not copies of AGENTS rules.
3. Add `.aider.conf.yml` containing only a `read` list naming `AGENTS.md`. Check its complete restricted shape
   without adding a YAML dependency. Inventory tracked and nonignored new client rule files using Git's
   NUL-separated path list (`--cached --others --exclude-standard`). Reject additional recognized entry points
   (including nested `AGENTS.md`, extra Cursor rules and Copilot
   instruction files) until the reviewed inventory explicitly supports a pointer there. Do not inspect ignored
   personal client settings or claim to understand every future client's configuration format.
4. Wire `agent rules` into G1's `xtask/src/check.rs` through `xtask/src/main.rs` and, if needed, the existing
   `steps::Outcome` adapter. Run it in fast and full checks, so pre-commit, pre-push and CI use the same rule.
   Preserve all seven G1 fast steps, its per-crate ratchets, move handling, exception/known-failure checks and
   coverage margin. This check has no allowance or ratchet.
5. Update `docs/development.md` with the checked entry-file inventory and explicit loading for clients without
   automatic discovery: attach/read root `AGENTS.md` at session start; for Aider use its read configuration or
   `/read AGENTS.md`. State that an entry-file check cannot prove that a client loaded or obeyed the rules.

Proposed implementation reservation: `xtask/src/agent_rules.rs`, `xtask/src/main.rs`, `xtask/src/check.rs`,
`xtask/src/steps/mod.rs` if an adapter is needed, the five client entry files above, and `docs/development.md`.
Do not change universal policy merely to make a fixture pass. Any needed policy change remains a separate review.

### Failing-before tests and acceptance

- First add fixture-driven regressions that insert an independent rule into each shim, remove a shim, break a
  relative pointer, disable Cursor auto-loading, or add another recognized entry file. Demonstrate that G1's
  gate accepts these mutations before the new step, and that the new gate rejects them with the offending path.
  Use temporary repositories via G1's `testrepo` helper; no changes to real client files for negative cases.
- Positive cases cover all current pointers, Cursor metadata, the single permitted Claude trailer, Aider's read
  list and CRLF input. Negative cases include a second trailer, hidden HTML-comment policy, extra YAML keys,
  symlinks and a missing root rules file. Check subprocess errors fail rather than yielding an empty inventory.
- Add a dispatch regression proving this step executes under both `check --fast` and the default full check.
  Run xtask tests in debug and release, then `cargo xtask check` without exclusions on the final candidate.
  Supply complete output and exit code, parent/candidate revisions and failing-before evidence for independent
  Claude review under [the review protocol](../process/review.md). The coordinator integrates and reruns the gate.
  No hardware is needed or verified.

Estimate: 0.5-1 engineering day including tests and review fixes, excluding review availability.
Risks: exact templates deliberately require a reviewed update for harmless shim wording changes; client discovery
conventions can change; G1 may change before merge. Re-read its integrated result before implementation and retain
newer `overhaul` privacy/output-isolation changes when resolving documentation or gate wiring.

Audit item: 1. Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: round 1 fixes prepared over `f300c5c`, uncommitted. Debug and release xtask suites each pass
- 2026-11-15: round 1 regression evidence: with new tests against the original checker, 10 passed and seven
- 2026-11-15: review round 1 fixes assigned over `f300c5c`, uncommitted. Plan: replace the Claude trailer
- 2026-11-15: follow-up for the gate owner: align working-tree checks with the staged index; gate-wide
- 2026-11-15: final implementation working tree over parent `a90b3968e3d77f830064ec4cc6c23ea1f8d28f70`;
- 2026-11-15: debug and release xtask suites each pass with 77 passed, zero failed, zero ignored (exit 0;
- 2026-11-15: plan items 1-5 implemented for review. Exact loader templates, relative links, regular-file checks,
- 2026-11-15: failing-before evidence: an isolated parent-revision fixture with added policy, a missing shim,
- 2026-11-15: implementation assigned by the owner to Codex on `t0020-agent-rules`, parent
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0020-agent-rules (based on G1 t0043-gate-g1 plus overhaul) by the coordinator.
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
