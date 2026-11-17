---
id: 87
title: Stable spectrum peak table and marker tracking
kind: fix
status: active
release: 0.2.0
owner: Codex
depends: []
---

## Goal

Keep the spectrum summary stationary when a noisy signal alternates between
three and four detected lines. Owner report: noisy 5 V rail, Peak, 50 ns/div,
10 dB/div; summary positions and peak identities jump. Review tier: unit,
owner-visible. This job uses host fixtures; hardware is not yet verified.

## Plan

1. Preserve raw ranked lines for measurements and auto-centre. Keep display
   tracking on main, in existing analyzer Work state, with fixed storage for
   eight visible tracks and eight candidates (less than 2 KiB per Work).
2. Match one-to-one nearest frequencies within max(2 display bins, 0.5 actual
   RBW). Qualify for 1 s and release after 1 s absent, using the UI frame clock,
   including STOP. Dim retained values; do not smooth real frequency/level
   changes. Preserve marker numbers across frames and explicit sort changes;
   renumber in frequency mode only on qualified set changes. Require a 1 dB
   advantage before level-order swaps. Keep constants together.
3. Reserve the maximum peak rows independently of detected count, including
   in Results. Use the same tracked identities for table and trace. Reserve
   unavailable distortion/limit facts too. Keep all fourteen standard summary
   rows visible in the full panel view; short panes prioritize peak slots.
4. Add hold, jitter, matching, ordering, reset, worker, STOP, pixel-layout and
   capacity regressions. Run focused debug/release suites, exact gate clippy,
   `cargo xtask check --fast` and read-only layout/golden comparison. Explain
   changed references and retain complete logs.
5. Prepare one-section `target/COMMIT_MSG.txt`, plain subject first and
   `Task: 0087`. Hand off for cross-vendor review/integration, without commits,
   pushes, merges or unit access.
6. Round 2: detect recalled sort changes inside the tracker, restore dim empty
   placeholders in both views, drive the layout regression through tracked
   rows, clarify adjacent-row hysteresis and remove the redundant centre reset.
   Prove testable fixes fail before code, then repeat focused debug/release,
   exact clippy, golden comparison and the fast gate.
7. Round 3: preserve foreign golden references when publishing the manifest
   and owned captures. Add a Python regression and prove it fails before the
   fix with `python3 -I`. Restore task 0074 updater references from HEAD and
   make missing references fail loudly. Rerun golden-update, compare the full
   reference inventory, then run debug/release UI tests, exact clippy and the
   fast gate. Correct evidence and add `Tested:` to the proposed message.
   If the complete UI suites expose task-local layout guard failures, fix the
   spectrum test fixtures to use actual UI/canvas dimensions and rerun checks.

Reserved paths: `docs/tasks/0087-spectrum-peak-hold.md`, `ui/src/ui/panes.rs`,
`ui/src/ui/panes/`, `ui/src/ui/analysis.rs`, `ui/src/ui/analysis/peaks*`,
`ui/src/ui/analysis/spectrum_view.rs`, `ui/src/ui/analysis/window.rs`,
`ui/src/ui/analysis/window/peak_tests.rs`, `docs/spec/scpi.md`,
`ui/tests/spectrum.txt`, affected `ui/tests/golden/` captures/manifest,
and worktree-local `target/` evidence.
Round 3 also reserves `ui/tools/golden_run.py`, `ui/tools/test_golden.py`,
`ui/src/ui/update/text_tests.rs` and `ui/tests/golden/update/` as explicitly
assigned by the owner.

## Status

- 2026-11-17: Round 3 requested fixes and host validation complete on the
  uncommitted candidate at `8fafc719`. Focused debug/release suites all exit 0:
  updater text 4, peak 34 (two existing opt-in performance guards ignored),
  spectrum 14, sort-touch 1 and layout guard 1 per profile. Python golden
  tests pass 9 with `python3 -I`; the new publish regression fails before the
  fix, exit 1. Missing-reference negative control exits 101 with the explicit
  filename/read error; restored references still match HEAD. Logs:
  `round3/{debug,release}-{update-text,peak,spectrum,sort-touch,layout-budget}-final.log`,
  `round3/python-final.log`, `round3/missing-reference.log`.
  Complete `cargo test --offline --locked [--release] -p oos-ui --no-fail-fast`
  runs both exit 101: main debug 1275 passed, release 1274 passed; both have
  exactly the same 35 socket failures and 47 ignored. Local TCP bind is denied
  by this sandbox, confirmed independently; no other main-suite failure
  remains. All integration targets pass in both profiles: golden 5 (one
  explicit-update export ignored), host-script 14, soak 11. Release golden
  comparison 116.14 s. Complete logs: `round3/{debug,release}-ui-final.log`;
  commands/exits: `round3/final-exits.json`; matched failure sets:
  `round3/full-ui-failures.json`. All evidence paths are under
  `target/t0087-evidence/`. Final golden hashes confirm all 1260 restored
  files unchanged, updater files match HEAD and no transaction leftovers:
  `round3/final-golden-preserved.json`. Only the previously documented twelve
  pane pictures, six new spectrum pictures and manifest differ from HEAD.
  Final exact clippy and fast pass as recorded below, including the printed
  `partial gate passed (not done): 8 steps in 36 s`. Proposed message now has
  `Tested:` and the complete-suite sandbox limitation. Task remains active
  for Claude re-review and coordinator validation where local sockets are
  available, then the integrated default full gate. All changes intentionally
  remain uncommitted; no commits, pushes, merges or unit access. Hardware
  remains unverified; no ratchet, hook, dependency or rule changes.
- 2026-11-17: Round 3 final debug complete UI run (`--no-fail-fast`)
  exits 101 solely for the 35 unavailable local socket tests: main suite
  1275 passed, 35 failed, 47 ignored. The layout guard now passes.
  All integration targets pass: golden 5 (one explicit-update export ignored),
  host-script 14, soak 11. Debug golden comparison takes 113.17 s.
  Log: `round3/debug-ui-final.log`; command/exit summaries in
  `round3/final-exits.json`. Release focused/full verification running.
  Formatted candidate exact clippy passes again, exit 0, host/ARM, no new
  findings; `round3/clippy-verified.log` and
  `target/check-logs/80865-1794873423371246000/`.
  Final fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 36 s`, all eight steps pass.
  Logs: `round3/fast-verified.log` and
  `target/check-logs/80859-1794873423454181000/`.
- 2026-11-17: Round 3 spectrum pixel fixtures now construct canvases from
  `Ui::scr` and slice rows with `Canvas::w`; no drawing or guard change.
  The previously failing layout guard passes in debug, exit 0. Final debug
  focused suites also pass: updater text 4, peak 34 (two existing opt-in
  performance guards ignored), spectrum 14, sort-touch 1. Logs:
  `round3/debug-{update-text,peak,spectrum,sort-touch,layout-budget}-final.log`.
  Baseline release UI exits 101 too: 1273 passed, 36 failed, 47 ignored,
  with the same socket and fixture failures; all four release updater text
  tests pass. Full suites are being rerun with `--no-fail-fast` so integration
  targets also execute. A direct local TCP bind probe confirms sandbox
  `PermissionError: [Errno 1] Operation not permitted` (`round3/socket-probe.log`).
  First post-fixture fast run exits 1, printing `gate FAILED in 46 s: fmt`;
  only fmt fails, requiring one expression to collapse to a single line.
  Applied cargo fmt and rerunning fast/exact-clippy. Failed-run logs:
  `round3/fmt-final.log`, `round3/fast-final.log` and
  `target/check-logs/63602-1794873354043687000/`.
- 2026-11-17: Round 3 complete debug UI run exits 101: 1274 passed,
  36 failed, 47 ignored. Thirty-five failures involve local socket listeners
  unavailable in the sandbox (direct calls report `Operation not permitted`).
  One task-local failure is real: `layout::tests::layout_panel_budget`
  rejects ten fixed panel constant uses in the new spectrum pixel tests.
  Fixing those test fixtures to use UI/canvas dimensions, without changing
  the layout guard or drawing code. Updater text suite already passes all
  four tests in debug, exit 0. Logs: `round3/debug-{ui,update-text}.log`;
  baseline release verification running before the fixture correction.
- 2026-11-17: Round 3 golden-update exits 0, printing
  `PASS: 266 cases, 532 fresh processes, 1254 exact-repeat captures`;
  renderer 139.889 s. All 1260 reference files are byte-identical to the
  restored pre-run tree, including all four updater files. Against HEAD,
  exactly the twelve pane pictures listed below and manifest change; six new
  spectrum pictures are `demo/{normal,large,larger}/spectrum/` with basenames
  `s0-spectrum-qualifying.png` and `s1-spectrum-tracked.png`. No tracked
  reference is missing. Evidence: `round3/golden-update.log`,
  `round3/golden-before.json`, `round3/golden-diff.json` under
  `target/t0087-evidence/`. The first inventory assertion used the wrong
  expected s1 basename; corrected to the actual tracked-picture name and
  the complete inventory comparison passes, exit 0.
  Exact unchanged gate `steps::clippy` passes host/ARM, exit 0, no new
  findings (UI one improvement). Scratch harness differs only in entry point
  and root lookup: `round3/clippy-harness.json`; logs: `round3/clippy.log`
  and `target/check-logs/62661-1794872919191080000/`.
  Fast gate exits 0: `partial gate passed (not done): 8 steps in 35 s`;
  all eight selected steps pass. Logs: `round3/fast.log` and
  `target/check-logs/62660-1794872914850503000/`. Default/full and image
  checks remain unselected. Complete debug/release UI suites running.
- 2026-11-17: Round 3 publisher now stages the existing reference tree,
  replaces manifest-owned captures/metadata and publishes with the existing
  rename/rollback transaction. Foreign subtrees survive, including beneath
  `demo/`; stale owned captures are removed. The new regression fails on the
  old code, exit 1, then all nine Python golden tests pass with `python3 -I`,
  exit 0. Logs: `target/t0087-evidence/round3/python{,-before}.log`.
  Restored all four updater files byte-for-byte from HEAD; their diff is empty.
  The requested checkout was denied because the sandbox cannot write the
  shared Git index; restoration used read-only `git show HEAD:<path>` instead.
  Updater picture tests now panic with the reference path and read error
  instead of substituting empty bytes. Golden-update and host checks running.
- 2026-11-17: Round 3 started on the existing uncommitted candidate at
  `8fafc719`. Verification P1 confirmed: golden-update deleted the unrelated
  task 0074 updater references. The earlier unchanged-reference claim below
  was incorrect. Plan and added paths recorded before code. No commits or
  unit access; host verification and cross-vendor re-review remain pending.
- 2026-11-16: Round 2 requested host validation complete on the uncommitted
  candidate at `8fafc719`. Final read-only
  `sh ui/tools/layout-check.sh golden-check` exits 0, printing
  `PASS: 266 cases, 532 fresh processes, 1254 exact-repeat captures`;
  Cargo comparison passes in 153.64 s, renderer 152.006 s, 627 references.
  Log: `target/t0087-evidence/round2/golden-check.log`; review artifacts:
  `target/golden/release/run-eajp84wq/`. Focused debug/release suites, exact
  clippy and all eight fast steps pass as recorded below. Fast result remains
  `partial gate passed (not done): 8 steps in 51 s`, exit 0. Commit-message
  format/privacy and final diff whitespace checks pass, exit 0. Task stays
  active for Claude verification of these fixes and the coordinator's final
  integrated default full gate. No commits, pushes, merges or unit access;
  hardware remains unverified. No ratchet loosening or rule changes.
- 2026-11-16: Round 2 golden update exits 0: `PASS: 266 cases, 532 fresh
  processes, 1254 exact-repeat captures`, 152.312 s rendering. Complete log:
  `target/t0087-evidence/round2/golden-update.log`. Pixel comparison against
  saved round-1 references finds exactly three changed pictures, solely the
  restored dim placeholder, with no summary movement:
  `ui/tests/golden/demo/normal/spectrum/s0-spectrum-qualifying.png`,
  `ui/tests/golden/demo/large/spectrum/s0-spectrum-qualifying.png`,
  `ui/tests/golden/demo/larger/spectrum/s0-spectrum-qualifying.png`.
  All other 624 pictures, including Results, are byte-identical; manifest
  refreshed. New Results empty-state pixels are covered by the direct renderer
  regression. Visually inspected the new qualification captures at all three
  text sizes. Pixel counts/bounds and before/after/diff reports:
  `round2/golden-diff.json`, `round2/golden-diffs/` under the evidence directory.
  Updated `target/COMMIT_MSG.txt` with all reference reasons and exact round-2
  paths. Read-only golden comparison remains running.
- 2026-11-16: Round 2 final focused suites pass in debug and release: peak 34,
  spectrum 14 and sort-button touch 1 per profile, all exit 0. Peak runs ignore
  the same two existing opt-in performance guards. Commands and complete logs:
  `target/t0087-evidence/round2/{debug,release}-{peak,spectrum,sort-touch}.log`;
  exits in `focused-exits.json`. Exact original `steps::clippy` passes host
  and ARM for all applicable crates, exit 0, no new findings; UI has one
  improvement. Logs: `round2/clippy.log` and
  `target/check-logs/19452-1794868168064290000/`. Scratch harness sources match
  repository gate sources except entry point and root lookup. The initial
  harness launch exited 101 because its standalone lock needed pruning;
  reran offline after allowing Cargo to refresh that scratch lock only.
  Repository lock, ratchets, hooks and rules remain untouched.
- 2026-11-16: Round 2 implementation complete for P2 and P3.1-P3.4. Table
  remembers the applied sort mode and forces sorting on either transition in
  refresh/tick, retaining marker numbers. Removed the separate table-sort call.
  Recall regression exercises both directions with unchanged peaks and a
  0.5 dB level difference. Empty lists and Results show the original dim
  placeholder during qualification and after release; retained rows suppress it.
  SpecPeak leaves tracks to the existing grid-change reset. SCPI paragraph now
  documents adjacent-row hysteresis and uses the surrounding line width.
- 2026-11-16: Round 2 regressions for recall, placeholder pixels and redundant
  SpecPeak reset each fail against round 1, exit 101, then pass after correction.
  Logs: `target/t0087-evidence/round2/{recall,placeholder,centre}-before.log`.
  Retained and corrected the fluctuating-count test: it now qualifies tracker
  rows, asserts their actual count and checks six geometry/pane combinations.
  The existing pixel test separately covers live appearance/release transitions.
  An initial corrected fixture hit floating-point 1 s rounding; starting its
  synthetic clock at zero fixes the fixture (`debug-peak.log` has the final run).
- 2026-11-16: Round 2 focused debug tests pass: peak 34 (two existing opt-in
  performance guards ignored), spectrum 14, sort-button touch 1, all exit 0.
  Fast gate exits 0, printing `partial gate passed (not done): 8 steps in 51 s`.
  Evidence: `target/t0087-evidence/round2/{debug-*,fast}.log`, complete gate
  logs under `target/check-logs/` as named by `fast.log`. Release tests, exact
  clippy and golden verification still running; no full-gate claim.
- 2026-11-16: Round 2 started on the existing uncommitted candidate at
  `8fafc719`, same accepted claim and reserved paths. Claude review findings
  P2 and P3.1-P3.4 accepted for correction. No commits or unit access authorized.
- 2026-11-16: Host candidate remains active for coordinator handoff. Independent
  Claude review (Codex author), integrated default full gate and any required
  owner-authorized unit round remain pending. No commits or unit access.
  `target/COMMIT_MSG.txt` has one message and `Task: 0087`; its privacy scan
  passes, exit 0. No ratchet, hook, dependency or architecture-rule edits.
- 2026-11-16: Latest fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 36 s`
  (`target/t0087-evidence/fast-verified.log`, complete logs in
  `target/check-logs/20313-1794861220695799000/`). All eight selected steps
  pass; full/default and image steps are not selected. Final exact clippy
  rerun exits 0, host and ARM for all applicable crates, no new findings;
  UI has one improvement (`target/t0087-evidence/clippy-verified.log`,
  complete logs in `target/check-logs/90870-1794861476005488000/`). A scratch
  harness invokes unchanged `steps::clippy`; repository gate code is untouched.
- 2026-11-16: Focused tests pass in both debug and release: peak suite 31,
  spectrum suite 13, existing sort-button touch regression 1, all exit 0.
  Logs: `target/t0087-evidence/{debug,release}-peak-verified.log`,
  `{debug,release}-spectrum-verified.log`, `{debug,release}-sort-touch.log`.
  Peak suites explicitly ignore two pre-existing timing/step-count guards
  requiring `OOS_PERF_TESTS=1`; they are unrelated to this tracker.
- 2026-11-16: Final `sh ui/tools/layout-check.sh golden-check` exits 0:
  `PASS: 266 cases, 532 fresh processes, 1254 exact-repeat captures`;
  Cargo's comparison test passes (120.66 s). References contain 627 captures.
  Log: `target/t0087-evidence/golden-check.log`; review run:
  `target/golden/release/run-tehx2x57/`. Update log:
  `golden-update-verified.log` (147.932 s, same case/repeat counts).
  Inspected all full-view text sizes and before/after spectrum/Results panes.
- 2026-11-16: Golden reasons: twelve existing `demo/{normal,large,larger}/panes/`
  p0/p1 spectrum and p2/p3 Results pictures change only in the right table:
  reserved slots, tracked identities and fixed side-row/column spacing.
  Six new `demo/{normal,large,larger}/spectrum/` pictures cover qualification
  and confirmed peaks with all fourteen summary rows visible. Other existing
  generated pictures are unchanged. The unrelated updater references were
  accidentally deleted by golden-update, discovered in round 3; the earlier
  assertion that they were unchanged was incorrect. Manifest updated.
  Pixel counts/bounds: `target/t0087-evidence/golden-diff.json`. Six review
  PNGs are copied under the evidence directory as `{size}-s{0,1}-spectrum-*.png`.
  Visual QA caught and fixed lost summary rows by reclaiming the unused
  analysis zoom-button row and using fixed minimum row spacing.
- 2026-11-16: Negative controls fail with exit 101: exact parent
  `8fafc719` peak-count layout formula violates the eight-row contract;
  actual tracker tests with zero appearance/release holds fail four cases.
  Controls use scratch only and never patch production sources. Driver:
  `target/t0087-evidence/negative-control.py`; logs/exits: `negative/` there.
- 2026-11-16: Implementation uses bounded fixed arrays without per-frame
  tracker allocation. Worker results preserve main's Work state; only accepted
  results update detections, and frame ticks advance existing timers. Grid,
  RBW, source/window/trace changes and lifecycle reset clear identities. Raw
  lines stay immediate; SCPI has no numbered peak-table query and continues
  to calculate from raw waveforms, documented in `docs/spec/scpi.md`.
- 2026-11-16: Earlier feedback retained under `target/t0087-evidence/`:
  initial fast gate printed `gate FAILED in 83 s: size, known-failures, agent-rules`
  (exit 1). Fixed extra Ui impl-file/State-field growth through free-function
  extraction and existing Work ownership; disk exhaustion also interrupted
  initial command/log retention. Removed only local disposable incremental
  artifacts. Corrected extraction compile errors and two new renderer lints.
  An early golden build rejected changed source during compilation, exit 1,
  before binary consumption. Later visual QA fixed full-view capacity; all
  final functional and golden results above include that fix.
- 2026-11-16: Owner-assigned claim on `t0087-spectrum-peak-hold`, clean base
  `8fafc719`. Plan recorded before code. Read repository architecture, picture,
  development, delivery, task and review/integration rules. This oscilloscope
  repository is outside the OptiSource-only operating model.
