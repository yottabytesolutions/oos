# UI migration

Source audit: 2026-11-17. [Current structure and target](README.md) are separate.
This plan replaces the old M0-only status and calendar estimate. Code present is not unit acceptance.
Tasks retain their ids and evidence; this document does not close them or claim new paths.

## Rules for every step

Keep behaviour unless an approved specification says otherwise. Preserve acquisition identity, SINGLE/STOP,
setup compatibility, SCPI sequencing and golden pictures. No hardware run is implied by a host result.
Each package is one reviewable topic; repeat family/group packages rather than one large cutover.
A package needs tests in debug and release, the gate and independent review before completion.
Fixes add failing-before tests; moves preserve named characterisation tests and goldens.
Each package removes its replaced production path; unfinished paths remain explicit.
Run `cargo xtask ratchet` for improvements, `--move` for planned moves (ADR 0006).
Use declarative tables with row-complete tests (ADR 0029), not parallel handwritten inventories.
Source provenance and contribution rules are ADR 0003.

## Steps

M0..M11 prerequisites remain below so existing task links retain their meaning.
After drawing acceptance, M13/M14 precede the M15 parser and M17 write-descriptor cutovers;
M17 read descriptors need only E1 and the refreshed inventory
([ADR 0038](../adr/0038-menus-and-parser.md)).
The ADR 0023 IO experiment addresses transport blocking separately; it keeps the existing interpreter.
No package author may infer that unfinished drawing work has passed M12.

### G1: gate prerequisites

Implemented: `cargo xtask check` and the fast/full split. [Task 0043](../tasks/done/0043-gate-g1.md) is done.
The current gate also checks links, text, test time, layers and ratchets.

### G2: performance gates

Implemented primitives: ratio/allocation guards and timing diagnostics. [Task 0051](../tasks/0051-g2-performance-gates.md)
is still active. The [host budget rows](README.md#host-budget-rows) do not certify future complete stages.

### M0: picture counters (done)

`pacing.rs` and drawpipe counters report stale/partial/composition observations.
Their presence does not prove whole-batch accumulation.

### M1: golden pictures and the host harness

Implemented; [task 0002](../tasks/done/0002-m01-golden-pictures.md) is done.
Golden comparisons follow [ADR 0008](../adr/0008-behaviour-baselines.md).
`ui/tests/golden.rs`, `host_script.rs` and `soak.rs` are the host regression harness.

### M1b: safe start and exit

Implemented; [task 0048](../tasks/done/0048-m1b-safe-start.md) is done.
Startup quarantine, durable markers, watchdog and ordered shutdown remain contracts for later moves.

### M2: one change-detection point (D1a)

Core extraction, live normalization, identity publication, deferred internal proposals and class-owned resets
are implemented. SCPI queries/program ends now settle changes. Production legacy reset comparison is retired.
[Task 0003](../tasks/0003-m02-change-detection.md) remains active, awaiting review/unit smoke in its latest status.
ADR 0029's field table is implemented. Grouped settings and universal command-only writes are not.

### M3: acquisition thread split (D1b)

Readout/publication/segment/stream helpers exist; `hw::spawn` still has 860 code lines.
**M3.0, loop harness.** Scope: a cancellable Device port over configure/arm/wait/read/reread/disarm and a
scripted device, before extraction. Files: `ui/src/board/dho/hw.rs`, `board/dho/` phase helpers and tests.
Acceptance: characterise STOP, SINGLE, mid-fill changes, roll/scan, segments and chunk-boundary cancellation;
10,000 scripted SINGLE-roll passes account for published bytes. Record current ended-fill reads for M4 to change.
Delta: +100..+200 for the port, measured against removed test scaffolding. Parallel: host fixtures only;
reserve `hw.rs` for wiring. Depends: M2.

**M3.1, one phase per move.** Scope: AcqLoop with phase-owned state, setup, apply, service, stream, segments,
then batch, following [acquisition](acquisition.md#thread-structure). Files: `board/dho/hw.rs`, phase files.
Acceptance: M3.0 traces, existing hardware host tests and goldens unchanged; no lock across IO.
Remove the moved closure bodies; `spawn` only wires the loop. Delta: near 0, `ratchet --move`.
Parallel: serial on loop state and `hw.rs`. Depends: M3.0. Unit: probe-comp/setup-B smoke in round 1.

### M3c: acquisition waiting

**Scope:** replace phase sleeps with command-eventfd/ppoll deadlines on M3's AcqLoop; bound helper work to
one deposit between checks, recompute deadlines after EINTR, cancel reads at chunk boundaries.
Files: `board/dho/` wait/apply/read paths, placement startup, scripted Device tests.
Acceptance: fake-clock tests cover every phase, 1,000 commands without deadline drift, STOP without hardware
polling, cancellation at the next chunk. Unit round 1 measures wake < 1 ms outside reads, chunk p99 <= 20 ms
per 4 MiB, nominal poll periods within 10%, timer slack 1 ns and configure <= 4 ms.
BAR-write gap changes require measured safe spacing; no guessed removal of delays.
Remove replaced sleeps/backoffs. Delta: -50..-150. Parallel: no other acquisition-loop edits.
Depends: M3.1 and G2. These timing bounds are perf/unit checks, not wall-clock unit assertions.

### M3d: main event loop

Partial: `system/device/` separates intake, graphics, remote and timers; the loop still polls transports.
**Scope:** vblank/page-flip events, timer fallback, paced full steps and light intake/change/acquire/reply steps;
carry ordered pending changes to the next full picture. Transport writes/wake delivery belong to M15a-0/a.
Files: `ui/src/system/device*`, `input.rs`, display/DRM/GPU event interfaces and pacing fixtures.
Acceptance: fake events preserve ordered transitions across four light steps, mapping preview at the next picture,
labels/trace consistency, per-output flips, failed flips and timer fallback after three missed periods.
A request flood obeys M15a's intake quota; rendering stays paced. Unit round 1 measures the README light-step,
frame, remote and STOP-idle budgets. Remove busy frame waits and replaced blocking flip waits.
Delta: -50..+100. Depends: M2, G2 and M15a-0/a for remote acceptance.
Parallel: display-event fixtures; serialize `device*` wiring with M15a-0/a.

### M4: mapping key, preview, generation for records (D1c)

MappingKey/WorkerViewKey and exact frame identities exist.
**Scope:** keep the batch's first view; reject another mapping, preview `last[0]`, interpret/refine only from
BatchMeta (including calibration, invert, coupling and bandwidth); drop early-ended fills before reading.
Files: `core/src/change/`, `ui/src/board/dho/`, `drawpipe*`, `ui/change*`, record intake in `ui.rs`.
Acceptance: mapping edits never blank, no batch uses two mappings, obsolete records never enter measurements;
STOP preview and mid-fill reconfiguration tests fail before the missing contracts are fixed.
Remove remaining view `hw_stamp` conventions and blank-on-change paths. Delta: -50..-150.
Depends: M3. Parallel: serial on publication/mapping seams. Unit round 1: change storm, resets, setup-B smoke.

### M5: typed channels replace `Shared`

`HwShared` remains `Arc<Mutex<Shared>>`; `Board::want` takes `&Model`.
**M5a, owned acquisition request.** Scope: replace borrowing core AcqIntent with owned, board-neutral
AcqRequest, including coherent `(gen, epoch)`, run/route and every acquisition want; Board::want maps only that
request. Files: `core/src/change/values.rs`, its publication callers, `ui/src/board/mod.rs`, `board/dho/` mapping,
`ui/change*` and board fake/contract tests. Acceptance: every consumed field maps identically to today's want;
a request remains valid after the source Model changes or drops; epoch-only/route-only changes preserve identity,
STOP plus reconfiguration stays held, and no Board API accepts `&Model`. Delete borrowing AcqIntent and the
Model-to-board bridge. Delta: near 0. Depends: M4. Parallel: serialize board/publication edits; prerequisite to M13.

**M5.1, one channel group per package.** Scope: latest AcqWant, id-bearing AcqCmd (levels/slots/final first),
then Readings and frame/segment outputs, following [acquisition](acquisition.md#port-boardacq).
Files: `ui/src/board/`, client publication/intake and board fake/contract tests.
Acceptance: two requests between passes both acknowledged; old-pair commands report Superseded, future-pair
commands wait, two SINGLEs each resolve, unbound cancel/average-clear survive reconfiguration, rearm is not lost.
Epoch changes discard armed-ahead/queued obsolete data. Enforce README count/byte bounds and overload outcomes.
Delete each replaced Shared field/counter; delete Shared/HwShared and its lock after the last consumer.
Delta: -50..-150 per group. Depends: M5a. Parallel: serial on shared port types and acquisition loop.

**M5.2, ownership and configuration.** Scope: immutable Arc SampleStore, newest frame as a view; long-lived
holders copy their range off main; Config read once and passed by value/reference. Move compression k into
Readings, deposit-pipe ownership into the application, nonmetric pacing/deep-read globals into their owners.
Files: `board/`, `wave.rs`, `drawpipe*`, `pacing.rs`, `deepmeas.rs`, startup wiring and core Config.
Acceptance: newest-frame publication copies no samples (4 x 10 M allocation probe); a stalled waveform client
pins no batch buffer; two Config values in one process give independent behaviour; statics boundary passes.
Remove replaced global reads/state. Delta: -50..-150. Depends: M5.1. Parallel: serial on buffer/publication paths.
Unit round 2 covers acquisition under a change storm; M6 adds the separate MeasRead type.

### M5b: memory model

Queue/snapshot/outbox bounds and RSS watchdog exist; the complete plan remains open.
**Scope:** pure MemPlan sweep, buffer allocation from the plan, byte bounds/overload counters, immutable
roll/scan chunks and retaining-owner census, including [remote replies](README.md#memory-plan).
Files: `core/src/` memory plan/normalize, `ui/src/board/`, `wave.rs`, `drawpipe*`, `system/memory.rs`, SCPI metrics.
Acceptance: all Caps/modes/channels/depths/timebases fit or produce the specified clamp/rejection; reconfiguration
counts old buffers; SINGLE roll publishes once until Rearm; pin/release, full-pipe drops and M3.0 roll soak pass.
Remote budget arithmetic includes queued/parked/encoded replies and upload/snapshot retention below soft RSS.
Remove owner-local allocation estimates and replaced mutable ring copies. Delta: -50..+100.
Depends: M5. Parallel: pure sweep fixtures; serialize allocation/queue wiring. Unit memory soak at M12.

### M5c: DMA completion

**Scope:** measured DMA completion/IRQ policy only; retain kexec poll mode until a working alternative is proved.
Files: xdma/platform adapters, rcS IRQ placement, `system.md` evidence and SCPI mode/CPU counters.
Acceptance: authorized round 2 compares cold-boot MSI and kexec poll mode for CPU/read latency/timeouts over
10 min; record mode/CPU and README DMA budget. Keep failed alternatives out of defaults; persistent DMA mapping
is later measured driver work. Remove only superseded experimental paths. Delta: near 0.
Depends: G2 and platform evidence in [system](system.md). Parallel: no concurrent DMA/platform experiments.

### P1: platform settings

Placement/IRQ/clock adapters exist; target CPU/wake budgets are unproved.
**Scope:** only outstanding [platform tuning](system.md#platform-tuning) A/B rows: uclamp, log level,
preemption and explicit IRQ placement. Files: placement/HAL system adapters, rcS, kernel config/boot arguments.
Acceptance: host call fixtures; authorized round 2 records each row's before/after and keeps only passing settings.
DMC stays disabled. Remove rejected experiment switches. Delta: near 0.
Depends: G2; coordinate with M5c. Parallel: serial on platform configuration and the unit.

### M6: measurement reads never draw (D2)

Live measurement/analyser jobs and stale-result filtering exist; deep/raw reads still meet drawing in Ui::step.
**Scope:** separate MeasRead, worker-owned reusable decode buffers, requested channels/products only, one shared
statistics pass, cheap measures on codes; record building off main. Files: `board/`, `deepmeas.rs`, `framestats.rs`,
`ui/lifecycle*`, record intake in `ui.rs`. Acceptance: a raw measurement read leaves pictures unchanged; preserve
mask/zone/average/bus/SINGLE/segment behaviour; warmed jobs allocate/spawn nothing and codes equal volts results.
Remove `compress::hold` drawing, deep-read drawing cases and RECORDS_BUDGET as their consumers switch.
Delta: -100..-300. Depends: M5. Parallel: pure measure fixtures; serialize record/publication changes.

### M7: one lines kernel (D3)

Spans/codes/pairs/sparse kernels exist; record and frame routes still differ.
**Scope:** common weights for every source, code deposits with one trigger/extrema table per frame, reused by
folds; per-channel workers at 3/4 channels. Files: `fastdraw.rs`, `drawpipe*`, `wave.rs`, target `picture/deposit/`.
Acceptance: pair/raw and depth mixes at x1.000, record brightness within 3%, lines goldens with reasons,
4-channel perf guard counts refinement once per frame; unit round 2 benchmarks codes against f32.
Remove replaced deep-raw draw_frame branch; defer route/segment-joining removal to M12's choice.
Delta: -100..-300. Depends: M6. Parallel: serial on kernels/weights; no new draw.model switch is implied.

### M8: dots (D4)

**Scope:** one count per sample across raw/codes/pairs/records, with common source weights.
Files: dots kernels in `fastdraw.rs`/`picture/deposit/`, `drawpipe*`, golden/perf fixtures.
Acceptance: brightness within 3% across sources; no sample lost at 100 samples/column over 40 rows;
existing dots guard/goldens pass. Remove replaced source-specific counting paths. Delta: -20..-100.
Depends: M7 weights. Parallel: serial on deposit dispatch; standalone fixtures can prepare early.

### M9: `BatchUnit` (D5)

Workers deposit shares; whole-unit publication/window are open.
**Scope:** publish complete BatchUnits; accumulator in final `ui/src/picture/acc.rs`, with private maps and
read-only Picture. At 1/2 channels reduce shares by touched row bands; at 3/4 channels workers own channels;
main adds at most three map sets. Files: `drawpipe*`, `ui.rs`, `picture/`, core window types.
Acceptance: every merge contains whole batches including newest frame; route-equivalent content, one-frame/
48-frame perf ratios, no reduction at 4 channels, no allocations/blocking in reduction. Keep Standard alongside
Batch for M12's owner decision under [migration plan](migration.md#m12).
Remove replaced Acc lists, Tally if present, Deliver, MERGE_WAIT_MAX, early help_done and duplicate newest-frame
composition only where replaced; do not delete Standard here.
Delta: -100..-300. Depends: M7/M8. Parallel: serial on map ownership/publication. Unit round 3: partial=0 stress.

### M9b: GPU work proportional to change

GPU helper/layers and snapshot demand exist; dirty-row/fence acceptance remains open.
**Scope:** dirty-range uploads, three fenced buffers per output, two owned snapshot buffers on demand;
keep glFinish only for readback and preserve HDMI half-rate. Files: `gpu/`, `ui/src/gpu.rs`, display/DRM/snapshot paths.
Acceptance: gpu/test-host.sh has zero differences, unchanged pictures upload nothing, held snapshot cannot block
composition; unit round 3 measures CPU/GPU time and panel/HDMI budgets. Remove replaced row compare,
periodic snapshots and composition glFinish. Delta: -50..-150.
Depends: M9 dirty rows. Parallel: GPU internals after dirty-range API; serialize snapshot/display wiring.

### M10: grading and records together (D6)

**Scope:** weight-based grade, record pseudo-batches, bounded window in M9's accumulator; at Min rebuild
entering/leaving rows, above Min fuse decay/add into one pass. Files: `picture/acc.rs`, core window,
`ui.rs` grading, `drawpipe*` folds. Acceptance: brightness within 3% for 1..27 frames, <= 67 ms window,
single hit visible, stopped picture constant, replay=live, zoom=main, GPU zero differences; disjoint row bands
leave no ghost and both bands are dirty. Fix the measurement-selection regression retained in
[task 0011](../tasks/0011-m10-grading-records.md) before removing its M1 harness restriction.
Remove picture_ref, per_frame*, pic_frames and vmax-follow paths as replaced; final legacy redraw waits for M12.
Delta: -100..-300. Depends: M9/M9b. Parallel: serial on accumulator/grade. Unit round 4: full picture acceptance.

### M11: sparse-event pacing (D8)

**Scope:** deadline/rounding precision in Fill without losing startup/no-phase/impossible-rest behaviour.
Files: `ui/src/pacing.rs`, pacing tests; retain the reproduction in
[task 0012](../tasks/0012-m11-sparse-pacing.md).
Acceptance: synthetic-clock rest=24 ms gives <= 33.3 ms batch intervals, across start phases, bounded jitter
and ready-before-step deadlines; no tolerance added just to pass. Remove superseded rounding/pacing branches.
Delta: -20..+20. Depends: M10 and M3d pacing interface. Parallel: fixtures only alongside drawing edits;
serialize pacing state. Unit round 5: setup-B sparse events, empty pictures <= 1%.

## Remaining work packages

Order below is dependency order, not a new calendar estimate. Deltas estimate net production Rust lines
excluding tests, comments and dependency sources; they are planning ranges, not gate allowances.
Moves should be near zero. Each repeated slice must fit normal review size and preserve a green tree.
Risk is low for docs, medium for observable UI/protocol behaviour; acquisition timing also needs unit evidence.

<a id="m12"></a>

### M12: acceptance and removal (D9)

**M12a, evidence and owner selection.** Scope: compare current Standard/Frames and Batch routes after M2..M11
prerequisites pass. Files: `ui/tests/`, `tools/perf-guard.sh`, drawing task evidence, `docs/architecture/drawing.md`.
Acceptance: route-equivalent pictures/weights for raw, pairs, dots, zoom, deep reads, roll/scan, STOP/SINGLE and replay;
full gate; authorized unit load/soak with frame, latency, drop and memory evidence.
Under [migration plan](migration.md#m12), do not delete Standard before the cause of Batch's
drop to about 18 fps with four channels at 10 us/div is found.
Expected delta: 0. Risk: medium, with unit timing evidence. Parallel: evidence preparation alongside IO baselining;
final acceptance waits for the drawing prerequisites.

**Open owner question:** which of Standard/Frames and Batch survives after those measurements?
`Model::draw_batch`, `drawpipe::Mode` and the fast/record routes still exist. `draw.model` does not exist in this
checkout; there is no such switch to remove. M12 must keep one path and delete the loser under docs/architecture/migration.md#m12;
the surviving route remains pending measurement and owner selection.

**M12b, approved deletion.** Scope: delete the losing path and code made obsolete by the owner's selection.
Files: selected branches in `ui.rs`, `drawpipe*`, `fastdraw.rs`, `wave.rs`, `compress.rs`, `sysconf.rs` and core settings.
Acceptance: one surviving path, losing path and obsolete selectors removed, M12a pictures/perf unchanged for the
surviving behaviour, setup compatibility and full gate.
Expected delta: < 0, amount measured after the choice.
Risk: medium. Parallel: no overlapping draw-path edits. M12 is complete only after evidence, selection and deletion.

### E1: dispatch extraction before parallel families

**E1a, menu areas; E1b, SCPI subsystems.** Scope: enabling move-only slices, before any M14a, M15c or M17b
family work. Split `kind`, `label`, `value`, `options`, `adjust`, `activate` and row/visibility ownership into
`ui/src/ui/menu/<area>.rs`; split Scpi::exec_cmd branches into `ui/src/scpi/<family>.rs`.
Files: `ui/src/ui.rs`, existing area helpers, new area files, `ui/src/scpi.rs`, subsystem files,
`xtask/data/moves.txt` and generated ratchets. Keep one small routing entry per area/subsystem;
future family packages edit their own file, not the central dispatcher. Use subsystem functions rather than
spreading Scpi impl blocks across every family file. Do not rewrite tables or behaviour here.
Acceptance: existing menu/script/golden and SCPI session/alias/error tests unchanged; row/branch inventory has
exactly one owner, no moved logic remains in the central chains. Run `cargo xtask ratchet --move`.
Delta: near 0. Risk: medium. Depends: M12 before production moves. Parallel: E1a and E1b use disjoint paths;
area slices serialize until their routing seam lands. Later central registrations/wiring remain serial.

### M13: settings groups replace `Model`

**M13a, one group per package.** Scope: channels, timebase, trigger, acquire, display, measurements, analysis,
then interfaces; move ownership under `core/src/change/settings/`. Keep a temporary compatibility facade only
for consumers not yet converted. Reuse `field_table.rs`; do not duplicate it.
Files: the selected `core/src/model/` group, `core/src/change/`, `ui/src/model/` and that group's call sites.
Acceptance: every table row preserves defaults, normalization, class/key changes and setup round trips;
existing UI/SCPI/golden cases match. Delete each retired facade as its final consumer moves.
Expected delta: -50..-200 per group after facade removal; intermediate move near 0.
Risk: medium. Depends: M12 and M5a. Parallel: none between groups; every group touches
`core/src/model/state.rs` and `core/src/change/field_table.rs`, so M13 is serial. No new invariants/defaults.

### M14: commands per area

**M14a, one command family per package.** Scope: typed absolute/relative edits and queries; move that family's
menu, input and legacy setter writes into change. Preserve internal-before-external order and SCPI settlement.
Files: selected core command/settings family, E1 menu-area and `scpi/<family>.rs` files;
`cmd.rs`/setup callers only in separately reserved serial wiring slices.
Acceptance: equivalent menu, knob, SCPI and setup edits produce the same settings/classes; preserve each
edge's specified error behaviour. Relative edits and absolute writes in one intake retain their order. No direct writes remain for the converted group.
Expected delta: -50..-200 per family after deleting old dispatch.
Risk: medium. Depends: its M13 group and E1. Parallel: disjoint area/subsystem files after shared command
types land; shared command enum, `cmd.rs` and `store.rs` wiring serialize. No parallel claims on dispatch ranges.

**M14b, setup table.** Scope: one schema for setup keys, readers and typed edits; remove `store::set` as groups finish.
Files: `store.rs`, `store/`, core command/settings tables. Acceptance: strict/lenient parsing, legacy key aliases,
errors and all setup round trips unchanged; test walks every schema row.
Expected delta: -200..-500. Risk: medium. Depends: M14a families covered by each slice.
Parallel: IO transport work; serialize setup codec changes with M16 storage work.

### M15: remote adapters on commands and queries

**M15a-0, bounded wakeable intake.** Scope: synchronous owned request/reply ports with count/byte admission,
connection/request identities, cancellation and wake notification, before introducing tokio.
Files: `ui/src/input.rs`, `ui/src/system/device.rs`, `system/device/remote.rs`, host-serve wiring in `main.rs`
and a shared port module. Add an eventfd to Inputs' pollfd set: today libc::poll waits up to 33 ms, followed by
one remote poll per pass. Publish before signaling; drain/recheck with no lost wake, re-signal/revisit remaining
work when a quota expires, wake on reply capacity and disconnect, close descriptors through ordered shutdown.
Acceptance: event/completion-driven tests enqueue before/during/after a wait, fill the queue, exhaust a quota,
free reply space, disconnect and shut down. Every admitted request eventually receives service or cancellation;
no forced render, busy-spin or wall-clock speed assertion. Remove superseded inbox/wake plumbing.
Delta: 0..+150. Risk: medium. Depends: M2 settlement. Parallel: port fixtures only; serialize input/device wiring
with M3d. This package is independent of M12 and the new parser.

**M15a, raw-SCPI IO experiment and measurement gate.** Scope: top-level `ui/src/netio/`, one current-thread
tokio runtime on one IO thread; raw TCP framing/output only, existing interpreter on the frame thread.
Define RemoteRequest's application variants now; migrate their transports only in M15b.
Files: `ui/Cargo.toml`, `Cargo.lock`, `ui/src/netio/`, shared port, `scpi.rs`, `system/device*`, `main.rs`,
`xtask/data/architecture.txt`, transport/perf fixtures. Current `ui/src/remote.rs` is the telnet/firewall helper.
Acceptance tests implement the [thread/message contract](README.md#threads-and-messages):

- Credit one per connection lasts until reply sent/no-reply completion; fragmented/coalesced/pipelined programs
  preserve admission order. A whole `:WAV:SOUR;:WAV:DATA?` message is atomic, including across continuations.
  Keep shared wav_src/wav_fmt/error queue/ESR in the single interpreter; two-client tests pin that shared state,
  source/format selection, error consumption and ESR clearing. No per-client-state change is implied.
- Reserve response bytes before dispatch; all 8 clients at RESP_MAX (64 MiB) cannot allocate 512 MiB.
  Count allocated capacities once across assembled/parked/queued/IO-held replies and encoding chunks under
  one 64 MiB global budget; test exhaustion, release and disconnect. Include request/snapshot/scratch/upload
  bounds and headroom in the memory plan below the 768 MiB soft watchdog; no watchdog increase.
- A full reply channel parks one owned reply/reservation without rerunning setters. A stalled reader cannot
  block frames or input; completion-driven tests hold output full, advance frames, then release/drain in order.
- Intake visits <= 8 requests/continuations, checking the 0.5 ms stop-taking-work budget between interpreter
  turns. Yield without another program interleaving. Fake time pins quota rollover/FIFO admission; perf evidence
  covers large-message/indivisible-command cost and frame latency. Empty queries, delayed OPC and shutdown
  still complete/cancel.
- IO-side timers retain 2 s without progress/30 s total send limits. Injected timers test both limits,
  partial writes, credit return and snapshot release, with no blocking socket call on main.
- Owned RemoteRequest variants: Program; UiState; ImportWaveform { slot, name, payload handle };
  ExportWaveform { CSV/binary }; ExportLogicCsv; Stamp; UpdateStatus; Cmd { ordered lines, origin }.
  Contract fixtures map `/ui-state`, POST `/waveform`, `/wave.csv`, `/wave.bin`, `/la.csv`, `/stamp`,
  GET `/update` and `/cmd` to coherent app snapshots/actions. FIFO uses local Cmd syntax, HTTP keeps
  cmd::remote_ok; neither is SCPI. Preserve FIFO command order and Shot outcomes. POST `/update` keeps
  separate single-large-body admission, current limits and signed-package verification worker; only job
  handles/status/results enter intake. Test rejection/cancellation/release without moving large bodies to main.
- Add `confine ui tokio -> netio` and a top-level netio layer rule excluding Ui/settings/interpreter access.
  Public port types are synchronous; only netio uses ADR 0023 features. Boundary fixtures reject a tokio use
  outside netio and a netio-to-Ui import; cargo-deny passes.

Remove raw accept/read/send/poll code on successful cutover. Expected delta: +100..+300 for the experiment,
then <= 0 for raw transport after deletion. Risk: medium. Depends: M15a-0; no new-parser dependency.
Parallel: M12 evidence/spec audits with no shared production paths. Non-SCPI route execution remains on the
existing handlers until its M15b slice; the port fixtures define their migration contract, not a second implementation.

**Gate before rollout:** record before/after production LOC, dependency/binary size, threads, CPU, wakeups,
RSS, frame p50/p99 and remote enqueue latency/throughput, idle and under a stalled reader plus acquisition load.
Use the same build profile, workload and unit; no host ratio substitutes for unit results.
Retain the [target budgets](README.md#performance-budgets): frame p50 <= 8 ms / p99 <= 15 ms,
remote enqueue p99 <= 3 ms, >= 500 exchanges/s and picture-rate loss <= 10%.
The report must show the blocking write removed and justify any resource/code growth before broader adoption.
Owner decision 2026-11-18 ([ADR 0023](../adr/0023-io-layer.md)):
accept M15a for the behaviour gain verified on the unit. ADR 0023 now defines the wider IO-port policy,
including beneficial device-stream moves. The deferred size gate and `poll(2)` fallback are retired. M15a's host size cost is +685 production LOC,
+2.86 % aarch64 binary size and +9 dependencies after the gate fix. The unit and host measurements are recorded
in [task 0016](../tasks/0016-m15-remote-adapters.md#measurements).

**M15b, remaining network transports, one per package.** Scope: VXI-11, HiSLIP, HTTP/MCP, then mDNS reuse the
accepted request/reply/runtime seam; remove replaced accept/read/send/poll loops. Keep switches, ACLs, payload limits,
error codes, binary/text encoding and pending-program semantics.
Files: each transport and its helpers, `system/device/remote.rs`, `main.rs`, `netio/` and shared port types.
Acceptance: existing RPC/HTTP/discovery fixtures plus M15a application-route, FIFO/Cmd and separate signed-upload
contracts, stalled-client/cancellation/limit cases through the new IO;
repeat M15a load measurements. M15b reuses netio and tokio under ADR 0023; there is no
deferred size gate or `poll(2)` fallback. Each transport PR reports and minimises its production LOC and
aarch64 binary size delta. Expected delta: -100..-400 per transport, measured after deletion.
Risk: medium. Depends: M15a gate. Parallel: disjoint transport internals after the shared API lands;
serialize runtime registration/wiring. Encoding stays on CPU workers, not the IO executor.

**M15c, table-driven interpreter, one command family per package.** Scope: complete typed mappings in
[SCPI reference](../spec/scpi.md), then one header/alias/parameter table and a small dispatcher.
Keep legacy parsing as a host-only differential oracle. Retain one interpreter and its program/OPC/error state.
Files: E1 `scpi/<family>.rs`, future `remote/scpi/` family, core commands/queries and reference fixtures;
shared parser registration/state changes are separate serial wiring slices.
Acceptance: every table row, abbreviation, indexed expansion, parameter boundary, error and recorded session
runs through both parsers. Compare replies, final settings, ChangeSet and side effects. Every difference needs
an approved spec disposition or a fix. Cover sequential setter/query settlement and delayed OPC replies.
Expected delta: -100..-300 per family; delete corresponding legacy production branches as each family switches.
Risk: medium. Depends: E1, M14 mappings for that family and reviewed specification; parser cutover follows M12.
Parallel: accepted transport migration and disjoint families; serialize shared parser state/table edits.
Remove the test oracle only after full inventory/corpus parity and review.

WLAN, SNTP and Bode already own threads; USBTMC remains a blocking adapter. There is no M15d service migration.
A later service-to-runtime proposal must demonstrate a measured production-line reduction before admission.

### M16: stages and `Session` replace `Ui::step`

**M16a, storage off the frame loop.** Scope: bounded owned save snapshots and completion/errors; move manual
setup/export/screenshot writes and automatic `Startup::observe` promotion to a storage worker in small slices.
Preserve coalescing of automatic setups, explicit-save ordering, durable replacement and shutdown flush order.
Files: `store.rs`, `store/startup*`, `store/durable.rs`, UI save actions, `main.rs`, `system/device*`, `ui/lifecycle*`.
Acceptance: a blocked fake filesystem cannot block a frame; ordered completions/errors, queue/byte bounds,
rapid edits during a pending write, crash-marker/durable fault tests and shutdown flush all pass.
Expected delta: 0..+150 for the shared queue, offset by deletion of per-caller write paths.
Risk: medium. Parallel: network transports; serialize with M14b codec changes. Reuse the durable Files port.

**M16b, one state owner/stage per package.** Scope: records/history/segments, measurement state, picture state,
then intake/reply; extract from Ui with narrow inputs/outputs. Replace step orchestration last by Session.
Files: `ui.rs`, `ui/change*`, `ui/lifecycle*`, `wave.rs`, `drawpipe*`, new `app/`, `records/`, `picture/`.
Acceptance: same host command logs, identities, STOP/SINGLE/stale-result tests and goldens; no new allocation/copy
on touched hot paths; each state has one writer. Final Session has wiring, not Ui's fields.
Expected delta: near 0 per ownership move, -100..-300 on final orchestration cleanup.
Risk: medium; timing changes need unit evidence. Depends: accepted drawing owners, M13/M14 and typed IO seams.
Parallel: disjoint view extraction; serialize Ui fields, step order and publication edits.

### M17: views and menus as components

**M17a, one view per package.** Scope: spectrum, eye, Bode, power and remaining analysers/LA/XY/bus move with
state, inputs and rendering into components. Reuse existing Worker and registration abstractions.
Files: selected `ui/<view>*`, corresponding Ui fields and future `views/`/`compose/` bindings.
Acceptance: per-view jobs, stale/cancel results, source changes, navigation and golden pictures match.
Expected delta: near 0 for moves, -20..-100 per view from deleted glue.
Risk: medium. Depends: M14 commands and M16 owner interfaces. Parallel: separate view paths with serialized Ui edits.

**M17b-read, read-side descriptors, one area per package.** Scope: ordered visibility, kind, label, value
reader and options from [menu inventory](../spec/menus.md); shared renderer/navigation reads these descriptors.
Before starting, refresh that inventory: its summary says 456 Items, while main has 457. Resolve every row/expansion.
Files: E1 `ui/menu/<area>.rs`, `ui/active_choice*`, `ui/lint.rs`, future `menus/`, menu spec and golden fixtures.
Acceptance: row-complete read/visibility tests, unchanged labels/values/options, committed choice identity distinct
from focus, text-size/capability/picker/Back/Close goldens; reuse existing active_choice.
Remove the area's four read dispatch chains/visibility duplication as replaced. Delta: -50..-200 per area.
Risk: medium. Depends: E1, refreshed/reviewed inventory and shared read renderer; no M14 dependency, so it can
land before commands. Parallel: disjoint area files; shared renderer/navigation and registration serialize.

**M17b-write, actions, one area per package.** Scope: adjust/activate descriptors return typed commands or
component events, preserving knob/touch and remote-edits-while-open behaviour. Files: the area's descriptors,
core command family and action fixtures. Acceptance: all actions equal typed command contracts; ordered relative
edits and navigation side effects match. Delete the area's two legacy write chains.
Delta: -50..-100 per area. Risk: medium. Depends: M17b-read and its M14 family.
Parallel: disjoint area action files; shared navigation/wiring serialize.

**M17c, overview and final Ui removal.** Scope: overview reads the same descriptors, only after owner approval
of [overview](../spec/overview.md); remove Ui once every owner and caller has migrated.
Files: `menus/`, `views/`, `app/`, remaining `ui.rs` callers/tests. Acceptance: overview navigation has no setting
side effects, owner-approved pictures, full regression suites and no residual Ui state/dispatch facade.
Expected delta: overview estimated +100..+250; final facade removal -100..-300.
Risk: medium. Depends: M16b/M17a/b and explicit overview approval. Parallel: none on final wiring.

### M18: target layout

Move remaining adapters only with their owning packages; do not save one giant rename for the end.
Final acceptance: target layer rules hold and retired modules have no ratchet entries.
Moves have near-zero production delta. Repeat unit load/soak against M12 evidence after the final integration.

## Status

- 2026-11-17: E1b extracts SCPI branches into subsystem functions, preserving interpreter state,
  branch operations and reply literals. Host UI tests pass in debug and `test-release`, including
  `ui/lint.rs`, scripts, SCPI sessions/aliases/errors, goldens and soak. The fast gate and Clippy pass;
  moves use `cargo xtask ratchet --move`. Production delta: +471 lines of routing/extraction scaffolding.
  Full integrated gate and independent review remain pending. No hardware run is claimed.

- 2026-11-17: M15a-0 implements bounded synchronous owned intake, identities, cancellation and a coalescing
  pollable wake in device input polling and paced host intake. Replaces the HTTP inbox; raw transports remain for M15a.
  Production delta: +150 lines. Host evidence: fast tier passes 11 steps; all 10 Linux acceptance fixtures pass;
  UI debug passes 1486 tests, including goldens and layout lints. UI release passes 1484 tests and fails
  `scripted_modes_reach_a_plateau` in pairs mode (816-byte final growth against a 752-byte allowance);
  58 tests skipped in each profile. Isolated pairs runs pass on both the base and changed trees. Release
  acceptance and independent review remain open; no hardware run claimed.

- 2026-11-17: E1a source extraction is implemented; acceptance remains open. One exhaustive area registration
  routes the six item dispatches and menu rows; source visibility, trigger/decode rows and start-menu groups have
  area owners. Host comparisons against the original dispatch pass for all 457 item variants and 48 menus across
  decoder, trigger, offline and LA contexts, and for exercised adjustments in both directions.
  `cargo test -p oos-ui` and `cargo test -p oos-ui --profile test-release` pass, including existing menu lint,
  script, golden and SCPI tests. `cargo xtask check --fast`: partial gate passed (not done), 11 steps in 3 s.
  `cargo xtask ratchet --move` applied the three size moves; `cargo xtask ratchet` tightened the changed UI entries.
  Production delta: +1,250 lines for area routing and callbacks, above the near-zero estimate.
  `cargo xtask step clippy`: gate FAILED in 2 s, 17 relocated findings. The one-to-one move format cannot split
  inherited per-file lint counts across area files. Next action for the coordinator: provide that ratchet
  prerequisite and review the routing overhead before integration. Independent review remains open.
  No unit acceptance is claimed.

- 2026-11-17: review fixes restore M3..M11 packages, add move-only dispatch and wake-intake prerequisites,
  specify bounded atomic remote/app handoffs and split menu read/write work. Implementation remains open.
- 2026-11-17: source audit recorded implemented core change handling, field generation, generic Worker,
  browser-only host view and remaining UI/IO/draw debt. M12 owner choice, IO experiment measurements,
  overview approval and final hardware acceptance remain open. No refactor or unit run is claimed here.

## Strategy and draw-path acceptance

The incremental migration continues through M12, including removal of M2 bridges. Settings and views keep that
approach. Menus and the parser rebuild from approved specifications (ADR 0038); their soft freeze is in delivery.md.
The owned board-neutral AcqRequest belongs to M5. Specification work can precede production rebuilds.

Planning estimates from the retired hybrid record: 45 planned host days to M12, 70 to 90 adjusted; full rebuild
150 to 300 adjusted. The fresh-core spike estimated 74 to 132 days for parity and measured kernels at 49/183 us
against 13.9/7.7 us on the same host inputs. These are historical planning evidence, not current forecasts or
unit verification. A full rebuild was rejected because it delayed all shipping benefits until one switch-over.
Rebuilding menus/parser from specs avoids preserving accidental legacy behaviour; the legacy parser remains a
host differential oracle until deletion. Every difference needs a recorded spec decision or a fix.

Batch is today's generic-image default; the parser defaults to Standard/Frames without configuration.
The surviving Standard-versus-Batch choice is target M12.
Keep both until picture equality, performance ratios and unit round 5 select one and delete the other.
Verified on the unit: DHO924S, 2026-11-15, four channels at 10 us/div with a 100 kHz CH1 square wave and Auto
trigger, Batch refreshed at 18.2 fps versus Standard 26.1 fps. Find the cause before deleting Standard.
M9 adds whole BatchUnits alongside Standard; a one-frame step forms one unit. RecordBatch off the fast path
uses the same accumulator as a pseudo-batch. Keep the N-frame/67 ms window and 3-batch cap. Route-D composition
removal waits for M12. Compare one-frame units and 48-frame steps: one unit per frame can cost two full-map
passes per frame, about 2.5 GB/s at 48 frames per step. Host equality proves no hardware timing.

## ADR baseline follow-ups

These are follow-up issue briefs for the coordinator, not implemented checks or new unit evidence (#159).
The coordinator replaces each TBD issue placeholder before acceptance.

| Record | Issue | Follow-up and acceptance |
|---|---|---|
| ADR 0006 | TBD (coordinator) | Gate tightening: fail on lowered counts left uncommitted; tests cover slack and planned moves. Preserve the current file/function line definitions. |
| ADR 0008 | TBD (coordinator) | Baselines: replace current CPU RGB-1/floor(N/1000) tolerance with exact comparison; cover every layout-check script and all three text sizes with goldens, then remove the temporary procedure. |
| ADR 0011 | TBD (coordinator) | Protected areas: host-test every update write device/offset against the allowlist; require protected-area before/after evidence for releases and hardware-path PRs. |
| ADR 0012 | TBD (coordinator) | Hardware-key inventory: check every parsed key has a row and removal issue; reconcile enabled image defaults and environment switches with off-until-verified. |
| ADR 0015 | TBD (coordinator) | Device state: manifest all first-boot defaults, durable writes and update-preserved files; test absent/present values and signed sd.mode variant selection. |
| ADR 0016 | TBD (coordinator) | Automatic rollback: verify failure and rollback on a released unit, recording model, build, date and artifacts. |
| ADR 0017 | TBD (coordinator) | Reset evidence: verify whether RK3399 exposes a positive watchdog reset indication; preserve power-cut handling if absent. |
| ADR 0018 | TBD (coordinator) | Service policy: finish #139 telnet removal; add /cmd second ACL, a service-table coverage test and one-second stop/withdrawal tests; verify firewall on the unit. |
| ADR 0019 | TBD (coordinator) | M15b: verify HiSLIP on the scope with remote.hislip off by default until verification; check discovery withdrawal and VISA sessions. |
| ADR 0023 | TBD (coordinator) | IO ports: forbid std::net in non-IO layers, including alias fixtures; complete HTTP/mDNS/NTP/update moves and assess USB/WLAN benefits with explicit synchronous exceptions. |
| ADR 0025 | TBD (coordinator) | SCPI conformance: every documented command resolves, no undocumented command answers, and abbreviations/identity match the reference. |
| ADR 0028 | TBD (coordinator) | Registries: pin released identifiers, register remaining measurements and test release feature completeness. |
| ADR 0030 | TBD (coordinator) | Typed errors: add a shrinking Result<_, String> ratchet, migrate areas to typed errors and test sender/retry reporting. |
| ADR 0031 | TBD (coordinator) | Performance gates: required ratio-based host perf CI, a production sleep ratchet, and a complete budget/counter/assertion inventory; require measured unit release budgets. |
| ADR 0035 | TBD (coordinator) | Default alignment: change host/model and invalid-value fallbacks from Frames to Batch in a separate behaviour PR with tests and a changelog entry. |
| ADR 0036 | TBD (coordinator) | M5 ownership: reject Mutex outside named cells with alias tests after removing Shared. |

The registry migration also splits Protocol as one package: decode/trigger matching into dsp, metadata into core
and presentation into ui, preserving the static registration and released identifiers (ADR 0028).
