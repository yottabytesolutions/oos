---
id: 2
title: "M1: golden pictures and the host harness"
kind: feature
status: done
release: 0.2.0
owner: codex
depends: [41, 42]
---

# M1: golden pictures and the host harness

## Goal

Complete [migration M1](../architecture/migration.md#m1-golden-pictures-and-the-host-harness). The migration document is the specification.

## Plan

Reserved paths: existing `docs/tasks/0002-m01-golden-pictures.md`, `ui/src/main.rs`,
`ui/src/ui/script.rs`, `ui/src/ui/host.rs`, `ui/src/ui.rs`, `ui/src/board/mod.rs`,
`ui/tests/host_script.rs`, `ui/src/drawpipe.rs`, `ui/src/board/dho/mod.rs`,
`ui/tools/layout-check.sh`; new `ui/src/ui/script/host.rs`, `ui/src/ui/script/batch.rs`,
`ui/src/board/dho/scripted.rs`, `ui/src/drawpipe/host.rs`, `ui/tests/golden.rs`, `ui/tests/soak.rs`,
`ui/tests/support/mod.rs`, `ui/tests/support/cases.rs`, `ui/tests/m1-modes.recipes`, `ui/tests/golden/`,
`ui/tools/golden.py`, `ui/tools/test_golden.py`, `ui/tools/golden_run.py`,
`ui/src/ui/script/capture.rs`, `ui/src/ui/script/host/soak.rs`,
`ui/src/board/dho/scripted/records.rs`, `ui/Cargo.toml`, `target/COMMIT_MSG.txt`.
Review round 1 extension, authorized by the coordinator: `ui/src/board/scripted.rs`,
`docs/tasks/0011-m10-grading-records.md`, `gpu/README.md`.

Steps 3-6 review round 1 plan (2026-11-15): select about twenty representative golden cases for
debug and coverage, retain all release comparisons and opt in to the full debug matrix and 10 000-step
soak. Run a 1000-step release soak by default with a cycle-end no-growth regression. Trace and fix the
reported allocation growth, retaining the fixed storm and documenting any production change separately.
Rename the recipe catalogue, add persistence/grade matrix rows and record references; derive decode routes
from observed adapter callbacks. Limit full reports to failures and retain two runs. Clear child environments,
require an explicit update command and explain the library linkage cost. Fast gate and focused tests only;
no commits, hardware, held paths or xtask edits. Additional required reservations: `ui/src/pacing.rs`
(bounded diagnostic history allocation and regression), `ui/tests/m1-modes.recipes`, `ui/tests/golden/README.md`.

Review round 1 plan: replace the adapter re-export with a board-owned scripted source/batch contract;
constrain every scripted batch capture to measurements off, test attempted re-enablement and record the M10
follow-up; propagate GPU timing writes, document the ordinary-script HDMI environment, label Demo references
as batch pipeline mode and move the size diagnostic beside its caller. Keep the two rejected blank lines
removed. Extract small modules for any size growth; leave `xtask/data` untouched. No commits or hardware access.

The coordinator accepted these implementation reservations and released task 0001's overlaps.
The current assignment is implementation steps 3 to 6 on integrated steps 1 and 2. The coordinator accepted the four-path extension
for the explicit Host constructor, record-budget condition, scripted Board port and child integration tests.
Keep `ui/src/ui.rs` to the required seam, with no refactors; all other content stays read-only.
Keep `ui/src/board/mod.rs` to a few port lines, with no cross-boundary imports. The coordinator rebases this
branch onto task 0043 (G1) after it merges. The roll-fix forward-port adds tests to `ui/src/ui.rs` concurrently.
G1 owns `xtask/` and `docs/development.md`. Neither is reserved here.
`ui/src/board/dho/hw.rs`, `ui/src/model.rs`, `ui/src/fastdraw.rs`
and `ui/src/wave.rs` are read-only inputs to M1.

Build on the incoming [hermetic tests](0030-hermetic-tests.md) change before recording references.
Reuse its synthetic calibration, injected clock and host inputs. Do not add parallel fixtures or production
`cfg!(test)` branches. Record its integrated commit and the pre-M2 reference source in the golden manifest.
No device round is assigned to M1. Deletes: nothing.

Each numbered item is one small implementation commit. Run `cargo xtask check` before each commit.
Each commit must pass the full gate, including debug and release tests, coverage, arm64 and perf guards.
Keep all existing pictures and kernel equivalence tests unchanged until their new references are added.
New characterisation tests pass when introduced, as M1 specifies.
All new files stay below 400 lines, functions below 60 lines and structs below 12 fields.
Split responsibilities into the named files. Add no fields or inherent impl files to `Ui`.
Touched legacy size entries must shrink or stay. Do not edit ratchet files, lint limits or hooks.

1. **Prepare the headless script seam. Estimate: 0.25 day.**
   In `ui/src/main.rs`, split `run_script` setup, stepping and capture into small helpers.
   Put `HostScript`, `run_script`, `render_step` and `capture` in `ui/src/ui/script/host.rs`.
   Register it through `ui/src/ui/script.rs`. Keep normal script commands and `--gpu` behaviour unchanged.
   Use the existing `cmd::run`, `Ui::step(1.0 / 30.0)`, `Ui::render` and `store::png_of`.
   This is a script-runner refactor, not a library extraction or a new application pipeline.
   Preserve `ui/tests/*.txt` and `layout-check.sh record|check` output.
   Add `ui/tests/support/mod.rs` with `run_host` and `isolated_inputs` for child processes.
   Integration tests use `CARGO_BIN_EXE_oos-ui`, so they exercise the normal application binary.
   Test frame counts, capture names, malformed commands and child exit-code propagation.
   Use fresh storage and explicit hermetic inputs in each child. Never start remote listeners or a board thread.
   Fix the screen at 1024 x 600, CPU rendering, bundled fonts and a synthetic four-channel platform.
   Fix wall time at 1762862400 UTC, locale, host readings and the existing RNG seed 0x9e3779b97f4a7c15.
   Start Demo at t = 0. Pin step counts, batch timestamps and all configuration overrides through the incoming seams.
   Clear inherited OOS overrides in the child environment. Do not read user storage, real host identity or calibration.

2. **Feed scripted batches through the real workers. Estimate: 0.5 day.**
   Define `BatchKind`, `ScriptedBoard`, `BatchSource` and `Batch` in `ui/src/board/scripted.rs`.
   The active board selects the source through that contract. Implement `ScriptedBatchSource`, `ScriptBytes`
   and `next_batch` in `ui/src/board/dho/scripted.rs`, registered in `board/dho/mod.rs`.
   Keep DHO layout and decode calls inside that adapter. Reuse the incoming calibration fixtures.
   Generate fixed integer waveforms, bounded jitter and rare pulses from an explicit seed.
   `BatchKind` covers decoded raw, direct codes, live pairs and deep pairs.
   Use the actual `hw::decode_into` and `hw::codes_into` with captured settings and rate/depth metadata.
   `ui/src/ui/script/batch.rs` holds `BatchScript`, `feed_step` and `finish_step`.
   It constructs `Source::Hw { shared: HwShared::default(), seen: 0 }`, as existing UI tests do.
   Select it only with an explicit headless script option. Reject combinations with device-source options.
   Publish the current view and call the existing `Pipe::feed_with`, with two host workers and no affinity.
   Its `Deliver` queues only the newest decoded frame after the maps merge. Do not deposit that frame twice.
   Feed the other frames as one owned `RawBatch`. Reuse buffers from `Pipe::take_buffer`.
   Add `drawpipe::host::finish_batch` in `ui/src/drawpipe/host.rs`, registered in `drawpipe.rs`.
   It waits for the returned buffer with the existing pool receiver and a bounded `recv_timeout`.
   Buffer return means all worker shares were published. Then call `Ui::step` and compose normally.
   Do not wait for `Deliver` before merge, since merge runs it. Do not use timing sleeps to settle workers.
   Fail with batch id and counters on timeout. Keep one batch in flight for picture determinism.
   Count submitted, deposited, delivered and dropped frames. Assert buffer return and newest-frame delivery.
   Direct-code cases use frames above `CODES_MIN_BYTES`, stride at most `CODES_MAX_STRIDE`, no folds and lines/Auto.
   All scripted batch kinds start with `meas` and `meas2` empty. Re-enabling either list in a script is an
   error before the next step, including while stopped. These references exclude measurement correctness:
   the legacy worker view caches measurement selection with its mapping key. M10 must remove that coupling.
   Record `measurements=off` for every scripted-batch case in the manifest, including record fallbacks.
   Four-channel codes and dots deliberately exercise today's decode fallback. Label that route in the manifest.
   Average, zoom and slow sweeps use the existing queued-record path when `Ui::fast_ok` is false.
   Do not force them into the worker fast path or implement the future ownership model.
   Preserve `batch_picture_equals_the_frames_mode`, `batch_picture_from_the_codes_equals_the_frames_mode`,
   `drawing_from_the_codes_equals_drawing_the_decoded_frame` and newest-delivery tests.

3. **Add PNG comparison and review artifacts. Estimate: 0.5 day.**
   `ui/tests/golden.rs::compare_scripted_pictures` runs every existing `ui/tests/*.txt` on Demo.
   Preserve the three text sizes from `layout-check.sh`. Rewrite shot destinations only in temporary scripts.
   Add PNG captures through the existing encoder. Compare decoded RGB bytes, not compressed file bytes.
   Store references in `ui/tests/golden/demo/<text-size>/<script>/<shot>.png` and
   `ui/tests/golden/batch/<case>/<shot>.png`. Store case metadata in `ui/tests/golden/manifest.txt`.
   Record dimensions, seed, frame schedule, settings, source route, fixture version and source revision there.
   Demo references use `Host::headless` with `acq.pipeline=batch`; label that mode explicitly in the manifest.
   They do not characterise `acq.pipeline=frames`.
   `ui/tools/golden.py` provides `read_png`, `compare_rgb`, `write_diff` and `write_report`.
   Use Python's standard library for PNG decoding and encoding. Add no Rust or Python package dependency.
   Reject missing, extra, truncated or wrong-size pictures. Validate the expected capture count for every script.
   Initial tolerance: maximum absolute RGB component error 1 on the 0..255 scale.
   At most floor(0.001 * N) pixels may exceed it, where N is the compared region's pixel count.
   Apply that budget separately to each main plot, zoom plot and remaining screen region.
   This prevents a large unchanged background from hiding a trace regression. Mask no trace or label pixels.
   Exact equality remains the deterministic repeat-run self-check and the kernel equivalence requirement.
   Test equality, threshold edges, a translated trace, brightness loss, size errors and missing captures in
   `ui/tools/test_golden.py`. Invoke these tests from the Rust integration test so the gate runs them.
   On failure, keep expected.png, actual.png, diff.png, metrics.txt and index.html per case under
   `target/golden/<profile>/<case>/`. Highlight every nonzero RGB difference, including tolerated ones.
   The report shows the three images together, differing-pixel counts, maximum error and the changed box.
   Print artifact paths on failure and retain the child log. CI can archive this directory without regeneration.
   Extend `ui/tools/layout-check.sh` with `golden-check` and `golden-update` wrappers.
   Update command from the repository root: `sh ui/tools/layout-check.sh golden-update`.
   It runs the release golden integration test with an explicit update flag and locked dependency resolution.
   Stage all candidates outside the reference tree. Replace references only after every case rendered successfully.
   Ordinary tests never create missing references or update them. A changed reference needs a reason in its commit.
   Run twice from fresh processes and require identical decoded captures before accepting the initial set.

4. **Record the M1 mode matrix. Estimate: 0.5 day.**
   Add named cases in `ui/tests/support/cases.rs` and named-menu scripts in `ui/tests/m1-modes.recipes`.
   Use the following explicit matrix in addition to the existing screen scripts.
   Lines means `DotsMode::Off`, dots means `On`, Auto means `Auto`, not trigger Auto.

   | Cases | Settings and captures | Current route |
   |---|---|---|
   | Demo baseline | lines/dots/Auto x 1/2/4 channels x 200 ns and 200 us/div, Min | Demo records, `acq.pipeline=batch` |
   | Batch baseline | raw/codes/live pairs/deep pairs x lines/dots/Auto x 1/2/4 channels, Min | workers plus newest record where eligible |
   | Rate/depth | raw and codes at 200 ns and 5 us/div; live pairs at 200 us/div, 100 k; deep pairs at 1 ms/div, 10 M | actual batch metadata; bounded synthetic payload |
   | Persistence | lines and dots, 1 and 4 channels, 200 us/div, Min/500 ms/10 s/Infinite; warm-up and later captures | Demo and batch |
   | Colour grade | off/on, 1 and 4 channels, 200 us/div, Min/500 ms/Infinite | Demo and batch |
   | Average | software average count 16, lines/dots/Auto, 1 and 4 channels, 5 us and 200 us/div; first and settled captures | queued records; separate pre-averaged `hw_avg` case |
   | Zoom | lines/dots/Auto, 1 and 4 channels, 5 us and 200 us/div, Min/500 ms; main and zoom captures | existing record path, no worker zoom claim |
   | Frozen records | RUN/STOP/RUN, SINGLE and history replay at fixed step indices | existing record redraw path |
   | Roll and scan | synthetic `Frame::roll` or `Frame::scan`, 200 ms/div, lines/dots, 1 and 4 channels; two sweeps and STOP | current sweep record path only |

   Use fixed batch counts, including a 27,3,13,3,27,13 sequence, and fixed capture indices.
   Pair inputs retain their envelope flag and captured acquisition depth. Do not silently substitute raw samples.
   Auto currently draws lines outside XY. Verify its setting and equality with the corresponding line picture.
   Do not promise the future sparse-Auto or cross-route brightness behaviour.
   The roll/scan references characterise the consumer route that will become `Replace { Sweep }`.
   There is no typed `Replace` today. M1 must not add it.
   Real roll/scan/segments producer-loop coverage and the SINGLE roll memory case are explicitly M3.0.
   Cross-route brightness within 3 %, rare-pixel retention and zero partial merges remain later-step acceptance.
   Keep separate current references even where they expose today's different weights.
   The hardware setups in [drawing acceptance](../architecture/drawing.md#acceptance-criteria-unitlab) inspire
   these inputs. They are synthetic host characterisations, not verified unit measurements.

5. **Add the passing host allocation soak. Estimate: 0.75 day.**
   `ui/tests/soak.rs::scripted_modes_reach_a_plateau` launches a fresh headless process per mode.
   Exercise normal raw/codes, live pairs, deep pairs and average for 10 000 `Ui::step` calls each.
   Compose each step. Run two real workers where eligible, and use the existing record route for average.
   Repeat a fixed change storm from step 1: channel count, scale, offset, trigger level, persistence,
   zoom, CLEAR and RUN/STOP/RUN. Cycle every 100 steps so all capacities warm before step 1 000.
   Change mappings only after the prior batch completes. Add explicit old-stamp drops with exact expected counts.
   Record all available drop counters per mode and assert submitted-frame accounting after each drain.
   Expect zero unplanned drops. Do not assert M9's zero partial merges on today's asynchronous implementation.
   The standalone integration soak links the existing `main.rs` as a library target, with library unit tests and
   doctests disabled. Re-export the existing `Ui` type at the crate root and use its narrow scripted-soak facade.
   The process-wide counting `GlobalAlloc` lives in `ui/tests/soak.rs` only. No production allocator or native
   heap introspection is used. G1 must authorize this exact test-only unsafe path while holding `xtask/`.
   At step 1 000 set plateau B to the maximum sampled live bytes over the final complete 100-step storm cycle.
   Every later cycle peak must stay at most ceil(1.01 * B). Record cycle-end live and allocator high-water bytes.
   Include workers, queues, history, maps and source pool. Open fixed output buffers before warm-up; stream rows.
   It must track successful alloc, alloc_zeroed, realloc deltas and dealloc across workers, without recursion.
   Do not add an allocator to the production binary or duplicate G2's zero-allocation hot-path guards.
   Test the measurement with a deliberately retained buffer that grows beyond 1 %, plus a freed transient buffer.
   If the current modes fail the bound, record the evidence and return to the coordinator.
   Do not loosen 1 %, silently warm for longer, ignore the soak or repair memory behaviour inside M1.
   Save bytes by cycle, counter deltas and elapsed host time under `target/golden/soak/<profile>/<mode>/`.
   Do not infer M5b's unit RSS bound, a two-hour soak or zero allocations per frame from this plateau.

6. **Verify gate coverage and hand off references. Estimate: 0.25 day.**
   Both integration tests are ordinary, non-ignored Cargo test targets.
   The existing full gate runs them through its debug and release test steps. No xtask hook is needed.
   Keep comparisons read-only during coverage too. Fail a zero-case selection or a missing Python interpreter.
   Use a worktree-local target directory and separate artifact paths per profile and invocation.
   Give every child an outer timeout as well as the worker completion deadline. Reap children on failure.
   Keep `tools/perf-guard.sh` and its allocation counts and timing ratios unchanged.
   Continue running `gpu/test-host.sh` as required by migration rules; CPU goldens do not replace GPU parity.
   This base's xtask gate has no GPU step. Ask G1's owner to wire it, or retain its separate full result.
   Do not edit G1's reserved files here. G2 owns new p99 guards and general counting-allocator infrastructure.
   Supply a capture inventory and the expected/actual/diff gallery for independent review.
   Record full gate output, exit code, counts, soak peaks and source provenance in Status.
   Update this task's progress after each commit. The coordinator closes and moves it to `done/` at integration.
   Later steps M2 to M6 preserve these references. Intended M7/M8/M10 drawing changes explain each update.

Total implementation estimate: 2.75 host days after prerequisites, plus coordinator review.

### Risks and open questions for the coordinator

1. Which integrated hermetic-tests revision is the reference base before M2? Accept the implementation claims
   after releasing task 0001's overlapping paths. No initial references should be recorded on the old inputs.
2. Does the initial RGB tolerance of 1 with a 0.1 % outlier budget per region preserve the intended sensitivity?
   Review the first gallery and repeated-run results. Do not widen it to conceal a visible change.
3. Resolved by the coordinator: the Rust counting allocator is confined to the soak executable. Native heap
   introspection is not used. G1 owns the precise test-only unsafe authorization; the 1 % bound is unchanged.
4. Can the current record budget and worker handoff produce repeatable complete captures under host load?
   Use bounded completion and small record batches. If wall time still changes content, coordinate the deterministic
   seam with the hermetic-tests owner before recording, rather than changing drawing policy here.
5. Who owns adding `gpu/test-host.sh` to the full gate while G1 holds `xtask/`? M1 must retain a separate result
   until that hook lands. Roll/scan producer and bounded-memory claims remain owned by M3.0 and M5b.

## Status

- 2026-11-15: final-source soak confirmation for review round 1: release 8 passed in 44.24 s,
  including all five 1000-step modes and their cycle-end checks; debug 8 passed in 0.25 s, including
  allocator probes/trend tests and the documented release-only composed-soak selection. Logs:
  `target/m1-r1-soak-{release,debug}-final.log`. Completed all owned validation processes. The earlier
  34.33 s short run remains useful timing evidence; final source and its report changes were reconfirmed.
  `target/COMMIT_MSG.txt` has five proposed sections with file lists, including the separate production
  pacing fix. No commits. Pending coordinator/G1 allowance and independent Claude verification remain.

- 2026-11-15: steps 3-6 review round 1 fixes, intentionally uncommitted for the standard-tier Claude
  verification round. Read `delivery.md` from `overhaul`; fast gate and focused tests only. P1 runtime:
  ordinary release compares all 260 cases/615 PNGs; debug and coverage compare an explicit 20-case/44-PNG
  subset including every route and pre-averaged records. Full debug uses `OOS_FULL_GOLDEN=1`.
  Default composed soak is release-only, five modes x 1000 steps, baseline cycle 401-500; the explicit
  `OOS_LONG_SOAK=1` variant retains 10 000 steps and the original cycle 901-1000 baseline. README records
  these temporary image-tier commands pending task 0027. No `xtask/` change.
  P2 allocation source: bounded `pacing::State::settled` retention, with 56-byte `Settle` entries and
  VecDeque capacity jumps accounting for 3584/7168/14336 bytes. Reserve each diagnostic history's existing
  300-entry bound on first use, leaving samples and eviction unchanged. The capacity regression failed before
  (exit 101, one test failed), then passed in debug and release (exit 0, one test each):
  `target/m1-r1-history-{before,after,release}.log`. This production fix has its own message/file-list section
  in `target/COMMIT_MSG.txt`. The soak adds a fixed six-cycle end-live window: maximum of the last three ends
  must not exceed the prior three, independently of the 1 % cycle-peak allowance. The regression rejects
  a one-byte retained rise. Initial short release suite passed 8 tests in 34.33 s; the extended raw-only
  10 000-step test passed in 63.59 s with 300 planned drops and no unplanned or zone drops. Evidence:
  `target/m1-r1-soak-{release-short,release-long-raw}.log` and `target/m1-r1-soak-audit.json`.
  P2 matrix/recipes: renamed the catalogue to `m1-modes.recipes`; 32 additional cases cross codes/live pairs/
  deep pairs with 500 ms and grade Off/On, lines/dots and 1/4 channels, plus Demo/raw 200 ns persistence rows.
  Recorded 64 new references after two exact fresh-process captures each. All 551 existing PNGs remain
  byte-for-byte unchanged (`target/m1-r1-reference-audit.json`). No shot/metadata output is present in `ui/`.
  P2 artifacts: full image/HTML reports are failure-only, successful runs retain compact timings and child logs;
  prune to two `run-*` directories per profile. Seven Python regressions pass, including failure-only reporting
  and retention. Cleared the completed update's duplicate candidate tree. P2 library target: retained because
  the integration allocator must observe the UI and workers in its own process. Disabled library unit tests,
  doctests and benchmarks; the ordinary binary still drives goldens. README and Cargo comment explain the
  extra compilation and shared-main warning. No new application target or startup change.
  P3: record actual worker decode callback use (`codes`, `decoded`, `mixed`, or `records-decoded`) instead
  of parsing provenance, covering the 200 ns codes fallback with child/runner tests. The Python runner clears
  its environment except PATH and explicit coverage instrumentation. Removed the unrelated quit child from
  the allocation probe. Cargo tests never honor `OOS_GOLDEN_UPDATE`; only the wrapper's explicit Python
  `--update` can replace references. The read-only debug test passed with inherited `OOS_GOLDEN_UPDATE=1`
  and `OOS_PIPELINE=frames`; old reference hashes are unchanged.
  Measured comparison times (focused runs, not a full gate): release full 96.873 s (260 cases, 520 processes,
  1230 exact-repeat captures); debug smoke 112.773 s; coverage smoke 124.472 s (20 cases, 40 processes,
  88 exact-repeat captures each). Cargo suite times are 99.02/113.12/125.85 s respectively; compilation is
  separate. Logs: `target/m1-r1-golden-{release,debug,coverage}.log`. Focused host tests pass in both profiles:
  13 tests, debug 11.18 s, release 0.67 s. Final host clippy exits 0 with no new warning counts in touched paths;
  pacing retains its existing ratcheted findings. Changed-file privacy audit: 1255 files, zero findings;
  message scan: zero. The broad tree audit still finds 153 pre-existing findings outside these changed files.
  Final fast gate exits 1 exactly as printed: `gate FAILED in 14 s: unsafe`. Formatting, sizes, architecture
  and mutable statics pass; the sole failure is nine constructs in `ui/tests/soak.rs`, whose exact-path
  authorization remains with the coordinator/G1. No ratchet, held-path, hardware, hook or credential changes;
  no commit, push or full-gate acceptance claimed. All current source/reference changes are intentional.
  Coordinator owns the pending independent verification and full integration gate.


- 2026-11-15: steps 3-6 author handoff ready, intentionally uncommitted on `t0002-golden-pictures`, parent
  `2b5d170fd59d487bbf8851eab84820b9bd07a24c`. Final candidate source fingerprint:
  `a4a77c733be740cb051592f3feca8e489cee43fb13cdc63eff8e42f7730b369b` (`target/m1-candidate.json`).
  Golden references: 228 cases, 551 PNGs; two fresh runs per profile, all 551 decoded RGB hashes exactly equal
  across debug and release. Both final corpus runs pass. Explicit updates remain
  `sh ui/tools/layout-check.sh golden-update`; normal checks and coverage do not update references.
  PNG/comparison/environment regressions: 5 Python tests pass. Headless child integration: 11 pass in each profile.
  Ordinary-script metadata compatibility: 1 unit test passes in each profile; post-change Demo/raw reference
  sanity checks match exactly in two fresh processes per profile. Existing worker tests: 11 pass, 2 opt-in timing
  tests ignored per profile; source/record fixtures: 5 pass per profile. No kernel-equivalence references changed.
  Allocation soak: all five modes complete 10 000 composed steps in each profile. Debug target reports
  5 passed, 0 failed, 0 ignored; release reports 6 passed, 0 failed, 0 ignored. Each mode accounts for 136468
  acquired, 136168 used and exactly 300 planned old-stamp drops, with no unplanned/zone drops. Maximum later-cycle
  growth is 0.108 % in each profile, below the unchanged 1 % bound. The independent CSV audit verifies cycle
  901-1000 baselines, every later peak, complete step counts and counter sums (`target/m1-soak-audit.json`).
  Full gate remains exit 1: `gate FAILED in 611 s: unsafe, tests (debug), tests (release), coverage`.
  Final fast gate fails only the nine unsafe constructs of the approved soak-only allocator. All formatting,
  size, architecture, mutable-static, host/ARM clippy and perf checks pass; no ratchet, hook or xtask files changed.
  The full gate's socket and temporary-Git permission failures and HAL-stopped coverage require an unrestricted
  coordinator run. Focused instrumented child runs prove host/batch/PNG metadata coverage; no coverage floor
  pass is claimed. G1 owns the exact test-only allocator authorization; no new test-discovery step is needed.
  Privacy message scan and diff check pass. Changed files contain only the two independently verified synthetic
  IDAT false positives; the proposed exact policy and regression proof are in `target/m1-privacy-exceptions.json`,
  `target/m1-privacy-proof.json` and `target/m1-privacy-changed-files.json`. The policy is unchanged. Adopt reviewed
  exceptions before the reference fixture commit, as the privacy instructions require.
  Final galleries: `target/golden/debug/run-6px18b5q/index.html` and
  `target/golden/release/run-a4zcpn7p/index.html`. Final soak evidence uses invocation
  `40167-1794740275834307000-0` in debug and `5882-1794742030030821000-0` in release.
  Four proposed commit messages, reasons and per-step file lists are in `target/COMMIT_MSG.txt`; shared files
  require step-specific hunks. All source/reference changes are intentional. Build receipts verify through
  `tools/output.py`. Completed/superseded galleries and scratch files were cleaned, with final evidence and
  active build caches retained. The superseded finite soak exited normally; all known task processes are gone
  and its abandoned inputs were removed (`target/m1-active-processes.json`).
  Remaining coordinator actions: G1 authorization/integration, reviewed privacy-policy adoption, independent
  Claude review and the full gate/coverage on the final commits. GPU gate wiring stays with task 0027.
  The coordinator closes/moves this task at integration. No commit, push, hardware access or unit verification.


- 2026-11-15: late compatibility check confines PNG metadata sidecars to explicit headless execution. Ordinary
  script capture behavior stays intact; its unit regression passes in debug and release, and the full 11 child
  tests pass in each profile. Demo/basic and raw baseline reference sanity checks use two fresh processes per
  profile and match the recorded RGB hashes exactly (`target/m1-metadata-sanity.json`). Full corpus proof remains
  551/551 exact debug/release matches. All source checks are complete apart from G1's owned unsafe authorization.
  Debug live pairs has completed 10 000 steps within 1 %; raw, codes and pairs are now complete in both profiles.
  Deep-pairs and average debug validation remains running. The independent CSV audit agrees with every completed
  mode's 136468 acquired, 136168 used and 300 planned drops, with no unplanned or zone drops.


- 2026-11-15: reference validation checkpoint for steps 3-6. Final release and debug golden runs both pass:
  228 cases, 551 references, 456 fresh processes and 1102 exact-repeat captures per profile. Independent decoded
  SHA-256 comparison finds zero debug/release differences across all 551 captures
  (`target/m1-debug-release-equality.json`). Galleries are `target/golden/release/run-a4zcpn7p/` and
  `target/golden/debug/run-6px18b5q/`. New Python self-tests: 5 passed; child integration: 11 passed per profile;
  synthetic source/record tests: 5 passed per profile; worker tests: 11 passed, 2 opt-in timing tests ignored,
  per profile. The named existing kernel equivalence tests remain unchanged and pass.
  Final release allocation suite passes all 6 tests, including five modes x 10 000 composed steps, all within
  1 %, exactly 300 planned drops per mode and no unplanned/zone drops. The report buffer is preallocated before
  warm-up. Raw/average use 200 ns/div; codes use 5 us/div for direct-code eligibility; pairs/deep pairs retain their
  100 k/10 M acquisition metadata. Independent CSV audit is `target/m1-soak-audit.json`. Debug raw and codes have
  completed 10 000 steps; live pairs is running. Both test targets are ordinary Cargo targets, with library unit
  tests disabled to avoid duplication. Child environments preserve only instrumentation's `LLVM_PROFILE_FILE`
  alongside hermetic inputs; focused coverage proves execution in host, batch and PNG metadata modules
  (`target/m1-coverage-child-proof.json`). This is not a full coverage-floor result.
  Renewed full gate exits 1: `gate FAILED in 611 s: unsafe, tests (debug), tests (release), coverage`.
  Formatting, sizes, architecture, mutable statics, host/ARM clippy and perf guards pass. UI debug/release each
  report 529 passed, 21 socket-restricted failures, 29 ignored. HAL reports 329 passed, 1 permission failure,
  8 ignored; xtask reports 40 passed, 1 temporary-Git-commit permission failure. Coverage stops at that HAL failure.
  G1 owns authorization of the accepted test-only allocator path (nine unsafe constructs); M1 edits no `xtask/`.
  Privacy finds two accidental email-shaped byte sequences inside PNG IDAT payloads, absent from decoded RGB.
  No real addresses are present. Exact candidate exceptions and regression evidence are in
  `target/m1-privacy-exceptions.json` and `target/m1-privacy-proof.json`. Policy is unchanged. Under
  `tools/privacy/README.md`, independent review and policy adoption precede the fixture commit; the coordinator
  owns that ordering. Changed-file audit finds zero other findings; the message scan has zero findings.
  Final full gate/coverage outside the sandbox and independent Claude review remain required. Source fingerprint
  and evidence index are in `target/m1-candidate.json`; four proposed messages and per-step file lists are in
  `target/COMMIT_MSG.txt`. All source/reference edits remain intentional and uncommitted by instruction.
  Superseded galleries and scratch drafts were removed; active debug/soak evidence and build caches are retained.


- 2026-11-15: steps 3-6 resumed on `t0002-golden-pictures`, integrated base
  `2b5d170fd59d487bbf8851eab84820b9bd07a24c`; intentionally uncommitted. Step 3 adds strict stdlib RGB PNG
  decoding, per-plot and remaining-screen RGB-1/floor(N/1000) comparison, complete capture inventory checks,
  expected/actual/nonzero-diff galleries and explicit atomic reference replacement through
  `sh ui/tools/layout-check.sh golden-update`. Four comparator regression tests pass. Output directories use
  `tools/output.py`; headless children open no listener. The existing server harness is not duplicated.
  Step 4 records 228 cases and 551 initial PNGs plus a manifest. Final release recording passed:
  456 fresh processes, 1102 exact-repeat decoded captures, including exact Auto/lines equality. Reference base,
  hermetic-input revision, settings, acquisition metadata and regions are recorded. Worker cases keep both
  measurement lists off; Demo is batch mode. Synthetic roll/scan and pre-averaged records characterise consumers.
  Visual QA found an obscured zoom capture; the new tested `menu close` command and menu-state check prevent it.
  SINGLE exposed a pre-arm fixture batch: it now supplies one post-arm record. The regression fails with the
  old plan (exit 101) and passes with the fix (exit 0), in `target/m1-single-{before,after}.log`.
  Child integration tests pass in both profiles: 11 passed, none failed or ignored. Synthetic record/source
  tests pass in debug: 5 passed. Final reference gallery: `target/golden/release/run-a4zcpn7p/index.html`.
  Step 5 links the unchanged application module graph as a library target with library tests disabled; the
  sole allocator is in `ui/tests/soak.rs`. The retained-growth/free/realloc probe passes. Five release modes
  completed 10 000 composed steps each within the 1 % bound, with exactly 300 planned old-stamp drops per mode,
  no unplanned or zone drops. Debug remains running. Raw/average use 200 ns/div, pairs 200 us/div, deep pairs
  1 ms/div; the final codes selection uses 5 us/div to exceed the direct-code byte threshold and is being rerun.
  Fixed 100-step changes start at step one; baseline remains cycle 901-1000. No warm-up extension or bound change.
  Current evidence: `target/m1-soak-{debug,release}-final.log`, `target/golden/soak/`.
  Step 6 validation is in progress. First full gate exited 1: `gate FAILED in 745 s: unsafe, clippy,
  tests (debug), tests (release), coverage`. Formatting, size, architecture, mutable statics and perf guards passed.
  Fixed the new slow-vector-initialization lint; renewed gate and ARM clippy are running. G1 must authorize the
  exact test-only `unsafe ui/tests/soak.rs` path (nine constructs) for the accepted allocator; no `xtask/` edits.
  Socket/Git-fixture/coverage restrictions remain. Final debug reference comparison is running. An interrupted
  superseded dense-debug child could not be terminated under the sandbox; its private process handoff is in
  `target/m1-active-processes.json`, owner codex, next action coordinator cleanup outside the sandbox.
  No full-gate pass, independent acceptance, GPU parity or hardware verification claimed. GPU gate wiring stays
  with task 0027 per coordinator answers. No commit, push, ratchet, hook, credential or device changes.


- 2026-11-15: review round 1 resumed on `t0002-golden-pictures`, base `d12539c`, uncommitted by instruction.
  Coordinator rejected both blank-line restorations; removed them. `board/mod.rs` remains 454 measured lines,
  `drawpipe.rs` remains 1006 with no diff. P2-1: replaced the concrete DHO re-export with the board-owned
  `ScriptedBoard`, `BatchSource` and `Batch` port in `board/scripted.rs`; active-board selection and buffer/decode
  contract tests cover all four kinds. DHO layout and metadata remain in the adapter.
  P2-2: every scripted batch starts with both measurement lists empty and rejects re-enablement before stepping,
  including STOP. Added unit and child-command regressions, the manifest constraint in the plan and the M10
  measurement-view refresh follow-up. The unit regression failed with the old measurement behavior (exit 101)
  and passed after restoring the fix (exit 0): `target/m1-r1-measurement-{before,after}.log`.
  P3: GPU timing append/open/write errors now propagate through the runner's Result to exit 2; helper tests
  check append format and open failure on this host, with `/dev/full` write failure covered on Linux only.
  Documented ordinary GPU-script HDMI environment behavior in `gpu/README.md`; no GPU runtime verification.
  Labelled future Demo references `acq.pipeline=batch` in the plan and manifest requirements; no reference
  manifest exists yet in assigned steps 1-2. Moved `script_size_error` beside configure and tested the diagnostic.
  Focused offline, locked tests exited 0 in debug and release: `ui::script::` 5 passed; `board::` 58 passed,
  2 ignored; `drawpipe::` 11 passed, 2 ignored; child integration 8 passed. These retain the named existing
  picture-equivalence and newest-delivery tests. Logs: `target/m1-r1-{script,board,workers,child}-{debug,release}.log`.
  Host and ARM64 clippy exited 0, with no new or grown UI findings after the gate's source-location deduplication:
  `target/m1-r1-clippy-{host,arm}.{json,log}`, `target/m1-r1-clippy-comparison.json`.
  Final fast gate exited 0: `partial gate passed (not done): 5 steps in 16 s`. Formatting, size, architecture,
  mutable statics and unsafe passed; log `target/m1-r1-fast-final.log`. No `xtask/data` files changed.
  Determinism check exited 0: `PASS: 10 cases, 40 fresh processes, 80 decoded PNG captures`; repeat runs,
  debug/release and conflicting inherited OOS overrides match exactly. Logs/report: `target/m1-r1-determinism.log`,
  `target/m1-determinism/report.json`. Six cases intentionally differ from the previous handoff because the
  default measurement strip is now absent: raw, live/deep pairs, average, zoom and slow records. Demo and all
  codes cases retain prior hashes. Visual inspection confirmed the measurement-free raw plot and Demo strip.
  Comparison: `target/m1-r1-picture-comparison.json`. No checked-in golden references changed.
  `git diff --check` passed. Full gate and GPU parity remain for the coordinator outside the unchanged sandbox,
  as in the preceding handoff; the known socket, Git-fixture, coverage and Docker restrictions still apply.
  No full-gate pass, independent acceptance or hardware verification claimed. No commit, push or hardware access.
  Handoff: `target/COMMIT_MSG.txt`. All remaining source/doc changes and the new port file are intentional.

- 2026-11-15: review round 1 follow-up on committed base `d12539c`, owner requests uncommitted work only.
  Restored the deleted blank lines in `ui/src/board/mod.rs` and `ui/src/drawpipe.rs` first to check the
  coordinator's explicit size-ratchet stop condition. `cargo xtask check --fast` exited 1:
  `gate FAILED in 24 s: size limits`. Exact findings: `ui/src/board/mod.rs` grew `454 -> 455`;
  `ui/src/drawpipe.rs` grew `1006 -> 1007`. Formatting, architecture, mutable statics and unsafe passed.
  Stopped implementation as instructed; no ratchet files changed. Full output: `target/m1-review-r1-fast.log`.
  P2-1's board-owned trait, P2-2's measurement-off manifest/task constraint and regression test, the M10 note,
  GPU timing write-error propagation, plain-script HDMI documentation, Demo batch-mode manifest label and
  relocation of `script_size_error` remain pending the coordinator's size decision. No full gate or runtime
  tests run for this follow-up. Draft handoff: `target/COMMIT_MSG.txt`. No commit, push or hardware access.

- 2026-11-15: steps 1-2 implementation handoff on `t0002-golden-pictures`; intentionally uncommitted.
  Added `--headless-script --script FILE` and optional `--scripted-batch raw|codes|pairs|deep-pairs`.
  Entry precedes platform detection, source startup and configuration reads. It supplies a synthetic four-channel
  platform, fixed Host inputs/time, default switches, CPU panel rendering and two workers without affinity.
  Host configuration disables the record time budget; `Host::system` and existing unit fixtures retain it.
  The only `ui/src/ui.rs` change is the budget condition. The only Board-port change replaces one blank line with
  the scripted-source re-export; DHO types stay inside their adapter and existing Board/Host boundaries hold.
  The driver publishes the current layout/view, permits one in-flight batch, waits for buffer return, then lets
  `Ui::step` merge the maps and deliver the newest record. It checks acquired/used/dropped counts, newest sequence,
  delivery count and empty queues. The 27,3,13,3,27,13 schedule accounts for 86 frames with zero drops.
  Direct-code fixtures disable folds; four-channel codes and dots exercise decode fallback. Average, zoom and
  slow timebases use queued records. Slow Auto uses captured 100k-point metadata to keep the synthetic payload
  bounded, rather than materializing the streaming configuration's full read-out. This is not roll/scan producer
  coverage, which stays with the later mode matrix and M3.0.
  Child tests cover step/capture/quit behavior, PNG and PPM names, malformed commands, exit codes, incompatible
  options, exact repeat-process RGB equality, frame accounting and queue drains. A concurrent test run exposed
  timestamp collisions in temporary directories; exclusive directory reservation and a repeated-clock regression
  test now prevent children sharing inputs or deleting another test's files.
  Current child integration tests pass in both profiles: 7 passed, 0 failed, 0 ignored (exit 0).
  Broader UI binary runs before the final slow-fixture extension each exited 101:
  514 passed, 19 failed, 29 ignored. All 19 failures require sandbox-denied sockets: 13 HTTP tests,
  `net6::tests::dual_stack_or_ipv4`, `net::tests::reads_without_waiting`,
  `scpi::vernier_tests::compound_query_is_one_line_on_the_socket`, both SNTP socket tests, and
  `ui::tests::bode_sweep_drives_the_generator`. Exact names and limits are in
  `target/m1-sandbox-limitations.txt`; complete output is in `target/m1-resume-ui-{debug,release}.log`.
  The earlier full gate also could not complete HAL socket tests, xtask's temporary Git fixture commit or
  coverage. As instructed, the coordinator runs the full gate outside; this handoff does not claim a full pass.
  GPU parity could not start: Docker API access denied (preflight exit 1, `target/m1-gpu-preflight.log`).
  Final focused checks (offline and locked, exit 0 in each profile): script/source selection 7 passed,
  1 ignored (Node.js); `drawpipe::` selection 11 passed, 2 ignored (opt-in host timing guards).
  These include the worker completion barrier, newest delivery and the three existing frame/code picture-equivalence
  tests. Logs: `target/m1-focused-{debug,release}.log`, `target/m1-workers-{debug,release}.log`,
  `target/m1-child-{debug,release}.log`. Existing wall-time pacing diagnostics are permitted on stderr; exit failures
  and all other diagnostics still fail successful child assertions.
  Final picture check exited 0: `PASS: 10 cases, 40 fresh processes, 80 decoded PNG captures`.
  Demo, raw, codes, live/deep pairs, average, zoom, slow queued records, four-channel codes and codes/dots each
  match exactly across two fresh runs and debug/release, including conflicting inherited OOS overrides in run two.
  PNG dimensions and CRCs were checked; comparison uses decoded RGB, with hashes and retained captures in
  `target/m1-determinism/report.json`. Reproduce using `python3 target/m1-determinism/verify.py` after building
  the debug and release application binaries. Complete result: `target/m1-determinism.log`.
  Host and ARM64 `cargo clippy --all-targets` both exited 0, with no UI lint ratchet growth. Existing warnings remain.
  Logs: `target/m1-resume-clippy-{host,arm}.{json,log}`. `git diff --check` passed.
  Latest fast gate exited 0: `partial gate passed (not done): 5 steps in 12 s`.
  Formatting, size limits, architecture, mutable statics and unsafe placement passed (`target/m1-resume-fast.log`).
  An earlier fast run caught the Board file growing by one line; the port export now replaces a blank line.
  No ratchet file was changed.
  `COMMIT_MSG.txt` contains current proposed messages without draft-only notes. The coordinator owns the G1 rebase,
  independent Claude review and outside-sandbox gate/GPU verification. Steps 3-6 were not started.
  No hardware or external-service access, commit, push, ratchet change or cleanup of active build caches.

- 2026-11-15: coordinator ACCEPTED the claim extension for `ui/src/ui/host.rs`, `ui/src/ui.rs`,
  `ui/src/board/mod.rs` and `ui/tests/host_script.rs`, under the narrow shared-file conditions above.
  Resumed steps 1-2 on the existing uncommitted drafts. Owner explicitly requests no commit; maintain
  `COMMIT_MSG.txt` for handoff. Socket and temporary-commit sandbox failures are expected; the coordinator
  runs the full gate outside the sandbox. No hardware access is assigned.

- 2026-11-15: codex implementation part 1 on `t0002-golden-pictures`, integrated base `overhaul` 3a3d51d.
  Drafted steps 1 and 2 only: extracted the off-screen script runner and added PNG capture through the existing
  encoder; added synthetic raw/codes/live-pair/deep-pair read-outs, an owned reusable buffer, the pool-return
  completion barrier and a one-batch driver with newest delivery and frame-accounting checks.
  Added isolated child-process support, but it is not connected to integration tests yet.
  Blocked on claim clarification: the accepted plan keeps `ui/src/ui.rs` read-only, while the coordinator requires
  disabling its record time budget through an explicit Host/config seam. Requested the minimal extension for
  `ui/src/ui/host.rs`, the budget condition in `ui/src/ui.rs`, the scripted-source port re-export in
  `ui/src/board/mod.rs` and `ui/tests/host_script.rs` before editing them. The Board boundary currently rejects
  the draft driver's direct DHO import; use the port re-export after the claim is accepted.
  The current source uses the existing `decode_into_probed(..., false)`, so no edit to `ui/src/board/dho/hw.rs`
  is needed. The batch driver is not connected to the runner; do not treat either implementation step as complete.
  Focused final offline debug check (`cargo test --offline --locked -p oos-ui --bin oos-ui script`):
  6 passed, 0 failed, 1 ignored. This includes the three synthetic-source tests, runner capture/quit test and two
  existing script-related tests; the ignored test requires Node.js. Pair inputs now assert low <= high after decode.
  The worker barrier and direct-code/fallback tests are included for the full gate. The first direct-code test
  incorrectly expected the kernel itself to reject stride four; corrected it to include the worker's stride guard.
  Kept the panel-size error helper in `main.rs` to preserve the existing layout lint budget without changing it.
  Child exit handling,
  hermetic application startup, repeat-process and debug/release picture equality still need verification.
  First full gate, offline with worktree-local `target/`, exited 1:
  `gate FAILED in 968 s: clippy, tests (debug), tests (release), coverage`.
  Formatting, sizes, architecture, mutable statics, unsafe placement and perf guards passed.
  New unused batch code and the direct DHO import remain draft failures until the authorized wiring is possible.
  The first run also saw the subsequently corrected code-route expectation and panel-reference move.
  Other failures included sandbox permission denials in socket tests and xtask's temporary Git fixture commit;
  coverage stopped at the failing HAL tests.
  Final full rerun exited 1: `gate FAILED in 539 s: clippy, tests (debug), tests (release), coverage`.
  Formatting, sizes, architecture, mutable statics, unsafe placement and perf guards passed again.
  UI debug and release each reported 512 passed, 20 failed, 29 ignored: the draft Board import plus existing
  sandbox-restricted tests remained failures; the corrected code-route and layout checks passed.
  Coverage again stopped at the HAL socket-test permission error (329 passed, 1 failed, 8 ignored).
  Clippy also reported an idiom warning in the new pair assertion; changed it to `as_chunks::<2>()` after that
  stage ran. That one-line cleanup has not had a renewed clippy pass. The new unused items remain unfinished wiring,
  not approved exceptions. Do not report a passing full gate or a completed implementation.
  Complete evidence: `target/m1-part1-final-gate.log`; earlier gate: `target/m1-part1-gate.log`;
  focused tests: `target/m1-focused-final.log`. These worktree-local logs and build caches are intentionally retained.
  `COMMIT_MSG.txt` holds two draft messages and per-commit file lists. No project commits or pushes.
  All Cargo resolution was offline. No device or external service access.
  Hardware was not verified. Independent Claude review remains the coordinator's next step after implementation.
  Time log: 08:39-09:14 UTC, 35 minutes of measured implementation/checking and waiting, including both full gates
  and the pending claim extension. Initial documentation reading before 08:39 UTC was not separately timed.
  Next action: the coordinator accepts or declines the four-path extension; codex then connects hermetic startup,
  the explicit budget policy and batch execution, adds the child tests and checks repeat/debug-release equality.
  A full passing gate and cross-vendor review remain required before integration. Steps 3-6 were not started.

- 2026-11-15: codex planning claim on `t0002-golden-pictures`, base `overhaul` 9e5e895.
  Owner requested planning only. Wrote the implementation sequence, proposed reservations and coordinator questions.
  No implementation, network, device access, commit or push. Implementation claims await coordinator serialization.
  Planning validation: task structure, reserved paths, source entry points and local diff review.
  The full gate was not run for this documentation-only planning session.
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
