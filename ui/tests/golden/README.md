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
exact decoded equality between corresponding Auto and Off matrix cases. The F2 deposit kernel reads codes
at all channel strides and in dots mode. Sparse records retain their interpolation adapter. `decode_route` is observed: the scripted driver counts worker decoder callbacks,
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
wrong-size captures. Every decoded RGB byte must match, including each plot rectangle and the remaining screen.
A one-byte difference fails; there is no pixel budget, tolerance or mask. Region metrics only locate the change.
Fresh-process repeats and debug/release comparisons use the same exact references.
PNG compression and chunk encoding are not compared: decoded pixels are the rendering contract.

Each invocation keeps child output logs and a compact `summary.json`. Only failed comparisons write
expected/actual/diff PNGs, metrics, an HTML gallery and a full report. Magenta marks every changed pixel,
including a single component differing by one. A missing reference fails and leaves the actual capture and log.
Only the two newest `run-*` directories per profile are retained; builds and soak logs are separate.

## Soak and gate handoff

The ordinary `soak` integration target links the same application modules through a library target with
library unit tests, doctests and benchmarks disabled. The test's `GlobalAlloc` must share a process with
the real UI and workers; launching the production binary cannot expose its live allocations. The library
therefore remains despite Cargo's shared-main target warning and the extra library compilation. Golden
comparisons continue to drive the normal binary. Its counter bookkeeping lives in
`ui/tests/support/counting.rs`; only `ui/tests/soak.rs` installs the allocator and implements
its unsafe System adapter. G2 hot-path tests and M2.2 allocation censuses run inside that
same soak executable. Golden and host-script executables retain the default allocator.
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

The buffer return follows cleanup of the batch's decoder captures. Returning it earlier lets
the observer sample captured settings while their worker is still freeing them. A destructor barrier regression
pins this ordering without elapsed-time assertions; the one-channel-block trend allowance stays unchanged.

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
without publishing a waveform tap. The normal batches must drain and account for every frame.
The test includes real workers where eligible and the record route for average and zoom. It neither
claims zero partial merges nor changes the existing perf guards. No listener, port or device is opened.
The shared server harness remains `tools/host_harness.py` for tests that actually need a host server.

Default Cargo stages run all golden comparisons in release and an explicit 23-case subset in debug
and coverage, covering Demo/text sizes, worker raw/direct-code/decode fallback, live/deep pairs,
average, zoom, frozen/history, roll and scan. The subset compares against the full read-only manifest.
Until task 0027 supplies `cargo xtask check --image`, the explicit extended commands are:

```sh
OOS_FULL_GOLDEN=1 cargo test --offline --locked -p oos-ui --test golden compare_scripted_pictures -- --nocapture
OOS_LONG_SOAK=1 cargo test --offline --locked --release -p oos-ui --test soak scripted_modes_reach_a_plateau -- --nocapture
```

The long soak can also run in debug with the same environment switch; `OOS_SOAK_CASE=raw` selects one
mode for focused diagnosis. The allocator authorization is `unsafe ui/tests/soak.rs` (nine constructs). Shared safe
bookkeeping preserves M1 accounting.
M1 leaves `xtask/` untouched. CPU references do not replace GPU parity. Task 0027 owns its gate hook. Independent Claude review and a
full gate pass outside the restricted host sandbox remain required before integration.


Measured review-round comparison times on the host: full release 96.873 s; debug smoke 112.773 s;
coverage smoke 124.472 s. These exclude compilation and are focused test runs, not the full gate.
The five-mode short release soak took 34.33 s initially and 44.24 s on final-source confirmation; the
extended raw-only soak took 63.59 s.
The coordinator measures the complete integration gate against the approximately twenty-minute budget.

## Menu and layout coverage (issue #170)

The `layout-*.txt` scripts extend the existing scripted corpus at Normal, Large and Larger text sizes.
`support/cases.rs` checks `layout-menus.txt` against all 48 menu headings in `docs/spec/menus.md`.
Every script is automatically included in the full inventory; `layout-menus` runs at all three sizes in the smoke set.
These are captures of the current implementation, including its existing clipping and navigation limitations.
They do not implement the draft inventory's proposed H1/N1 changes.

| Inventory surface | Script and capture family |
|---|---|
| All 48 MenuId variants | `layout-menus`: capture names match inventory headings |
| Channel and ChMore indexed expansion | `layout-menus`: `Channel-1..4`, `ChMore-1..4` |
| Features decoder pages | `layout-menus`: `Decoders-1..2` (29 production protocols) |
| Protocol-specific Decode rows | `layout-decoders`: every production protocol in registry order |
| Trigger conditional rows, bus match/relation, LA source, hex picker | `layout-triggers`; existing `triggers` covers the twelve analogue types |
| Power pages; inverter page/angle layouts; Dpt fourth source | `layout-states`: `power-*`, `inverter-*`, `dpt-*` |
| Analyser pages and overflow menus | `layout-states`: `*-view`, `*-overflow`; existing `spectrum`, `eye`, `xy` |
| Generator waveform/modulation/frequency keypad; Bode generator-dependent rows | `layout-states`: `generator-*`, `bode-*` |
| Acquisition and cursor conditional rows | `layout-states`: `acquire-*`, `cursor-*` |
| Scope/Full/Split/Four densities and Windows assignment rows | `layout-states`: `windows-*`, `density-*`; existing `panes` |
| Catalogue categories, Selected pagination and all per-measurement settings layouts | `layout-catalogue`; existing `menus`, `results` |
| Cards/Compact chrome, sources and plot/Eye context popups | `layout-states`; existing `startmenu`, `picker`, `zoom` |
| Reference, storage, replay and event/result panes | existing `refs`, `storage`, `replay`, `search`, `results` |

### Exactness limits

There are **no exceptions in the deterministic CPU corpus**. All synthetic batch routes, Demo captures,
menus, fonts and layouts compare exactly. Time and input data are fixed by the headless runner.

The following comparisons are outside that deterministic contract:

- CPU versus GPU parity retains `gpu/test-host.sh`'s tolerance: the GPU's floating-point shaders and rasterisation
  differ from the CPU implementation. These are separate parity checks, not exceptions to CPU golden comparison.
- Live hardware waveforms and changing OS/network/calibration status cannot match a fixed screenshot exactly:
  their samples, timestamps and service results vary. Goldens use synthetic data and hermetic host inputs instead.
  Hardware verification remains separate.
- The removed native load-destination modal (menus-1618..1620) has no current rendered state; it is a tombstone.
  Dynamic WLAN/key lists need supplied entries and are not live network captures.
