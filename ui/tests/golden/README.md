# M1 CPU references

These initial synthetic host captures pin the pre-M2 drawing, including its current route differences.
They do not verify hardware or the acquisition producer loops. Demo captures use `acq.pipeline=batch`.
Every scripted hardware capture disables both measurement lists. M10 owns refreshing measurement views.

`manifest.txt` is JSON with the fixture version, integrated reference base and hermetic-input revision,
seed, batch schedule, clock, capture dimensions, decoded RGB hashes, scripts, actual settings, captured
acquisition metadata, source routes and plot rectangles. `demo/normal`, `demo/large` and `demo/larger`
cover every legacy screen script. `demo/matrix` and `batch` cover the M1 matrix in the task plan.
`m1-modes.recipes` is a named recipe catalogue expanded by `support/cases.rs`, not a standalone screen script.

Dots `Off` means lines, `On` means dots and `Auto` means the current lines outside XY. The runner requires
exact decoded equality between corresponding Auto and Off matrix cases. Codes with four channels or dots
exercise decode fallback. The 200 ns codes-rate fixture is below the 64 KiB direct-code threshold and also
falls back to decoding. `decode_route` is observed: the scripted driver counts worker decoder callbacks,
so `codes` means all deposited worker frames bypassed decoding, `decoded` means all used decoding and
`mixed` means both ran. `records-decoded` identifies the queued-record route; `source_route` separately
records workers versus queued records. Zoom and average use records. Roll/scan inject synthetic partial and complete
snapshots into the consumer route; they do not simulate the real producer loops. The `hw-average` fixture
marks a deterministic decoded record as pre-averaged; it does not implement FPGA averaging.

## Check and update

From the repository root:

```sh
sh ui/tools/layout-check.sh golden-check
sh ui/tools/layout-check.sh golden-update
```

Both use locked offline Cargo resolution and the selected worktree-local output directory from
`tools/output.py`. The update command explicitly enables replacement, stages every candidate outside
the references and waits for all cases to render identically in two fresh processes before replacement.
A later update preserves this README. Explain every changed reference in its commit message and review
the previous expected image beside the new actual image and diff. Ordinary tests and coverage are read-only. Children preserve `LLVM_PROFILE_FILE` for instrumentation while
clearing inherited application overrides. The Python runner inherits PATH and coverage instrumentation only.
`OOS_GOLDEN_UPDATE=1` has no effect in any Cargo test or gate. Only the wrapper's explicit `--update`
Python CLI invocation can publish references; exporting its inventory does not update anything.

The comparator decodes 8-bit RGB PNGs, verifies chunk CRCs and data lengths and rejects missing, extra or
wrong-size captures. Each actual plot rectangle and the remaining screen has its own allowance:
absolute RGB component differences up to 1, with at most `floor(N/1000)` pixels exceeding that threshold.
No trace or label pixels are masked. Initial repeated-run and debug/release checks require exact equality.

Each invocation keeps child output logs and a compact `summary.json`. Only failed comparisons write
expected/actual/diff PNGs, metrics, an HTML gallery and a full report. Magenta marks every changed pixel,
including tolerated differences. A missing reference fails and leaves the actual capture and log.
Only the two newest `run-*` directories per profile are retained; builds and soak logs are separate.

## Soak and gate handoff

The ordinary `soak` integration target links the same application modules through a library target with
library unit tests, doctests and benchmarks disabled. The test's `GlobalAlloc` must share a process with
the real UI and workers; launching the production binary cannot expose its live allocations. The library
therefore remains despite Cargo's shared-main target warning and the extra library compilation. Golden
comparisons continue to drive the normal binary. Its counting allocator is shared from
`ui/tests/support/counting.rs` with the G2 hot-path tests; each executable installs its own allocator.
It counts successful Rust global-allocator alloc, alloc_zeroed, realloc size deltas and dealloc using atomics
across the process,
including worker allocations. It measures requested live heap bytes, not RSS or zero allocations per frame.

The default composed soak runs in release only: five modes with 1000 steps each, a 100-step storm from
step one and a baseline at step 500 (the peak of cycle 401-500). Debug and coverage still exercise the
allocator probes, trend regression and inventory helpers. The image-tier variant runs 10 000 steps per
mode with its original baseline at step 1000 (cycle 901-1000). Every later cycle peak must stay at most
`ceil(1.01 * B)`. Both variants also require the maximum of the last three cycle-end live samples to be
no higher than the preceding three. Three-cycle windows cover the batch/storm schedule; this rejects
small retained growth independently of the 1 % allowance. Output is streamed and six samples are kept
in a fixed array before warm-up.

Raw and average use 200 ns/div; codes use 5 us/div for direct-code eligibility; live pairs use 200 us/div
at 100 k depth and deep pairs 1 ms/div at 10 M depth. Probe tests reject retained growth, verify transient
frees/realloc and reject a 1-byte end-of-cycle trend. Logs under
`target/golden/soak/<profile>/<mode>/<invocation>/` contain cycle peaks, ends, allocator high-water bytes,
operation counts, exact drop deltas and elapsed host time.

The former 3584/7168/14336-byte jumps came from bounded `pacing::State::settled` history capacity growth,
not an unbounded record queue. Each `Settle` is 56 bytes on this host. Pacing histories now reserve their
existing 300-sample bound at first use, including infrequent settings-change samples; a regression pins
stable capacity through three complete histories. Neither sample retention nor the bound changed.

Each cycle injects three old-stamp frames at a temporary newer pipe view, checks exactly three drops
and releases the no-op delivery callback. The normal batches must drain and account for every frame.
The test includes real workers where eligible and the record route for average and zoom. It neither
claims zero partial merges nor changes the existing perf guards. No listener, port or device is opened.
The shared server harness remains `tools/host_harness.py` for tests that actually need a host server.

Default Cargo stages run all golden comparisons in release and an explicit 20-case subset in debug
and coverage, covering Demo/text sizes, worker raw/direct-code/decode fallback, live/deep pairs,
average, zoom, frozen/history, roll and scan. The subset compares against the full read-only manifest.
Until task 0027 supplies `cargo xtask check --image`, the explicit extended commands are:

```sh
OOS_FULL_GOLDEN=1 cargo test --offline --locked -p oos-ui --test golden compare_scripted_pictures -- --nocapture
OOS_LONG_SOAK=1 cargo test --offline --locked --release -p oos-ui --test soak scripted_modes_reach_a_plateau -- --nocapture
```

The long soak can also run in debug with the same environment switch; `OOS_SOAK_CASE=raw` selects one
mode for focused diagnosis. The allocator authorization is `unsafe ui/tests/support/counting.rs` (nine constructs); G2 extracted it without changing
M1 accounting.
M1 leaves `xtask/` untouched. CPU references do not replace GPU parity. Task 0027 owns its gate hook. Independent Claude review and a
full gate pass outside the restricted host sandbox remain required before integration.


Measured review-round comparison times on the host: full release 96.873 s; debug smoke 112.773 s;
coverage smoke 124.472 s. These exclude compilation and are focused test runs, not the full gate.
The five-mode short release soak took 34.33 s initially and 44.24 s on final-source confirmation; the
extended raw-only soak took 63.59 s.
The coordinator measures the complete integration gate against the approximately twenty-minute budget.
