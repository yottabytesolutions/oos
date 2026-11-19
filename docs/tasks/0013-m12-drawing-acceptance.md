---
id: 13
title: "M12: acceptance and removal (D9)"
kind: chore
status: active
release: 0.2.0
owner: codex
depends: [12]
---

# M12: acceptance and removal (D9)

## Goal

Complete [migration M12](../architecture/migration.md#m12-acceptance-and-removal-d9). The migration document is the specification.

## Plan

Reserved paths: `ui/src/ui.rs` (test registration only),
`ui/src/ui/draw_routes.rs`, `ui/src/ui/draw_routes/`,
`ui/src/system.rs`, `ui/src/system/test_allocator.rs`, `ui/tests/support/counting.rs`, `ui/tests/soak.rs`,
`ui/src/board/scripted.rs`, `ui/src/board/dho/scripted.rs`,
`ui/src/board/dho/scripted/evidence.rs`, `tools/perf-guard.sh`, `docs/architecture/drawing.md`, this task.

M12a (#157): compare identical captured batches through both UI routes, characterize shared
record-only modes and weights, measure host route build/allocations, and isolate the four-channel
10 us/div cost. Publish a recommendation with explicit hardware/prerequisite limits. No route deletion
or production behavior change; owner selection and M12b remain separate.

Follow [migration M12](../architecture/migration.md#m12-acceptance-and-removal-d9), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

F0 (#202), branch `refactor/202-frames-f0`: reserve `ui/src/drawpipe.rs`, `ui/src/drawpipe/losses.rs`,
`ui/src/drawpipe/frame.rs` (timing removal only), `ui/src/drawpipe/feed.rs`, `ui/src/drawpipe/identity_tests.rs`,
`ui/src/drawpipe/budget_tests.rs`, `ui/src/framecost.rs` (renamed to `ui/src/claim_timing.rs`),
`ui/src/main.rs`, `ui/src/board/dho/mod.rs`,
`ui/src/board/dho/frame1/tests.rs`, `ui/src/netport/latency.rs` (shared ring formatter), `ui/src/perfguard/`, `ui/src/perfguard.rs`,
`ui/src/ui.rs` (counter wiring only), `ui/src/board/dho/hw.rs`, `ui/src/board/dho/hw/batch.rs`,
`ui/src/board/dho/publication.rs`, `ui/src/board/dho/frame1.rs`, `ui/src/board/dho/roll.rs`, `ui/src/wave.rs`,
`ui/src/cmd.rs`, `ui/src/ui/draw_routes/perf.rs`, `ui/src/ui/step_draw.rs`, `ui/src/ui/change.rs`,
`ui/src/board/dho/hw/batch/tests.rs`, `ui/src/board/dho/peak_tests.rs`, `docs/spec/state-query.md`,
`xtask/data/architecture.txt` (remove retired metrics), `tools/tests/fixtures/perfmap/state.txt`,
`tools/tests/fixtures/perfmap/README.md`, `ui/tests/snapshots/hot_paths.rs` and lowered ratchets for these files.
The owner supplied the F0 design because its documents are not in this base.

1. Salvage bounded quantiles, per-picture record counts including zeros, enabled-channel maps and empty-merge tests.
2. Replace per-record clock reads with per-claim timing. Expose current loss buckets and live route without changing
   acquisition, drawing, scheduling or overload policy. Keep legacy overload losses visible until F3 removes them.
3. Run fast gate, UI debug/optimized tests, draw-route equality and host Batch picture perf rows. Report production LOC.
   Hold for independent review, integrated full gate and coordinator/owner DHO804 unit round before merge.

F0 unit-regression fix (#213): also reserve `ui/src/drawpipe/test_scope.rs`,
`ui/src/drawpipe/counter_tests.rs`, `ui/src/ui/draw_routes/counters.rs`,
`docs/architecture/README.md` (host budget row) and `tools/tests/test_perf_budgets.py`.
Sample claims in producer-local state, publish counters at existing batch/share boundaries, and update timing rings
once per picture. Preserve claim sizes, intake limits and pacing. Restore legacy dropped semantics and expose
acquired-minus-used separately. Compare identical warmed pipelines with diagnostics enabled/disabled in one host run.

PR #213 review follow-up: also reserve `ui/src/board/counters.rs` and `ui/src/ui/lifecycle.rs`.
Move queue-loss accounting onto `Shared` so watchdog recovery and DHO publication reuse the same helper.
Exercise hard-limit recovery with queued hardware records and repeated stopped polls; preserve legacy dropped semantics.
Check the route finding against the hardware-only fast-path guard and reply with evidence if unreachable.

PR #213 failed-check follow-up: accept each pipeline edit through the existing change point before the
route-reporting test reads status. Reproduce with nextest's process isolation, then run the UI tests in debug
and optimized profiles and the fast gate. No production behavior change.

Private staged-budget proposal, exact #213 base: reserve this task, `docs/adr/0031-performance.md`,
`docs/architecture/README.md`, `docs/architecture/migration.md`,
`ui/src/ui/draw_routes/counters.rs` and `tools/tests/test_perf_budgets.py` in a private source snapshot.
Apply the owner's explicit instruction to allow recoverable overhead before cutover: propose 1.02 best elapsed
for F0 staging only, retaining 1.515 CPU p99. Preserve the six cases, sampler and actual picture/record checks.
Exclude the separately retained RMW optimization: disassembly proves fewer RMWs, not a causal timing benefit.
Validate source identity and synthetic acceptance/rejection fixtures without compilation or native timing.
At F3 frame cutover restore 1.01 in the assertion, table and fixtures, and provide all-six-row evidence before
cutover acceptance. Existing unit, independent review and integrated full-gate holds remain.

F1 (#203), assigned separately from F0: extract placement, projection and difference deposition into
`picture/deposit/kernel.rs`, with frozen inputs and a Standard adapter. Share spans and the dense mapping/trigger
logic without changing route choice, weights, trigger search policy or sparse-record rendering.
Reserved F1 paths: `ui/src/picture.rs`, `ui/src/picture/`, `ui/src/main.rs`, `ui/src/ui.rs`,
`ui/src/drawpipe.rs`, `ui/src/drawpipe/frame.rs`, `ui/src/fastdraw.rs`,
`ui/src/fastdraw/validation.rs`, `ui/src/trignear.rs` (generic sample test call sites),
`docs/architecture/README.md`, changed-file ratchets and this task.
PR #212 memory fix: reserve `ui/src/drawpipe.rs`, `ui/src/drawpipe/feed.rs`, `ui/src/ui/draw_routes.rs`,
`ui/src/ui/draw_routes/memory.rs` and `ui/src/system/test_allocator.rs` as well.
Size retained worker maps for enabled channels, release disabled planes on reuse, and measure allocator
high-water bounds for 1/2/4 channels at current sample geometry. Keep enabled buffers across pictures.
Validate unchanged route pictures, debug/optimized UI tests, the fast gate and host deposit timings.
Coordinator owns cross-vendor review, the full gate and the required DHO804 unit round before merge.

PR #212 quality fixes: keep the same reserved F1 paths. Replace the test-only opaque batch hold with its
named type, remove duplicate trigger classes, group projected-frame and trigger-search inputs, and share
record-band geometry and trace handling. Store one projection and fold layout for both plan routes, use
full-width dimensions, and preserve all route, trigger and pixel policies. Validate the existing route/oracle
and golden suites in debug and optimized profiles, add wide-dimension and record-band regressions, and run
the fast gate plus generated changed-file ratchets.

F2 (#204): replace per-worker maps and mailboxes with a persistent pool, owned column bands and one whole-deposit
slot. Fold on the UI thread, preserve Standard and existing source/trigger/sparse policies, and compare raw/codes
at 1, 2 and 4 channels. Source back-pressure belongs to F3. Reserved paths: `ui/src/drawpipe*`,
`ui/src/picture/`, `ui/src/fastdraw.rs`, Batch callers in `ui/src/board/dho/`, `ui/src/ui.rs`,
`ui/src/ui/draw_routes*`, `ui/src/ui/host.rs`, `ui/src/ui/script/`, related contract tests,
`ui/tests/host_script.rs`, `ui/tests/golden/manifest.txt`, `ui/tests/golden/README.md`,
`docs/architecture/README.md`, changed-file ratchets and this task. Validate UI debug/optimized suites,
fast gate and four-channel host timing rows. Coordinator retains publication, independent review and unit acceptance.

PR #224 review follow-up: exercise watchdog reset with public, private, recycled and in-flight deposits.
Retain the current-identity waveform when a mapping or layout change rejects the painted planes.
Keep the F2 paths and reserve `ui/src/drawpipe/shutdown.rs` for the reset. Run debug and optimized UI tests
and the fast gate. The coordinator retains the hardware hold.

PR #224 host performance follow-up in the existing F2 paths.
Compare the shipping-profile compressed-frame guard serially on published F2 and its F1 parent.
Drain the worker fixture's bounded deposit slot and remove redundant frame placement if confirmed by the comparison.
Retain full host gate evidence for the coordinator before the combined unit trial.

PR #224 intake review follow-up: return the newest waveform to the existing record path when the coordinator
rejects a batch. Test full and disconnected intake, buffer return and unclaimed-record accounting.

PR #224 window follow-up: prove the whole batch's hit bound before painting and preserve a delivered waveform's
picture eligibility while it waits behind a fallback. Reserve the F2 producer, waveform queue and host fixture
paths. Compare Auto and Off from equal startup settings and retain a separate mapping-transition regression.

F2 staged tap funding integrates the verified fixture, admission and application inputs on `b10e6a4e`.
Retain the accepted Handoff ordering, watchdog reset, redraw, and narrow and wide picture paths. Validate
real board descriptors and full B-tap steps with explicit staged funding and legacy defaults. Reserved paths
for this follow-up include the existing F2 paths, `ui/src/board/frame_summary.rs`, `ui/src/board/dho/hw.rs`,
`ui/src/wave.rs`, `ui/src/ui.rs`, startup switches and mode reporting, and the changed-file ratchets.

PR #221 upload integration (#216) on published F2 `29bbadd57bad48fdd33d5b4a15469186c64de42a`:
reserve `ui/src/picture/acc.rs`, `ui/src/picture/deposit/planes.rs`, `ui/src/wave/phosphor.rs`,
shared drawing and acquisition batch fixtures,
`ui/src/drawpipe/tests/deposit.rs`, changed-file ratchets and this task. Route whole-deposit map writes through
upload row tracking, reconcile private-map callers, and retain captured projections, routing, accounting and goldens.
Run the full host gate, drawing/four-channel regressions and serial shipping-profile dots/upload guards with
same-session references. Record failures and production LOC. Keep 30 fps, zero lost records and every frame hold;
hardware acceptance, independent review and publication remain with the coordinator and owner.

PR #221 span-row review follow-up on `072cac8e`, stacked on F2 `e13efdb0`:
reserve `ui/src/drawpipe/frame.rs`, `ui/src/wave/dirty/tests.rs`, changed-file ratchets and this task.
Reuse `fastdraw::span_rows` for dense Standard uploads and normalize its empty sentinel to `0..0`.
Check visible, vertically clipped and horizontally empty spans without changing painting or route policy.
Run UI debug/optimized tests, serial shipping-profile upload guards and the fast gate. Preserve the stacked base,
all performance thresholds and the coordinator's hardware hold; F2 performance repair remains separate.

PR #221 failed-check follow-up: reserve `ui/src/fastdraw/whole/dots.rs`, changed-file ratchets and this task.
Replace constant-size chunk iteration with array chunks, retaining the four lanes and scalar remainder.
Compare chunk boundaries with the scalar dots oracle, then run host/ARM clippy, UI debug/optimized tests and fast gate.
No drawing or routing behavior change; publication, independent review and hardware holds remain with the coordinator.

PR #221 counter-budget review follow-up: reserve this task and, if reproduction identifies a regression,
the existing upload-row paths above and their tests. Compare the unchanged shipping-profile counter guard
before changing production code. Preserve its six workloads, 1.01 best-time limit and 1.515 CPU-tail limit.
Resolve any demonstrated overhead without changing pictures, membership or pacing, then run the fast gate,
UI debug/optimized tests and the full host gate serially. The runner retains Git mutations and publication.

PR #221 replay on F2 `861170b3`: retain exactly the two published upload topics, using old F2
`8c0254ee` as their verified parent. Reserve `ui/src/picture.rs` for the moved fold integration,
existing upload tests, changed-file ratchets and this task. Preserve the F2 delivery, pool and performance
repairs. Run serial shipping upload guards, the fast gate and the full combined host gate.
Keep the prior optimized census failure unresolved, coordinate before any census edit, and retain the unit hold.

PR #221 current-F2 replay and coordinator-routed #230 census repair: keep the two upload topics above
`refactor/204-frames-f2`. Reserve `ui/src/drawpipe.rs` for the moved non-window fold and
`ui/src/drawpipe/tests/window.rs` for read-only map access and independent
GPU-consumer coverage of retained and expired deposits. Keep every fold behind the dirty-row edit boundary.
Also reserve `ui/src/system/test_allocator.rs`, `ui/src/ui/change/census.rs`, `ui/src/ui/script.rs` and
`ui/tests/soak.rs` for the verified caller-thread census patch. Preserve all six 256-call regions, strict zero
assertions and process-wide accounting. Run the fast gate, UI debug/optimized tests and serial shipping upload
guards. The coordinator retains independent review, the final combined full gate and the DHO804 unit round.

PR #221 three-topic replay on F2 `7c7135e7`: retain the upload edit boundary, retained/expired
window regression and #230 calling-thread census with all six strict zero checks. Preserve F2 capacity,
hidden-trigger and delayed-waveform corrections and both layers' existing task evidence. Reserve this task
and generated changed-file size ratchets for integration reconciliation. Also reserve `ui/src/gpu.rs`,
`ui/src/gpu/upload.rs` and its tests for the private upload-source name collision found by the fast gate.
Reserve `ui/src/drawpipe/tests/deposit.rs` to adapt the new hidden-channel regression to read-only map access.
Validate the fast gate and targeted upload, dirty-row, window, deposit and census checks in debug and optimized
profiles on this intermediate tree.
The F2 admission follow-up remains active; its published head needs an exact final restack and the full
combined host, image, independent review and unit gates. The coordinator's hold remains in place. No scope access.

PR #221 final combined validation on F2 `43d85974b4efebb1c01e4c5f0b2a27b933ece10e`:
retain the runner-selected upload topics through `1eb18ea51b3d3f08217afd9dda97285574b4eb21`.
Reserved paths: this task for exact-tree host evidence and `ui/src/board/dho/hw/batch/tests.rs` for the replayed
private-map assertion. The verified #230 caller-thread census patch is already
present and will be reused without modifying its six 256-call regions, strict zero checks or process counters.
Run combined upload/window/capacity/waveform regressions and the census in debug and optimized profiles, serial
shipping upload/dots/F2 guards, the fast gate and the full normal host gate. Freeze source and runner metadata
before builds and verify their fingerprint afterwards. Retain prior failures separately. No scope access.

PR #221 clippy compile follow-up: reserve `ui/src/ui/draw_routes.rs` and this task.
Adapt the hit-budget rejection regression to the existing read-only phosphor map accessor, preserving its
nonempty and exact-pixel assertions. Run host/ARM Clippy, the fast gate and UI debug/optimized tests.
The runner retains Git operations and publication; hardware acceptance remains with the coordinator and owner.

PR #221 official-F2 replay: reserve `ui/src/picture/deposit/{planes.rs,kernel.rs,standard.rs}`,
`ui/src/fastdraw/difference.rs`, `ui/src/drawpipe.rs`, `ui/src/drawpipe/stage.rs`,
`ui/src/wave/{phosphor.rs,dirty/tests.rs}`, `ui/src/drawpipe/tests/{deposit.rs,window.rs}`
and generated changed-file ratchets.
Publish the existing fold hull through the phosphor edit boundary for single and retained deposits.
Collect dot rows during existing painting, invalidate stamps at storage-format changes, preserve exact wide counts,
and test old clears, new rows,
disjoint bands, skipped consumers and unchanged pictures. Also reserve `ui/src/ui.rs`,
`ui/src/ui/draw_routes.rs`, `ui/src/wave.rs` and `ui/src/gpu{.rs,/upload.rs,/upload/tests.rs}` to remove the
replayed full-map mark, place record drawing with its sole phosphor caller, and share GPU count-source selection
without growing existing size allowances. Run debug/optimized UI tests, both allocation
censuses, host/ARM Clippy, shipping upload/F2 guards and the fast gate. No hardware access or hold changes.

PR #221 replay through the current #224 queue-order and measurement layer:
reserve `ui/src/ui/draw_routes/tap_recovery.rs` and this task. Preserve the six upload topics and all incoming
reader, watchdog and measurement fixes. Extend the reader/fallback regression to check independent incremental
consumers across remapping and an unchanged publication. Run the combined drawing checks, complete UI tests and
both six-region allocation censuses in debug and optimized profiles, the fast gate and serial shipping guards
on frozen source. The runner owns Git refs and publication. No hardware access or hold changes.

PR #221 funded-F2 transfer checkpoint on `bb96f34f`: reserve this task only.
Verify the runner-completed replay against the exact seven upload topics from the `b10e6a4e` fork.
Keep the upload implementation and funded-plan parent unchanged. With the corrective F2 job active,
run focused upload, deposit, reader and allocation regressions in debug and optimized profiles plus the normal
fast gate. Record exact source receipts; defer full host and timing suites to the subsequent exact F2 restack.

PR #221 corrected funded-F2 validation: reserve this task for exact-tree host evidence.
The runner completed the unchanged eight-topic upload replay on corrected F2
`bbece988a741098d6994019d03a8bc67fdecc99b`; validate head
`f16b588f0bc85fd4d928536d7570664adfaa6da5` without changing either implementation.
Freeze the file manifest, source receipt and exact refs before the fast and bare full host gates.
Inspect all 23 normal stage results, complete UI suites, ordinary goldens and shipping guards;
retain each original 1% and temporary 2% counter observation and both six-region allocation censuses.
Record actual production LOC against corrected F2 and protected main. No scope access or hold changes.

## Status

- 2026-11-19: Owner-authorized private staged-budget proposal prepared against exact #213 base.
  Source/fixture validation passed: 11 budget and 17 perfmap tests; exact-base patch applies.
  All production source, other budgets, unit acceptance and sampler/runner remain identical.
  No acceptance or review state changed. Native timing awaits experiment-supervisor release.
  The earlier counter-owner candidate remains separate and unaccepted; this proposal has no production edits.

- 2026-11-19: PR #224 placement borrows the supplied newest waveform. The host allocation fixture now exercises
  supplied waveforms at 1, 2 and 4 channels. The warmed one-channel 100,000-sample row fell from 400,280 to 9,560
  peak bytes above entry, and the four-channel 100,000-sample row passed at 9,752 bytes. DHO804 acceptance remains open.

- 2026-11-19: PR #224 review fixes publish the row hull of cleared and drawn phosphor counts. The deposit
  coordinator sends cheap measurement values with each waveform tap for the latest and displaced record gates.
  Host regressions cover disjoint bands, expiry, idle publication, and tap statistics without reader-thread scans.
  The fast gate and Clippy passed. Nextest passed 1,839 debug and 1,838 optimized UI tests, with 62 skips in
  each profile. Shared-process test runs failed the wide-dot drop-counter assertion in both profiles.
  The production delta is +158 UI Rust lines. DHO804 acceptance remains open.

- 2026-11-19: PR #221 corrected funded-F2 host validation qualifies source head
  `f16b588f0bc85fd4d928536d7570664adfaa6da5` on exact F2
  `bbece988a741098d6994019d03a8bc67fdecc99b`, against protected main
  `dd5bb013b11714668345683b32ca1927e6a8a810`. The eight upload topics compare unchanged with the prior
  transfer stack, and golden references are identical to corrected F2. This job changes this task only.
  Fast printed `partial gate passed (not done): 12 steps in 8 s` (exit 0). The single bare
  `cargo xtask check` printed `gate passed (default): 23 steps in 646 s` (exit 0), under the ordinary
  profile with no ambient-idle prerequisite or timing/selection overrides. All 23 results were inspected:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, agent-rules,
  test-time, links, text, protected-storage, doctor-base, clippy, plan-matrix, decoders, tests-debug,
  tests-release, perf-build, perf, coverage, script-syntax and python-tests passed. No ratchet change was needed.
  Complete UI suites passed 1,864 debug and 1,863 optimized tests, with 64 existing skips per profile.
  Normal golden comparisons and the optimized composed soak passed. Extended image/unit selections were not
  requested. UI coverage was 90.4% against the unchanged 86.2% floor; every crate met its coverage floor.
  Host/ARM Clippy and all 65 synthetic register plans passed. The shipping UI run passed all 30 guards.
  STOP/resume reservation cancellation, counted Standard redraw, direct newest-waveform reuse, whole-B staged
  funding, original final-reader lifetimes, tap/Shared fallback ordering, frozen routes, current route labels,
  three-deposit and per-channel boundaries all passed in the complete debug and optimized suites.
  The existing upload regressions passed for separate-band row hulls, cleared gaps/full clears, skipped narrow/wide
  consumers, true narrow Dots, indexed wide counts and no-op frames. Both explicit observer censuses passed:
  prepare_keys, compare, finish_capture, stage, live_normalize and consumers each made 256 calls with zero
  caller-thread allocations. Process-wide accounting and all census assertions remain unchanged.
  Every counter workload passed both the original 1% best-wall target and temporary 2% limit:

  | Channels | Time/div | Reference / counted best (ns) | Ratio | 1% | 2% |
  |---|---|---|---|---|---|
  | 1 | 1 us | 2,283,559 / 2,288,309 | 1.002080086 | pass | pass |
  | 1 | 10 us | 8,360,140 / 8,339,606 | 0.997543821 | pass | pass |
  | 2 | 1 us | 3,057,041 / 3,053,437 | 0.998821082 | pass | pass |
  | 2 | 10 us | 6,999,140 / 7,027,955 | 1.004116934 | pass | pass |
  | 4 | 1 us | 3,891,559 / 3,902,114 | 1.002712281 | pass | pass |
  | 4 | 10 us | 6,176,710 / 6,168,575 | 0.998682956 | pass | pass |

  Counter p99 CPU passed the unchanged 1.515 limit for all six rows. Step-p99, CPU and every other timing
  threshold are unchanged. No unrelated builds were launched before the timing stage finished.
  Actual production code is 111,084 lines, including 74,500 UI Rust lines: +262 against corrected F2
  (110,822 total; 74,238 UI), +3,073 against protected main (108,011 total; 71,427 UI), zero elsewhere.
  This evidence update adds zero production lines. Every command retained exact head/base and a complete file
  manifest; manifests and source receipts matched before/after. The build-input SHA-256 remained
  `e0ff2aae018f638a04206158114b21c2d088beafe7d0d3bcdabf7274385abde7` throughout.
  Evidence: `target/corrected-upload/fast.log`, `target/corrected-upload/full.log`,
  `target/corrected-upload/full-steps.log`, `target/corrected-upload/perf-guard.log`,
  `target/corrected-upload/perf-guard-step.log`, `target/corrected-upload/counter-results.json`,
  `target/corrected-upload/census-debug.log`, `target/corrected-upload/census-optimized.log`,
  `target/corrected-upload/loc-f2.log`, `target/corrected-upload/loc-main.log`,
  `target/corrected-upload/contract-results.json`, `target/corrected-upload/upload-range-diff.txt`,
  `target/corrected-upload/receipts.json` and per-command `*-manifest.json` in that directory.
  Complete gate subprocess logs are in `target/check-logs/43889-1795065034482276000/`.
  Evidence documentation was updated after the builds; production/build inputs were not edited.
  Independent review, exact-candidate publication, image qualification and coordinator/owner hardware acceptance
  remain open. A newer F2 head requires another exact replay and validation. Host only; no scope access,
  combined unit round, tag, release or hold change is authorized here.

- 2026-11-19: PR #221 funded-F2 transfer checkpoint has no active rebase or unresolved paths. Exact source head
  `75ff5031d2bb10a8a88b178559feea0974233398` has immediate parent
  `f668b75f0a36095bc5119a075be78d976bcea79d`; the exact F2 stack parent remains
  `bb96f34f1cb19e4bb145676aa2d7a67f76d477d3`. Range comparison against the seven upload topics from
  `b10e6a4e6405a1b8308462d832f632bda3448846` through `2d28d0bad583ea92fae3d62a460ffc5e73d2ee1a`
  shows only task-context and inherited size-baseline adjustments. No production changes were needed.
  The 06:05 coordination record confirms current-head F2 review is failing and the corrective F2 job is active.
  Focused process-isolated upload, deposit, reader and batch regressions passed 114 tests in each of debug and
  optimized profiles; the selection skipped 1,759 and 1,758 tests respectively, including timing guards.
  Both explicit observer allocation censuses passed, each with 256 calls and zero caller-thread allocations
  in all six regions. Every test's before/after source receipt matched
  `7fcb92d94569753981a31262aa915c50fdaea6ef18ebb3f95d08c18cb2c58aa5`; the Git head also stayed unchanged.
  Fast printed `partial gate passed (not done): 12 steps in 8 s` (exit 0), with the same before/after source
  receipt and no ratchet updates needed. Logs and command/source receipts are `target/funded-upload-transfer-*`;
  the fast log is `target/check-logs/50081-1795064805867398000`.
  Net production delta is +262 UI Rust lines over exact F2, zero elsewhere; this checkpoint adds zero production
  lines. Full host and performance suites were deliberately not selected while the F2 correction is active.
  The prior 30 shipping guards remain intermediate evidence for their actual earlier source, with no new
  original 1% or temporary 2% counter observations here. Subsequent exact F2 restack, full combined host gate,
  independent review, signed qualification and hardware acceptance remain open. Host only; hardware holds remain.

- 2026-11-19: Integrated the three SHA-256-verified F2 slices from `e15920b2` onto `b10e6a4e`, preserving
  the ordering and reader changes through `cc7fa6f5` and `b10e6a4e`. Build input SHA-256
  `d216d72448a49a282716bf6d9d4071d86e3cd2be870db6b191364b6626b15bd1`. Production UI Rust totals
  74,217 lines by `cargo xtask loc`, up 638 from `b10e6a4e`, 2,459 from reviewed F1 `113ccb97`,
  2,489 from replayed F1 `38defc1d`, and 2,790 from protected main `dd5bb013`.
  The unconfigured path keeps its existing read, queue and fallback rules. The staged profile charges the existing
  Handoff before readout and retains the grant through the last original waveform reader. Independent vector
  copies use the declared copies allowance. STOP, identity retirement and watchdog reset wake staged waiters.
  The four-channel 1 and 10 us/div Auto Lines rows keep 142 raw, 14 raw and 99 compressed records per
  read respectively at 312.5 MSa/s. Raw and replay backing measured 56,784 and 563,024 bytes.
  The synthetic 500 us interval gives B=70, 140 FIFO entries and 142 waveform slots, with software capacity
  for 2,000 or 4,000 records/s at one or two useful records per read. Every whole-B UI fixture consumed 70 taps.
  The 271-column raw Dots row needs 69,216 count units. It used a derived 16,666,667 ns cycle at B=5 with the larger
  synthetic partition and used Standard fallback with the smaller partition.
  The first fast run printed `gate FAILED in 17 s: size`. After splitting the new code and regenerating the
  three changed-file size entries, fast printed `partial gate passed (not done): 12 steps in 30 s`.
  Clippy printed `partial gate passed (not done): 1 steps in 61 s`. All 65 synthetic register plans matched.
  Focused debug, test-release and ARM64 Linux container runs each passed 153 tests with 14 timing probes ignored.
  The first container run needed a read-only source mount for the existing layout budget test.
  Full UI Nextest suites passed 1,834 debug and 1,833 optimized tests, with 62 skipped in each run.
  Plain parallel Cargo testing first saw a shared drop count in the existing wide-remap test. The gate's
  process-isolated runner passed that test with the rest of the suite. Source receipts matched.
  Device qualification needs an audited exclusive partition plus peak DMA, window/graphics, shared frame,
  record/analysis copy and runtime bounds, including plan overlap. The one-byte owner declarations belong to
  synthetic fixtures. On-device minimum cycle, whole-step timing, protected capture rates and 30 fps remain
  open, along with full gate, independent review and the signed image. The hardware hold remains in place.

- 2026-11-19: PR #221 replay through the current #224 reader layer preserves all six upload topics from
  checkpoint `0284498d` on F2 `cc7fa6f5`; the runner supplied replay head `ddbe573e` with no stopped conflicts.
  Extended the reader/fallback regression to compare independent incremental consumers before and after remapping,
  including a skipped consumer and empty unchanged revisions. The first added fixture failed because enabling
  measurements resized the picture but its simulated arena retained the old length; resizing that arena corrected
  the fixture. Production Rust delta is zero; no golden, timing limit, allocation assertion or hardware policy changed.
  Final focused debug/optimized checks each passed 93 tests with nine existing timing skips. Complete UI suites
  passed 1,839 debug and 1,838 optimized tests, with 64 existing skips each, including unchanged golden references.
  Both explicit allocation censuses reported 256 calls and zero caller-thread allocations in all six regions.
  Host/ARM Clippy printed `partial gate passed (not done): 1 steps in 33 s` (exit 0); fast printed
  `partial gate passed (not done): 12 steps in 8 s` (exit 0), with no ratchet updates needed.
  All 30 serial shipping UI guards passed, including narrow/full uploads and the original counter target.
  Worst counter overhead was 0.8594%; mask-3 learning p99 CPU was 1.158333 ms on the host.
  Source fingerprints remained unchanged for every final check. Complete logs: `target/replay224-*.log`;
  command/source receipts: `target/replay224-receipts.json`. The initial fixture failure is retained separately.
  Independent review, the combined full gate and coordinator/owner hardware acceptance hold remain open.
  Host only; no hardware was accessed.

- 2026-11-19: PR #221 official-F2 final host checks passed: complete UI suites ran 1,830 debug and
  1,829 optimized tests, with 64 existing skips per profile. All 30 shipping UI guards passed serially.
  Host/ARM Clippy printed `partial gate passed (not done): 1 steps in 32 s` (exit 0); fast printed
  `partial gate passed (not done): 12 steps in 8 s` (exit 0). Both explicit observer censuses reported
  256 calls and zero allocations in all six regions. Production delta against the replayed head is -6 UI Rust
  lines, zero elsewhere. No golden reference changed. The retained-band and skipped-format regressions failed
  before their fixes and pass now. Ordinary narrow dense/fallback, one/two/four-channel and idle-picture checks
  remain strict; independent wide count/index, capacity, settlement and measurement-order checks pass.
  The worst counter best-time overhead was 0.4746%, below the original 1% target. Mask-3 learning p99 CPU was
  1.150666 ms on the host; the provisional 4x estimate remains unverified on hardware. Historical failures remain
  separate. The command receipts confirm unchanged input fingerprints during optimized tests, Clippy, fast,
  both censuses and shipping guards. Logs: `target/e159-qualified-{ui-debug,ui-optimized,clippy,fast,`
  `census-debug,census-optimized,shipping,loc}.log`; receipts: `target/e159-qualified-receipts.json`.
  This evidence update changes documentation only. The combined full gate, independent review and the existing
  coordinator/owner hardware acceptance hold remain open. Host only; no hardware was accessed.

- 2026-11-19: PR #221 official-F2 reconciliation publishes single and retained fold hulls through phosphor edits,
  collects dot rows during the existing painting pass, and removes the replayed whole-picture dirty mark.
  The retained disjoint-band regression failed before the repair (empty upload instead of rows 2..3).
  A skipped wide/narrow storage transition also reproduced a stale monitor upload; format changes now invalidate
  all narrow stamps. Ordinary narrow deposits remain bounded to their touched rows. Added dense/fallback,
  one/two/four-channel, idle-picture, wide-index and skipped-consumer comparisons. No golden, timing threshold,
  depth/geometry policy, measurement order or allocation-census assertion changed.
  Intermediate UI suites passed 1,829 debug and 1,828 optimized tests, with 64 skips in each. Both explicit
  six-region censuses reported 256 calls and zero allocations. Host/ARM Clippy and all 30 shipping UI guards passed.
  The initial fast gate printed `gate FAILED in 8 s: size`; the next printed `gate FAILED in 13 s: size` for
  unused allowances. A helper relocation initially missed its HDIV import; it is corrected. Generated ratchets
  lower four entries in changed files only. The first final debug run passed 1,829 tests and failed the old narrow-row
  fixture: its fixed sample interval selected wide storage at the default timebase. Dense and sparse spacing are now
  explicit, with a narrow-format assertion; the independent legal wide test is unchanged. The corrected full debug
  suite passed 1,830 tests with 64 skips. Final-source validation continues. Review, the combined full gate
  and the coordinator/owner hardware hold remain open. Host only; no hardware was accessed.

- 2026-11-19: PR #224 integration uses the reachable projection envelope and preserves the four-channel normal
  Lines read plans. The 271-column, four-channel raw Dots case at 500 us/div and fixed 1M depth needs up to 69,216
  count units at q = 24. Host fixtures compare the wide picture with per-sample counts and verify the GPU indices.
  The FIFO reserves analog readout sample capacity before newest decode. A consumed waveform keeps one preview
  obligation until picture acceptance, so an intervening mapping change can redraw it once without repeating its
  reader work. F2 still needs a production allocation owner for the plan-derived `2B + 2` waveform slots and
  the `B`-tap drain bound. Source-wide buffer-before-arm and command-event waits remain F3 work. The hardware
  hold remains in place.
  On unchanged source, 118 focused checks passed in each profile, Linux cross-check passed, and both full
  golden runs matched 1,314 reference captures across 284 cases. UI coverage was 90.2%. The full gate passed
  its 22 non-performance steps and printed `gate FAILED in 736 s: perf` when the quiet-host wait was stopped
  before timing. The separate ordinary performance step passed on the same source after other builds ended.
  All 28 shipping UI guards passed. The worst diagnostics ratio was 1.004739, below both the original 1.01
  target and temporary 1.02 limit. The 4 MB mask 3 learning p99 was 1.385 ms on the host, or 5.540 ms using
  the provisional 4x A72 projection. This is host checkpoint evidence. Fresh review and unit acceptance remain open.

- 2026-11-19: PR #221 clippy compile repair uses `maps()` in both hit-budget rejection assertions.
  Production behavior and the regression's nonempty/exact-pixel checks are unchanged; production LOC delta is zero.
  Host/ARM Clippy printed `partial gate passed (not done): 1 steps in 31 s` (exit 0).
  Fast printed `partial gate passed (not done): 12 steps in 9 s` (exit 0), with no lowered ratchets.
  Complete UI nextest suites passed 1,808 debug and 1,807 optimized tests, with 64 skipped in each profile (exit 0).
  The repaired regression passed in both profiles. Logs: `target/pr221-clippy-fix-{clippy,fast,debug,optimized,loc}.log`.
  Host only. Runner synchronization, final CI and independent review remain open; the earlier learning-budget
  failure and coordinator/owner hardware hold are unchanged.

- 2026-11-19: PR #221 failed-check reproduction printed `gate FAILED in 34 s: clippy` (exit 1).
  Both host and ARM compilation report E0616 for three private map reads in the hit-budget rejection test.
  Repair only those reads through `maps()`; retain the original waveform, nonempty and pixel-equality checks.
  Fetch was sandbox-blocked; HEAD matches the cached feature ref. Runner owns remote synchronization.

- 2026-11-19: PR #224 cross-vendor follow-up makes the newest waveform drawable after whole-batch hit-budget
  rejection. Tap accounting preserves eligibility for rejected batches interleaved with batches in a coalesced
  deposit. The record consumer uses explicit picture eligibility, and host regressions exercise both cases.
  The UI suites passed 1,786 debug and 1,785 optimized tests, with 62 skipped in each profile. The 12 fast
  gate steps and Clippy passed. Unit timing remains open.

- 2026-11-19: PR #224 review follow-up moves newest-record taps to a bounded FIFO independent of picture
  coalescing and hit-budget rejection. The UI schedules readers for taps consumed on later steps. Decoded
  dots use worker scratch for placement and reuse one scratch frame for painting across bands. The host
  debug and optimized suites passed 1,733 and 1,732 unit tests respectively, plus 8 goldens, 17 scripts,
  10 socket tests and 16 soak and support tests in each profile. All 12 fast gate steps passed after
  regenerating the UI file and step size ratchets. Independent review and unit timing remain open.

- 2026-11-19: Final combined upload/F2 host validation used exact F2 base
  `43d85974b4efebb1c01e4c5f0b2a27b933ece10e`, upload head
  `1eb18ea51b3d3f08217afd9dda97285574b4eb21` and the one-line read-only-map test repair below.
  Source receipt `c637f7f701a0b20915b7910f90c570b1e0112a94e2b7275cd8e3031218d26164` remained unchanged
  after every validation command, including the full gate; runner metadata stayed fixed during builds.
  Focused debug and optimized regressions each passed all 79 selected tests. Both census profiles reported
  256 calls and zero calling-thread allocations in each of six regions. The already-present #230 patch
  reverse-checks cleanly against the verified handoff; all process-wide counters and tests remain intact.
  Serial shipping upload/dots/F2 guards passed all 10 selected tests with unchanged limits.
  Final handoff fast printed `partial gate passed (not done): 12 steps in 9 s` (exit 0), retained in
  `target/replay221-final-v2-fast-handoff.log`.
  The full normal host gate printed `gate FAILED in 913 s: perf` (exit 1). Its UI tests passed
  1,804 debug and 1,803 optimized cases, with 64 ignored in each profile. All other selected steps passed,
  including 65 synthetic register plans, host/ARM Clippy, decoder variants, Python and 90.2% UI coverage.
  The full performance step passed 29 UI guards and failed `perf_guard_batch_core_learning` at mask 3:
  host p99 CPU 2.304 ms projected to 9.216 ms, exceeding the unchanged 9 ms A72 projection limit.
  The measured learning implementation and guard are unchanged from exact F2; the overrun is not attributed.
  No retry, baseline result, fixture change or relaxed limit replaces this failed combined result.
  Complete evidence: `target/replay221-final-v2-{focused-debug,focused-optimized,census-debug,census-optimized,
  shipping,full}.log`, command/source manifest `target/replay221-final-v2.json`, retained performance output
  `target/replay221-final-v2-perf-step.log` and `target/check-logs/33769-1795051346181340000/`.
  Earlier census failures, the interrupted receipt and this job's pre-repair compile failure remain separate.
  Production delta for this repair is zero; the retained upload stack is +255 UI Rust lines over F2, zero
  elsewhere. Changed files: `ui/src/board/dho/hw/batch/tests.rs` and this task. No ratchet reductions occurred.
  The combined gate remains blocked by the learning budget. The coordinator owns its disposition, renewed
  independent review, image-tier validation and the DHO804 unit round under the unchanged hold. No scope access.

- 2026-11-19: The first final-tree focused debug build exited 101 with E0616 in the replayed
  `four_channel_line_batches_keep_every_record_with_matching_and_raw_newest` test. Its assertion still accessed the private
  phosphor map; it now reads `maps()` without changing the asserted pixels or acquisition behavior.
  Evidence: `target/replay221-final-focused-debug.log` and `target/replay221-final.json`.
  The source fingerprint remained stable throughout this failed build. Restart all final validation on the
  corrected combined tree; no partial suite is a pass.

- 2026-11-19: Combined validation starts from exact upload head
  `1eb18ea51b3d3f08217afd9dda97285574b4eb21`, with exact F2 base
  `43d85974b4efebb1c01e4c5f0b2a27b933ece10e`. The supplied #230 patch reverse-checks cleanly:
  the calling-thread counter, census documentation and soak callback are already present. No replay or
  census source modification is needed. Retain all previous census and receipt failures as separate evidence.
  The coordinator retains Git operations, independent review and the existing unit hold.

- 2026-11-19: The #224 admission follow-up preserves the pre-pacing Lines plans and checks the projection held at
  worker claim. A 14-record, two-channel 10 us/div Lines read can exceed the u16 limit if claimed as Dots at 177
  columns. Its conservative bound is 4,970 hits at q = 24, against the limit of 2,730 hits. The worker returns the
  DMA buffer and accounts for the whole batch as `lost.budget`. A lossless claim of this geometry needs a wider
  representation with a revised memory budget or an approved change to the projection lifetime. Live rate and F3
  source waits remain unmeasured. The DHO804 hold remains in place.

- 2026-11-19: PR #221's three upload topics are replayed through `d3d68b55` on published F2 `7c7135e7`.
  Both layers' prior task evidence is preserved. The new hidden-channel trigger test now reads `maps()`;
  its first ratchet build failed with E0616 before that adaptation. The private GPU upload type is named
  `UploadData`, retaining the local `Source` alias without changing behavior. This separates its size census
  from the acquisition `Source` impls. Producer capacity, hidden-trigger and delayed-waveform corrections,
  the upload edit boundary, retained/expired-deposit regression and all six strict #230 zero checks remain intact.
  Initial fast gate printed `gate FAILED in 8 s: size` (exit 1), for the impl-name collision and stale size slack.
  Intermediate formatting attempts printed `gate FAILED in 8 s: size` and `gate FAILED in 27 s: size` (exit 1).
  The final alias avoids that extra production line. `cargo xtask ratchet` generated only the `ui.rs` allowance
  reduction from 8,576 to 8,574. Final-source fast printed
  `partial gate passed (not done): 12 steps in 9 s` (exit 0).
  Targeted debug and optimized upload, dirty-row, window, deposit and delayed-waveform checks each passed 28 tests;
  their two ignored upload timing cases passed separately in the shipping profile. Debug and optimized observer
  censuses each reported 256 calls and zero allocations for all six regions. Optimized UI nextest passed
  1,796 tests with 64 skipped (exit 0). Serial shipping-profile dots and both upload guards passed all three
  tests (exit 0) with unchanged limits. Eight-row best/CPU-tail ratios were 0.029/0.037; full-upload ratios were
  0.431/0.493. The validation driver verified an unchanged source fingerprint after each final command.
  Production delta for this repair is zero; the three-topic stack is +255 UI Rust lines over F2, zero elsewhere.
  Files: `ui/src/gpu.rs`, `ui/src/gpu/upload.rs`, `ui/src/gpu/upload/tests.rs`,
  `ui/src/drawpipe/tests/deposit.rs`, `xtask/data/ui/size.txt` and this task.
  Complete logs and the source fingerprint use `target/replay221-f2-intake-*`.
  This is intermediate host evidence only. The F2 admission follow-up is active. Its published head still needs
  an exact final restack and full combined host, image, independent review and DHO804 unit gates.
  The existing coordinator hold remains. No scope access or Git index writes.

- 2026-11-19: Final PR #224 host verification passed. Active Batch reads reserve for the smallest live plot so a
  mapping change during a read still fits. Standard reads keep their original size. An empty sparse record
  preserves trigger status without painting. All six acquisitions and retained maps agree between Auto and Off
  for each of raw, codes, live pairs and deep pairs at one channel.
  Fast printed `partial gate passed (not done): 12 steps in 17 s`. The unchanged-source retry printed
  `gate passed (default): 23 steps in 445 s` with matching source receipts, 1,775 debug and 1,774 optimized UI
  tests passed, 90.1% UI coverage, all shipping guards passed and all 65 synthetic plans matched.
  Full debug and optimized golden comparisons each matched 1,314 captures across 284 cases.
  Earlier runs found a conservative sparse estimate that rejected the 500-record shipping fixture, a test
  Clippy finding, and intermittent Python socket-pacing and diagnostics timing failures. The final run exercised
  the original fixture and limits. Production Rust grew by 147 UI lines. The unit hold and review remain open.

- 2026-11-19: PR #224 window follow-up reproduced the 32-record dot overflow and the delayed duplicate waveform
  on the host. The full slot now hands off at 31 records and also separates the original two 16-record reads.
  The tests wait for the blocked sender and verify buffer return, plane reuse and one wake publication.
  Auto and Off start with the same settings and synthetic reads. At the one-channel settled capture both
  retain the deposits ending at records 73 and 86. A newest-only control matches the pre-window RGB in all
  26 reviewed cases. For multi-channel deep pairs the final map agrees with the control, but retention at
  record 46 changes the running grade. Min persistence and grade cases retain the 13-record and 3-record
  deposits together. The four Dots On transition fixtures whose original pixels match are restored.

- 2026-11-19: PR #221 current-F2 replay retains exactly two upload topics above `refactor/204-frames-f2`
  (`155021d5`). The targeted pre-fix build exited 101 with E0616 in the moved non-window fold.
  That fold now uses `edit_channel`; the window fixture reads `maps()`. An injected-clock regression checks
  retained deposits, expiry without new input, independent upload consumers and once-only record accounting.
  The coordinator-routed #230 patch exposes the existing calling-thread allocation counter and selects it for
  the warmed observer census. All six measurement boundaries, 256-call counts, strict zero assertions and
  process-wide accounting remain intact. Both debug and optimized focused runs reported 256 calls and zero
  allocations in every region; no calling-thread allocation was observed.
  Initial fast gate: `gate FAILED in 8 s: size` (exit 1). `cargo xtask ratchet` generated only the changed
  `ui.rs` allowance reduction, 8,587 to 8,576. Final fast gate:
  `partial gate passed (not done): 12 steps in 8 s` (exit 0). Host/ARM Clippy passed (exit 0).
  UI nextest passed 1,792 debug and 1,791 optimized tests, with 64 skipped per profile; goldens are unchanged.
  Serial shipping-profile dots and both upload guards passed all three tests (exit 0) with unchanged limits.
  Production delta for this repair is zero; the upload stack is +255 UI Rust lines over current F2, zero
  elsewhere (109,189 total). Files: `ui/src/drawpipe.rs`, `ui/src/drawpipe/tests/window.rs`,
  `ui/src/system/test_allocator.rs`, `ui/src/ui/change/census.rs`, `ui/src/ui/script.rs`, `ui/tests/soak.rs`,
  `xtask/data/ui/size.txt` and this task. Complete logs use `target/replay221-current-f2-*.log`.
  Host only. Renewed independent review, the final combined exact-head full gate after the F2 correctness
  follow-up and the combined DHO804 unit round remain required under the coordinator's hold. No scope access.

- 2026-11-19: PR #224 review follow-up retains up to three whole deposits at minimum persistence. The UI grades
  the retained record count and counts each accepted deposit once. Injected-clock regressions cover the 67 ms
  age boundary, expiry without a new batch and the combined u16 hit bound. Regenerated 26 changed golden captures
  and verified debug and optimized golden comparisons. The fast gate passed all 12 selected steps.
  Host evidence only. Independent review and the DHO804 unit round remain open.

- 2026-11-19: PR #221 stable-input combined host gate reported
  `gate FAILED in 421 s: tests-release` (exit 1). Optimized UI nextest passed 1,785 tests and failed
  `warmed_ui_observer_allocation_census`, with 64 skipped. Its six 256-call regions counted allocations
  `[3, 0, 0, 3, 0, 0]`: `prepare_keys=3` and `stage=3`. Debug passed all 1,787 UI tests, with 64 skipped.
  Every other selected step passed, including host/ARM Clippy, all 65 synthetic register plans, decoder variants,
  Python, 90.1% UI coverage and all 30 UI timing guards: `perf guards: all passed (209 s)`.
  The earlier two-allocation optimized failure remains recorded and unresolved; 24 isolated main-reference
  passes do not explain either failure. No passing retry replaces this result.
  Bus job #230 owns the census repair and records controlled background-worker and libtest allocations entering
  the process-wide count while the calling UI thread allocated zero. This run's individual events remain
  unattributed. The allocator callback and all shared census code are unchanged from exact F2 `861170b3`.
  Complete evidence: `target/replay221-full-stable.log`, its retained command logs, and
  `target/replay221-census-handoff.md`. Independent review, census resolution and the combined DHO804 unit trial
  remain open under the coordinator's hold. Host only; no scope access or image-tier checks.

- 2026-11-19: The first combined host gate reported `gate FAILED in 746 s: plan-matrix` (exit 1).
  Writing runner metadata during the plan-matrix build changed its input digest; the receipt correctly refused
  consumption. All other selected steps passed, including 1,787 debug and 1,786 optimized UI tests (64 skipped
  per profile), coverage and all 30 UI timing guards: `perf guards: all passed (307 s)`.
  The stable-input rerun above cleared the receipt failure and reproduced the optimized census failure.
  The first failure and complete logs remain in `target/replay221-full.log`.

- 2026-11-18: Runner supplied replay head `d6089214` above exact F2 `861170b3`. Range comparison confirms
  only published upload commits `7b98e7ba` and `674af593` were replayed from their verified old F2 parent
  `8c0254ee`, rather than from the common ancestor with amended F2. F2 moved the fold into `picture.rs`;
  the replay now routes that fold through `edit_channel`. The existing independent-GPU-consumer regression
  failed to compile before this correction and passes after it. F2 delivery, placement, producer/worker and
  release performance repairs, shared span bounds, clipped dots, upload contents and goldens are preserved.
  Generated ratchets lower only the changed `ui.rs` file allowance from 8,623 to 8,612.
  Initial fast gate: `gate FAILED in 8 s: size` (exit 1); after ratcheting:
  `partial gate passed (not done): 12 steps in 8 s` (exit 0).
  Serial shipping-profile dots and both upload guards passed (3 tests, exit 0) with unchanged limits.
  Eight-row best/p99 CPU ratios were 0.028/0.041; full-upload ratios were 0.433/0.469.
  Production delta over F2 is +252 UI Rust lines, zero elsewhere (108,862 total); this fold correction adds zero.
  Changed files for the correction are `ui/src/picture.rs`, `xtask/data/ui/size.txt` and this task.
  Final fast gate: `partial gate passed (not done): 12 steps in 9 s` (exit 0).
  Complete current-run logs use `target/replay221-*.log`.

- 2026-11-18: PR #224 replays published F2 `20879ac4` from its exact F1 parent `113ccb97`
  onto F1 `38defc1d`. The three F2 commits replay to `22ba4e1f` with the reviewed waveform delivery
  and worker and compressed-frame repairs intact. The `ui.rs` size allowance carries the same 45-line
  reduction from F1, ending at 8,623. Production delta over F1 is +298 UI Rust lines.
  The fast gate passed all 12 selected steps. The full host gate passed all 23 steps in 607 s against F1.
  UI nextest passed 1,766 debug and 1,765 optimized tests, with 62 skipped per profile and 90.1% coverage.
  All 65 synthetic register plans matched. The compressed-frame ratio was 0.413, and the three guarded
  worker ratios were 1.25, 1.08 and 0.72. Complete gate logs were retained. Renewed independent review and
  the combined DHO804 unit trial remain open under the coordinator's hold.

- 2026-11-18: PR #224 code-quality review uses one pool operation and one drawing trigger policy, reuses pair
  span buffers, and submits only active placement slots. Deposit handoff returns rejected work by value.
  The acquisition fill timings still feed pacing and cap accounting. Production delta is -11 UI Rust lines.
  Fast gate and Clippy passed on the host. UI nextest passed 1,764 tests in each profile, with 62 skipped in each.
  The debug run also saw one intermittent allocation census failure, which passed in isolation.
  The shipping-profile codes guard measures the production worker against Frames mode and passed all five rows.
  The worst best-of-rounds and p99 ratios were 0.739 and 0.843. Renewed review and DHO804 acceptance remain open.

- 2026-11-18: PR #221 counter-budget review on unchanged production head `cbc4975f`: the recorded
  candidate-specific counter regression did not reproduce. The standalone shipping-profile guard passed all six
  workloads (exit 0), then all six passed again inside the full host gate. Both use the existing shared workers
  and buffers, 101 alternating rounds, 16 pictures per sample and unchanged best/p99 limits of 1.01/1.515.

  | Channels | Time/div | Records | Standalone best / p99 CPU ratio | Full-gate best / p99 CPU ratio |
  |---|---|---|---|---|
  | 1 | 1 us | 142 | 0.998531 / 1.019316 | 1.006208 / 0.986460 |
  | 1 | 10 us | 20 | 0.995717 / 0.969637 | 1.002958 / 0.987312 |
  | 2 | 1 us | 142 | 0.999422 / 0.952731 | 1.007346 / 1.119348 |
  | 2 | 10 us | 14 | 1.003453 / 0.936712 | 0.992244 / 0.987403 |
  | 4 | 1 us | 142 | 0.997418 / 1.007466 | 1.002371 / 1.042824 |
  | 4 | 10 us | 14 | 0.998729 / 0.988709 | 1.003190 / 1.046590 |

  The reported one-channel 20-record case measured disabled/enabled best times of 8,400,031/8,364,057 ns
  standalone and 8,336,382/8,361,039 ns in the full gate. No counter, upload, drawing, routing or pacing code,
  fixture, threshold or ratchet changed. Production delta against HEAD is zero (108,903 total lines).
  The full gate's performance step reported `perf guards: all passed (208 s)`, including all 30 UI guards.
  Its final result was `gate FAILED in 604 s: tests-release` (exit 1): optimized
  `warmed_ui_observer_allocation_census` counted two `prepare_keys` allocations instead of zero.
  UI nextest passed all 1,789 debug tests; optimized passed 1,787 and failed that one test, with 64 skipped per
  profile. All other selected full-gate steps passed, including Clippy, coverage, decoders and Python tests.
  This job does not change the settings-observer allocation census. Its failure still blocks landing; independent
  review and DHO804 acceptance also remain open. Host only; no hardware access or image-tier checks.
  Evidence: `target/counter-{before,full-gate,fast,loc}.log` and the retained full-gate command logs.
  The initial fast gate reported `partial gate passed (not done): 12 steps in 8 s` (exit 0).

- 2026-11-18: PR #221 counter-budget investigation starts on `perf/216-upload-rows`, head `cbc4975f`.
  HEAD matches the cached feature-branch ref. The sandbox refused the requested fetch when opening
  `FETCH_HEAD`; remote freshness remains runner-owned. The historical failing gate remains unresolved pending
  current-head reproduction and full host-gate evidence. No hardware access is authorized or planned.

- 2026-11-18: PR #221 check fix replaces `chunks_exact(4)` with `as_chunks::<4>()`, preserving lane order,
  remainder handling and drawing behavior. The scalar-oracle regression passes for tail lengths 0 through 8,
  clipped/non-finite samples and saturating weights. Host/ARM `cargo xtask step clippy` reported
  `partial gate passed (not done): 1 steps in 131 s` (exit 0). UI nextest passed 1,789 debug and 1,788
  `test-release` tests, with 64 skipped per profile (exit 0 each). The fast gate reported
  `partial gate passed (not done): 12 steps in 31 s` (exit 0). No changed-file ratchet reductions were reported.
  `cargo xtask loc --compare HEAD` reports zero production line delta. Changed files are
  `ui/src/fastdraw/whole/dots.rs` and this task. Evidence: `target/upload-{clippy,fast,ui-debug,ui-optimized,loc}-fixed.log`.
  Host only; no hardware access. CI confirmation, renewed independent review and publication remain coordinator-owned.

- 2026-11-18: PR #221 check-fix plan on `perf/216-upload-rows`, head `ccd23810`:
  `cargo xtask step clippy` reproduced `gate FAILED in 142 s: clippy` (exit 1), solely the new
  `chunks_exact_to_as_chunks` finding in `fastdraw/whole/dots.rs`. The baseline fast gate passed all 12 steps.
  The checked-out head matches the locally recorded remote branch; network and Git mutations are runner-owned.

- 2026-11-18: PR #224 intake rejection returns the newest waveform through existing record publication and
  counts only its undrawn neighbours as unclaimed. The acquisition regression reproduced the missing waveform
  before the fix. Host tests cover full and disconnected intake for both decoder paths and buffer return.
  Fast gate passed all 12 selected steps. UI nextest passed 1,767 debug and 1,766 optimized tests, with 62 skipped
  in each profile. Production delta is +11 UI Rust lines. Renewed review and DHO804 acceptance remain open.

- 2026-11-18: PR #224 host performance follow-up on published F2 `e13efdb0`, stacked on exact F1 `113ccb97`.
  Three alternating shipping-profile guard runs on macOS arm64 measured best raw/compressed times of
  32.556/12.098, 32.066/12.863 and 30.571/12.065 us on F1 (0.372, 0.401, 0.395), and
  29.523/17.111, 28.903/16.705 and 31.798/18.658 us on F2 (0.580, 0.578, 0.587).
  F2's shared placement built raw spans and then replaced them with pair spans. Select the pair builder
  directly. Published F2's worker guard reached the
  120 s buffer-return timeout. The fixture now takes and recycles each published deposit after that event,
  checks its record count and keeps the UI fold outside the timed work. The first full run measured
  30.768/12.850 us raw/compressed (0.418) after the fix, and the three guarded worker rows passed at
  1.27, 1.00 and 0.71. The fast gate passed all 12 steps. Production Rust LOC delta is zero.
  The first full gate failed an experimental allocation assertion, Python socket pacing, sparse interpolation and
  the diagnostic-counter ratio. The existing pixel and timing tests cover pair placement.
  The Python case passed in isolation. Serial sparse guards passed on F1, published F2 and the candidate;
  the row that failed at 1.15 in the first gate measured 1.07, 1.06 and 1.07 respectively in isolation.
  The second run passed every performance guard (compressed ratio 0.417) and Python tests. Its debug,
  optimized and coverage test steps still included the experimental allocation probe, since removed.
  The optimized run also observed an intermittent allocation census failure.
  The coordinator retains the combined DHO804 trial and merge hold.

- 2026-11-18: PR #221 span-row review follow-up on `perf/216-upload-rows`, published head
  `072cac8ef8b1105eaa1ffc96d2a414db107da699` above F2 `e13efdb0`. Dense Standard painting now reuses
  `fastdraw::span_rows` and maps its empty sentinel to `0..0`. Painting, clipping, routing, upload accounting,
  golden references and performance thresholds are unchanged. Added coverage for visible, vertically clipped,
  non-finite and horizontally empty spans, including unchanged empty upload revisions. The initial fixture's
  assumptions about default offsets and channel-local revisions were corrected; the focused regression passed.
  Final `oos-ui` nextest runs passed 1,783 debug and 1,782 optimized tests, with 64 skipped per profile.
  `cargo xtask check --fast` reported `partial gate passed (not done): 12 steps in 8 s`, exit 0.
  No changed-file ratchet reductions were reported. Production delta against the published head is -5 UI Rust
  lines, zero elsewhere, 108,904 total production lines (`cargo xtask loc --compare HEAD`).
  Modified files are `ui/src/drawpipe/frame.rs`, `ui/src/wave/dirty/tests.rs` and this task.
  Serial shipping `release` upload guards passed both cases with `OOS_PERF_TESTS=1`, one test thread,
  101 measurement rounds and exact reference/candidate buffer equality. Narrow-band best reference/candidate
  was 69,265/2,067 ns and p99 CPU 96,375/3,875 ns; full-height best was 112,718/59,078 ns and p99 CPU
  157,750/99,125 ns. Best/p99 limits remain 0.2/0.3 for narrow rows and 1.0/1.5 for full height.
  Evidence: `target/span-row-{ui-debug-verified,ui-optimized-verified,upload-guards,fast-verified,loc}.log`.
  Full gate was not rerun in this review job; the previously recorded compressed-frame and counter-overhead
  `perf` failures remain unclosed.
  Exact rebase after the separate F2 performance repair, renewed independent review and DHO804 acceptance remain open.
  The stacked base and every coordinator/owner hardware hold are preserved. Host only; no hardware was accessed.

- 2026-11-18: PR #224 review follow-up on `a6ff1fe7`. The UI redraws the delivered waveform under the current
  mapping when the worker planes were rejected. The regression failed before the fix and passed after it.
  Host `oos-ui` nextest passed 1,765 debug and 1,764 optimized tests, with 62 skipped in each. The fast gate
  passed all 12 selected steps. Production delta is zero. Renewed review and DHO804 acceptance remain open.

- 2026-11-18: PR #224 review fixes on published head `29bbadd57bad48fdd33d5b4a15469186c64de42a` above F1.
  P1 reset disables the view and fences the handoff before waking the coordinator. Tests cover idle cleanup with
  a full public slot and a private deposit, spare release, and rejection of in-flight and queued batches after reopen.
  Both P2 findings are covered by current waveform delivery after a mapping or dimension rejection, including a
  private deposit replaced before publication. The tests verify obsolete acquisition identity drops the waveform.
  The two watchdog regressions and public waveform regression failed on the published code. All four passed in
  both profiles. `oos-ui` nextest passed 1,764 debug and 1,763 optimized tests, with 62 skipped in each.
  The fast gate and Clippy step passed. Production delta is +52 UI Rust lines against the published head.
  Host only. Renewed review and coordinator DHO804 acceptance remain open.

- 2026-11-18: PR #221 upload integration reconciled on exact published F2
  `29bbadd57bad48fdd33d5b4a15469186c64de42a` (F1 `113ccb97`, F0 `9a802cfe`). The upload topic is
  `447c2ea5c2824846ceabb2d0088bb7cea02f7b4f` before reconciliation. Whole-deposit folds now return their
  existing touched rows through the phosphor edit API. Shared fixtures use read-only map access; the legacy
  map-add and channel-clear helpers are test-only. The worker timing fixture recycles each completed whole deposit
  after buffer return, checking its record count, without timing a UI fold or filling the bounded slot.
  No record accounting, captured projections, worker routing, accepted golden reference or threshold changed.
  Four generated size-ratchet reductions are retained for fastdraw, UI and wave. Production delta: +269 UI Rust
  lines against F2, -11 against the supplied upload topic; zero elsewhere, 108,857 total production lines.
  Modified paths: `ui/src/picture/{acc.rs,deposit/planes.rs,deposit/tests.rs}`, `ui/src/wave/phosphor.rs`,
  `ui/src/drawpipe.rs`, `ui/src/drawpipe/{budget_tests.rs,tests/deposit.rs}`,
  `ui/src/board/dho/hw/batch/tests.rs`, `xtask/data/ui/size.txt` and this task.

  Host: native `aarch64-apple-darwin`, Rust 1.99.1. UI guards use shipping `release`, opt-level 3, LTO,
  one codegen unit, one test thread, `OOS_PERF_TESTS=1`, 101 rounds. Dots use 22 runs per sample over nine
  100,000-point signals. Upload uses eight runs per sample over four 970 x 448 maps. Picture probes use eight
  warmups and 101 measured steps, 48 records per picture. Baseline and candidate measurements were serial.

  Implementation `cargo xtask check --fast`: `partial gate passed (not done): 12 steps in 33 s`, exit 0.
  After the evidence update, the handoff fast gate reported
  `partial gate passed (not done): 12 steps in 32 s`, exit 0.
  Final default `cargo xtask check`: `gate FAILED in 828 s: perf`, exit 1. All 22 other selected steps passed,
  including Clippy, decoder configurations, coverage, 65 acquisition plans, script syntax and 379 Python tests
  with zero skips. UI line coverage was 90.1% against the unchanged 86.2% floor. Rust test results:

  | Crate | Debug passed | Optimized passed | Ignored per profile |
  |---|---:|---:|---:|
  | HAL | 491 | 491 | 10 |
  | DSP | 346 | 346 | 4 |
  | Core | 41 | 41 | 0 |
  | UI, including host integrations and goldens | 1777 | 1776 | 64 |
  | FPGA daemon | 51 | 51 | 0 |
  | xtask | 242 | 242 | 0 |

  The focused drawing run passed 48 isolated nextest cases. Four-channel host scripts passed in both profiles.
  The new whole-deposit upload regression checks lines and dots, independent panel/monitor revisions, erased rows
  and skipped monitor frames. Existing live-route accounting checks read=28, deposited=28, shown=28 and zero
  dropped records for both Standard and Batch. Golden references are unchanged against F2.

  The final UI guard suite passed 28 and failed 2. Compressed-frame best reference/candidate was
  33,990/20,035 ns (0.589438, limit 0.5), p99 CPU 60,320/37,000 ns (0.613395, limit 0.75).
  Exact F2 also failed: 31,445/17,971 ns (0.571506); p99 CPU 40,240/21,120 ns (0.524851).
  The counter guard failed at one channel, 10 us/div, 20 records: best 10,017,796/10,191,278 ns
  (1.017317, limit 1.01), p99 CPU 19,574,937/19,856,312 ns (1.014374, limit 1.515).
  It stopped after two geometry rows; the exact F2 baseline passed all six. Its corresponding row was
  7,616,497/7,633,302 ns best and 14,971,687/18,182,562 ns p99 CPU.
  The corrected worker guard passed all five scenarios, with three finite-limit rows: worst best ratio 1.099424
  against 1.5, worst p99 CPU ratio 1.266199 against 2.25. No threshold was relaxed.
  The separate unchanged budget reader also exited 1 for the compressed-frame best ratio.

  A subsequent back-to-back shipping-profile run passed all three F2 baseline guards and all five candidate
  guards: dots, four-channel picture and merge probes on both, plus candidate narrow/full upload guards.
  Upload reference/candidate times were 68,927/1,989 ns best and 216,625/5,375 ns p99 CPU for eight rows;
  full height was 113,161/58,145 ns best and 157,750/108,250 ns p99 CPU.
  Narrow ratios 0.028857/0.024812 retain limits 0.2/0.3; full ratios 0.513825/0.686212 retain 1.0/1.5.
  Across nine dots signals, worst best/p99 CPU ratios were 0.986086/1.197840 on F2 and
  1.191959/1.201185 on the candidate, retaining limits 1.25/1.875.
  Four-channel picture build observations below are p50/p99 in nanoseconds, 48 records per picture:

  | Time/div | Route | F2 p50 / p99 | Upload p50 / p99 |
  |---|---|---:|---:|
  | 1 us | Standard | 3386500 / 48681166 | 3430583 / 50184125 |
  | 1 us | Batch | 3948625 / 9104625 | 4224917 / 40145125 |
  | 10 us | Standard | 14683459 / 128085417 | 15384041 / 169542958 |
  | 10 us | Batch | 14108250 / 56757166 | 13851541 / 149275625 |

  Matched merge p50/p99 at 1 us/div, 90 records, was 65,917/278,708 ns on F2 and
  235,500/369,250 ns on the candidate. At 10 us/div, 14 records, it was 111,542/6,141,000 ns on F2 and
  228,583/1,935,167 ns on the candidate. Both retain the 16,500,000 ns p99 host merge limit.

  Final-gate Batch p99 was also wide: 46,829,459 ns at 1 us/div and 120,916,083 ns at 10 us/div.
  These are host wall-time observations, not evidence of the required four-channel 30 fps unit rate.
  The acceptance target stays 30 fps and zero lost records. Compressed-frame and counter-overhead guard failures,
  independent review and the owner-controlled DHO804 round remain open. Every existing frame acceptance/merge hold
  remains. No hardware was accessed; image-tier checks were not selected.

  Earlier failures remain recorded: private-map and test-adapter compilation errors, four unused size allowances,
  an overstrict narrow-band assertion for F2 dots, two Clippy findings, and an exact-base environment override leaking
  into synthetic Git fixtures (eight xtask failures in each profile and coverage). The first full run reported
  `gate FAILED in 1060 s: clippy, tests-debug, tests-release, perf, coverage`. Its worker guard and the immutable
  F2 baseline both timed out because the old fixture never consumed deposits. The reconciled guard now passes.
  Complete final command logs remain under `target/check-logs/99875-1795032538970327000/`;
  `target/upload-integration/verified-host-gate.log`, `verified-perf-full.log`, `f2-baseline-guards.log` and
  `matched-{f2,upload}-guards.log` retain the comparisons. The overwritten initial summary is not final evidence.

- 2026-11-18: PR #224 replays F2 `3f926b6c` on exact F1 `113ccb97ddea35284467d52f513d74e95f2937f6`
  (F0 parent `9a802cfe`). The deposit coordinator publishes F0 record and measurement counts at batch completion.
  The UI records sampled claim timing at picture acceptance. Obsolete intake still counts as a settings loss.
  Invalid completed deposits retain separate stale-key and epoch counts at the coordinator and the UI.
  The acquisition fixture checks that a whole deposit includes its newest record exactly once while preserving
  the fallback buffer and frame-zero loss classifications. F1 projection and route tests and F2 pixel tests passed
  in 54 isolated nextest processes. Against exact F1, the golden manifest changes only 18 decode-route entries.
  Initial fast runs failed `size` on unused allowances and on the length of the draw function. The draw function
  now delegates the tile phase, and `cargo xtask ratchet` tightened allowances for changed UI files.
  `cargo xtask check --fast` with F1 as the gate base reported `partial gate passed (not done): 12 steps in 11 s`.
  The Clippy step also passed against F1. Debug and `test-release` runs of `oos-ui` and `xtask` exited zero.
  UI unit tests passed 1,709/1,708 with 61 ignored per profile. Each profile passed 8 goldens, 17 scripts,
  10 sockets, 16 soak/support cases and 242 xtask tests. One golden was ignored in each profile.
  The shipping-profile host probe matched Standard and Batch pixels at four channels and 48 records per picture.
  At 1 us/div, build p50/p99 was 1.554/1.688 ms for Standard and 1.694/1.856 ms for Batch.
  At 10 us/div, build p50/p99 was 8.971/10.315 ms for Standard and 7.120/8.225 ms for Batch.
  The production delta is +246 UI Rust lines against exact F1. Host only. The DHO804 acceptance and merge hold remain.


- 2026-11-18: PR #224 contains one F2 (#204) topic above the supplied F1 (#212) parent `ce496614`. Batch placement reuses
  `RawTrace` span storage; Batch tiles, Standard and the newest-record adapter use F1's full-width projection
  and shared trace representation. The pool retains F2's whole-deposit handoff. The F1 allocator probe now
  runs the production F2 placement and tile phases synchronously. Its 12 geometry rows passed on the host,
  with at most 9,672 bytes of warmed transient allocations. Calls using the old plan shape and worker helpers
  had blocked the combined build. The first fast gate reported `gate FAILED in 9 s: size` for seven unused allowances.
  `cargo xtask ratchet` regenerated the size, Clippy and mutable-static allowances for changed UI files.
  `cargo xtask check --fast`: `partial gate passed (not done): 12 steps in 26 s`.
  `cargo xtask step clippy`: `partial gate passed (not done): 1 steps in 3 s`.
  `RUST_TEST_THREADS=1 cargo test --locked -p oos-ui` and the `--profile test-release` run passed
  1,682/1,681 unit tests, 8 goldens, 17 scripts, 10 sockets and 16 soak/support tests per profile.
  Both profiles retained 59 unit ignores and one golden ignore. The `xtask` suite passed 242 tests per profile.
  The optimized four-channel host probe matched Standard and Batch pictures at 48 records per picture.
  Host build p50/p99 at 1 us/div was 1.576/4.865 ms for Standard and 4.025/8.728 ms for Batch.
  At 10 us/div it was 14.255/71.012 ms for Standard and 7.887/32.554 ms for Batch.
  The production delta is +185 Rust lines against the supplied F1 parent. The four-channel DHO804 acceptance
  and merge hold below still apply.

- 2026-11-18: F2 (#204) host implementation verified before the stack was replayed onto current F1.
  A persistent coordinator plus workers place slices of at most 64 records and move enabled-channel column bands
  through synchronous `map`/`each` phases. One slot publishes whole deposits, coalesces complete batches and wakes
  without another read. The UI accepts/folds the differences once and passes the newest waveform to readers without
  drawing or counting it twice. Sparse interpolation, prepared corrections, pairs and the legacy Standard route remain.
  Per-worker maps/mailboxes, partial merges, acquisition help, delivery callbacks and code thresholds are removed.
  Deposits close before the fixed weight can wrap; shutdown retains in-flight owners. Statistics changes refresh
  warm placement slots, and coalesced waveform taps retain their cheap folds.
  Initial fast checks failed `size` and `mutable-statics`; changed-file ratchets were regenerated. Initial Clippy
  dead-code/manual-contains findings were corrected. Early fixture assertions were corrected; golden/script contracts
  were updated for whole-deposit counts and direct codes at all strides/dots. No limits or lints were loosened.
  Final `cargo xtask check --fast`: `partial gate passed (not done): 12 steps in 13 s`.
  Final `cargo xtask step clippy`: `partial gate passed (not done): 1 steps in 5 s`.
  `RUST_TEST_THREADS=1 cargo test -p oos-ui` and the `--profile test-release` run both exited zero:
  1,628/1,627 unit tests, 6 goldens, 15 scripts, 8 sockets and 14 soak/support tests per profile.
  Each profile retains 59 ignored unit tests and one ignored golden; the additional ignored test is the F2 host probe.
  The explicit full golden update passed 269 cases, 538 fresh processes and 1,266 exact-repeat captures.
  All 633 RGB hashes and PNG files are unchanged; only 18 observed `decode_route` entries changed.
  `cargo xtask loc --compare HEAD`: +185 production Rust lines, all UI, against the -200 to -350 estimate.
  The owned pool/handoff and transitional source/reader adapters outweigh the removed mailboxes; this variance
  remains explicit for review. No dependency or hardware-register change. Source back-pressure remains F3.
  Final shipping-profile host probe passed on Darwin arm64, 4 channels, 48 records per picture. Times below are
  elapsed best/p50/p90/p99 milliseconds, not unit acceptance:
  Standard at 1 us/div: 1.987/3.623/6.123/80.358; Batch: 1.702/4.020/4.834/11.663.
  Standard at 10 us/div: 9.326/24.048/80.594/189.735; Batch: 7.633/18.633/77.255/140.655.
  Batch accept-block (`draw.merge`) p50/p99: 0.137/0.601 ms at 1 us/div, 0.075/18.508 ms at 10 us/div.
  Elapsed tails varied substantially on this host. No hardware access or unit verification is claimed.
  Coordinator retains independent review, the integrated full gate and the DHO804 Normal 4-channel 1/10 us/div
  round: 30 fps, zero lost records and UI step p99 below 33 ms. Merge remains held until that evidence passes.

- 2026-11-18: F2 plan recorded before code on `refactor/204-frames-f2`, stacked on F1. The implementation
  used the supplied design in the earlier checkout. Host work only.

- 2026-11-18: Replayed the published F1 delta for #212 onto F0 `65c45c47` using old F0 `9a802cfe`
  as the fork boundary. The new base carries the Telnet removal. The `ui.rs` size allowance is 8,668,
  preserving F1's reduction of 157 lines from the new F0 allowance. F0 loss accounting and claim timing,
  the reviewed F1 kernel, and the golden references are intact. Production delta over F0 is +106 UI Rust
  lines. The fast gate passed all 12 selected steps. `cargo test --locked -p oos-ui` with one test thread
  passed in debug and `test-release`: 1,705/1,704 unit tests, plus 8 golden, 17 script, 10 socket and 16
  soak/support tests per profile. `cargo test --locked -p xtask` passed 242 tests in each profile.
  Independent review, the integrated full gate and the required DHO804 unit run remain open. Host validation only.

- 2026-11-18: Stack #212 over #213 conflict resolution validated on exact F0 base
  `9a802cfef9a8f27bc6db48cc23da17afd35f6367`. Preserve both task sections and host golden evidence.
  Keep F0 record reuse, intentional-loss accounting, producer-local claim timing and the shared enabled-channel
  accumulator initializer. Adapt direct near/wide searches to F1 grouped inputs without restoring per-frame clocks;
  retain F1 immutable projection/kernel inputs and both counter and allocator route test modules.
  Generated ratchets reconcile drawpipe.rs to 627 and ui.rs to 8,670; no unrelated ratchet changes.
  `RUST_TEST_THREADS=1 cargo test --locked -p oos-ui` and the `--profile test-release` run both exited 0:
  1,704/1,703 unit tests, 8 goldens, 17 scripts, 10 sockets and 16 soak/support tests per profile.
  Existing 60 unit and one golden ignores remain. Record reuse, intentional selection/loss conservation, frozen
  projection, near/wide trigger policy, counter equivalence and allocator high-water tests pass in both profiles.
  No golden reference changed. Production delta over this F0 base: +106 UI Rust lines, zero elsewhere.
  Fast gate passed all 12 selected steps (exit 0). Initial fast gate failed only size slack; regeneration fixed it.
  Initial host compilation and ratchet attempts required explicit slices at the newly generic search call sites;
  those conflict resolutions are corrected. Complete host logs: `target/f1-stack-rebase-evidence/`.
  No hardware access. Independent review, integrated full gate and coordinator/owner DHO804 unit acceptance remain held.

- 2026-11-18: PR #213 inline-storage sentence corrected without code changes. The existing compiled-layout
  assertion proves the draw timing owner's bound below 17 KiB with all eight rings and bookkeeping retained.
  Host draw-ring tests passed in debug and `test-release`: 2 passed per profile, including layout and independent
  frame-ring wrapping. Fast gate passed all 12 selected steps (exit 0); no lowered ratchets.
  Production line delta is zero. No hardware access; the existing frame hold remains:
  renewed independent review, integrated full gate and coordinator/owner DHO804 unit acceptance are still required.

- 2026-11-18: PR #213 inline-storage review fix claimed for `docs/spec/state-query.md`.
  Reconcile the pre-F0 sentence with the existing compiled-layout bound for `perfguard::draw::Timing`:
  seven duration rings plus the independent frames-per-picture ring, below 17 KiB including bookkeeping.
  Documentation only; preserve samples, diagnostics, counters and the existing F0 acceptance hold.
  Run the host layout test and fast gate; GPU-upload changes from #197 remain outside this fix.

- 2026-11-18: Route-reporting regression fixed by settling each pipeline edit before reading status, as SCPI does.
  Isolated nextest regression passed. Full `cargo nextest run --locked -p oos-ui --color never` exited 0:
  1,733 passed, 61 skipped. The same command with `--cargo-profile test-release` exited 0:
  1,732 passed, 61 skipped, including all five soak modes. Fast gate exited 0:
  `partial gate passed (not done): 12 steps in 11 s`. No lowered ratchets; production delta against HEAD is zero.
  Logs: `target/f0-check-fix-{focused-debug,debug,release,fast,loc}.log`. Hardware was not accessed.
  Runner synchronization, final CI and coordinator review-state verification remain open; no review threads were supplied.

- 2026-11-18: Failed-check follow-up reproduced with `cargo xtask step tests-debug`: the route-reporting test
  fails in an isolated process because it queries the startup pipeline before accepting its model edit.
  The other 1,732 UI tests pass. Initial fast gate passed all 12 steps. Fetch was sandbox-blocked;
  runner owns branch synchronization. Review-state verification needs the coordinator's GitHub evidence.

- 2026-11-18: PR #213 watchdog finding fixed by sharing `Shared::clear_queue` with recovery, settings and sweeps.
  Queued recovery records enter `lost.error` once; legacy dropped semantics and acquisition/drawing policy are unchanged.
  The extended hard-limit regression failed before the fix with two reads and zero losses, then passed in both profiles.
  The route finding is a false positive: replay/stopped routes return first, `fast_ok` requires hardware, and hardware
  switches already use the same model selector. Existing route-reporting tests pass in both profiles.
  `cargo xtask check --fast` exited 0: `partial gate passed (not done): 12 steps in 29 s`; no lowered ratchets.
  Complete `cargo test --locked -p oos-ui -- --test-threads=1` and the same command with `--profile test-release`
  exited 0: 1,631/1,630 unit tests passed (60 ignored each), plus 8 goldens (1 ignored), 17 scripts, 10 sockets and
  16 soak/support tests in each profile. Logs: `target/f0-watchdog-{before,debug,release,fast,loc}.log`.
  Production delta against the job base is +2 UI lines and zero elsewhere. No hardware access; renewed independent
  review, integrated full gate, runner Git updates and the existing coordinator/owner unit acceptance remain open.

- 2026-11-18: PR #213 review follow-up claimed on the existing F0 branch at `67af815d`.
  Watchdog queue clearing bypasses loss accounting. The route report returns records for non-hardware sources
  before consulting the model pipeline. Runner owns Git updates; cached feature refs diverge, so no fetch/reset.

- 2026-11-18: Unit-regression host fix validated. Producers sample every 64th claim with local cursors, carry
  samples with existing maps, and never lock timing rings. The UI publishes at most one sample/age per picture.
  Deposited/measured totals publish at existing share boundaries. Claim sizes, intake budgets and pacing are unchanged.
  Legacy dropped sites are restored; `not_used` separately reports the saturating acquired-minus-used gap.
- 2026-11-18: Final shipping-release paired guard passed all six production-plan rows, 101 alternating rounds of
  16 pictures after warm-up, reusing the same pipeline/buffers/maps with a test-only diagnostics switch.
  Candidate/reference best ratios at 1/10 us/div: 1 ch 1.0026/0.9960, 2 ch 1.0030/0.9968, 4 ch 0.9986/0.9956.
  Maximum p99 CPU ratio 1.096. Limits remain 1.01 best and 1.515 p99 CPU. The budget-reader tests reject a missing
  row and observations above 1 %. Complete observations: `target/f0-unit-fix-counters-perf-final.log`.
- 2026-11-18: Full UI debug/optimized runs exited 0: 1,631/1,630 unit tests passed, 60 ignored in each;
  each also passed 6 goldens (1 ignored), 15 scripts, 8 sockets and 14 soak/support tests. Final focused counter
  checks after the test-only guard switch changed passed in both profiles: 15 passed, 1 timing guard ignored.
  The helper regression exercises 64 single-record claims: counts wait for publication and rings wait for merge.
  The six on/off cases retain identical pixels, picture membership, waveform totals and intake pacing.
  Python budget/perfmap suites passed 9/17 tests. Final fast gate exited 0:
  `partial gate passed (not done): 12 steps in 14 s`. Host/ARM Clippy exited 0:
  `partial gate passed (not done): 1 steps in 46 s`. Production delta against the job base: +50 UI lines,
  zero other crates/tools. Generated ratchets lower only the touched drawpipe size entry by one line.
- 2026-11-18: Validation corrections: the first fast gate found an unwritten test module and failed fmt, size,
  architecture, mutable-statics, unsafe, method-exceptions, known-failures and test-time. Later size growth/slack
  findings were corrected by keeping counter methods together and regenerating the lowered entry.
  An optimized helper fixture initially released its zero-worker batch before claiming; retaining its handle fixed it.
  The first performance probe used separate pipeline allocations and exceeded the best-ratio limit (1.0475);
  the final probe reuses one warmed pipeline and measures longer groups. No performance limit was loosened.
  Logs retain the `target/f0-unit-fix-` prefix. No hardware access; renewed independent review, integrated checks,
  runner Git operations and the matched coordinator/owner DHO804 A/B remain open. F0 is not unit-accepted.

- 2026-11-18: Unit-regression fix claimed on the existing F0 branch. The supplied DHO804 A/B evidence blocks
  acceptance. Remaining claim clocks and the shared timing lock still run per helper record. Runner owns Git writes;
  cached refs diverge and no fetch/reset/rebase is performed by this network-restricted worker. Host work only.

- 2026-11-18: Review-fix host validation complete. Full single-threaded UI debug and optimized crate commands both
  exited 0: debug 1,627 unit tests passed, optimized 1,626 passed, 59 ignored in each. Both also passed 6 golden
  tests (1 ignored), 15 host scripts, 8 host sockets and 14 soak/support tests. The final average-accounting assertion
  passed a focused rerun in both profiles. Golden pictures are unchanged. No hardware access or verification claimed;
  renewed independent review, the integrated full gate and coordinator/owner unit evidence remain required.

- 2026-11-18: PR #213 review fixes implemented. Queue clearing is shared, decode selection returns skip counts
  without recording losses, and the caller accounts them once. Loss/rejection reasons own their labels; frame 1 uses
  the common loss path. Claim timing uses named rings in `claim_timing`; shared ring formatting always includes p90.
  Picture accounting takes a composition value and returns nothing; enabled-channel map sizing happens once.
  Roll already incremented acquired in this checkout. New tests pin roll/scan conservation across supersession,
  STOP and rearm, both map-rejection sites, distinct labels and repeated side-effect-free decode selection.
- 2026-11-18: Review-fix fast gate exit 0: `partial gate passed (not done): 12 steps in 46 s`. Earlier size failures
  (growth, then unused slack) were corrected; generated ratchets retain only the lowered `ui.rs` and `Ui::step` counts.
  Final host/ARM Clippy exit 0: `partial gate passed (not done): 1 steps in 214 s`. Production delta against HEAD:
  +42 UI lines, zero other crates/tools. The real-handler state fixture was refreshed; all 17 perfmap tests passed.
  Average-selection caller accounting also passed focused debug and optimized checks. Complete review-fix logs use
  the `target/f0-review-` prefix. Fetch was sandbox-blocked; HEAD equals the cached remote feature ref.
  Runner owns Git writes; coordinator owns renewed review, integrated full gate and the DHO804 unit round.

- 2026-11-18: F0 (#202) host handoff ready, uncommitted. Added read/deposited/shown accounting and named source
  losses/map rejections, current route reporting, claim-level timing and per-picture record quantiles including zeros.
  Salvaged enabled-channel allocation and empty-completion coverage from closed #186 without its reduction mechanism.
  No acquisition, drawing, overload or core-placement policy change. Production Rust: +101 UI, +0 other crates and tools,
  measured by `cargo xtask loc --compare HEAD`. Generated ratchets lower only drawpipe.rs, ui.rs and Ui::step.
- 2026-11-18: F0 final `cargo xtask check --fast` exit 0: `partial gate passed (not done): 12 steps in 25 s`.
  `cargo xtask step clippy` exit 0: `partial gate passed (not done): 1 steps in 83 s` (host and ARM Clippy).
  `python3 -m unittest discover -s tools/tests -p test_perfmap.py`: 17 passed. Real SCPI state fixture regenerated
  from the handler's printed reply; no golden pictures changed. Complete logs: `target/f0-evidence/`.
- 2026-11-18: F0 optimized complete UI crate run, `RUST_TEST_THREADS=1 cargo test --locked -p oos-ui
  --profile test-release`, exit 0: 1,624 unit tests passed, 59 ignored; golden 6 passed/1 ignored, host scripts 15,
  host sockets 8, soak/support 14 passed. Debug crate run passed 1,623 unit tests/59 ignored, golden 6/1,
  scripts 15 and sockets 8, but soak initially failed its old diagnostic-string assertion after adding p90.
  Corrected debug soak rerun: all 14 passed. Final focused debug routes: 8 passed/2 ignored; map-budget tests:
  3 passed/1 ignored, including allocation, empty completion, key/epoch rejection and live conservation coverage.
- 2026-11-18: F0 host perf, arm64, 4 workers, Normal, 970 x 448, 101 measured rounds after 8 warmups.
  Quiet `test-release` baseline (`OOS_PERF_TESTS=1`, ignored `perf_guard_batch_picture_merge`): exit 0;
  `BATCH_PICTURE` merge/UI p50/p90/p99 in ms: 1 us/div, 90 records, 0.395/0.786/1.017;
  10 us/div, 14 records, 0.355/0.404/0.874. Earlier shipping-release run during builds also passed:
  0.432/1.102/2.810 and 0.415/1.631/8.370 ms respectively. No unit performance claim.
- 2026-11-18: F0 validation corrections retained in the evidence logs: first fast run started before its new test
  module existed; subsequent size growth was removed by placing reporting/allocation helpers in companion modules.
  Initial UI debug/optimized runs failed three diagnostic-order/fixture assertions, corrected by retaining the old
  field adjacencies and regenerating the fixture. The added conservation test initially exceeded the scripted source's
  32-record bound, corrected to 27. Two new Clippy test findings were fixed. An initial ratchet run exposed a macOS
  non-test cfg error in roll publication; roll now records its clear directly without a Linux-only helper dependency.
  All remaining failures were corrected and checked as recorded above; no unrelated changes or ignores were added.
- 2026-11-18: F0 remains held for independent cross-vendor review, the coordinator's integrated full gate and the
  coordinator/owner DHO804 unit run before merge. Hardware is not yet verified. Record placement for that run:
  workers on A53 cores 0..3, UI/acquisition on A72 cores 4..5, per the owner's #196 evidence. This host job neither
  accessed hardware nor changed placement. Runner owns commit/publication; coordinator owns integration and closure.

- 2026-11-18: F0 owner assignment claimed on `refactor/202-frames-f0`, base `a8eb80da`. Plan above written before code.
  F1 kernel extraction remains outside this job; only obsolete timing is removed from the existing kernel callers.
  No Git writes, publication or hardware access; the runner and coordinator own those actions.

- 2026-11-18: PR #212 quality follow-up: the test-only codes feeder returns its named batch lease;
  trigger outcomes supply statistics indices directly. Projected-frame, trigger-search and band geometry inputs
  are grouped without argument-count exemptions. Record and raw plans share projection/fold storage; record row
  bounds have one projection helper, and both deposit layouts share channel admission and record tracing.
  Projection dimensions use `usize`. Added regressions for dimensions above 65,535 and record fallbacks
  (sparse, envelope, roll, scan), partial bands, disabled channels and empty channels. Existing route, oracle,
  allocator and golden tests remain unchanged apart from the grouped-input call sites. The crate documentation
  spacing is restored. Production delta: +12 lines. `cargo xtask ratchet` lowered only the changed drawpipe file
  allowance, 659 to 657; no other ratchet changed.
  `RUST_TEST_THREADS=1 cargo test --locked -p oos-ui` and the `--profile test-release` run passed:
  1,625/1,624 unit tests, 8 goldens, 17 scripts, 10 sockets and 16 soak/support tests per profile, with the existing
  58 unit and one golden ignores. Final test-only shorthand/input-reuse edits also passed focused near/wide and
  independent raw/smoothed oracle checks in both profiles. Fast gate passed all 12 steps; Clippy printed
  `partial gate passed (not done): 1 steps in 42 s`. Complete logs: `target/cx-debug-final.log`,
  `target/cx-release-final.log`, `target/cx-fast-final.log` and `target/cx-clippy-final.log`.
  Initial fast gates failed size growth, then unused slack; grouping call sites and the generated ratchet fixed
  both. Initial test builds found an inaccessible codes feeder and one unmigrated caller; both were corrected.
  The wide-dimension fixture initially assumed the default channel offset was zero; its explicit zero offset now
  isolates truncation. Initial Clippy failed redundant field spelling and an expanded test's line count;
  shorthand and reused trigger inputs fixed both. Host only, no unit access or hardware acceptance.
  Fetch was denied by the sandbox before shared Git metadata could be written; remote freshness is unverified.
  Coordinator retains independent review, the full integration gate and the DHO804 unit round.

- 2026-11-18: PR #212 memory fix: worker accumulators now allocate phosphor planes only for enabled channels,
  release disabled planes on reuse, and retain enabled buffers across pictures. Geometry changes replace the spare.
  Captured samples remain borrowed; spans remain bounded by the projected screen columns. A synchronous production
  helper probe measures the calling thread's allocator high-water increase after input, view and display-map setup,
  over four cold and four warmed pictures. It covers 1/2/4 channels, 100 ns/div, 10 us/div and 200 us/div, and
  automatic versus explicit 1,000-point depth. Its limits allow one plane per enabled channel, current decoded
  sample storage and 256 KiB first-use bookkeeping; warmed transient allocations are capped at 32 KiB.
  A separate regression verifies channel transitions, zero capacity for disabled planes, enabled-buffer reuse,
  cleared picture contents and changed geometry. The following host allocator values are requested live bytes,
  not process RSS; each cell is cold peak / warmed transient peak. No unit access or RSS recovery is claimed.

  | Timebase / depth | 1 channel | 2 channels | 4 channels |
  |---|---:|---:|---:|
  | 100 ns/div / auto | 1,089,792 / 1,584 | 1,747,840 / 1,584 | 3,486,592 / 1,584 |
  | 10 us/div / auto | 894,216 / 9,616 | 1,750,240 / 9,616 | 4,046,688 / 5,264 |
  | 10 us/div / 1,000 points | 877,088 / 1,584 | 1,750,208 / 1,584 | 3,496,448 / 1,584 |
  | 200 us/div / auto | 881,312 / 9,808 | 1,750,432 / 9,808 | 5,088,672 / 9,808 |

  The focused probe passes all 12 rows, including a freed-transient allocator control. Initial fixture stamp and
  batch-lifetime failures were corrected. Initial fast gates failed size growth, then unused size slack; reuse of
  `Phosphor::clear` and the generated ratchet update corrected both. Only the changed file's size allowance fell,
  from 661 to 659. Production delta is -2 lines. Final debug/optimized UI suites passed: 1,623/1,622 unit tests,
  6 goldens, 15 scripts, 8 sockets and 14 soak/support tests per profile; 58 unit tests and one golden remain ignored.
  The isolated optimized probe reproduces every debug allocator value above. Host Clippy completed without new
  findings in the changed code. Fast gate: `partial gate passed (not done): 12 steps in 38 s`.
  Coordinator retains rebase/integration, cross-vendor review, the full gate and matched DHO804 RSS A/B acceptance.

- 2026-11-18: PR #212 cross-vendor review fixes: `SamplesRef` implements `Samples`; generic edge search and
  fold helpers replace wrapper dispatch. `FrameTiming` separates the software trigger level from captured timing;
  `Projection` retains caller spacing and is reused for placed bands. Typed trigger classes feed shared statistics
  recording; row bounds and deposit names have one clear owner. Deterministic regressions cover explicit spacing
  for samples/pairs, class/distance accounting and projection reuse across bands after settings changes.
  Fast gate: `partial gate passed (not done): 12 steps in 55 s`; Clippy:
  `partial gate passed (not done): 1 steps in 208 s`. Initial size and Clippy failures were corrected before these
  passing checks. `cargo xtask ratchet` tightened only three size entries for the changed files; production delta +6.
  Optimized UI tests passed: 1,620 unit tests, 6 goldens, 15 scripts, 8 sockets and 14 soak/support tests.
  Final-source debug UI tests passed: 1,621 unit tests and the same golden/script/socket/soak counts. All five
  focused optimized kernel/frame regressions also passed on final source. Both full profiles retain 58 ignored
  unit tests and one ignored golden. Host only; no unit access. Coordinator retains review, full gate and unit acceptance.

- 2026-11-18: F1 kernel extraction implemented with captured `SamplesRef`, `BatchMeta`, `Projection` and
  `Placed` inputs/results. Standard retains its sparse-record adapter; Batch retains its search policy and painter,
  sharing trigger classification and projection. One generic spans representation serves both painters.
  Codes/volts placement, folds and difference planes agree at 1/2/4 channels with lines/dots and both conversion
  signs. Captured-input and existing route/golden tests pass. No route, weight or concurrency change intended.
  `cargo xtask loc --compare HEAD`: +111 production Rust lines, all in UI, above the F1 -30 to +30 estimate;
  the frozen input types and transitional route adapters account for the growth. Variance remains for review.
  Initial fast runs failed size checks; those findings were fixed and `cargo xtask ratchet` tightened only the
  changed UI files. Fast gate evidence: `partial gate passed (not done): 12 steps in 16 s`.
  Final-source `RUST_TEST_THREADS=1 cargo test -p oos-ui` and the `--profile test-release` run passed:
  1,618/1,617 unit tests, 6 goldens, 15 scripts, 8 sockets and 14 soak/support tests per profile; 58 unit tests
  and one golden remain ignored as before. `cargo clippy -p oos-ui --all-targets` completed with no new findings.
  Optimized host probes (`OOS_PERF_TESTS=1`, one test thread) passed pixel/step picture equality and all 18
  deposit-span timing rows per test target. On this Darwin arm64 host, 4 channels, 35,160 samples/channel,
  48 records: before F1, pixel best 2.79-2.80 ms, step differences best 2.19-2.42 ms; after F1,
  pixel best 3.01-3.04 ms, step differences best 1.59-1.63 ms. Final pixel p99 4.91-5.65 ms,
  step differences p99 9.51-10.70 ms. Host timings are observations, not hardware acceptance.
  Cross-vendor review, full integration gate and the DHO804 unit round remain with the coordinator; merge held.

- 2026-11-18: F1 (#203) started on `refactor/203-frames-f1`, base `a8eb80da`. The supplied frame design
  is the specification because frames.md, migration F1, rust-style.md and the glossary are not in this checkout.
  F0 counters and perf-guard work remain outside this claim. Plan recorded before implementation.

- 2026-11-18: PR #161 review fix: both M12a fixture types now use `Route::dispatch` for acquisition preparation,
  route selection, frame delivery, the buffer-return barrier and UI stepping. The production-plan fixture implements
  the existing batch interface and retains its boxed buffer. The one/full-step matrix now checks two consecutive
  steps, counted frames and newest sequence numbers. Focused host route tests: 5 passed, 2 ignored per test target.
  Full single-threaded UI debug and optimized suites passed: 1,515/1,514 unit tests, plus 6 golden, 15 script,
  8 socket and 14 soak/support tests in each profile. The optimized route timing probe passed all 42 rows per test
  target. Host Clippy passed with no findings in the changed files; fast gate: `partial gate passed (not done):
  11 steps in 2 s`. No lowered ratchets reported. Production delta +0; host only.
- 2026-11-18: M12a host evidence is in [drawing](../architecture/drawing.md#m12a-host-draw-route-evidence-2026-11-18-157).
  Five deterministic route tests cover the requested inputs/transitions, including actual software segment completion
  and selection of an older history record. Live-pair weights differ; shared STOP/deep-read contract gaps are explicit.
  Final release evidence probes: 2 passed; 42 route timing/allocation rows plus the paired deposit profile. Four-channel plan:
  312.5 MSa/s, 35,160 samples/channel, k=1, 14 frames/read. Worker decode/deposit costs are identified; the historical
  unit 18.2 fps cause is not established. No local production fix, route deletion or owner selection.
- 2026-11-18: `cargo xtask loc --compare HEAD`: production delta +0 in every crate and total. Initial fast-gate
  module-path failures were corrected using default module registration; final `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps in 4 s`. No lowered ratchets reported. Debug UI crate tests passed.
  Initial optimized UI run passed unit/golden/script/socket suites but failed the existing process-wide observer
  census (prepare_keys=4, stage=1 unexpected allocations); the isolated rerun passed with all six phases at zero.
  Final `RUST_TEST_THREADS=1 cargo test -p oos-ui --profile test-release` passed: 1,506 unit tests (58 ignored),
  6 golden tests (1 ignored), 15 host scripts, 8 host sockets and 14 soak/support tests. The earlier concurrent
  observer-census failure remains recorded; no unrelated test change or ignore was added. Final focused release
  route tests: 5 passed, 2 timing probes ignored. No hardware tests claimed.
- 2026-11-18: Validation limits: host only; no hardware access. Coordinator still owns independent review, full gate,
  M2..M11 prerequisite acceptance, matched unit profile/load/soak, owner selection and M12b. M12 remains active.

- 2026-11-18: M12a evidence assigned on `test/157-m12a-evidence`, base `e846b2cf`.
  Host-only worker brief; runner owns git writes and coordinator owns review/full gate/unit acceptance.

- 2026-11-14: seeded from task 0001. Not yet claimed.
