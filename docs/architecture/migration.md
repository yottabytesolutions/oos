# Overhaul migration

How `oos-ui` gets from the code on `overhaul` (056c020) to the target in `README.md`. Strangler: the old code
keeps running until the new path replaces it, one small step per merge. This file absorbs the implementation plan of
[drawing implementation plan](drawing.md#implementation-plan) and its change-detection amendment; where they differ, this file applies.

## Rules for every step

- **One branch, one step.** A branch implements one step. Larger steps split into numbered parts (M2.1, M2.2).
- **Failing-before tests.** The step's tests fail on the parent commit and pass after. A refactor step names the
  existing tests and golden pictures that must stay unchanged instead.
- **Deletions.** The step deletes what it lists. The reviewer checks with grep that the names are gone.
- **Gate.** The overhaul gate passes (`../development.md`, pending the guardrails merge): `cargo xtask check` and the test runs, fmt, clippy, ui and hal tests in release and debug with full counts, arm64
  check, size limits, boundary rules, perf-guard, `gpu/test-host.sh`.
- **Ratchet.** The ratchet files shrink or stay. Module rules have no ratchet: a step adds the rules in
  `xtask/data/architecture.txt` that its modules now meet ([Enforcement](README.md#enforcement)).
- **Moves.** The size check counts a moved or renamed item as new, which would reject the migration's own moves (M2.1,
  M3, M13 to M18) at their first commit. So the gate is move-aware before any moving step starts (G1): the branch
  lists its planned moves in `xtask/data/moves.txt` (`<old> -> <new>`, one per line, part of the step's plan, not a
  ratchet file), and the branch gate accepts a listed item at its new place if it is not larger than at the merge
  base. A moving branch passes its own gate from its first commit. At merge the integrator runs
  `cargo xtask ratchet --move`, which transfers the entries and empties `moves.txt`.
- **Shared ratchet files.** Branches do not edit the ratchet and coverage files. The integrator runs
  `cargo xtask ratchet` at merge, so two branches never conflict on them. The coverage gate allows a margin of 0.5
  points below the floor per branch; the integrator raises the floor after the merge. The ratchet files are split per
  crate so that unrelated merges touch different files. Implemented in G1. `docs/development.md` lives on
  `overhaul-guardrails`; its ratchet text is aligned with these rules when that branch merges (team-lead).
- **Hotfix lane.** A fix the unit needs before the overhaul ships branches from `dho804` as `fix/<name>`, is merged to
  `dho804`, and is forward-ported to `overhaul` as a regression test plus the fix in its target place, not as a
  cherry-pick of the old code.
- **Known failures.** A test that records a known bug before its fix is marked `#[ignore = "known: task NNNN"]`, with
  the task file that fixes it. The gate lists every known failure (from G1); the list may only shrink, and the fixing
  step removes the mark.
- **Docs.** The step updates the "code still differs" notes and, if a decision changed, adds an ADR.
- **Unit rounds.** Only where marked. Install rule: `oos-update install <pkg> --no-kexec --execute`, then reboot.
  Unit rules as in the project: no hand FPGA register writes while oos-ui runs, no reads of 0x205c, never FPGA flash,
  p4, p5 or U-Boot, cal files read-only, wait for OOSK re-arm, never two reboots within 60 s.

## Prerequisite: guardrails

[Guardrails](../tasks/0041-guardrails-baseline.md) land first: `rustfmt.toml` and the format commits, the clippy set (adr/0009), the gate
script, the size checker with its allowlist (adr/0010), the module and crate rules (`xtask/data/architecture.txt`),
the pre-commit hook. Migration steps start when the gate script exists.

## G1: gate prerequisites

Owner: auxout. Lands before M2.1, because M2.1 is the first step that moves code. Each item is a gate change with its
own tests in `xtask`:
- move-aware validation: `xtask/data/moves.txt` on the branch, no growth against the merge base, and
  `cargo xtask ratchet --move` for the integrator at merge;
- the coverage margin of 0.5 points per branch;
- ratchet files split per crate;
- the function limit counts code lines only (no comments, no blank lines; adr/0010);
- module rules over external crate paths (`forbid ui <module> oos_hal::dho`), replacing the `board::boundary` lint;
- a shrink-only count of `#[allow(clippy::disallowed_methods)]` per crate, so an adapter-level exception cannot spread
  without showing in review;
- the known-failure listing.

Estimate: 2 days, auxout, in parallel with M1 and M1b.

## G2: performance gates

Owner: auxout. Lands before M3c, the first step with a performance budget of its own ([Performance
budgets](README.md#performance-budgets)):
- tail-aware perf guards: p99 over repeated runs against a reference path, next to today's best-of-rounds ratios;
- a counting-allocator harness for the hot paths, asserting zero allocations per frame after warm-up;
- unitlab reads the budget table from the architecture and asserts each row it can measure.

Estimate: 1.5 days, auxout, in parallel with M2.

## Steps

D-numbers are the steps of [drawing implementation plan](drawing.md#implementation-plan). Unit marks the steps followed by a unit round.

| # | Step | D | Unit |
|---|---|---|---|
| M0 | Picture counters: stale, partial, age, composition | 0 | |
| M1 | Golden pictures and the host harness | | |
| M1b | Safe start, exit contract, shutdown order (adr/0015) | | round 0 |
| M2.1 | `oos-core` crate with today's `Model` | | |
| M2.2 | `classify` in shadow mode next to the old mechanisms | 1a | round 0 (shadow) |
| M2.3 | Consumers switch to the `ChangeSet`, one class per part; old mechanisms deleted last | 1a | |
| M3 | Acquisition thread split | 1b | |
| M3c | Acquisition waiting: command `eventfd`, hardware polls inside `poll` | | |
| M3d | Main event loop: `epoll`, page-flip pacing, light step, non-blocking remote output | | round 1 |
| M4 | Mapping key, preview, generation for records | 1c | round 1 |
| M5 | Typed channels replace `Shared` | | |
| M5b | Memory model: `memplan`, byte bounds, ring chunks, watchdog, drop counters | | |
| M5c | DMA completion: MSI, INTx and poll measured; default decided | | round 2 |
| M6 | Measurement reads never draw | 2 | |
| M7 | One lines kernel, behind `draw.model` | 3 | round 2 |
| M8 | Dots: one count per sample | 4 | |
| M9 | `BatchUnit`, workers pre-sum by row bands | 5 | round 3 |
| M9b | GPU work proportional to change: dirty rows, fenced pool, on-demand snapshots, HDMI budget | | round 3 |
| M10 | Grading and records together, behind `draw.model` | 6 | round 4 |
| M11 | Sparse-event pacing | 8 | |
| M12 | Acceptance, remove `draw.model` and the old paths | 9 | round 5 |
| M13 | Settings groups replace `Model` | | |
| M14 | Commands per area | | |
| M15 | Remote adapters on commands and queries | | |
| M16 | Stages and `Session` replace `Ui::step` | | |
| M17 | Views and menus as components; `Ui` deleted | | |
| M18 | Moves into the target layout; allowlist clean for `ui.rs`, `hw.rs`, `model.rs` | | round 6 |

The drawing work (M2 to M12) comes before the UI breakup (M13 to M18) because it carries the user's first goal: a
fast, responsive UI without hiccups or flicker. The UI breakup is guarded by the golden pictures and host tests that
M1 and the drawing steps leave behind.

### M0: picture counters (done)

3692085 and the review fixes 73236d4 on `overhaul`. Tests: counters count in pipe and UI tests. Known limit: stale is
decided when a share is drawn, so a share drawn just before a change and merged after it is not counted
([drawing status](drawing.md#status)). M4's mapping-key counter replaces it as the measure of stale.

### M1: golden pictures and the host harness

- Turn the scripted screen tests (`ui/tests/*.txt`: keys, frames, shot) into golden comparisons on the Demo source,
  with references made from `overhaul` before M2 (adr/0011).
- Add the display modes the drawing steps change: lines, dots, persistence Min and 500 ms, zoom, colour grade, at a
  fast and a slow time base.
- Cover the batch path, not only the Demo frames: a minimal scripted-batch source (raw, codes, pairs, deep pairs)
  feeds the real `drawpipe` with its workers on the host, and its pictures get references too. The drawing steps
  change that path most.
- Tests: new, pass on creation. They pin today's pictures so that M2 to M6 can prove "no change".
- The host soak (`ui/tests/soak.rs`, README.md, [Testing](README.md#testing)) as a passing
  characterisation test: today's `Ui` (not yet `Session`) on the scripted-batch source, for the modes that source
  covers (normal, pairs, deep pairs, average). It asserts the allocation plateau those modes already reach. It does
  not claim to cover the acquisition producer: the scripted source bypasses it. The producer part (roll, scan,
  segments through the real loop) comes with the shared acquisition-loop harness in M3.0, and its bounded-memory
  assertion becomes mandatory with the fix that makes it pass (M5b, or earlier with the forward-ported roll fix).
- Deletes: nothing.

### M1b: safe start and exit

Prerequisite: the `dho804` roll fix (`fix/roll-queue`) is forward-ported to `overhaul` through the hotfix lane as
soon as it merges, so the roll path that bypasses the hold check is closed before M1b relies on STOP. M1b does not
wait for M5 or M5b for roll.

adr/0015, [Safe start and exit](README.md#safe-start-and-exit). Independent of the drawing steps; it lands
early because every later unit round depends on a scope that cannot loop.
- rcS: the exit counter and `--safe`. `oos-ui`: the markers, `--safe`, persist after `stable_s`, the bounded
  shutdown order, the GPU helper fallback, the RSS watchdog.
- Tests, failing before: a scripted `oos-ui` stand-in that aborts at start reaches `--safe` within 3 restarts in a
  host test of the rcS loop; a setup changed less than `stable_s` before an abort is not restored; the durable-write
  interruption tests for every marker, counter and setup transition; the shutdown order joins the threads that exist
  at M1b (main loop, remote listeners, deposit workers, GPU helper) within their bounds; a killed helper gives CPU
  pictures at the next step. Cancellation of the acquisition thread through trigger waits and split reads comes with
  the scripted `Device` in M3.0, and its join bound is tested there; `AcqCmd`-based shutdown is tested in M5.
- Tests also: three aborts 20 minutes apart reach `--safe`; exit 75 goes to `--safe` at once; a same-boot restart
  after an abort restores the last stable settings from `/tmp`; a clean power-off within `stable_s` persists.
- Unit check in round 0: a restart-loop case with a setup that aborts at start; a clean shutdown by update.
- Estimate: 2 days.

### M2: one change-detection point (D1a)

M2 is split into three steps, because every clear and reset moves to one point and a wrong class shows only on the
unit.

**M2.1** creates the crate `oos-core` (adr/0014) and moves today's flat `Model` into it, with the types it names (math,
decode, zone and protocol settings types). Its calls into the UI and the board (`board()`, `la::available`, the
protocol registry) become values passed in: platform capabilities in `Config`, the registry's option lists in
`Context`. The crate rules in `xtask/data/architecture.txt` hold from here on. Pure move; moves recorded with
`ratchet --move`. Estimate: 2 days.

**M2.2** adds `classify`, `normalize`, the outbox and `Generations` in shadow mode. The old mechanisms still act, and
the shadow writes nothing:
- **Separate shadow state.** The shadow keeps its own previous `Model` snapshot, its own `Generations` and its own
  outbox. `normalize` runs on a copy of the step's `Model`, never on the live one; the old `clamp_offsets` and
  `fit_platform` calls stay where they are until M2.3.
- **Observe-only outputs.** The shadow `ChangeSet`, the shadow outbox and the shadow `normalize` result are only
  compared and counted. Nothing reads them to act.
- Each step the shadow `ChangeSet` is compared with what the old mechanisms did (which clears ran, whether
  `hw_stamp` moved), the shadow `normalize` result with the live `Model` after the old clamps, and the shadow outbox
  with the old internal writes. Every disagreement is counted per class in `:SYSTem:STATe?` (`shadow acq=N
  mapping=N ...`) and logged once with the field.

Items 5 and 6 of the list below (normalize moving in, the outbox) therefore take effect in M2.3, not M2.2. **Round 0 (shadow):** knob spins, SCPI writes, setup loads, autoset and Bode on the unit, with all counters
at 0 or each disagreement explained as a bug of the old code. Estimate: 3 days.

**M2.3** switches to the `ChangeSet` in parts, each through the gate. A part switches a writer and its consumers
together, because switching one side alone acts twice or not at all:

| Part | Switch together |
|---|---|
| M2.3a `normalize` | the live `normalize` call before `classify`; the removal of the per-step `clamp_offsets` and `fit_platform` calls (both at once, or the clamps run twice) |
| M2.3b `acq` | the `gen` and `epoch` writers (`hw_stamp` bumps in `ui.rs`, `ui/bode.rs`, `ui/autoset.rs` replaced by tokens); `AcqWant` publication from the change point; the records and measurement filters on `gen` and `epoch` |
| M2.3c outbox | SINGLE or segments done to STOP, autoset and Bode steps and `auto_step` writing through the outbox; the run-state transition consumers that react to them |
| M2.3d `mapping`, `phosphor` | the 16 `dirty_map = true` sites; the accumulator's preview and map reset on the `ChangeSet` |
| M2.3e `stats`, `history`, `mask`, `zone`, `hw_average`, `math`, `view` | `settings_changed` and `mark_changed` callers for these classes; `stats_gen` on measure jobs; each view's `RESETS_ON` |

The old mechanisms are deleted in the last part, when the shadow counters have been 0 since M2.2's round. Estimate:
2.5 days.

Design: [Change detection](README.md#change-detection), adr/0003. It includes compress's four review changes and
upstreamarch's review:
1. Requests next to the diff, returned by stages as values.
2. `Generations`, private to the change point: `classify` calls `new_gen`, request resolution calls `new_epoch`
   (`epoch` replaces `hw_stamp`). Autoset and Bode get their epoch by `Request::Epoch(token)`.
3. `UiAcq` derived each step as a value: XY, the protocol watch, the mask reference, autoset or Bode active.
4. Run state by transition.
5. `normalize` before `classify`, in a fixed group order: `clamp_offsets`, `fit_platform` and the cross-group clamps
   move in and stop running elsewhere each step.
6. The outbox for internal writers: SINGLE or segments done to STOP, autoset and Bode steps, `auto_step`.
7. The change point publishes `AcqWant` (still through `Shared` until M5) with its `route` tag, and the mapping for
   the pipe, so no other code writes the acquisition request.
8. Each existing view declares the classes it resets on (trend, eye, XY persistence, cursors).

Classes: `acq`, `mapping`, `phosphor`, `stats`, `mask`, `zone`, `hw_average`, `history`, `math`, `view`. Measurement state
(the analysis results, statistics and `meas_n`) is in `stats`. No `decode` class: the decode cache is keyed.

`classify` is split per future settings group from the start, as hand-written functions, each under 60 lines: one
or more per group, each destructuring its own fields of today's flat `Model`. No macro generates them (macro bodies
are a blind spot of the gate). One destructuring of `Model` without `..` checks at compile time that every field
belongs to a group. It lives in a `#[test]` function, which the size limit exempts, so its length (178 fields) is
not a size violation. M13 then moves the fields without reclassifying.

Tests, failing before:
- every class: changing a representative field resets exactly its consumers;
- SCPI and HTTP writes (`store::set`) reach the same point as a knob;
- a mapping change in STOP redraws from `last[0]` and never blanks;
- an autoset or Bode stage accepts only records of the epoch its token resolved to;
- `normalize` is idempotent, `fit_platform` switching a channel off yields `acq`, a clamped offset yields `mapping`
  and a preview;
- SINGLE done switches to STOP through the outbox, and the STOP is in the next `ChangeSet`;
- every view's declared classes match what it resets in a change of each class;
- the mapping key changes exactly when a `mapping` field changes, a relayout (panes, HDMI) included;
- CLEAR, the RUN and SINGLE edges and Create mask reset their consumers with no settings difference;
- a snapshot test lists every field classified `Unrelated`, so adding one shows in review;
- in M2.2: the shadow counters stay 0 over the scripted change storm on the host.

Deletes: `settings_changed`, `mark_changed`, the 16 `dirty_map = true`, the `hw_stamp` bumps in `ui.rs`, `ui/bode.rs`
and `ui/autoset.rs`, the per-step calls of `clamp_offsets` and `fit_platform` outside the change point. Risk: medium. Every clear and reset moves to one point.

### M3: acquisition thread split (D1b)

M3.0 comes first: the `Device` trait (configure, arm, wait, read, read again, disarm, each cancellable,
[Cancellation](README.md#cancellation)) over today's hardware calls, a
scripted device, and characterisation tests of the current loop on the host: one pass per phase, STOP, SINGLE, a
settings change mid-fill, roll and scan. They record today's behaviour, including what the design calls wrong (the
ended fill that is still read), and M4 then changes those expectations on purpose. The roll soak case lives here: roll
after SINGLE over 10 000 passes with a byte count of what the loop publishes. Unless the `dho804` fix
(`fix/roll-queue`) has been forward-ported by then, it is a known failure (`#[ignore = "known: ..."]`) until M5b.
Estimate: 1.5 days.

Then split the closure in `hw.rs` `spawn` (lines 1263-2154 on 32041d0) as in `acquisition.md`:
- an `AcqLoop` struct for the about 30 loop locals (1328-1393);
- set-up (1263-1394);
- apply settings (1476-1548);
- service: self-cal, generator, DVM, levels, trim, slots;
- stream: roll and scan, scan as an option (1610-1786);
- segments;
- batch, including the STOP hold (1797-2154).

Pure refactor. Tests: the M3.0 characterisation tests, the existing hw tests and all golden pictures unchanged. Deletes: nothing, moved code. The
allowlist entry for `hw.rs` and its `spawn` shrinks. Unit: smoke check (probe comp, setup B) inside round 1.

### M3c: acquisition waiting

[Waiting](README.md#waiting), adr/0016. After M3, on its `AcqLoop`.
- One blocking call: `poll` on the acquisition's command `eventfd`, with the hardware period as timeout. The trigger
  poll (500 us) and the ring-count poll (200 us) move inside it; helper work between ring reads stays.
- The 9, 5, 2, 20 and 500 ms sleeps of roll, hold, SINGLE and scan, the 50 ms settings poll, the 10 ms segment
  polls and the error back-offs become deadlines in the same call.
- Tests, failing before: with the scripted `Device`, STOP, a settings change and cancel are handled within 1 ms
  in every phase (armed, filling, roll, hold, segments); an armed scope in STOP does no hardware poll.
- Estimate: 1.5 days.

### M3d: main event loop

[Main loop and pacing](README.md#main-loop-and-pacing), [Hand-offs](README.md#hand-offs). Independent of the
drawing steps; it comes early because every later unit round measures latency against it.
- Main blocks in one `epoll_wait`: the DRM page-flip fd, a wake `eventfd`, input fds, the GPU helper socket. Steps
  start on the flip; `wait_flip` and its 100 ms timeout go; the 33 ms timer and its busy last millisecond go.
- The light step for input and remote requests: intake, change, acquire, reply.
- Remote replies go into per-connection output queues written by the listeners on `EPOLLOUT`; main never writes to a
  socket (today `scpi.rs` 46 and `vxi11.rs` 319). The request inbox becomes a bounded MPSC queue with main's wake.
- Tests, failing before: a stalled SCPI client does not delay a step; a settings round trip stays under 2 ms on the
  host with a busy picture; an idle main wakes only on flips and input.
- Unit check in round 1: settings round trip, remote throughput (`ifload.py`), idle CPU in STOP.
- Estimate: 2 days.

### M4: mapping key, preview, generation for records (D1c)

As D1c: the batch keeps its first `View`; the UI drops published batches of another mapping; preview of `last[0]` on
a mapping change; the acquisition generation on records; the batch's trigger snapshot for refinement; invert, coupling
and bandwidth in the mapping key; the queue cleared on reconfigure; an early-ended fill dropped; decode reads the
batch's calibration.

Tests, failing before:
- a mapping change gives a preview at the next step and never a blank trace;
- no batch is drawn under two mappings;
- records of an old generation are not measured.

Deletes: `hw_stamp` use for the view, blank-on-change.

**Unit round 1** (M2 to M4, with M3c and M3d): knob spin, settle over 20 changes; clears and resets as before (screenshots, measurement
statistics); probe comp and setup B smoke for M3.

### M5: typed channels replace `Shared`

Design: [Threads and messages](README.md#threads-and-messages), adr/0007. This supersedes "split `Shared` only if
races show" in [the change-detection design](drawing.md#one-change-detection-point): the overhaul splits it as part of the structure.

- `AcqIntent` (board-neutral, from the change point) and `Board::want`, which maps it to the board's `AcqWant` with
  `gen` and `epoch` from M2; `AcqCmd` with ids and `Readings` replace the remaining `Shared` fields,
  one group per part (M5.1 requests with ids: levels, slots, final; M5.2 the unit readings). Follows
  `acquisition.md`, Migration 2 and 3. `MeasRead` as its own type comes with M6.
- Atomics for metrics, one writer each, on the statics allowlist.
- The `Frame` and `MeasRead` latest-value cells and the segment channel; the rest of the memory model is M5b.
- Hidden global state goes in this step:
  - `Config` read once at start and passed in; no policy function reads a `OnceLock` or the environment;
  - the compression `k` moves into `Readings`;
  - `Session` (today the UI) creates and owns the deposit pipe;
  - the `pacing` state other than metrics, and `deepmeas::FORCE`, move into the stages that own them.
  The statics rule of `cargo xtask check` is enabled at the end of M5.
- `board::fake` drives the thread's client side; `board/contract.rs` holds the `Board` contract tests.

Tests, failing before:
- two explicit requests between two loop passes are both served (today a flag can be overwritten);
- an epoch-only change drops the batch armed ahead and the queued frames of the older epoch;
- two `Config` values in one test process give two behaviours (no global read);
- a rearm during configure is not lost;
- a command bound to an older pair is acknowledged as `Superseded`, not executed; two SINGLE presses within one loop
  pass give two acknowledgements; an unbound command (`CalCancel`, `AvgClear`) is served across a `gen` change;
- STOP (`hold` in `AcqWant`) and a `gen` change in one loop pass end held;
- the contract tests against the fake.

Deletes: `board::HwShared`, `Shared`, its lock.

Data path in M5 ([One ownership path per batch](README.md#one-ownership-path-per-batch)): the batch storage
becomes `Arc<SampleStore>`, immutable after the read; the newest frame is a view into it; no sample data is cloned
under a lock; holders that keep data past a step copy their range once, off main, and only in-flight work pins the
store. Tests, failing before: publishing the newest frame of a 4 x 10 M batch copies no sample data (counting
allocator); a stalled remote client holding a waveform does not reduce the batch pool. Adds 1 day.

### M5b: memory model

[Memory and queues](README.md#memory-and-queues). After M5, because the bounds sit on M5's channels.
- `memplan(&Settings, &Caps)` and its sweep test; the depth clamp only if the sweep shows it is needed.
- Byte bounds and overload policies on every channel; the degradation order; drop counters in `:SYSTem:STATe?`.
- The roll and scan ring as immutable chunks; a sweep as min/max plus chunk `Arc`s; nothing after a complete SINGLE
  sweep until `Rearm`.
- The RSS watchdog.
- Tests, failing before: the `memplan` sweep; a SINGLE roll sweep publishes once; a reader's pin holds chunks without
  a copy; a full pipe drops and counts instead of blocking or moving deposit to main; the M3.0 roll soak case passes.
- Estimate: 2 days (moved out of M5).

### M5c: DMA completion

[DMA completion](README.md#dma-completion). A measurement step with a unit round, no new code beyond switches.
- `oos-irq-place` puts the xdma IRQ on an A53; under kexec the completion poll threads are pinned to an A53.
- `:SYSTem:STATe?` shows the mode and CPU (`xdma=`).
- Round 2 measures, on cold boot and after kexec: MSI, legacy INTx (MSI off, as stock) and poll mode, each for CPU per
  read, read latency and errors. The result sets the default and goes into `system.md`, Platform tuning.
- Estimate: 1 day plus the round.

### M6: measurement reads never draw (D2)

As D2. Tests, failing before: a raw measurement read leaves the picture unchanged. Unchanged: mask counts, zones,
average, Bus trigger, Single, segments off the fast path. Deletes: the `compress::hold` draw branch, the `deepmeas`
draw cases, `RECORDS_BUDGET`.

Data path in M6 ([Measurements](README.md#measurements)): a `MeasRead` decodes only the channels and window it needs
into the measure thread's reusable buffer; the shared statistics come from one pass; only requested products are
computed; the measure thread and its helpers are persistent; the cheap set runs on codes. Tests, failing before: a
measure job spawns no thread and allocates nothing after warm-up; the cheap set on codes equals the set on volts.
Adds 1.5 days.

### M7: one lines kernel (D3)

As D3, behind `draw.model = old|new` in `system.conf`. Tests, failing before: `pair_raw_mix_report` and
`depth_mix_report` at x1.000; record-path pictures alone within 3 %. Golden references for lines change in this step,
each with the reason. Deletes: the deep-raw branch in `draw_frame`; segment joining for lines after M12.

Data path in M7 ([Codes until volts](README.md#codes-until-volts)): the kernel deposits from codes for every channel
count, with a fused four-channel decode-and-deposit kernel; the per-frame folds reuse the deposit's trigger position and
extrema. A unit benchmark decides whether the four-channel code path replaces the `f32` one. Adds 1.5 days.

**Unit round 2** (M5 to M7, with M5c): screenshot pairs old and new per mode for the user's eye; the acquisition under change
storm for M5.

### M8: dots (D4)

As D4. Tests, failing before: dots brightness within 3 % across raw and record frames; no dot lost at 100 samples per
column over 40 rows.

### M9: `BatchUnit` (D5)

As D5, adr/0006. Tests, failing before: every merged picture holds whole batches; the newest frame is in the same
merge; frames and batch mode give the same picture; one step's frames in frames mode form one unit. Frames mode
becomes the same `BatchUnit`, deposited by the UI thread, one unit per step (adr/0013). The accumulator lands in its
final module, `picture/acc.rs`, with its target signature (`merge(&mut self, units, pseudo, replace: Option<Replace>, &ChangeSet, now)`,
output `&Picture`), so M10 and M16 do not move it again. Gate: the perf-guard ratios of adr/0013 (one-frame unit and
48-frame step against `draw_frame`). Deletes: the `Acc` lists, `Tally` (if fix/batch-newest was merged), `Deliver`, `MERGE_WAIT_MAX`, the
early `help_done`, the `fast_ok` frame branch in `Ui::step`, route D's `draw_frame` composition.

Data path in M9 ([Maps: workers pre-sum](README.md#maps-workers-pre-sum)): at 3 and 4 channels each worker owns one
channel; at 1 and 2 channels the workers reduce the shares by row bands; each share records its touched rows; main
adds at most 3 map sets per step over touched rows; no arithmetic under a lock. Tests, failing before: main's adds per
step are at most 3 map sets in every channel count; at 4 channels no reduction runs; the reduction allocates nothing
and no thread blocks on it. Adds 1.5 days.

### M9b: GPU work proportional to change

[GPU work proportional to change](README.md#gpu-work-proportional-to-change). After M9, because the dirty rows come from
the accumulator.
- The helper uploads dirty row ranges only; the arena row compare goes.
- A pool of 3 fenced buffers per output; `glFinish` goes.
- Snapshots on demand into a pool of 2 readback buffers, handed to the encoder by ownership; the snapshot every 15
  frames goes.
- HDMI with its own budget and the half-rate rule.
- Tests, failing before: `gpu/test-host.sh` with 0 differences; an unchanged picture uploads nothing; a snapshot does
  not delay a composition.
- Unit check in round 3: helper CPU and GPU time per composition, panel alone and with HDMI; the helper's core (first
  A72 or an A53).
- Estimate: 3 days.

**Unit round 3:** stress set, `partial` 0.

### M10: grading and records together (D6)

As D6, adr/0005, inside `picture/acc.rs` from M9 and `oos_core::window`. Behind `draw.model`. Tests, failing before: as in D6 (brightness within 3 % for 1 to 27 frames per
picture, window age <= 67 ms, single hit visible, stopped picture constant, replay equals live, zoom equals main,
`gpu/test-host.sh` 0 differ). Deletes: `picture_ref`, `per_frame*`, `pic_frames`, the `vmax` follow; the 24 x 6
redraw after M12.

Data path in M10 ([Fused accumulation](README.md#fused-accumulation)): at Min one pass over touched rows rebuilds the
window and the column maxima; above Min one fused full pass decays, adds and updates the maxima; a frozen picture does
no work. Tests, failing before: at Min a step with one touched row band sweeps only that band; above Min the pass
count per step is one; replay equals live. Adds 1 day.

**Unit round 4:** full acceptance set ([drawing acceptance criteria](drawing.md#acceptance-criteria-unitlab)), screenshot pairs per mode.

### M11: sparse-event pacing (D8)

As D8. Tests, failing before: the fill plan with a rest of 24 ms yields a batch every <= 33.3 ms in the pacing tests.
Unit check inside round 5: setup B at 5.1 V, empty pictures <= 1 %.

### M12: acceptance and removal (D9)

Remove `draw.model` and the old kernels and grade. Tests: full suites, golden references from M10 unchanged.

**Unit round 5:** full acceptance set, plus memory: roll at 0.5 s/div and 50 s/div in RUN and after SINGLE for
10 min each, scan, and 4 channels at maximum depth, each with RSS flat within 10 MB and all drop counters as expected;
then the restart-loop case of adr/0015; then the postponed stress round (matrix, feature pass, change storm, interfaces
under load, soak, restart and power cycle) on this build.

**Stable stop point.** After M12 the branch is shippable: the drawing goals are met, every step is through the gate
and unit-tested, and the structure has its domain crate, change point, typed channels and accumulator. The user can
take a build from here while M13 to M18 continue, or stop the overhaul here. M13 to M18 are estimated at this point.

### M13: settings groups replace `Model`

Design: [Settings and commands](README.md#settings-and-commands), adr/0001.

- Move `Model`'s fields into the groups in `change/settings/`, private, with accessors and invariants.
- `classify` from M2 splits into per-group parts.
- Parts: one group per sub-step (M13.1 channels, M13.2 timebase, ...), each through the gate.

Tests: invariants per group, failing before where an invariant is new (for example a range `Model` did not check);
setup round trip on every setup file in the tests; golden pictures unchanged. Deletes: `Model` and `model.rs`'s
remainder.

### M14: commands per area

adr/0002. Per-area command enums; menus and input return commands, knobs relative ones; setup files load as command
lists; the merge order store, remote, input.

Tests, failing before: for every settable key, the SCPI write and the menu action give the same `Settings` and the same
`ChangeSet`; an out-of-range value returns the same `CmdError` on every edge; a knob turn and a remote write to the
same field in one step both take effect. Deletes: the string-keyed `store::set`
dispatch and every direct write to settings outside `change`. Boundary rules for `settings` and `change` switch to
enforce.

### M15: remote adapters on commands and queries

Transports generic over `Read + Write`, tested with in-memory streams; `RemoteRequest` to main; SCPI, HTTP, MCP, VXI-11 and the control FIFO parse
into `Command` and `Query`; answers come from the `reply` stage.

Tests, failing before: a query answered from one step's state (no mixed values across a change); `:SINGLE;
:WAV:DATA?` waits for a record of the SINGLE's epoch (`After { gen, epoch }`); a slow client does not block the step. Deletes: listener-thread access to UI state other than the request channel and the picture
snapshot.

### M16: stages and `Session` replace `Ui::step`

adr/0004, adr/0008. `Session` wires intake, change, acquire, records, accumulate, measure, views, compose, present, reply.
Ports with fakes: `display::Present`, `input::Source`, `clock::Clock`; `store` on paths and `Read`/`Write`.

Tests: `Session` end to end on the fakes; a change storm replayed from a command log on the host; golden pictures
unchanged. Deletes: `Ui::step`.

### M17: views and menus as components

Each view (spectrum, eye, Bode, power, LA, XY, bus) and the menus own their state. Parts: one view per sub-step.
Tests: per view, its inputs and its picture. Deletes: the `Ui` struct.

### M18: target layout

Move the remaining modules into the layout of [Module map](README.md#module-map): `system/`, `store/`,
`display/`, `remote/`, `instruments/`. Every boundary rule enforces. The allowlist holds no entry for `ui.rs`, `hw.rs`
or `model.rs`, since they no longer exist.

**Unit round 6:** the full stress round again on the final overhaul build, against the M12 numbers.

## Estimate

M0 to M12: about 49 days of host work plus six unit rounds (0 to 5) of about half a day: the drawing plan's 22 days
with D1a (3 days) replaced by M2.1 to M2.3 (7.5 days), M1 and M5 at about 1.5 days each, M1b at 2 days, M3.0 at 1.5
days, M5b (memory model) at 2 days, and the performance work of adr/0016 at 14 days (M3c 1.5, M3d 2, M5c 1, M9b 3,
the data path in M5, M6, M7, M9 and M10 6.5). G1 and G2 add 3.5 days of gate work by auxout, in parallel. M13 to
M18: estimated at the stable stop point after M12, when the remaining sizes are known.

## Status

- 2026-11-14: plan written (perfui, with compress). Waiting for review (archreview, upstreamarch) and team-lead
  approval. M0 done (73236d4). adr/0013 recorded (team-lead: one batch path).
