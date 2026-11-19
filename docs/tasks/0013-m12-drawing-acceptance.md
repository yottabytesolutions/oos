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

## Status

- 2026-11-19: Owner-authorized private staged-budget proposal prepared against exact #213 base.
  Source/fixture validation passed: 11 budget and 17 perfmap tests; exact-base patch applies.
  All production source, other budgets, unit acceptance and sampler/runner remain identical.
  No acceptance or review state changed. Native timing awaits experiment-supervisor release.
  The earlier counter-owner candidate remains separate and unaccepted; this proposal has no production edits.

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

- 2026-11-19: The #224 admission follow-up preserves the pre-pacing Lines plans and checks the projection held at
  worker claim. A 14-record, two-channel 10 us/div Lines read can exceed the u16 limit if claimed as Dots at 177
  columns. Its conservative bound is 4,970 hits at q = 24, against the limit of 2,730 hits. The worker returns the
  DMA buffer and accounts for the whole batch as `lost.budget`. A lossless claim of this geometry needs a wider
  representation with a revised memory budget or an approved change to the projection lifetime. Live rate and F3
  source waits remain unmeasured. The DHO804 hold remains in place.

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

- 2026-11-19: PR #224 review follow-up retains up to three whole deposits at minimum persistence. The UI grades
  the retained record count and counts each accepted deposit once. Injected-clock regressions cover the 67 ms
  age boundary, expiry without a new batch and the combined u16 hit bound. Regenerated 26 changed golden captures
  and verified debug and optimized golden comparisons. The fast gate passed all 12 selected steps.
  Host evidence only. Independent review and the DHO804 unit round remain open.

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
