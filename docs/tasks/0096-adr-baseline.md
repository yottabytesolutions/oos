---
id: 96
title: Rewrite the ADR baseline
kind: docs
status: active
release: none
owner: author
depends: []
---

## Goal

Deliver issue #159 as one flat, unambiguous ADR baseline with a repository-wide citation check.
The owner-approved brief supplies the claim. Branch: docs/159-adr-rewrite.
Reserved paths: docs/adr/, docs/architecture/adr/, xtask/src/text*, and repository-wide ADR citations and their
named procedural destinations. The review fix restores pre-PR runtime defaults and their tests; the final PR is documentation and checker only.

## Plan

Reserved paths: docs/adr/, former architecture records, their named procedural destinations,
repository-wide citations, xtask text/link tooling, CODEOWNERS, core model defaults and UI default tests.

1. Verify current code facts, consolidate 38 decisions and move procedures to their owners.
2. Remap references once from the old-to-new mapping; validate numbers and paths in text.
3. Preserve runtime defaults, run fast gate and touched-crate tests; report mapping, moves and follow-ups.
4. Restore forcing evidence and rejected alternatives from the former records; correct State and Check lines.
5. Test dotfile/source references and obsolete category paths; prepare mapping and follow-up issue placeholders.

## Status

- 2026-11-18: PR #164 review findings fixed: reject legacy numbered architecture citations rather than resolving
  their digits in the flat ADR series; regression covers all 19 former IDs and fails before the fix.
  The first fast gate exited 1, `gate FAILED in 2 s: text`, exposing two stale drawing-evidence citations;
  both now cite ADR 0035. Corrected parser-default claims in the key inventory, network and migration docs;
  removed the unsupported Batch-default changelog entry. Runtime defaults are unchanged.
  Fast gate exited 0: `partial gate passed (not done): 11 steps in 3 s`; 638 links and 2803 text files pass.
  xtask debug and test-release suites each passed 233 tests; the existing UI pipeline parser regression passed.
  xtask Clippy with warnings denied and diff whitespace checks passed. No ratchet counts fell.
  Production LOC remains 109537 (+0 against HEAD); host Rust tooling is +2. No hardware access.
  Evidence: target/adr-review-fix-tests-debug.log, target/adr-review-fix-tests-release.log,
  target/adr-review-fix-parser.log, target/adr-review-fix-clippy.log and target/adr-review-fix-loc.log.
  Fetch was blocked by shared Git metadata permissions; HEAD equals the cached remote branch.
  Coordinator/runner owns remote synchronization, renewed independent review, full gate and integration.

- 2026-11-18: Blocking-review validation complete on the restored runtime defaults. Final fast gate exited 0:
  `partial gate passed (not done): 11 steps in 7 s`. All 638 links and 2803 text files pass.
  Debug and test-release crate suites for core/dsp/hal/ui/xtask both exited 0; xtask passed 232 tests per profile.
  The previously reported process-group helper passed in both profiles. xtask Clippy with warnings denied passed.
  Regression proof: the old ADR path parser fails the two expanded reference tests; the final parser passes all
  six ADR tests. No syntax ratchet counts fell. Production LOC is 109537 (+0 against the PR parent);
  host Rust tooling is +109, including the original reference checker. No hardware access.
  Evidence: target/adr-review-fast-final.log, target/adr-review-tests-debug.log,
  target/adr-review-tests-release.log, target/adr-review-clippy.log, target/adr-review-loc.log and
  target/adr-review-regression-before.log / target/adr-review-regression-after.log.
  Coordinator owns follow-up issue placeholders, copying the supplied old-to-new mapping into the amended
  commit body (without the removed default-alignment sentence), renewed independent review and integration.

- 2026-11-18: Review fixes restore alternatives in all 38 records, measured contexts, accurate State lines and
  executable Check IDs. Dotfiles and five test-time citations corrected. The checker rejects extensionless
  category paths and invalid Check step IDs, with regression fixtures covering dotfiles and source files.
  The Batch runtime/default tests are restored to the parent; the separate default change has a follow-up row.
  Fast gate exited 0: `partial gate passed (not done): 11 steps in 13 s`; 638 links and 2803 text files passed.
  No syntax ratchet counts fell. Debug/release crate tests, LOC and xtask Clippy validation are in progress.

- 2026-11-18: PR #164 blocking-review fix accepted on the existing claim. Restore decision evidence and
  alternatives, distinguish targets, extend reference regressions, and remove the default change.
  Coordinator/runner owns fetch, reset, rebase and renewed independent review; network use is builds only.

- 2026-11-18: Local rewrite complete; coordinator owns independent review, full gate/CI and integration.
  Release-profile validation remains blocked by the unchanged process-group helper described below. No unit access,
  Git writes, publication or external service calls were performed.
- 2026-11-18: Final fast gate exited 0: `partial gate passed (not done): 11 steps in 2 s`.
  All 633 Markdown links and numeric ADR references resolve; 38 flat records have the required fields.
  Debug: 2424 passed, 71 skipped across core/dsp/hal/ui; xtask 228 passed in debug and release.
  Release: 2649 passed, 1 failed, 71 skipped across those crates and xtask before the final added xtask regression;
  the final xtask-only release rerun passed all 228. Failure: unchanged
  `ui/tests/support/mod.rs:147`, `support::group_kill_reaches_grandchildren_and_nothing_else`,
  reported `the kill reached another group` after 120 seconds. No retry or out-of-scope helper edit.
  Next action: coordinator investigates this release-test failure before acceptance.
- 2026-11-18: xtask Clippy with warnings denied passed. Python code-growth tests: 4 passed; stacked-PR tests: 5 passed.
  Production LOC remains 109537 (+0). No ratchet counts fell. Logs: target/adr-fast.log,
  target/adr-crate-tests.log, target/adr-release-tests.log, target/adr-xtask-release.log and target/adr-xtask-clippy.log.
  Earlier default-expectation failures were corrected while retaining explicit Standard selection and demo-route semantics.
- 2026-11-18: Drafted the consolidated records and relocated procedural context. Reference gate and verification pending.
