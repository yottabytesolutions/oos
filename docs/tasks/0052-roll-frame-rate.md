---
id: 52
title: "Roll at slow time bases redraws below 1 frame/s"
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

# Roll at slow time bases redraws below 1 frame/s

## Goal

Roll at 200 ms/div redraws at about 30 frames/s with a bounded step time.

## Plan

Reserved paths: `docs/tasks/0052-roll-frame-rate.md`, `ui/src/ui/script/roll.rs` (new host
profile helper), `ui/src/ui/script.rs`, `ui/src/ui/analysis.rs`,
`ui/src/ui/analysis/` (worker scheduling, DSP context and regression tests), `ui/src/ui/eyepage.rs`,
`ui/src/ui/bwtest.rs`, `ui/tests/eye.txt` and its new golden pictures under
`ui/tests/golden/demo/`, `ui/tests/golden/manifest.txt`, `docs/architecture/records.md`,
`docs/bandwidth-test.md`, `ui/src/ui.rs` (measurement job dispatch and poll),
`ui/src/ui/lifecycle.rs` (existing worker). No model/settings, G2
harness or doctor edits.

1. Revert analyser input caps. Run Spectrum, Eye, Power and Bandwidth on full
   measurement windows through the persistent measurement worker. Main only submits
   bounded screen records with shared raw frames and displays finished results.
2. Tag jobs/results with acquisition stamp, measurement reset generation and view
   configuration revision. Drop stale results; coalesce busy updates to the newest
   record. Keep offline scripts deterministic through the existing synchronous
   measurement mode. Show an updating marker when a finished view lags.
3. Pin zero main-step analysis samples with the busy-worker roll/deep regressions.
   Test deep 1 MHz Spectrum, 115200 Bd UART Eye, whole-cycle Power, Bandwidth and
   stale completion filtering. Clamp the displayed span to actual Nyquist.
4. Preserve M1 references; regenerate only the six new Eye captures for full data.
   Run focused debug/release tests and fast gate only, prepare
   `target/COMMIT_MSG.txt`, and hand off for Claude verification. No commit or unit
   access is authorized.

Review fixes (standard tier, base `45bfff1`): retain the accepted full key and mark
settings-stale results immediately; derive span labels from the model and current
record Nyquist. Separate worker history identity from in-flight acceptance so only
selected-source data settings, analyser settings and explicit clears reset pooling.
Recover failed worker sends synchronously with a diagnostic. Add raw-DSP/offline
equivalence and envelope/pair measurement assertions. Run focused debug/release
tests, fast and full gates, then leave one commit-message section without commits.

Unit round 0 (2026-11-15) measured 0.6 frames/s at 200 ms/div with the step time growing from 1.0 to 1.3 s, on overhaul and on dho804 alike: pre-existing. Reproduce on the host with the scripted batch harness (roll route), find where the step time goes and why it grows, fix with a failing-before test, confirm on the unit.

Verification follow-up (base `f4405dc`): restart only Bandwidth history/throttling
when the acquisition stamp, time scale, delay or memory depth changes. Include
Spectrum reference and margin in full acceptance and Spectrum history identity.
Pin both with failing-before tests, preserve Spectrum/Eye pooling, run focused
debug/release tests and the fast gate, then replace the single commit-message
section. No full gate, commits or hardware access in this sandbox follow-up.

## Status

- 2026-11-15: Verification follow-up handoff, active and uncommitted on
  `t0052-roll-fps`, HEAD `f4405dc` plus the two narrow P2 fixes and tests.
  Focused debug and release checks each exit 0: 41 passed, 2 manual Eye checks
  ignored per profile. Commands: `cargo test --locked -p oos-ui [--release]
  --bin oos-ui FILTER -- --nocapture`; FILTER = `ui::analysis::window`,
  `ui::eyepage`, `ui::bwtest`, `the_pane_follows_a_square_wave`,
  `bounded_analysis_work`, `layout_panel_budget`. Exact commands and exits:
  `target/task0052-verification/focused-exits.txt`; complete logs:
  `target/task0052-verification/{window,eye,bw,pane,bounded,layout}-{debug,release}.log`.
  Fast gate exits 0: `partial gate passed (not done): 8 steps in 20 s`,
  `target/task0052-verification/fast.log`. Full gate intentionally skipped per
  the sandbox assignment. `target/COMMIT_MSG.txt` replaced with one section.
  `git diff --check` passes; commit-message privacy scan reports 0 findings.
  All six changed/new paths are intentional source/tests and task/records docs;
  no index changes, commits, pushes, merges or hardware access. Codex retains
  ownership; next action: Claude verification of these two P2 fixes, then the
  coordinator's full integration gate and previously recorded unit follow-up.
  This handoff does not establish full-gate or hardware acceptance.

- 2026-11-15: Both remaining P2 fixes implemented under the existing claim.
  Bandwidth-only history identity now includes acquisition stamp, time scale,
  delay and memory depth; a change discards the previous result and throttle.
  Spectrum/Eye pooling identity remains independent of those fields. Spectrum
  reference and margin now participate in both full acceptance and Spectrum
  history keys. Six new regressions fail on unchanged parent production code,
  exit 101 (`target/task0052-verification/before-debug.log`), then pass with the
  fixes. They pin the four Bandwidth identity changes within 0.5 s, same-identity
  throttling, completion acceptance after STOP, and stale marking/rejection plus
  averaging/waterfall restart for each Spectrum level change. An initial test
  compile used a nonexistent SINGLE run-state variant; corrected to the existing
  acquisition-stamp transition before the failing-before run. Trial log:
  `target/task0052-verification/test-compile.log`. Final checks follow.

- 2026-11-15: Accepted the two remaining P2 verification fixes at `f4405dc` on
  `t0052-roll-fps`, clean tree. Existing Codex claim and reserved analysis/tests
  and records documentation paths apply. Plan above recorded before code.
  Earlier five fixes are verified by the owner. Next: failing-before regressions,
  narrow key fixes, focused debug/release and fast-gate evidence for Claude.

- 2026-11-15: Final review-fix handoff, active and uncommitted on `t0052-roll-fps`,
  HEAD `45bfff1` plus the intentional review-fix diff. Final focused debug and
  release tests exit 0: 35 passed, 2 manual Eye checks ignored per profile.
  Complete commands/results: `target/task0052-r2/final-focused-exits.txt` and
  `target/task0052-r2/final-*-{debug,release}.log`. Final fast gate exits 0:
  `partial gate passed (not done): 8 steps in 24 s`,
  `target/task0052-r2/fast-final.log`. Final full `cargo xtask check` exits 1:
  `gate FAILED in 586 s: tests-debug, tests-release, coverage, python-tests`.
  Complete output: `target/task0052-r2/full-final.log`; individual command
  logs and exit codes: `target/check-logs/47378-1794760412426974000/`.
  Clippy, perf, doctor, all eight fast steps and script syntax pass. Rust test
  failures are the existing local socket/listener cases denied by this sandbox.
  Coverage stops at the instrumented HAL WLAN socket failure, before producing
  coverage evidence. Python's HostSockets test likewise cannot start its local
  listeners. No lint, ratchet, test exemption or permission change was made.
  The two lint failures from the first full attempt are fixed and final clippy
  passes. P2 fixes and P3-1/P3-3/P3-5 are implemented; P3-2/P3-4 remain the
  recorded follow-ups below. `target/COMMIT_MSG.txt` contains one commit section.
  Final privacy checks: message 0 findings; tree 151 pre-existing findings,
  none in changed paths (`target/task0052-r2/privacy-*-final.log`).
  `git diff --check` passes and the index is unchanged.
  Remaining source/tests/docs are intentional. No commits, pushes, merges or
  hardware access. Codex retains task ownership; next action: Claude standard
  verification of P2 fixes, coordinator full gate with local sockets available,
  then the authorized unit round including measurement-result age. Full gate
  did not pass, so this is a review-fix handoff, not task completion.

- 2026-11-15: Review fixes implemented, still active and uncommitted on
  `t0052-roll-fps` at `45bfff1` plus the working diff. P2-1 retains the accepted
  full key in Work, marks settings mismatches immediately and derives span
  labels from the current model band and record Nyquist. P2-2 uses a distinct
  worker history key with only active-analyser settings, selected-source data
  settings, acquisition mode, layout and local clear revision. Tests preserve
  Spectrum Average/MaxHold counts, waterfall and limit counters across CH3
  position edits and SINGLE stamps; source-data changes restart them. Eye hits
  pool across SINGLE stamps and restart on EyeClear. Full acceptance/drop keys
  remain unchanged. P3-1 logs disconnected sends and recovers synchronously.
  P3-3 compares run() with direct Spectrum/Power DSP on nonzero-offset raw
  slices and with the meas_bg=None synchronous path. P3-5 assertions confirm
  Eye threshold and Spectrum f1 match measurement-path values on raw envelope,
  screen pair and HiRes records. Added implementation/test files stay under
  the claimed `ui/src/ui/analysis/` prefix; no model/core or held paths changed.
  Failing-before command exits 101: three review regressions fail on the parent
  production code (`target/task0052-r2/before-debug.log`). Final focused
  debug/release commands exit 0: 35 passed and 2 manual Eye checks ignored per
  profile. Commands: `cargo test --locked -p oos-ui [--release] --bin oos-ui
  FILTER -- --nocapture`, FILTER = `ui::analysis::window`, `ui::eyepage`,
  `ui::bwtest`, `the_pane_follows_a_square_wave`, `bounded_analysis_work`,
  `layout_panel_budget`. Logs: `target/task0052-r2/*-{debug,release}.log`.
  Fast gate exits 0: `partial gate passed (not done): 8 steps in 20 s`,
  complete log `target/task0052-r2/fast.log`. Full gate initially found two lint failures in the committed rework:
  manual_is_multiple_of in the window UART test, and items_after_test_module
  in Bandwidth. Fixed the expression and placed the existing Bandwidth tests
  last; no behaviour or ratchets changed. All 35 focused tests pass again per
  profile after that cleanup (`target/task0052-r2/final-*-{debug,release}.log`).
  The initial full run also encounters denied local sockets in existing suites;
  final full-gate evidence follows. Initial full command exits 1:
  `gate FAILED in 785 s: clippy, tests-debug, tests-release, coverage, python-tests`
  (`target/task0052-r2/full.log`, with complete per-command logs linked there).
  Debug UI: 600 passed, 21 socket/listener failures, 30 ignored. HAL debug and
  release: 340 passed, 1 denied WLAN socket, 8 ignored. Instrumented HAL: the
  same WLAN failure, so coverage is unavailable. Python: 108 executed, one
  HostSockets error from denied host startup. Perf and script syntax pass.
  Final fast command exits 0: `partial gate passed (not done): 8 steps in 24 s`,
  `target/task0052-r2/fast-final.log`. Final full result is recorded above. An intermediate new-test compile failed on missing DSP imports,
  then passed after explicit imports. No reference pictures changed.

- 2026-11-15: Accepted review-fix assignment on `t0052-roll-fps`, HEAD `45bfff1`,
  clean tree. P2-1/P2-2 and P3-1/P3-3/P3-5 are in scope under the existing claim.
  Chosen stale-display policy: retain the accepted picture with an immediate
  updating marker when its full key no longer matches current settings.
  P3-2 follow-up: Codex/coordinator to measure ordinary measurement-result age
  behind analysis on the shared worker during the authorized unit round.
  P3-4 follow-up: async dispatch and the updating marker still lack golden
  picture coverage; extend the capture harness in a follow-up. Existing offline
  goldens exercise the deterministic synchronous path. No hardware access.

- 2026-11-15: Review round 1 fix-first handoff, still active and uncommitted.
  Full inputs restored for Spectrum, Eye, Power and Bandwidth on the existing
  measurement worker. Jobs/results retain raw record identity and filter by
  acquisition/reset tokens, local clear revision and view/source configuration.
  Main polls after STOP, coalesces busy updates and displays finished results
  with an age-based updating marker. Offline scripts keep synchronous measurement.
  P2 span labels use the real Nyquist range; P3 full-frame Bandwidth branch is
  documented as necessary for equivalent-time phase coverage.
  Final focused tests exit 0: 32 passed in debug and 32 in release, with 2 manual
  Eye checks ignored per profile. Commands and full logs:
  `target/task0052-r1/review.md`, `target/task0052-r1/final-*-{debug,release}.log`.
  Correctness pins 1 MHz at 1 MHz in a deep record, a 115200 Bd UART eye at
  1 ms/div, five whole 50 Hz cycles and an early inrush peak, and 5000-bin
  Bandwidth from the full raw frame. In 36 measured main steps per profile
  (roll 200 ms/div and 4 ch 10M, all four views and both Spectrum panes), main
  analysis DSP work is 0 samples; largest observed job copy is 3600 screen
  samples, bounded in implementation by 7200 for two Power sources.
  Guarded full release golden update exits 0: all 615 prior M1 captures,
  metadata and manifest entries remain byte-identical; only six Eye images and
  their hashes now reflect full-window edge statistics. Final ordinary release
  comparison exits 0: 5 passed, 1 explicit-update test ignored; 263 cases,
  526 fresh processes, 1242 exact-repeat captures. Logs:
  `target/task0052-r1/golden-{update,compare-final}.log`.
  Final fast gate exits 0: `partial gate passed (not done): 8 steps in 25 s`;
  complete log `target/task0052-r1/fast-final.log`.
  Intermediate fast checks failed size, architecture and mutable-statics on
  a non-default test module path, then size growth/impl spread. Normal module
  layout and function extraction resolved them without ratchet/limit changes.
  Privacy additions and commit message scans: 0 findings; whole-tree scan
  retains 151 pre-existing findings. `git diff --check` passes.
  `target/COMMIT_MSG.txt` and the review handoff are prepared. Held model/core
  paths are unchanged. No commits, pushes, merges or hardware access. Next:
  Claude verification, coordinator full integration gate, authorized unit
  confirmation of absolute step/fps/SCPI budgets. Full gate not selected here.

- 2026-11-15: Review round 1 fix-first assignment accepted on `t0052-roll-fps`,
  base `7a6ef4f`, Codex author. Replaced the rejected caps plan with full-data
  background analysis. Additional strictly required paths reserved above:
  `ui/src/ui.rs` and `ui/src/ui/lifecycle.rs`; model and core remain held by M2.
  Worker fast gate and targeted tests only; no commit, push or hardware access.

- 2026-11-15: Worker handoff, still active and uncommitted on `t0052-roll-fps`.
  Final focused commands exit 0 in debug and release: `cargo test -p oos-ui
  [--release] --bin oos-ui FILTER -- --nocapture`, for
  `ui::analysis::window`, `ui::eyepage`, `ui::bwtest`,
  `the_pane_follows_a_square_wave`, `bounded_analysis_work`, and
  `layout_panel_budget`: 21 passed per profile, 2 manual Eye tests ignored per
  profile. Complete logs `target/task0052/{window,eye,bw,pane,bounded,layout}-{debug,release}.log`.
  Final release golden comparison exits 0: 5 passed, 1 explicit-update test
  ignored; 263 cases, 526 processes, 1242 exact-repeat captures. Final fast gate
  exits 0: `partial gate passed (not done): 8 steps in 21 s`. No full gate selected.
  `git diff --check` passes. Commit-message privacy scan: 0 findings; tree audit
  retains 151 findings outside changed paths and none in changed paths. Complete
  audit `target/task0052/privacy-tree-fix.log`.
  `target/COMMIT_MSG.txt` and `target/task0052/review.md` are prepared. All remaining
  source, documentation and new Eye references are intentional; no held paths,
  ratchets, dependencies, commits, pushes, merges or hardware access. Codex owns
  follow-up fixes. Next action: coordinator arranges standard cross-vendor review,
  runs the full integration gate, then obtains the task's unit release to confirm
  step, picture rate and SCPI latency. Full-rate deep RF accuracy is not claimed.


- 2026-11-15: Production fix selected and implemented under the final plan above.
  `analysis/window.rs` builds a bounded snapshot of only the selected sources,
  before record cloning or raw-sample copies. The existing analyser dispatch,
  spectrum pane throttling, Eye pooling/refresh and Bandwidth rate cap remain in
  place. No view consumes a record-sized input through that dispatch. The window
  helper owns an added 8-byte cumulative sample counter, grouped with the existing
  8-byte spectrum timestamp for the UI lifetime. A snapshot owns at most 32 KiB
  of samples (two Power sources); Eye owns at most 8 KiB of snapshot samples.
  Copies inside DSP are bounded by those inputs and its existing fixed limits.
  No queues or threads were added. Reserved the two unheld documentation paths
  to describe the changed analysis limits before updating them. The full absolute step and SCPI latency
  budgets still require measurement on the unit; host work counts do not prove
  those absolute timings. M2/model/settings and G2 paths remain untouched.

- 2026-11-15: Failing-before evidence, exit 101: the instrumented uncapped input
  ran `cargo test -p oos-ui --bin oos-ui bounded_analysis_work -- --nocapture`
  and read 99,999 Eye samples in 200 ms/div roll and 2,500,000 in the 4 ch 10M
  fixture, above both the initial 512 and final 2048 budgets. Complete log
  `target/task0052/bounded-before.log`. Final regressions drive `step(dt)` with
  1k, 25k and 100k roll snapshots, or four enabled channels at the 10M depth
  setting with 2.5M raw points per channel in the shown window. They assert actual
  copied analysis work per step (Eye <= 2048, Spectrum/Bandwidth <= 4096, Power
  <= 8192), including both spectrum panes; records and waveform pictures keep
  updating while measurement handoff stays busy. In-memory SCPI `*IDN?;*OPC?`
  executes after every step. Listener sockets are unavailable in this sandbox;
  this checks query handling, not network round-trip latency. No wall-time
  threshold is used for these regressions.

- 2026-11-15: Golden update and comparison exit 0: 263 cases, 526 fresh processes,
  1242 exact-repeat captures. All 615 existing capture entries and their RGB
  hashes, metadata and contracts are identical to the parent. Six new Eye PNGs
  plus metadata cover CH1 and CH2 at normal/large/larger text sizes, including
  the short-window jitter-spectrum hint. Existing references were not changed.
  Update log `target/task0052/golden-eye-update.log`; comparison log
  `target/task0052/golden-fix-final.log`, gallery
  `target/golden/release/run-hsb413zu`. These are intentional Eye coverage additions,
  recorded in `target/COMMIT_MSG.txt`.

- 2026-11-15: Intermediate broad debug run, exit 101: 580 passed, 26 failed,
  29 ignored (`target/task0052/ui-debug.log`). Twenty-one failures reject local
  socket/listener operations with sandbox permission errors; Bode's local
  generator server is among them. Three initial window-size regressions (Eye
  pooling, Bandwidth slow square, spectrum harmonics) drove the final selection
  policy and now pass focused checks. The diagnostic profile collided with
  process-global batch counters under parallel tests; it is now an explicit
  ignored host timing test and passes when invoked alone. The parallel run also
  failed `layout_panel_budget` because the new fixture used panel constants;
  fixed it to use `ui.scr.w/h` and included that layout check in focused reruns.
  No layout rule or timing threshold was changed. This broad run is not a passing test-suite claim.

- 2026-11-15: Initial fast gate exit 1: size failed because `analysis.rs` grew
  722 -> 726 lines and `Ui::analyzer_update` 90 -> 92 code lines; the other seven
  steps passed. Complete logs `target/check-logs/37645-1794756083069495000/`.
  Extracted window selection and spectrum-follow policy into the claimed helper;
  no ratchet, lint or limit changes. Latest code gate exit 0: `partial gate passed
  (not done): 8 steps in 21 s`, log `target/task0052/fast-gate-fix-final.log`.


- 2026-11-15: Owner confirmed Eye on both round-0 builds, CH2 on at 200 ms/div
  roll, plus 4 ch 10M at 0.1 fps, 6 s steps and SCPI timeouts. This resolves the
  missing setup from the investigation below. Continued with the bounded-window
  plan above; reserved the additional unheld analysis and Eye golden paths before
  editing. Tier standard; fast gate only; no commits or unit access.

- 2026-11-15: Final golden rerun exit 0: `cargo test -p oos-ui --release --test golden -- --nocapture`.
  5 passed, 0 failed, 1 explicit-update test ignored; all 260 cases, 520 fresh
  processes, 1230 exact-repeat captures pass existing references. Complete log
  `target/task0052/golden-final.log`, gallery `target/golden/release/run-rmh7r8yd`.
  Production and reference pictures remain unchanged. This closes the host
  investigation checks, not task 0052's fix or unit acceptance.

- 2026-11-15: Final fast gate exit 0: `partial gate passed (not done): 8 steps
  in 35 s`; complete log `target/task0052/fast-gate-final.log`. `git diff --check`
  passes. Full gate remains for integration, after a production fix and review.

- 2026-11-15: Final host profile runs passed in debug and release for Scope,
  Spectrum, Bandwidth (`step`) and Eye, exit 0 for all eight commands. Command:
  `OOS_ROLL_PROFILE_VIEW=VIEW cargo test -p oos-ui --bin oos-ui [--release] growing_roll_step_profile -- --nocapture`.
  Complete logs: `target/task0052/profile-{scope,spectrum,step,eye}-{debug,release}.log`.
  Host: macOS arm64, Rust 1.99.1, normal workspace debug/release profiles. Release
  100k-point step times: Scope 1.60 ms, Spectrum 5.27 ms, Bandwidth 21.92 ms,
  Eye 29.77 ms. No production change or demonstrated unit-regression fix.
  `target/COMMIT_MSG.txt` describes the investigation only; no commit, push,
  merge or hardware access. Owner Codex retains the unfinished task; next action
  is to reproduce the recorded active analyser/pane/signal, then pin and fix
  its cost with a failing-before test. Standard cross-vendor review remains open.
- 2026-11-15: Commit-message privacy scan: 0 findings. Working-tree privacy audit
  reports 151 findings, all outside the three changed paths; preserved in
  `target/task0052/privacy-tree.log`. No unrelated privacy changes. No held
  M2/G2/task 25 files or ratchets changed; all remaining changes are intentional
  investigation code and this task record.

- 2026-11-15: Investigation remains active, no production fix chosen. The detailed
  round-0 `checks_trial.txt` attributes the slow step to analysis: roll-start step
  391.8 ms, acquisition 1.0 ms, drawing 4.5 ms, analyse 397.1 ms (independently
  smoothed fields). Later short roll logs omit analyser settings and parts. The
  100k-point buffer fills within about 2 s at 200 ms/div; the later 1016 to 1325 ms
  rise over tens of seconds can be the 0.05 profiler smoothing approaching a
  steady cost, rather than continued buffer growth. The exact active analyser,
  pane and signal in that run are not recorded. Requested that context from the
  owner before selecting a fix; this is not yet a failing-before regression.
- 2026-11-15: Added a test-only growing-snapshot profile beside the M1 batch runner.
  It first exercises the existing roll batch route, then publishes 1k, 25k, 50k
  and 100k-point overlapping roll snapshots (20 us point interval, envelope flag,
  200 ms/div), and checks consumption through the real UI step. It times isolated
  record conversion/drawing and the existing step parts. This exercises the
  consumer, not the hardware producer. Select Scope (default), `spectrum`, `step`
  or `eye` with `OOS_ROLL_PROFILE_VIEW`. Debug measurements at 100k points before
  any fix: Scope 21.7 ms, Spectrum 48.2 ms, Bandwidth 141.0 ms, Eye 580.3 ms;
  conversion and drawing each below 1 ms. These synthetic signal timings do not
  establish which analyser caused the unit failure.
- 2026-11-15: Baseline `cargo test -p oos-ui --release --test golden -- --nocapture`
  passed, exit 0: 5 passed, 0 failed, 1 explicit-update test ignored. All 260 cases,
  520 fresh processes and 1230 exact-repeat captures passed existing references;
  complete log `target/task0052/golden-before.log`, gallery under
  `target/golden/release/run-6w2ntv4c`. No reference edits. `cargo xtask check --fast`
  exit 0: `partial gate passed (not done): 8 steps in 25 s`; complete log
  `target/task0052/fast-gate.log`. Full gate, independent review and unit
  confirmation are not selected in this worker job.
- 2026-11-15: Diagnostic attempts: first compile failed because a test in
  `ui/tests/roll` could not reach private `script::batch`; moved the helper into
  `ui/script/roll.rs` and reran successfully. A separate Protocol Watch probe
  panicked at `dsp/src/identify.rs:846` (unsigned subtraction in `analog_burst`)
  on the synthetic triangle stream. Logs `target/task0052/profile-watch-before.log`
  and `target/task0052/profile-watch-backtrace.log` preserve the failure. Watch is
  not enabled by the retained profile and no unrelated DSP fix is included.

- 2026-11-15: Codex claimed the assigned task on `t0052-roll-fps`, base `18ac5c7` (clean tree). Plan recorded before code; standard review, no commits or hardware access. The assignment accepts this claim and excludes the held M2/G2/task 25 paths.

- 2026-11-15: seeded by the coordinator.
