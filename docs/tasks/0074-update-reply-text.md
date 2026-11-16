---
id: 74
title: Describe the actual update and rollback starts
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Make upload, panel, job and CLI messages describe the code's kexec, direct install,
trial and restore paths. Review tier: host, as assigned. No boot-policy changes.

## Plan

Reserved paths: `docs/tasks/0074-update-reply-text.md`, `hal/src/update/mod.rs`,
`hal/src/update/text.rs`, `hal/src/update/tests.rs`, `fpgad/src/bin/oos-update.rs`,
`fpgad/src/bin/oos_update/messages.rs`, `fpgad/tests/unit_identity/mod.rs`, `ui/src/ui/update.rs`,
`ui/src/ui/update/upload.rs`, `ui/src/ui/update/text_tests.rs`, `ui/tests/golden/update/`, `target/COMMIT_MSG.txt`.

The owner's direct task assignment accepts this narrow text/test claim from
`11a8b2b9`, superseding earlier updater reservations for these paths only.
Coordinator retains cross-vendor review and batch integration/full gate.

1. Trace every install/rollback message through HAL, CLI, web handler and watcher,
   using `docs/update.md` as context. Record timing conditions, restart guards and
   restoration/arming failures before changing text.
2. Correct and share path-specific wording, preserving required power cycles.
   Add host regressions for replies, panel/web guidance, reports and notices.
   Keep existing oversized files/functions within their size ratchets.
3. Run focused debug/release tests, the gate's exact clippy step and
   `cargo xtask check --fast`; retain complete logs and exit codes. Prepare one
   section in `target/COMMIT_MSG.txt`. No commits or hardware access.

## Status

- 2026-11-16: review-round handoff ready, uncommitted. All five requested fixes
  are addressed. Focused debug/release checks pass, exit 0 in all eight commands:
  HAL update (100 passed, two existing private-image/tree diagnostics ignored),
  UI update (12), CLI unit (eight), direct CLI integration (one). Reproduction
  and complete logs/exits: `target/t0074-evidence/review-focused-checks.py` and
  `review-{hal,ui,cli,cli-integration}-{debug,release}.{log,exit}`.
  Exact `steps::clippy` exits 1 on inherited ratchet overruns; comparison exits 0:
  parent 471 keys/1243 findings, final 471 keys/1242 findings, zero new findings
  relative to both parent and pre-review candidate, one removed from parent.
  Evidence: `clippy-step.py`, `review-clippy-step.{log,exit}`,
  `review-compare-clippy.py`, `review-clippy-comparison.{log,exit}` in that directory;
  command logs: `target/check-logs/81241-1794845087238593000`.
  `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 75 s`; all eight selected steps pass.
  Complete output/exit: `review-fast-gate.{log,exit}`; command logs:
  `target/check-logs/89129-1794845127958445000`. Existing pending rebaselines remain
  unrelated. Candidate source/test/reference hashes: `review-candidate.sha256`.
  `git diff --check` passes; `target/COMMIT_MSG.txt` updated with one section and
  the reasons for pending/kexec golden changes. No commits, pushes or hardware
  access. Task stays active for coordinator review verification and full batch
  gate/integration; doctor, full Rust suites, perf, coverage, script/Python and
  image checks remain unselected by the fast gate.

- 2026-11-16: review fixes implemented: pending trials say
  `Trial pending; power-cycle to start`; kexec sessions say
  `Restart refused or pending; power-cycle`. Direct upload replies now say
  `after the power cycle`, retaining `after the restart` for kexec. Restore
  notices include `No automatic restart` once, retaining warnings and empty-report
  fallback. Updated regressions fail on the pre-fix text (UI and CLI exit 101);
  complete evidence: `target/t0074-evidence/review-before-{ui,cli}.{log,exit}`.
  Pending/kexec panel captures inspected visually and refreshed for these explicit
  power-cycle instructions; running reference is byte-identical. The stale
  references fail the golden check, exit 101, before refresh; evidence:
  `review-golden-before-refresh.{log,exit}`. Actual PNGs and README use
  `update-golden/`, with no task identifier in the test. Validation in progress.

- 2026-11-16: review-round plan: replace transient-notice references with explicit
  pending-trial power-cycle guidance, distinguish the direct reply's power cycle
  from kexec's restart, and avoid duplicate restart guidance in restore notices
  while preserving arming warnings. Update path/message regressions, pending and
  kexec panel references, and the golden output directory name. Rerun focused
  debug/release tests, the exact clippy step with parent comparison, and the fast
  gate; refresh `target/COMMIT_MSG.txt`. Same accepted claim; no commits.

- 2026-11-16: author handoff ready as uncommitted changes based on `11a8b2b9`.
  Final `cargo xtask check --fast` exits 0 and reports
  `partial gate passed (not done): 8 steps in 107 s`.
  All eight selected steps pass: fmt, size, architecture, mutable-statics, unsafe,
  method-exceptions, known-failures and agent-rules. Existing upstream pending
  rebaselines are printed for unrelated paths; no task-owned size overrun remains.
  Complete output/exit: `target/t0074-evidence/fast-gate.log` and `.exit`;
  command logs: `target/check-logs/55440-1794843575308322000`.
  Clippy's exact step still fails inherited overruns, with zero new findings
  relative to the parent; focused debug/release suites pass as recorded below.
  Doctor, full Rust suites, perf, coverage, script/Python inventory and image checks
  are not selected. `git diff --check` passes. `target/COMMIT_MSG.txt` contains
  one section. Task remains active for coordinator cross-vendor host review and
  full batch gate/integration. No commits, pushes or hardware access.

- 2026-11-16: final size measurement uses the gate's actual syntax walker:
  HAL remains 723 file lines, install 123 and kexec_stage 74; CLI shrinks from
  961 to 955 file lines (ratchet 973), finish_kexec falls below 60 code lines,
  and watch remains 103. No existing oversized function grows. New files stay
  within the ordinary limits. Evidence: `target/t0074-evidence/size-comparison.log`
  and `.exit`. No ratchet, lint, limit or architecture rule was changed.

- 2026-11-16: final exact `steps::clippy` run exits 1 on inherited ratchet
  overruns, as does the parent. The actual gate modules run all six host crates
  and five ARM crates; no fast/pending override is used. JSON diagnostics compared
  with the same exact step on parent sources: parent 471 keys/1243 findings,
  final 471 keys/1242 findings, zero new findings and one removed. All parent
  source substitutions were restored immediately. Reproduction and full evidence:
  `target/t0074-evidence/clippy-step.py`, `clippy-step.log`, `.exit`,
  `clippy-parent.log`, `.exit`, `compare-clippy.py`, `clippy-comparison.log`, `.exit`.
  Final command logs: `target/check-logs/42502-1794843551233313000`; parent command logs:
  `target/check-logs/9347-1794843261043168000`. Ratchets remain unchanged. Initial helper visibility and
  unused-helper compile/lint findings were corrected before this final comparison.
- 2026-11-16: focused debug/release suites pass, exit 0: HAL update tests
  (100 passed; two existing private-image/tree diagnostics ignored), UI update
  tests (12, including exact panel references), CLI unit tests (eight), and the
  direct-install CLI integration regression (one). Commands and complete logs with
  per-command exit files: `target/t0074-evidence/focused-checks.py` and
  `{hal,ui,cli,cli-integration}-{debug,release}.{log,exit}` in that directory.
  Final CLI debug/release unit and integration reruns also pass after the last
  Linux-only error-format adjustment, which captures the reboot error before
  reading build identity.
  No hardware access or verification. Text and reference changes only; no new
  state owner, queue, dependency, unsafe code or boot-policy change.

- 2026-11-16: install reports are now neutral about the caller's start method;
  direct CLI installs explicitly request a power cycle only after a write with
  OOSK selected. Web replies/page describe automatic kexec installation and warm
  trial restart, direct power cycles and the watcher timing. Panel guidance
  distinguishes pending, running and kexec trials, including same-build reinstall.
  Restore notices preserve the actual report's arming outcome; failed restore or
  restart guards no longer promise an unverified next build. No install, rollback,
  trial or restart policy changed. New panel references cover all three states;
  their `.png.ref` suffix keeps focused unit fixtures separate from M1's scripted
  PNG inventory. Visually inspected rendered overlays for clipping and readability.
- 2026-11-16: the report regression fails against the exact parent HAL module,
  exit 101, because it requests a power cycle inside the shared install report.
  Final report code restored immediately. Evidence: `regression-before.log` and
  `.exit` in `target/t0074-evidence/`. Initial focused debug checks pass: 100 HAL
  update tests (two ignored), 11 UI update tests, eight CLI unit tests and one
  direct CLI integration test. The additional rendered-reference test is being
  included in final debug/release validation.
- 2026-11-16: initial fast gate passes, exit 0:
  `partial gate passed (not done): 8 steps in 106 s`. Exact clippy step reports
  existing ratchet overruns plus one new unused test helper; helper coverage was
  corrected. Parent comparison and final clippy/fast runs remain in progress.
  Initial builds hit host disk exhaustion (exit 101); removed only this worktree's
  disposable dev build with Cargo clean. Subsequent checks disable incremental
  compilation and dev/test debug info, with no lint or ratchet changes.

- 2026-11-16: codex claims owner-assigned Task 0074 on `t0074-upload-reply`,
  clean base `11a8b2b9`. Plan recorded before code. Traced kexec install through
  `finish_kexec`, guarded `restart_for_trial` and ordinary watcher confirmation;
  direct installs and rollback only write/arm, without restarting. `judge` needs
  20 s stable UI plus startup completion, or stable UI at the 180 s timeout;
  three UI restarts or no stable UI by timeout fails. No fixed upload deadline.
  Owner reports round 5 warm restart and automatic confirmation 65 s after upload;
  that observation is supplied evidence, not a new hardware verification.
