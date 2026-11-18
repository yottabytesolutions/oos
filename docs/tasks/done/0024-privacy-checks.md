---
id: 24
title: "Scan changes and commit messages for private data"
kind: feature
status: done
release: 0.2.0
owner: codex-c
depends: []
---

# Scan changes and commit messages for private data

> Historical unit-image references below are superseded by ADR 0015; use generic images and device state.

## Goal

Catch private data before commits reach publication.

## Plan

Reserved paths: tools/privacy/, .github/workflows/, .github/CODEOWNERS, .gitignore, docs/development.md (privacy section),
any docs/ file and tools/ script that hardcodes a device address (including docs/fork/unit-dho804.md),
and this task file. Coordinator expanded the accepted reservation on 2026-11-15. No xtask/ edits.

Before parallel migration, scan staged changes locally and incoming diffs and messages in CI.
Commit generic patterns. Keep private strings in an untracked local file. Use a reviewed allowlist for synthetic
fixtures. Remove default device addresses. Audit item: 8.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

Implementation sequence: add a stdlib scanner and synthetic tests; scan staged additions and all incoming commits
including messages; add a dedicated CI job; document local literals and reviewed exceptions; replace unit addresses
without changing network semantics; run available offline validation. Preserve the existing gate hooks.
The G1/task 0043 follow-up will call the same scanner from xtask, without changing it in this task.

## Status

- 2026-11-15: review round 4 fixes prepared, uncommitted, for renewed Claude review. Fallback scanner candidates
  must be reachable from another origin ref; the pushed branch and origin/HEAD alias are excluded before remote
  ref expansion. Commits reachable only from this push are rejected. No eligible range scanner now fails closed,
  including a fallback branch with no adoption. Ordinary valid pre-adoption bases retain the documented skip.
  README explains PR workflow changes from the head, required branch protection and CODEOWNERS approval, the
  commented ownership-map limitation, and scanner-first/workflow-second CLI flag rollout. Added .github/ and
  tools/privacy/ to the existing commented CODEOWNERS area map.
- 2026-11-15: round 4 validation: all 69 Python tests passed (`target/privacy-round4-python-tests.log`). The exact
  two-commit attack adds an always-pass scanner, then a private fixture. It returned 0 before the fix and now fails
  closed for plain and nested branch names, even with origin/HEAD pointing at the pushed branch. Additional checks
  reject a real scanner available only on that ref and a failed origin reachability query. Real Git archives still
  scan transient additions when another origin ref supplies adoption. Before-fix evidence records three failing
  assertions (`target/privacy-round4-before-tests.log`); synthetic histories use raw objects without commit commands.
  Offline fast gate exited 0: `partial gate passed (not done): 5 steps in 17 s`
  (`target/privacy-round4-fast-gate.log`). Compilation, shell syntax, hook patch applicability, whitespace and
  unchanged unit configuration checks exited 0 (`target/privacy-round4-validation.log`). All 23 complete candidate
  files and draft message scan clean (`target/privacy-round4-candidate-scan.log`,
  `target/privacy-round4-candidate-manifest.sha256`). Draft: `target/COMMIT_MSG.txt`. Full gate and renewed Claude
  review remain with the coordinator under the existing sandbox assignment. No staging, commit, hook/configuration
  change, push, merge or hardware access performed.
- 2026-11-15: resumed for Claude review round 4 under the existing no-commit sandbox assignment. Owner explicitly
  added .github/CODEOWNERS to this round's scope. Plan: reject fallback scanner commits reachable only from the
  pushed ref, reproduce the two-commit scanner-injection attack with real synthetic Git objects, document workflow
  trust and required branch protection, and explain the two-step rollout of scanner CLI flags. Run offline checks
  and refresh the draft message; leave final full gate, commit and renewed Claude review with the coordinator.
- 2026-11-15: review round 3 fixes prepared, uncommitted, for renewed Claude review. One LF-only splitter now
  handles patch additions, new blobs, revision lists, recorded messages, tree scans, hook cleanup and local literals;
  it strips one trailing CR and file reads preserve embedded CR. Merge regressions cover both parent orders.
  Audited every splitlines call: remaining calls only parse fixed test inputs or test instrumentation.
  CI exports all scanner modules from the base into runner scratch and runs that copy before independent head
  tests. Missing push-base fallback selects the oldest adopted non-upstream range commit in reverse topological
  order, scans the whole range with base policy, and fails closed if adoption exists only outside that range.
  Full-history lookup preserves side-branch adoption evidence. README documents the trust and skip rules.
- 2026-11-15: round 3 validation: 65 Python tests passed (14 added), including real Git archives and traversal
  proving a neutered head cannot hide PR findings or a transient addition on a new branch. LF regressions failed
  before fixes (37 assertions); a focused merge replay with the old splitter also failed. Export, scan-order and
  fallback regressions failed against the pre-fix workflow. Evidence: `target/privacy-round3-lines-before.log`,
  `target/privacy-round3-merge-before.log`, `target/privacy-round3-workflow-before.log` and
  `target/privacy-round3-python-tests.log`. Synthetic repositories use raw Git objects, without commit commands.
  Offline fast gate exited 0: `partial gate passed (not done): 5 steps in 15 s`
  (`target/privacy-round3-fast-gate.log`). Python compilation, simulator/workflow shell syntax, hook patch
  applicability, unchanged private unit configuration and diff checks exited 0 (`target/privacy-round3-validation.log`).
  All 22 complete candidate files scan clean (`target/privacy-round3-candidate-scan.log`,
  `target/privacy-round3-candidate-manifest.sha256`). Draft message: `target/COMMIT_MSG.txt`.
  Full gate and renewed cross-vendor review remain with the coordinator under the existing sandbox assignment.
  No staging, commit, hook/configuration change, push, merge or hardware access performed.
- 2026-11-15: resumed for Claude review round 3, fix first, under the existing claim and no-commit sandbox.
  Plan: regress LF-only parsing and merge line numbers; export historical scanner code for CI; regress missing
  push-base adoption in branch history; document the fallback trust rule and run offline validation.
  Preserve the existing uncommitted work, private unit configuration and unapplied hook patch.
- 2026-11-15: review round 2 fixes prepared, uncommitted, for renewed Claude review. Merge scans now intersect
  result-line additions against every parent, excluding inherited upstream text while checking new resolutions.
  CI prints a notice and exits 0 for a valid base without the scanner; adopted bases still use trusted base policy,
  with an empty missing allowlist. Added two exact firewall ULA fixture exceptions covering four lines; existing
  scanner examples and network ACL examples already have matching synthetic exceptions. MAC detection now accepts
  mixed separators and bare values near hw, hwaddr or lladdr. Hook message cleanup reads core.commentChar,
  defaults to #, and identifies auto's marker from scissors; without scissors auto retains ambiguous comments.
  README documents adoption, combined additions, comment cleanup and the required upstream fetch failing closed.
- 2026-11-15: round 2 validation: 51 Python tests passed (14 added across round 2). Before fixes, the suite ran
  49 tests and reported 14 failed assertions, reproducing merge, adoption, MAC, comment and fixture failures.
  Synthetic repositories use raw Git objects, with no commit command or real-repository metadata changes.
  Evidence: `target/privacy-round2-before-tests.log`, `target/privacy-round2-python-tests.log`.
  An additional failing regression proved an unavailable PR base must fail instead of taking the adoption skip;
  CI now validates the base commit first (`target/privacy-round2-base-before-test.log`).
  Full-tree audit exited 1 with 153 redacted findings in existing files outside the candidate; all 20 task files
  scan clean in their entirety, including unchanged lines (`target/privacy-round2-tree.txt`,
  `target/privacy-round2-candidate-scan.log`, `target/privacy-round2-candidate-manifest.sha256`).
  Python compilation, simulator and workflow shell syntax, hook patch applicability, unchanged unit configuration
  and diff checks each exited 0 (`target/privacy-round2-validation.log`). Offline fast gate exited 0:
  `partial gate passed (not done): 5 steps in 15 s` (`target/privacy-round2-fast-gate.log`).
  Draft message remains `target/COMMIT_MSG.txt`. Final full gate and cross-vendor review remain with the coordinator.
  No staging, commit, hook installation, repository configuration change, push, merge or device access performed.
- 2026-11-15: resumed for Claude review round 2 under the existing claim and no-commit sandbox rules.
  Plan: regress merge-only additions with synthetic Git objects; fix CI's pre-adoption exit; verify synthetic
  exceptions against complete changed files; extend MAC detection and Git comment cleanup; document CI trust
  and fetch failure behaviour. Keep the hook patch unapplied and leave final gate, commit and review to the coordinator.
- 2026-11-15: review round 1 fixes prepared for renewed Claude review, still uncommitted and not marked done.
  Session URLs now include Claude code sessions; only an exact Claude-Session line in explicitly identified
  commit-message records is exempt. File names cannot impersonate messages. Added sentence-final IPv4,
  shared-space IPv4, ULA, EUI-64 MAC, colon-prefixed/dotted MAC and nearby-keyword bare-MAC regressions.
  The unapplied hook patch strips Git # comments and scissors content through an opt-in message mode;
  recorded messages in CI remain unfiltered. Exceptions now key exact paths, rules and matched-value hashes,
  with migrated entries and a documented fixture-review workflow. CI falls back from missing, zero or non-ancestor
  push bases, excludes commits reachable from fetched upstream branch refs, and uses an empty missing-base policy.
  Removed the redundant ignore entry. Unit notes describe publication scrubbing and a fixed fallback address;
  the private unit configuration is unchanged. The prepared message is now `target/COMMIT_MSG.txt`.
- 2026-11-15: round 1 validation: 37 Python tests passed (18 added); the initial scanner/hook/range regressions
  failed before fixes; CI-shell regressions recorded five failing assertions against the saved pre-fix workflow. Logs:
  `target/privacy-python-tests.log`, `target/privacy-round1-before-tests.log`,
  `target/privacy-round1-before-workflow-tests.log`. Python compilation, simulator and workflow shell syntax,
  hook patch applicability, unchanged unit configuration and `git diff --check` each exited 0; details in
  `target/privacy-round1-validation.log`. Candidate working additions (19 files) and the prepared message each
  scanned with 0 findings; candidate scan and manifest: `target/privacy-round1-candidate-scan.log` and
  `target/privacy-candidate-manifest.sha256`. The offline fast gate exited 0:
  `partial gate passed (not done): 5 steps in 14 s` (`target/privacy-fast-gate.log`).
  The coordinator still owns the final full gate, commit and renewed cross-vendor review; no staging, commit,
  hook installation, remote/config change, push, merge or device access was performed.
- 2026-11-15: round 1 tree audit exited 1 with 157 redacted candidates: 87 private IPv4, 11 private IPv6,
  18 MAC, 11 serial-like, 18 email, 5 private-key-header, 4 global IPv6 and 3 home-path findings.
  The expanded rules identify additional existing fixtures and private configuration; these are review candidates,
  not confirmed disclosures. The deliberate private unit configuration remains untouched for the owner's publish
  scrub. Redacted evidence: `target/privacy-round1-tree.txt`. Earlier baseline evidence remains preserved.
- 2026-11-15: Claude review round 1 returned `fix first`; resumed under the same sandbox and no-commit rules.
  Plan: add failing regressions for session trailers, address and MAC boundaries, matched-value exceptions,
  hook cleanup and CI range/policy trust; fix each finding; update publication notes and run offline checks.
  Keep the private unit configuration untouched. Prepared commit message moves to `target/COMMIT_MSG.txt`.
- 2026-11-15: implementation prepared for coordinator review, not marked done. Added the typed stdlib scanner,
  exact-file synthetic exceptions, ignored local literals, regression suite, Privacy CI range job and usage docs.
  Candidate working additions and the prepared commit message each scanned with 0 findings.
  Staged scans read index blobs; ranges scan every incoming commit and message, including transient additions and
  merge resolutions. Reports redact matching content and sensitive filenames. No xtask/ edits or device access.
- 2026-11-15: offline validation passed: 19 Python tests; `python3 -m py_compile tools/privacy/*.py`;
  shell syntax for the two changed simulator scripts; `git diff --check`; `git apply --check tools/privacy/hooks.patch`;
  `cargo fmt --all --check`; host and aarch64 `cargo clippy --locked --offline --workspace --all-targets`
  (target pass adds `--target aarch64-unknown-linux-musl`). Both Clippy commands exited 0 with the existing
  repository warnings. `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$PWD/target" cargo xtask check --fast` exited 0:
  `partial gate passed (not done): 5 steps in 12 s`. Full gate is assigned to the coordinator by the resume brief.
- 2026-11-15: `cargo test --locked --offline --workspace --no-fail-fast` and the same command with `--release`
  both exited 101: each had 1,098 passed, 21 failed and 40 ignored across targets. HAL: 329 passed, 1 failed;
  UI: 508 passed, 19 failed; xtask: 39 passed, 1 failed. Failures match the sandbox's denied sockets and temporary
  Git commit: HAL `wlan::tests::join_commands`, UI HTTP/net/SCPI/SNTP/Bode tests and xtask's existing base test.
  No tests, gate rules or ratchets were changed to hide these failures. Complete logs remain in worktree-local
  `target/privacy-tests-debug.log`, `target/privacy-tests-release.log`, `target/privacy-clippy-host.log`,
  `target/privacy-clippy-arm64.log`, `target/privacy-fast-gate.log` and `target/privacy-python-tests.log`.
- 2026-11-15: tree audit exited 1 with 144 redacted candidates across 38 existing files: 86 private IPv4,
  17 MAC, 11 serial-like, 18 email, 5 private-key-header, 4 global IPv6 and 3 home-path findings. Detailed redacted
  output: `target/privacy-tree.txt`. These include deliberate source fixtures and public attribution, not 144
  confirmed disclosures. Concrete unit configuration in `rootfs/unit/dho804/system.conf` and home paths
  in three `docs/fork/specs/` files remain outside this reservation and require coordinator routing. No history edit.
- 2026-11-15: replaced concrete unit IPv4, hostname and MAC examples in `docs/network.md`,
  `docs/fork/unit-dho804.md` and `docs/fork/upstream-audit.md`; firewall/SSH simulators now use documentation ranges.
  The WLAN simulator retains its invented subnet with an exact-file exception because the existing ignored
  `ui::wlan::tests::live_radio` pins that prefix in UI source outside this reservation. Broad IPv6 allocation
  prefixes retain their rejection-test meaning, with exact-file exceptions. Device test tooling already requires
  `--host` or `OOS_SCOPE`, so no default-host edit was needed there. Simulator runtime was not verified in Docker.
- 2026-11-15: hook integration is supplied as the tested, unapplied `tools/privacy/hooks.patch` with the documented
  one-command installer. Applying it requires an explicit exception for `.githooks/pre-commit` and
  `.githooks/commit-msg`: AGENTS.md forbids hook edits and neither path is reserved. No hook or Git configuration
  was changed. Coordinator next actions: decide that exception, obtain Claude review, run the full gate, commit
  with the prepared `COMMIT_MSG.txt` and integrate. No commit, staging, push or merge was performed here.
- 2026-11-15: time log for this continuation: approximately 20 minutes reading the resumed brief, implementing,
  inspecting edge cases, running offline validation and preparing the handoff. Builds and logs stayed in this
  worktree's target directory. Existing build caches and required evidence were preserved.

- 2026-11-15: resumed by coordinator decision on branch `t0024-privacy-checks`, base `overhaul` at `1d5fa1b`.
  Expanded reservation recorded above. Sockets and Git commits are blocked in this sandbox; coordinator owns
  the final full gate, cross-vendor review, commit and integration. No commit or hardware access authorized here.

- 2026-11-15: offline validation: `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$PWD/target" cargo xtask check`
  exited 1. Exact summary: `gate FAILED in 877 s: tests (debug), tests (release), coverage`.
  Formatting, size limits, architecture, mutable statics, unsafe, Clippy and perf guards passed. Both test profiles
  failed in HAL (`wlan::tests::join_commands`, permission denied), UI (19 tests, with permission-denied failures)
  and xtask (temporary-repository commit in `base::tests::the_base_is_the_parent_on_the_branch_and_a_lost_file_fails`).
  Coverage failed on the same HAL test. No failing code or gate rules were changed. `git diff --check` passed.
- 2026-11-15: stopped before implementation under the assignment's reserved-path rule. Removing concrete unit
  configuration requires `docs/fork/unit-dho804.md` (lines 11, 25 and 350), outside the accepted reservation.
  Coordinator next action: extend the claim for the required address cleanup before resuming codex-c's work on
  `t0024-privacy-checks`, assigned base `overhaul` at `1d5fa1b`. No scanner, hooks or CI changes were made.
- 2026-11-15: read-only preliminary RFC1918 search found 28 matching lines across `docs/fork/unit-dho804.md`,
  `docs/network.md`, `tools/fw-sim.sh`, `tools/ssh-sim.sh` and `tools/wlan-sim.sh`. These are search candidates,
  not scanner findings; synthetic network examples need review rather than blanket removal. Values are omitted.
  `tools/device-test.py` already requires `--host` or `OOS_SCOPE` for device use. No history was edited and no device
  was accessed.
- 2026-11-15: time log: approximately 5 minutes reading rules, inspecting scope and recording the blocker;
  full-gate validation took 14 minutes 37 seconds with a worktree-local `target/` (mostly waiting). No repository
  commit, staging or hook installation was performed. No device was used. Implementation and cross-vendor review
  remain pending.
- 2026-11-15: claimed for codex-c on branch t0024-privacy-checks by the coordinator, base overhaul.
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: done. Merged via integ-4 after cross-vendor review (Claude) and a passing full gate.
