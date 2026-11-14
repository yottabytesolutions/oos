# Overhaul migration

How `oos-ui` gets from the code on `overhaul` (056c020) to the target in `architecture.md`. Strangler: the old code
keeps running until the new path replaces it, one small step per merge. This file absorbs the implementation plan of
`fork/draw-architecture.md` (section 5) and its change-detection amendment; where they differ, this file applies.

## Rules for every step

- **One branch, one step.** A branch implements one step. Larger steps split into numbered parts (M2.1, M2.2).
- **Failing-before tests.** The step's tests fail on the parent commit and pass after. A refactor step names the
  existing tests and golden pictures that must stay unchanged instead.
- **Deletions.** The step deletes what it lists. The reviewer checks with grep that the names are gone.
- **Gate.** `tools/check/gate.sh` passes: fmt, clippy, ui and hal tests in release and debug with full counts, arm64
  check, size limits, boundary rules, perf-guard, `gpu/test-host.sh`.
- **Ratchet.** The size allowlist shrinks or stays. A module that reaches its target layout switches its boundary rules
  from report to enforce (`arch-rules.toml`).
- **Docs.** The step updates the "code still differs" notes and, if a decision changed, adds an ADR.
- **Unit rounds.** Only where marked. Install rule: `oos-update install <pkg> --no-kexec --execute`, then reboot.
  Unit rules as in the project: no hand FPGA register writes while oos-ui runs, no reads of 0x205c, never FPGA flash,
  p4, p5 or U-Boot, cal files read-only, wait for OOSK re-arm, never two reboots within 60 s.

## Prerequisite: phase 1 guardrails

Phase 1 of `overhaul.md` lands first: `rustfmt.toml` and the format commits, the clippy set (adr/0009), the gate
script, the size checker with its allowlist (adr/0010), the boundary checker reading `arch-rules.toml` in report mode,
the pre-commit hook. Migration steps start when the gate script exists.

## Steps

D-numbers are the steps of `fork/draw-architecture.md` section 5. Unit marks the steps followed by a unit round.

| # | Step | D | Unit |
|---|---|---|---|
| M0 | Picture counters: stale, partial, age, composition | 0 | |
| M1 | Golden pictures and the host harness | | |
| M2 | One change-detection point | 1a | |
| M3 | Acquisition thread split | 1b | |
| M4 | Mapping key, preview, generation for records | 1c | round 1 |
| M5 | Typed channels replace `Shared` | | |
| M6 | Measurement reads never draw | 2 | |
| M7 | One lines kernel, behind `draw.model` | 3 | round 2 |
| M8 | Dots: one count per sample | 4 | |
| M9 | `BatchUnit` | 5 | round 3 |
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
(`fork/draw-architecture.md`, Status). M4's mapping-key counter replaces it as the measure of stale.

### M1: golden pictures and the host harness

- Turn the scripted screen tests (`ui/tests/*.txt`: keys, frames, shot) into golden comparisons on the Demo source,
  with references made from `overhaul` before M2 (adr/0011).
- Add the display modes the drawing steps change: lines, dots, persistence Min and 500 ms, zoom, colour grade, at a
  fast and a slow time base.
- Tests: new, pass on creation. They pin today's pictures so that M2 to M6 can prove "no change".
- Deletes: nothing.

### M2: one change-detection point (D1a)

Design: [Change detection](architecture.md#change-detection), adr/0003. It includes compress's four review changes:
1. Requests next to the diff (`change::request`).
2. `Generations::new_gen` and `new_epoch` as the only writers of `gen` and `epoch` (`epoch` replaces `hw_stamp`), returning the
   value at once.
3. `UiAcq` for UI state that changes acquisition or drawing: XY, the protocol watch, the mask reference, autoset or
   Bode active.
4. Run state by transition.

Classes: `acq`, `mapping`, `phosphor`, `stats`, `mask`, `zone`, `hw_average`, `history`, `math`. Measurement state
(the analysis results, statistics and `meas_n`) is in `stats`. No `decode` class: the decode cache is keyed.

`classify` destructures today's flat `Model` without `..`, with the fields grouped in the order of the future settings
groups so that M13 can split it without reclassifying.

Tests, failing before:
- every class: changing a representative field resets exactly its consumers;
- SCPI and HTTP writes (`store::set`) reach the same point as a knob;
- a mapping change in STOP redraws from `last[0]` and never blanks;
- an autoset or Bode stage that stores the stamp in the same call sees the new generation;
- CLEAR, the RUN and SINGLE edges and Create mask reset their consumers with no settings difference.

Deletes: `settings_changed`, `mark_changed`, the 16 `dirty_map = true`, the `hw_stamp` bumps in `ui.rs`, `ui/bode.rs`
and `ui/autoset.rs`. Risk: medium. Every clear and reset moves to one point.

### M3: acquisition thread split (D1b)

Split the closure in `hw.rs` `spawn` (lines 1263-2154 on 32041d0) as in `architecture/acquisition.md`:
- an `AcqLoop` struct for the about 30 loop locals (1328-1393);
- set-up (1263-1394);
- apply settings (1476-1548);
- service: self-cal, generator, DVM, levels, trim, slots;
- stream: roll and scan, scan as an option (1610-1786);
- segments;
- batch, including the STOP hold (1797-2154).

Pure refactor. Tests: the existing hw tests and all golden pictures unchanged. Deletes: nothing, moved code. The
allowlist entry for `hw.rs` and its `spawn` shrinks. Unit: smoke check (probe comp, setup B) inside round 1.

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

**Unit round 1** (M2 to M4): knob spin, settle over 20 changes; clears and resets as before (screenshots, measurement
statistics); probe comp and setup B smoke for M3.

### M5: typed channels replace `Shared`

Design: [Threads and messages](architecture.md#threads-and-messages), adr/0007. This supersedes "split `Shared` only if
races show" in `fork/draw-architecture.md` 3.9: the overhaul splits it as part of the structure.

- `AcqWant` with `gen` and `epoch` from M2; `AcqCmd` with ids and `Readings` replace the remaining `Shared` fields,
  one group per part (M5.1 requests with ids: levels, slots, final; M5.2 the unit readings). Follows
  `architecture/acquisition.md`, Migration 2 and 3. `MeasRead` as its own type comes with M6.
- Atomics for metrics, one writer each.
- `board::fake` drives the thread's client side; `board/contract.rs` holds the `Board` contract tests.

Tests, failing before:
- two explicit requests between two loop passes are both served (today a flag can be overwritten);
- a rearm during configure is not lost;
- the contract tests against the fake.

Deletes: `board::HwShared`, `Shared`, its lock.

### M6: measurement reads never draw (D2)

As D2. Tests, failing before: a raw measurement read leaves the picture unchanged. Unchanged: mask counts, zones,
average, Bus trigger, Single, segments off the fast path. Deletes: the `compress::hold` draw branch, the `deepmeas`
draw cases, `RECORDS_BUDGET`.

### M7: one lines kernel (D3)

As D3, behind `draw.model = old|new` in `system.conf`. Tests, failing before: `pair_raw_mix_report` and
`depth_mix_report` at x1.000; record-path pictures alone within 3 %. Golden references for lines change in this step,
each with the reason. Deletes: the deep-raw branch in `draw_frame`; segment joining for lines after M12.

**Unit round 2** (M5 to M7): screenshot pairs old and new per mode for the user's eye; the acquisition under change
storm for M5.

### M8: dots (D4)

As D4. Tests, failing before: dots brightness within 3 % across raw and record frames; no dot lost at 100 samples per
column over 40 rows.

### M9: `BatchUnit` (D5)

As D5, adr/0006. Tests, failing before: every merged picture holds whole batches; the newest frame is in the same
merge; frames and batch mode give the same picture. Frames mode becomes the same `BatchUnit`, deposited by the UI thread; a single delivered frame is a batch of
one (adr/0013). Deletes: the `Acc` lists, `Tally` (if fix/batch-newest was merged), `Deliver`, `MERGE_WAIT_MAX`, the
early `help_done`, the `fast_ok` frame branch in `Ui::step`, route D's `draw_frame` composition.

**Unit round 3:** stress set, `partial` 0.

### M10: grading and records together (D6)

As D6, adr/0005. Behind `draw.model`. Tests, failing before: as in D6 (brightness within 3 % for 1 to 27 frames per
picture, window age <= 67 ms, single hit visible, stopped picture constant, replay equals live, zoom equals main,
`gpu/test-host.sh` 0 differ). Deletes: `picture_ref`, `per_frame*`, `pic_frames`, the `vmax` follow; the 24 x 6
redraw after M12.

**Unit round 4:** full acceptance set (`fork/draw-architecture.md` section 4), screenshot pairs per mode.

### M11: sparse-event pacing (D8)

As D8. Tests, failing before: the fill plan with a rest of 24 ms yields a batch every <= 33.3 ms in the pacing tests.
Unit check inside round 5: setup B at 5.1 V, empty pictures <= 1 %.

### M12: acceptance and removal (D9)

Remove `draw.model` and the old kernels and grade. Tests: full suites, golden references from M10 unchanged.

**Unit round 5:** full acceptance set; then the postponed stress round (matrix, feature pass, change storm, interfaces
under load, soak, restart and power cycle) on this build.

### M13: settings groups replace `Model`

Design: [Settings and commands](architecture.md#settings-and-commands), adr/0001.

- Move `Model`'s fields into the groups in `settings/`, private, with accessors and invariants.
- `classify` from M2 splits into per-group parts.
- Parts: one group per sub-step (M13.1 channels, M13.2 timebase, ...), each through the gate.

Tests: invariants per group, failing before where an invariant is new (for example a range `Model` did not check);
setup round trip on every setup file in the tests; golden pictures unchanged. Deletes: `Model` and `model.rs`'s
remainder.

### M14: commands per area

adr/0002. Per-area command enums; menus and input return commands; setup files load as command lists.

Tests, failing before: for every settable key, the SCPI write and the menu action give the same `Settings` and the same
`ChangeSet`; an out-of-range value returns the same `CmdError` on every edge. Deletes: the string-keyed `store::set`
dispatch and every direct write to settings outside `change`. Boundary rules for `settings` and `change` switch to
enforce.

### M15: remote adapters on commands and queries

`remote::Transport` with an in-memory fake; `RemoteRequest` to main; SCPI, HTTP, MCP, VXI-11 and the control FIFO parse
into `Command` and `Query`; answers come from the `reply` stage.

Tests, failing before: a query answered from one step's state (no mixed values across a change); a slow client does
not block the step. Deletes: listener-thread access to UI state other than the request channel and the picture
snapshot.

### M16: stages and `Session` replace `Ui::step`

adr/0004, adr/0008. `Session` wires intake, change, acquire, accumulate, measure, views, compose, present, reply.
Ports with fakes: `display::Present`, `input::Source`, `clock::Clock`, `store::Files`.

Tests: `Session` end to end on the fakes; a change storm replayed from a command log on the host; golden pictures
unchanged. Deletes: `Ui::step`.

### M17: views and menus as components

Each view (spectrum, eye, Bode, power, LA, XY, bus) and the menus own their state. Parts: one view per sub-step.
Tests: per view, its inputs and its picture. Deletes: the `Ui` struct.

### M18: target layout

Move the remaining modules into the layout of [Module map](architecture.md#module-map): `system/`, `store/`,
`display/`, `remote/`, `instruments/`. Every boundary rule enforces. The allowlist holds no entry for `ui.rs`, `hw.rs`
or `model.rs`, since they no longer exist.

**Unit round 6:** the full stress round again on the final overhaul build, against the M12 numbers.

## Estimate

M0 to M12: about 22 days of host work plus five unit rounds of about half a day (from the drawing plan, plus M1 and
M5 at about 1.5 days each). M13 to M18: not estimated yet. It depends on M1's harness and is estimated after M12,
when the remaining sizes are known.

## Status

- 2026-11-14: plan written (perfui, with compress). Waiting for review (archreview, upstreamarch) and team-lead
  approval. M0 done (73236d4). adr/0013 recorded (team-lead: one batch path).
