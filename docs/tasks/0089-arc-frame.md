---
id: 89
title: Share deep frames through publication and UI intake
kind: chore
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Remove sample-vector copies after acquisition on the deep-frame main path (perf plan P1),
and eliminate the duplicate identical main-view conversion, while keeping pictures
and acquisition plans exact. No commits or hardware access.

## Plan

1. Inventory acquisition, publication, intake, record, measurement and render ownership;
   capture unchanged-base release deep-step timing and acquisition plan matrix.
2. Move acquired frames into shared Arc ownership at publication. Carry those handles
   through the UI queue and into the already shared Record.raw. Preserve queue bounds,
   identity rejection, STOP, roll/scan and segmented publication semantics.
3. Pin sample pointer identity across actual deep Ui::steps and observe transient
   sample-sized allocations, including shallower pairs. Keep goldens unchanged.
4. Round 2: borrow math, identification and autoset raw inputs; strengthen queued raw
   and shallow-pair census coverage; remove ineffective clone guards/diagnostics.
   Reuse the acquired display record only after checking frame and conversion inputs,
   with exact-picture, resample-count and real double-conversion ratio controls.
5. Run debug/release tests, exact clippy, goldens, plan-matrix comparison, touched-path
   perf guards and the fast gate. Record results and prepare target/COMMIT_MSG.txt.

Reserved paths: docs/tasks/0089-arc-frame.md; docs/architecture/README.md;
ui/src/board/mod.rs; ui/src/board/dho/publication.rs; ui/src/board/dho/roll.rs;
ui/src/board/dho/hw/segmented.rs; ui/src/board/dho/hw.rs; ui/src/wave.rs;
ui/src/perfguard/deep.rs; ui/src/perfguard/deep/tests.rs; ui/src/ui.rs;
UI/source test and scripted fixture consumers strictly required by the Arc handoff.
Additional required paths: ui/src/board/record.rs; ui/src/drawpipe.rs;
ui/src/ui/script/batch.rs; ui/src/ui/script/roll.rs; docs/spec/state-query.md.
Reservations follow the owner assignment of this P1 worktree. No ratchet changes.

## Copy inventory

| Path / operation | Previous sample ownership | Disposition |
|---|---|---|
| Acquisition decode (`hw/raw`, batch, raw reread, segmented read) | DMA bytes decoded into owned channel Vecs | Unchanged acquisition work; Vecs move into a single Arc at publication. No new sample copy. |
| `publication::push_frames`, `publish_held` | Latest Frame cloned from/to queue under Shared | Queue and latest now clone the same Arc. Count, sample bounds, stale rejection and final acknowledgement preserved. |
| `roll::publish`, `publish_roll`; segmented read publication | Snapshot cloned for latest plus queue | Move snapshot into Arc and share handles. Producer roll ring/sweep snapshot construction remains acquisition-side work. |
| `Source::probe` | Cloned resident Frame before selecting probe points | Resident handle clone only; requested bounded probe output still samples into its own output Vec. |
| `Source::drain_newest`, `acquire`; mixed Bus candidate | Frame moved from queue, sometimes wrapped in a new Arc | Carry existing Arc through drain, acceptance and record conversion. Replay/segment shown frames have a single owner and move into Arc at conversion. |
| `Ui::step` fast raw selection | `f.clone()` copied all samples before `hw_record` | Now a handle clone. Fast drawing takes the queue Arcs directly. |
| `hw_record`, `board::to_record`, `resample_record` | Converted screen arrays plus Raw Arc clone | Same screen resampling, at most 3600 values/channel; retain the acquisition Arc, with identical window and trigger. |
| `drawpipe::draw_frame` record fallback | Full frame copied, then resampled again after intake | Share/borrow frame; reuse the acquired deep-pair record only after exact frame, window, grid, normal mode and analog-edge input checks. Other cases retain independent `display_record`. |
| Record clones, pending measurements, history and STOP | Screen Vecs cloned; Raw already Arc; history removes Raw | Deep samples stay shared. Bounded screen copies and history policy unchanged. |
| `meas_frame`, `meas_slice`, ordinary measurement jobs | Arc clone or borrowed sample window; fallback Own clones screen Vec | Already shared. Retain and pin pointer identity. |
| Background analysis `analysis/window` | Copies selected screen vectors and shares Raw | Already shared; full-input DSP copies run on the worker. |
| Math CH inputs (`mathwave::Signal`) | Cloned full raw channel on main before worker submission | Share acquisition Arc, channel and sample range; preserve full-record time origin and worker alignment. Computed/reference vectors keep their owned/shared buffers. |
| Protocol Watch and Identify | `meas_wave` copied every enabled channel window; cloned screen record | Borrow record and `meas_slice` windows; pass slices directly to Identify. Its existing DSP scratch is counted separately against direct borrowed Identify calls. |
| Autoset search | Cloned full raw channel per search | Borrow raw channel for search/statistics; bounded screen fallback still owns sanitized samples. |
| Owned-output adapters (`meas_wave`, legacy math source/result adapters, math scale fit) | Requested sample-window or computed-result copy | Retained where callers require owned output. These are distinct from live acquired CH input preparation; scale fit copies its computed result. |
| Render kernels, `fast_plan`, `fast_deposit` | Borrow samples; convert screen points when necessary | Unchanged pixels. Only the record fallback ownership changes. |
| Sparse/framestats test reference implementations | Deliberate full Frame clone in test-only reference | Retained as independent correctness/performance references, never live-step production work. |
| Scripted batch/roll and test fixtures | Constructed Frame, latest clone and queue move | Adapted to share after metadata writes; fixture-only fresh acquisition construction stays outside timing. |

`producer.clone` measures Arc cloning. Removed the constant-zero `producer.clone_size`
metric and its vacuous copy timing comparison. The checked-in device-test parser has no
reference to either producer metric; the requested perfmap scripts are absent from this
tree. No unsafe or crate added.

## Host measurements

Host release, four channels, 10M acquisition depth, production k=16 and 625,000
f32 values per channel. These observations are not unit acceptance.

The final guard measures actual `hw_record` plus drawing: the reference calls
`draw_frame`, which independently converts again, while the candidate calls
`draw_frame_reusing`. Both the acquired record and pixel maps are observed by
black_box. Before timing, counters assert two versus one resamples and identical
maps. Across 101 alternating rounds of three calls, best elapsed is
2.194333 -> 0.971611 ms (ratio 0.442782), and p99 process CPU is
2.394666 -> 1.106333 ms (ratio 0.461999). Limits remain 0.8 and 1.0.
This measures conversion and drawing, not the complete step.

Separate round-1/final executables run the actual 300-step cadence. Each run has
38 record steps; producer fixture construction is outside the timer. Process
order reverses in round two. All six runs exit 0.

| Round | Before mean / median / p99 elapsed (ms) | After mean / median / p99 elapsed (ms) |
|---|---|---|
| 1 | 14.779282 / 14.675417 / 16.297875 | 13.623017 / 13.516750 / 14.820750 |
| 2 | 15.022764 / 15.015167 / 16.002667 | 13.830483 / 13.833875 / 16.151542 |
| 3 | 15.311515 / 15.276042 / 17.438000 | 14.113680 / 13.791500 / 19.249125 |

The median of the three run medians is 15.015167 -> 13.791500 ms: 1.223667 ms,
or 8.15%, lower. An earlier batch with the same production changes measured
14.395542 -> 13.220625 ms (8.16% lower); absolute host timings vary. Cadence p99
is nearest-rank over 38 events, hence the maximum, unlike the guard's 101-run
CPU distribution. There is no claim of a consistent cadence-tail improvement.

The first whole-step guard chose a provisional 20% saving requirement and failed:
14.320611 -> 13.134597 ms best (ratio 0.917). It was replaced by the actual
conversion-and-draw guard above, isolating the removed work without relaxing its
0.8/1.0 limits. End-to-end cadence is reported separately. The round-1 shared
frame guard was removed entirely: it could not detect copies inside Ui::step.
Allocation censuses guard that ownership contract instead.

Evidence: target/t0089/r2/{cadence-comparison.json,cadence-*.log,
initial-cadence/,perf-guard.log,perf-guard-final.log}. Round-1 observations and
logs remain under target/t0089. No hardware or 2-4 ms unit claim is made.

## Status

- 2026-11-17: Round-2 worker deliverable ready for Claude verification, uncommitted.
  Final source fingerprint c523edaca70caa9f55b475a469050f608cbc54fcd4f72ea9be3eb5845afb2187.
  Final debug UI exits 101: 1,303 passed, 38 failed, 48 ignored. Final release
  exits 101: 1,302 passed, 38 failed, 48 ignored. Both have exactly the unchanged
  base's 38 socket/listener failure names, with zero new names. Final focused
  debug pointer/pixel/counter tests: seven passed, three explicit timing tests
  ignored. Logs: target/t0089/r2/tests-{debug,release}-final.log,
  focused-final-debug.log and final-test-evidence.json. Both allocation census
  profiles pass, including four 64-observation cases and freed transient controls.

- 2026-11-17: Final full release goldens exit 0: 263 cases, 526 fresh processes,
  1,242 exact-repeat captures. Independent decoded-RGB comparison finds zero
  differences in all 621 reference captures. Reference files unchanged. Plan
  matrix rebuilt and byte-identical across 8,112 lines, diff exit 0. Complete
  logs: target/t0089/r2/{goldens-release.log,goldens-exact.log,plan-*.txt}.

- 2026-11-17: Final perf script exits 0 and prints exactly
  `12 timing budgets and 6 zero-allocation budgets passed`, then
  `perf guards: all passed (61 s)`; all 13 UI timing tests pass. The final real
  conversion-and-draw guard is 2.194333 -> 0.971611 ms best, CPU p99 ratio 0.462.
  Final three-run whole-step median is 15.015167 -> 13.791500 ms (8.15% lower).
  The first script exited 101 because the provisional whole-step 0.8 requirement
  failed (14.320611 -> 13.134597 ms); its 12 other UI timing tests passed. That
  case was replaced with the actual conversion-and-draw reference, retaining
  0.8/1.0 limits and separate end-to-end reporting. Logs preserve both results.

- 2026-11-17: Final exact host/ARM clippy exits 0: two improved UI entries, no
  new/grown findings, xtask zero. Final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 46 s`. The first exact clippy run
  found three new test lints (default-field reassignment, needless borrow and a
  64-line allocation test); fixed by initialization and small helpers, without
  suppressions. Logs: target/t0089/r2/{clippy-first.log,clippy-verified.log,
  fast-first.log,fast-final.log}. Draft target/COMMIT_MSG.txt includes Tested:.
  Added-text privacy and whitespace checks pass. No commits, staging, unit access,
  unsafe, dependency, golden-reference, hook or ratchet changes. Task remains
  active for cross-vendor verification and the coordinator's full integrated gate.

- 2026-11-17: Round-2 negative controls all fail by assertion, not compilation:
  restored full math CH clone, fast-selected raw Frame clone, autoset raw clone,
  Identify window copies, and disabled display-record reuse. Source restored after
  each mutation; target/t0089/r2/red-*.log and mutation logs retain evidence.
  Identify's existing DSP scratch needs its own direct-call reference in the
  census: two direct Identify calls cost the same large allocations as the two
  live handlers. The reader threshold is the actual 156,256-byte raw window;
  full step thresholds are 2,500,000 and 500,000 bytes. Correct code passes all
  four 64-observation cases; restored Identify copies add eight allocations.

- 2026-11-17: Reuse proof is explicit and scoped to normal analog-edge deep pairs.
  Snapshot before Source::acquire and compare at drawing: exact window/delay,
  3600-point output grid, source/slope/level/coupling and all trigger hysteresis
  inputs, plus identical retained Frame address. Frame identity fixes samples,
  dt/pre/fixed_ti, LA and acquisition metadata. Edge span is zero and other
  trigger kinds/modes decline reuse. Direct pixel maps are identical for shifted,
  untriggered and ordinary windows. Counter controls show fallback resamples
  1 -> 0, and real Ui::step main-view resamples 2 -> 1 with the independent
  zoom-detail cache primed. No golden-reference change is intended.

- 2026-11-17: Round-2 focused debug tests pass: seven pointer/pixel/resample
  cases, three explicit timing diagnoses ignored. The fast raw selection is
  covered by two queued frames, raw newest-but-one, deepmeas::want active and
  the selected raw Arc retained in pending measurements while the pair stays
  displayed. The raw identity test now uses MAX_DEPTH + 1 samples per channel.
  Allocation census passes four 64-observation cases, including 125k channels
  at a 500k-byte threshold and all synchronous raw readers. Full logs under
  target/t0089/r2. The first fast gate failed size ratchets (no limits changed);
  extracted drawing and autoset helpers. Second fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 39 s`. Mutation checks and final
  debug/release, clippy, goldens and perf validation remain in progress.

- 2026-11-17: Round 2 owner assignment accepted on the existing uncommitted claim.
  Additional reservations: ui/src/ui/mathwave.rs; ui/src/ui/mathwave/samples.rs;
  ui/src/ui/analysis.rs; ui/src/drawpipe/reuse.rs; ui/src/drawpipe/reuse/tests.rs;
  ui/src/drawpipe/frame.rs; ui/src/ui/deep_draw.rs; ui/src/ui/autoset/samples.rs;
  ui/src/ui/script/host/hot_paths/deep_steps.rs; ui/src/ui/host/tests/publication.rs;
  xtask/data/moves.txt (drawing-function extraction only).
  Plan expanded before code. Captured round-1 cadence and acquisition matrix under
  target/t0089/r2. No commits or unit access.

- 2026-11-17: Worker deliverable ready for independent review, uncommitted.
  Final source fingerprint e6a6318ba814a55d9470e922bc041ab0475f3da433e24c35bcd9eb321f44998d.
  Final debug UI tests exit 101: 1,298 passed, 38 failed, 48 ignored. Release
  exits 101: 1,297 passed, 38 failed, 48 ignored. The unchanged-base release
  executable has exactly the same 38 failures (1,295 passed, 47 ignored), all
  involving unavailable host sockets/listeners; no new failure names in either
  profile. The added raw/pair pointer and bit-exact borrowed conversion tests
  pass in both profiles. Full logs: target/t0089/tests-{debug-final,release,
  baseline-release}.log and test-failure-comparison.json. Final debug isolated
  allocation guard exits 0, including a freed 625k-channel negative control and
  64 sample-copy-free Ui::steps; alloc-debug-verified.log. The release version
  also passes through the perf script.

- 2026-11-17: Full release goldens exit 0: 263 cases, 526 fresh processes,
  1,242 exact-repeat captures. An additional decoded-RGB equality check against
  every reference confirms 621 captures with zero differences, beyond the normal
  golden tolerance. target/t0089/goldens.log and goldens-exact.log retain results;
  actual captures remain under target/golden/release/run-apu8m_rs. References
  unchanged. The 8,112-line acquisition matrix remains byte-identical.

- 2026-11-17: Final serial perf-guard retry exits 0 and prints exactly
  `12 timing budgets and 6 zero-allocation budgets passed`, then
  `perf guards: all passed (68 s)`. All 13 UI timing tests pass, including the
  new deep-frame case. Complete log: target/t0089/perf-guard-retry.log. The first
  full script exited 101 on the unchanged HAL bench_batch_decode read tail:
  candidate/reference p99 ratio 2.228 exceeds 1.500. Its complete output is
  retained in perf-guard.log; no limits changed. An earlier isolated deep ratio
  passed too (15.951319 -> 15.138041 ms best, tail ratio 0.940). The final serial
  run supersedes that early timing; separate base/candidate cadence results
  above do not demonstrate a consistent end-to-end host speedup.

- 2026-11-17: Final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 45 s` (fast-verified.log).
  Initial fast run printed `gate FAILED in 55 s: size`: SegmentIo::read grew to
  62 lines. Rewriting the Arc mapping kept it below 60, without ratchet changes.
  Exact clippy exits 0 with two improved UI entries and no new/grown findings;
  clippy-final.log names all retained child logs. Added-text privacy, message
  privacy and diff whitespace checks pass. The whole-tree privacy audit reports
  139 inherited findings; added-text audit reports zero. target/COMMIT_MSG.txt
  contains the reviewable draft. No commits, staging, hardware access, dependency,
  unsafe, golden-reference, hook or ratchet changes. Task stays active for the
  coordinator's cross-vendor review and full integrated gate.

- 2026-11-17: Exact unmodified xtask clippy step passes, exit 0, across every
  registered host/ARM invocation: UI has two improved ratchet entries and no new
  or grown findings; xtask zero findings. Driver and full child logs are retained
  under target/t0089/exact-clippy and the log directory named in clippy-final.log.
  Fast gate exits 0 and prints `partial gate passed (not done): 8 steps in 97 s`.
  Debug deep intake/pointer tests pass (2 passed, 3 explicit timing tests ignored).
  The isolated deep-step allocation guard passes (1 passed): 64 warmed steps have
  zero allocations >=2 MB, below a single 625k-f32 channel copy (2.5 MB).
  Existing smaller kernel guards remain allocation-free. An initial 1 MB total
  requested-byte ceiling failed on 8,985,617 bytes of existing small allocations;
  replaced it with the sample-sized allocation census so it specifically detects
  retained or transient deep copies without claiming zero whole-step allocations.
  Initial type adaptations, six redundant Into conversions, and a two-line size
  excess were fixed without suppressions or ratchet edits.

- 2026-11-17: Rebuilt plan matrix after changes is byte-identical to baseline
  (8,112 lines; diff exits 0). Full debug UI run currently reports 1,297 passed,
  38 failed, 48 ignored; failures require host sockets/listeners denied by this
  sandbox. A final-source rerun, matching base failure comparison, release tests,
  goldens and serial performance measurements remain in progress. Baseline release
  diagnosis: 38 record steps, mean 16.362153 ms, median 16.261083 ms,
  p99 elapsed 19.662750 ms; producer fixture creation is excluded from timing.

- 2026-11-17: Inventory complete. Shared/latest and UI queue now use Arc handles;
  Record.raw already did. Added borrowed display conversion to remove the previously
  unlisted deep render fallback clone. Existing decode, resampling and pixel kernels
  are unchanged. Added pointer-identity coverage for 10M raw and 10M pair acquisitions,
  and a process-isolated large-allocation census over 64 real deep steps, which catches
  freed transient copies as well as retained copies. Additional fixture reservations:
  ui/src/ui/script.rs; ui/src/ui/script/host/hot_paths.rs;
  ui/tests/support/counting.rs; ui/tests/snapshots/hot_paths.rs;
  ui/src/ui/bus_candidate.rs; ui/src/ui/bus_candidate/tests/change_point.rs;
  ui/src/ui/host.rs; ui/src/ui/lifecycle.rs; ui/src/ui/autoset.rs;
  ui/src/ui/tests/change.rs; ui/src/ui/tests/change_picture_contracts.rs;
  ui/src/ui/tests/epochs.rs; ui/src/ui/tests/outbox.rs; ui/src/ui/tests/producer.rs;
  ui/src/scpi/vernier_tests/depth_tests.rs; ui/src/board/dho/peak_tests.rs.
  Baseline plan matrix captured (8,112 lines, default calibration, no hardware).
  Release baseline cold LTO build and validation are in progress.

- 2026-11-17: Claimed by Codex under the owner assignment on t0089-arc-frame,
  base b582fa27. Initial tree clean. Plan written before code. Host-only work;
  independent cross-vendor review and full integrated gate remain coordinator work.
