# OOS architecture (target)

This is the architecture OOS is moving to on the `overhaul` branch. Where the code still differs, the migration plan
(`overhaul-migration.md`) says which step closes the gap. Decisions and their reasons are in `adr/`. How to add things
is in `extending.md`. Rules for humans and agents cite the headings of this file by anchor.

Reading order: [Layers](#layers), [State and ownership](#state-and-ownership), [Change detection](#change-detection),
[Threads and messages](#threads-and-messages), [Data flow](#data-flow). The acquisition thread, the picture and the
record path have their own files: `architecture/acquisition.md`, `architecture/picture.md`,
`architecture/records.md`. Boot, kernel and the other crates: `architecture/system.md`.

## What stays from upstream

- **The crate split:**
  - `oos-hal`: register sequences, buses, boot;
  - `oos-dsp`: pure signal processing;
  - `oos-ui`: the application;
  - `oos-fpgad`: the bring-up CLI;
  - `gpu/oos-gpu.c`: the GPU helper process.
- **Pure write lists in hal.** hal builds pure write lists and applies them through `dho::bus::Bus` (`Bar1` on the
  unit, `Recorder` in tests).
- **Pure dsp.** `dsp` is pure functions over `&[f32]` volts with a fixed `dt`.
- **The `Board` port.** The UI speaks in its own terms and gets frames in volts. The `board::boundary` lint keeps
  `oos_hal::dho` inside `board/dho`.
- **Decoder modules.** The `Protocol` trait for decoder modules (`protocol-modules.md`).
- **The rest of the system.** Boot chain, kernel, update and the network services are unchanged
  (`architecture/system.md`).
- **One dependency:** `libc`.

What changes is the inside of `oos-ui` (`overhaul.md`, Motivation).

## Layers

Hexagonal where it pays, plain modules where it does not. The domain is its own crate, so Cargo enforces the direction.

| Layer | Where | Contents | May depend on |
|---|---|---|---|
| domain | crate `oos-core` | settings and their commands, `normalize`, change detection, `Generations`, `AcqIntent`, the acquisition data types (`BatchMeta`, `Frame`, `Record`), the picture model (`MappingKey`, window and weights), `Config` | `oos-dsp`, std |
| application | `oos-ui`: `app`, `records`, `picture`, `compose`, `views`, `menus` | the step pipeline: stages with typed inputs and outputs, wired by `Session` | `oos-core`, port traits |
| ports | `oos-ui`: traits next to their adapters | board, display, input, clock, external instruments, decoders | `oos-core` types |
| adapters | `oos-ui`: `board/dho` (hal), `display`, `input`, `remote`, `store`, `system`, `instruments`, `protocols` | DRM, GPU helper, VPU, panel UART and touch, SCPI, HTTP, MCP, VXI-11, USBTMC, setups and `system.conf`, network services | the port they implement, `oos-core`, their device or protocol |

Rules:
- **Domain is pure.** `oos-core` depends on `oos-dsp` and std only. It does no IO, starts no thread, reads no clock and
  holds no global state. Time enters as an argument: the window's 67 ms age cap and pacing take `now` from the caller,
  which reads `clock::Clock`. Cargo enforces the dependency; the crate's tests run on the host.
- **Configuration is read once.** `Config` (from `system.conf`, the environment and the platform) is read at start and
  passed in. No policy function reads a `OnceLock` or the environment.
- **No hidden global state.** A `static` with interior mutability (atomics, `Mutex`, `Cell`, `OnceLock`) is allowed
  only for metrics, on an allowlist the checker enforces. Today's `pacing` state and `deepmeas::FORCE` move into the
  stages that own them.
- **Adapters stay apart.** Adapters do not name each other. Two adapters that must cooperate do it through the
  application.
- **One user per outside interface.** Only `board/dho` uses `oos_hal::dho`. Only `display/` uses DRM, the GPU helper
  and the VPU.
- **Views and menus don't write settings.** They read settings and results, and change settings only by returning
  commands.

How the rules are checked: [Enforcement](#enforcement).

Small pure helpers (`font`, `layout`, `json`, `gfx`) are plain modules; a port would cost more than it gives.

## Module map

The target layout of `core/src` (crate `oos-core`) and `ui/src` (crate `oos-ui`). One responsibility per module;
size limits as in [Size limits](#size-limits). Today's files are in brackets where they are split or moved. Each
module is a top-level module of its crate, so the rules in [Enforcement](#enforcement) can name it.

`oos-core`:

| Module | Responsibility |
|---|---|
| `settings/` | `Settings`, `normalize`, and one module per group: `channels`, `timebase`, `trigger`, `acquire`, `display`, `measure`, `analysis`, `interfaces`. Each: struct, invariants, command enum, `apply`, its `classify` function, tests [`model.rs`] |
| `command.rs` | `Command` (top-level enum of the group enums), `CmdError`, `Query`, the reply type |
| `change/` | the change stage: owns `Settings` privately and hands out `&Settings`; applies commands in merge order, normalizes, classifies; `Request`, `ChangeSet`, `Generations`, `MappingKey` construction, `AcqIntent`, `PipeView` |
| `acq.rs` | the acquisition data types: `BatchMeta`, `Frame`, `MeasRead`, `Record`, `RecordBatch` headers (`architecture/acquisition.md`) |
| `window.rs` | the picture model: window bounds (N, the 3-batch cap, the age cap), decay in 16.16, weights; pure, `now` as argument |
| `config.rs` | `Config`: parsed once at start from `system.conf`, the environment and the platform |

`oos-ui`:

| Module | Responsibility |
|---|---|
| `app/` | `session.rs`: creates the stages, the channels and the deposit pipe, runs one step, holds no domain state. `intake.rs`: collects one step's inputs into commands, requests and acquired data. `acq_client.rs`: maps `AcqIntent` to the board's `AcqWant` through `Board::want`, publishes it and `PipeView`, sends `AcqCmd`, takes `Frame`, `MeasRead`, `Readings`. `outbox.rs`: internal commands and requests for the next step |
| `picture/` | accumulator (`acc.rs`, using `oos_core::window`), grade; `picture/deposit/`: the batch pipe, `BatchUnit`, kernels (spans, dots), workers (`architecture/picture.md`) [`drawpipe.rs`, `fastdraw.rs`, `sparse.rs`, drawing parts of `compress.rs`, `wave::Phosphor`] |
| `records/` | the records stage: record store (`last`, history, segments), measurement jobs, statistics, compression and deep-read policy (`architecture/records.md`) [`wave.rs`, `framestats.rs`, `deepmeas.rs`, parts of `ui.rs`] |
| `compose/` | the screen from layers: plot, traces from the accumulator, overlays, top bar; `gfx`, `font`, `layout` [`ui.rs` rendering] |
| `views/` | scope, windows, spectrum, eye, Bode, power, LA, XY, bus view: one component each, with its own state, inputs and drawing [`ui/analysis.rs`, `ui/eyepage.rs`, `ui/bode.rs`, `ui/la.rs`, ...] |
| `menus/` | the menu tree, items, dialogs, the layout lint, script access [`ui.rs` menus, `ui/lint.rs`, `ui/script.rs`] |
| `input/` | panel keys and knobs, touch, mapping to commands [`input.rs`, `ui.rs` input handling] |
| `remote/` | SCPI, HTTP, MCP, VXI-11, the control FIFO, `netacl`, `net6`: transports and parsers to commands and queries [`scpi.rs`, `http.rs`, `json.rs`, `vxi11.rs`, `cmd.rs`, `net.rs`] |
| `store/` | setup files, CSV, PNG, `system.conf` [`store.rs`, `sysconf.rs`] |
| `display/` | DRM output, the GPU helper, the VPU encoder [`drm.rs`, `gpu.rs`, `vpu.rs`] |
| `system/` | network services and the unit: `mdns`, `sntp`, `ssh`, `wlan`, `ethlink`, `hostname`, `time`, `update`, `backlight`, `placement` |
| `clock.rs` | the `Clock` port and the system clock |
| `instruments/` | external instruments: the USBTMC client for an external generator [`usbtmc.rs`] |
| `board/` | the `Board` port (`want`, capabilities, the thread's channel ends), `to_record`; `board/dho/`, the DHO800/900 adapter, its acquisition thread split as in `architecture/acquisition.md` |
| `protocols/` | decoder modules behind `Protocol` (unchanged) |

`Ui` (today 130 fields in `ui.rs`) does not move into `Session`. Its state is divided among the stages and components
above, each the single writer of its part.

## State and ownership

Every piece of state has one writer. Readers get it by value, by `&` within the step, or as a message to another
thread.

| State | Writer | Readers | Lifetime |
|---|---|---|---|
| `Settings` | the change stage, through `Command`s and `normalize` | every stage, by `&` | process; saved by `store` |
| `Config` | read once at start | every stage, by `&` | process |
| `ChangeSet` | the change stage | stages after it in the same step | one step |
| `Generations` (`gen`, `epoch`, `stats_gen`) | `change` (`new_gen` from `classify`, `new_epoch` from request resolution, `stats_gen` on the `stats` class) | every stage, as values in `ChangeSet` | process |
| outbox of internal commands and requests | the stage that emits them | the change stage, next step | one step |
| `AcqIntent` with `gen` and `epoch` | the change stage | `app/acq_client` | one step |
| `AcqWant` (board-specific) | `app/acq_client`, through `Board::want` | acquisition thread, latest value | until the next change |
| hardware, batch buffers, `AcqLoop` state | acquisition thread (`board/dho`) | nobody; results leave as `Batch`, `Frame`, `MeasRead`, `Readings` | process |
| `Readings` (status, counters, acknowledged ids, the read plan, `k`) | acquisition thread | UI, latest value | process |
| `PipeView { gen, epoch, mapping_key, route }` | the change stage, from the same `ChangeSet` as `AcqIntent` | deposit pipe and workers, latest value | until the next change |
| the deposit pipe | `Session` (created and owned) | workers by claim, main by `BatchUnit` and `RecordBatch` | process |
| a batch's share maps | the worker or helper drawing that share | the accumulator, after the batch is published | one batch |
| accumulator maps (`ph`, `phz`), weights, window | `picture` accumulator | `compose`, by `&` | until a `ChangeSet` resets them |
| records, the last record, history, segments | `records` | measurement, views, store, remote queries | per class |
| measurement results and statistics | `records` measurement | views, remote queries | per class |
| view state (spectrum, eye, Bode, LA) | its view component | `compose` | until the view closes or its class resets |
| menu state (open menu, focus, dialogs) | `menus` | `compose` | process |
| system settings (`system.conf`) | `store::sysconf`, through `interfaces` commands | `system` services | file |
| a network service's state | that `system` service | its menu page | process |
| composed picture for remote clients | `present` | web PNG thread, by snapshot | one picture |

### Enforced by visibility

Ownership is enforced by the compiler, not by a checker rule:
- The change stage holds `Settings` in a private field and hands out `&Settings`. Group setters are
  `pub(in crate::change)`.
- `Generations` is private to `change`; other stages see `gen` and `epoch` only as values in `ChangeSet` and
  `AcqIntent`.
- `ph`, `phz`, W and the window are private fields of the accumulator, exposed only as `&Picture`.
- Owned types have no `pub` fields.

The checker covers what visibility cannot: which modules may name which ([Enforcement](#enforcement)). It finds
fully qualified paths inside expressions as well as `use` lines.

## Settings and commands

`Settings` is one owned struct of grouped sub-structs. Change detection needs one snapshot of everything; ownership
per area comes from the group modules.

Each group module (`settings/trigger.rs`, ...) holds:
- the group struct, private fields, `pub` accessors, setters visible only to `crate::change`;
- its invariants within the group, checked in `apply` (ranges, combinations of its own fields);
- its command enum (`TriggerCmd::Level(f32)`, `TriggerCmd::Slope(Slope)`, ...);
- `apply(&mut self, cmd, &Context) -> Result<(), CmdError>`. `Context` is read-only and narrow: the model's limits
  and, per group, a few named accessors of other groups' values (`ctx.channel_range(ch)`), never a whole `&Settings`;
- its part of `classify`, a function of its own;
- host tests of the invariants and of every command.

A group with variants that carry their own parameters uses an enum of per-variant structs
(`TriggerKind::Edge(EdgeCfg)`, `TriggerKind::Pulse(PulseCfg)`), not one flat struct of every parameter
(`extending.md`, [Add a setting](extending.md#add-a-setting)).

`Command` is a top-level enum: `Command::Channel(ChannelCmd)`, `Command::Timebase(TimebaseCmd)`,
`Command::Trigger(TriggerCmd)`, and so on. Adding a setting touches one group module.

Commands come from the edges only:
- **input:** a key or knob in a menu context maps to a command (`input/`, `menus/`). Knobs produce relative commands
  (`TimebaseCmd::ScaleStep(i32)`); `apply` clamps. A knob turn and a remote write in the same step then both take
  effect;
- **remote:** SCPI, HTTP, MCP and VXI-11 parse their text into commands and queries (`remote/`);
- **store:** a setup file is parsed into a list of commands and applied like any other. The `key = value` names live
  only in `store`.

### Merge order

The change stage applies one step's commands in a fixed order of sources: the outbox (internal writers, see
[Internal writers](#internal-writers)), then store, then remote, then input. Within a
source, arrival order. A setup load is the base; remote writes apply on top; the panel, where a person acts, applies
last and wins a conflict between two absolute values of the same field. Relative commands never conflict. A remote
set's reply, a later query on the same connection and `*OPC?` report the value after `normalize`, so a client sees
when its write was overridden; SCPI also queues `-221` (settings conflict) for it.

### Normalize

Cross-group constraints have one model: `Settings::normalize()`, run by the change stage after the step's commands and
before `classify`. It is the only code with `&mut` access to more than one group. It visits the groups in a fixed
order (channels, timebase, trigger, acquire, display, measure, analysis, interfaces), so a clamp in an earlier group is
seen by a later one. It holds today's per-step rewrites:
- `clamp_offsets` (offsets within the range of the current scale);
- `fit_platform` (what the board lacks: CH3 and CH4 off on a 2-channel model, an unsupported coupling to DC, LA off
  without the LA port, sources that point at a missing input);
- clamps across groups, such as the trigger level within the source channel's range after a scale change.

Normalize is idempotent: running it twice gives the same `Settings`, and a test checks it. A value it changes is part
of the diff like any other: `fit_platform` switching a channel off is an `acq` change, and a clamped offset is a
`mapping` change that gets a preview.

### Internal writers

Some settings changes have no outside source: SINGLE done or segments done switching to STOP, the multi-step writes of
autoset and Bode, `auto_step`. These stages do not write `Settings`. They put commands and requests into the outbox
(`app/outbox.rs`), and the next step's change stage applies them like any other source, before the store, remote and
input commands of that step. The result is one writer and one diff, at the cost of one step (33 ms).

## Change detection

One point per step, inside the change stage:

```
apply commands -> normalize -> ChangeSet = classify(previous Settings, current Settings, previous UiAcq, current UiAcq)
                                           | requests
```

- **`classify`** compares the groups and maps each changed field to classes.
  - It is split per group from the start (one function per group), and destructures every group without `..`. A new
    field does not compile until it is placed in a class or in `Unrelated`.
  - Values the change stage writes itself (`gen`, `epoch`, `AvgClear` ids) are outputs, excluded from the diff.
- **`requests`** are events with no settings difference: CLEAR, the RUN and SINGLE edges, Create mask, and
  `Request::Epoch(token)` from an autoset or Bode stage (also a retry with the same settings). SINGLE arm, CLEAR and
  RUN after STOP are seen in the change stage itself (from intake or the run transition) and get their epoch in the
  same step. Requests from later stages go through the outbox and resolve at the next step.
- **`MappingKey`** is built by `classify` from exactly the fields of `Settings` and `UiAcq` it puts in the `mapping`
  class, in the same destructure. A field cannot be in the class and missing from the key.
- **`UiAcq`:** UI state that changes what is acquired or drawn but is not a setting: the XY view, the protocol watch
  (both need raw records), the mask reference, autoset or Bode being active, and the main and zoom plot sizes from the
  layout (panes, HDMI, menus). The plot sizes are classified `mapping`, so they enter `MappingKey` through the same
  destructure as the settings fields. It is not stored. Each step derives it as
  a value from the views' and menus' state (`UiAcq::of`), and the change stage diffs it against the previous step's
  value.

Classes and their consumers. A setting can be in more than one class: V/div is `acq` (the AFE gain changes) and
`mapping` (traces move on screen). Each consumer, views included, declares the classes it reacts to next to its state
(`const RESETS_ON: Classes`), and a test checks every view against it.

| Class | Consumers |
|---|---|
| `acq` | `AcqIntent` and `gen`; records of an older `gen` after a reconfiguration are not measured |
| `mapping` | accumulator: preview, drop maps of the old mapping key (`architecture/picture.md`) |
| `phosphor` | accumulator: maps and weights reset |
| `stats` | measurement statistics and results; raises `stats_gen` |
| `mask` | mask counts |
| `zone` | zone drops of the hardware zone |
| `hw_average` | the hardware average restarts |
| `history` | history and segments |
| `math` | math and FFT state that holds or averages |
| `view` | view state that holds over time: trend, eye, XY persistence, cursors on a time-base change; each view lists its classes |

A keyed cache needs no class. The decode cache is keyed by record and decoder settings, so its key decides.

### Generations

Two numbers, private to `change` (`Generations`):
- **`gen`** rises when the `ChangeSet` contains `acq`. `classify` calls `new_gen`. The acquisition thread tags every
  output with the `gen` it applied.
- **`epoch`** rises on requests whose settings may not differ: SINGLE armed, CLEAR, RUN after STOP, an autoset or Bode
  stage. Request resolution calls `new_epoch`. It replaces `hw_stamp`.
- **`stats_gen`** rises with the `stats` class. Measurement jobs carry it; a late result of an older `stats_gen` is
  dropped (`architecture/records.md`).
- **Tokens instead of synchronous returns.** An autoset or Bode stage runs in `views`, after `change`, and cannot
  write generations without breaking the stage order (adr/0004). It returns `Request::Epoch(token)` through the outbox.
  The next step's change stage resolves it, and `ChangeSet::epochs` carries `(token, epoch)`. Cost: one step (33 ms)
  before its acquisition starts, against stages that wait several batches anyway. Rules:
  - **Commands first, same step.** The settings commands a stage emits with its token (new time base, generator
    frequency) are applied in the same change step, before the epoch. `AcqIntent` carries the new `gen` and the new
    `epoch` together, so no record of the old settings carries the new epoch.
  - **An epoch-only change is an acquisition boundary.** The acquisition thread drops a batch armed ahead and queued
    frames of the older epoch, as for `gen` (`architecture/acquisition.md`, `apply`).
  - **A token without an epoch accepts nothing.** Until its `ChangeSet` arrives, the stage takes no records.
  - Tokens are unique per request (requester, run id, stage). Several requests in one step may share one epoch.
- The pair is published inside `AcqIntent` and `PipeView`, so it is always consistent.
- Readers filter on the pair: the picture by its mapping key, records and measurements by `epoch` and `gen`
  (`architecture/records.md`).

### Acquisition intent

The change stage produces `AcqIntent`: what to acquire, in the domain's terms (channels, time base, depth, trigger,
modes, the raw and deep-read needs, the unit wants), with `gen` and `epoch`. It is board-neutral. `app/acq_client`
maps it to the board's `AcqWant` through `Board::want(&AcqIntent)` (`architecture/acquisition.md`). `Board::want` is
pure: the same intent gives the same `AcqWant`. The domain never names a board type. The change stage also produces
`PipeView { gen, epoch, mapping_key, route }` for the deposit pipe from the same `ChangeSet`; a route change is an
`acq` change (`architecture/picture.md`, route per batch).

### Run state

Run state goes by transition, not by value:
- `run` and the trigger sweep together with `run` are not settings changes.
- Each consumer acts on the run-state transition of the step. RUN to STOP keeps the picture; STOP to RUN restarts the
  average.
- A mapping change in STOP redraws from the last record and never blanks.

## Step pipeline

`Session::step` runs fixed stages. Each stage owns its state, takes typed inputs and returns typed outputs. The order
is in the code; no stage relies on a side effect of another.

```
intake     (input, remote requests, outbox, BatchUnits, RecordBatches, Frames, MeasReads, Readings)
                                                       -> commands, requests, acquired data
change     (commands, requests, UiAcq)                 -> Settings', ChangeSet, AcqIntent, PipeView, AcqCmds
acquire    (AcqIntent, PipeView, AcqCmds)              -> AcqWant through Board::want, PipeView, published
records    (RecordBatches, Frames, MeasReads, ChangeSet) -> records, record store, the step's pseudo-batch
accumulate (BatchUnits, pseudo-batch, ChangeSet, now)  -> &Picture
measure    (records, ChangeSet)                        -> results, statistics
views      (results, records, events, ChangeSet)       -> view state, outbox (commands, requests for the next step)
compose    (picture, results, views, menus)            -> layers and trace jobs
present    (layers)                                    -> DRM or the GPU helper; snapshot for remote clients
reply      (results)                                   -> answers to remote queries
```

`records` runs before `accumulate` because off the fast path the picture is drawn from records: the step's
records form one pseudo-batch (`architecture/picture.md`). History and segments are part of the record store, so
`records` updates them, not `measure`.

A remote query is answered in `reply` from that step's state. Each query carries a typed wait condition: `After {
gen, epoch }`, set by the commands that came before it on the same connection. `:SINGLE; :WAV:DATA?` therefore waits
for a record of the SINGLE's epoch and never returns the old record. The transport thread holds the connection while
the query waits. `*OPC?` uses the same condition: it answers when the acquisition has applied the connection's last
`gen` and `epoch`.

## Threads and messages

| Thread | Core | Policy | Owns | Talks through |
|---|---|---|---|---|
| main | first A72 | SCHED_OTHER | `Session` and every stage | the channels below; never blocks on IO |
| GPU helper (process) | first A72, with main | SCHED_OTHER | nothing; reads the arena | the arena and its pipe |
| acquisition (`board/dho`) | second A72 | SCHED_OTHER | hardware, batch buffers, `AcqLoop`, the helper's share | `AcqWant` and `AcqCmd` in; `Batch`, `Frame`, `MeasRead`, `Readings` out |
| deposit workers | A53, one per little core | SCHED_OTHER | their share maps; the route per batch from `PipeView` | `BatchUnit` (fast path) or `RecordBatch` (records, off it) to main (`architecture/picture.md`) |
| measure | A53, scoped per channel | SCHED_OTHER | one measurement job | job and result channels (`architecture/records.md`) |
| input | A53 | SCHED_OTHER, blocked on read | the panel UART, the touch device | `InputEvent` channel to main |
| remote listeners | A53 | SCHED_OTHER, blocked on sockets | sockets of SCPI, HTTP, MCP, VXI-11, the control FIFO; encoding of large answers | `RemoteRequest { command or query, wait, reply sender }` to main |
| web PNG / send | A53 | SCHED_IDLE (nice 10 where refused) | encoding and sending pictures | the composed-picture snapshot |
| ntp, ssh, mdns, wlan | A53 | SCHED_OTHER | their service | status values the menus read; settings by command |

Placement comes from `system/placement` (`perf.place`), with the cores read from `cpu_capacity`. The xdma interrupt
goes to a little core from the rootfs. No thread runs a real-time policy: a stuck real-time thread on this SoC stalls
the panel, and the budgets hold without it.

Large answers are encoded off main: a deep `:WAVeform:DATA?` gets an `Arc` snapshot of the record in its reply, and
the listener thread encodes and sends it.

Typed outputs replace `board::HwShared`, today an `Arc<Mutex<Shared>>` of about 40 fields with settings, one-shot
requests, outputs and readings under one lock. Each kind gets one writer (`architecture/acquisition.md`, Port):
- **`AcqWant`:** everything the UI wants applied, with `gen` and `epoch`. A latest-value cell, replaced whole.
- **`AcqCmd`:** one-shot commands with an id (force, rearm, hold and final, average clear, segments, self-cal, levels,
  slots). A bounded queue; each is taken once and acknowledged in `Readings`.
- **`Batch`, `Frame`, `MeasRead`:** acquired data with its `BatchMeta`. Every `Batch` moves into the deposit pipe; the
  acquisition thread knows no drawing policy. `Frame` and `MeasRead` go to the record path. No kernel accepts a
  `MeasRead`.
- **`BatchUnit`, `RecordBatch`:** the pipe's outputs to main. The pipe routes each batch by its view: deposited into a
  `BatchUnit` on the fast path, decoded only into a `RecordBatch` off it. Decoding stays off the first A72.
- **`Readings`:** status, unit counters, board readings, acknowledged ids, the read plan. A latest-value cell.
- **Atomics:** metrics read often and changing all the time (frame counters, pacing) stay atomics, one writer each.

No lock is held across a register access, a C2H read or a decode. The migration does this after the change point
(`overhaul-migration.md`, M5). Until then `Shared` stays.

## Data flow

The picture:

```
FPGA ring -> acquisition thread: read a batch under `gen` g, tagged with its `BatchMeta`
          -> pipe: route by view
               fast path: deposit workers and helper draw share maps, one weight model per column per frame
                          -> BatchUnit: all shares, published when the last is in
               off it:    workers decode -> RecordBatch -> records -> the step's pseudo-batch
          -> accumulator: window of whole batches (minimum persistence) or decay (persistence), weights W
          -> compose: reads the accumulator's `Picture` (`ph`, `phz`, W, grade) by `&`; count / W through the LUT, layers
          -> present: DRM (CPU) or the GPU helper (vmax = W)
```

Details: `architecture/acquisition.md`, `architecture/picture.md`, and `fork/draw-architecture.md`.

Settings:

```
knob, key, touch           -> input  -> Command --+
SCPI, HTTP, MCP, VXI-11    -> remote -> Command --+-> change: apply -> Settings' -> classify | requests -> ChangeSet
setup file                 -> store  -> Commands -+                                 -> consumers
                                                                                     -> AcqWant -> acquisition thread
```

Records and measurements: `architecture/records.md`.

## Ports and adapters

| Port (trait) | Adapters | Fake for tests |
|---|---|---|
| `board::Board` and the acquisition outputs | `board/dho` (hal) | `board::fake`: scripted batches, frames, readings |
| `board::dho::Device` (configure, arm, wait, read, read again, disarm) | the DHO hardware | scripted device for `AcqLoop` host tests |
| `hal::dho::bus::Bus` | `Bar1` | `Recorder` (exists) |
| `display::Present` | DRM dumb buffers, the GPU helper | `display::fake`: keeps the last layers for picture comparison |
| `input::Source` | panel UART, touch evdev | scripted events |
| `clock::Clock` | the system clock | a stepped clock |
| `instruments::Bulk` | usbfs | scripted replies (exists in `usbtmc`) |
| `protocols::Protocol` | the decoder modules | `protocols::example` |

A fake lives next to its trait, in a `fake` module or under `#[cfg(test)]`, so a test of any stage runs without the
unit.

No port where `std` already has the abstraction. Remote transports are generic over `Read + Write` (a `TcpStream`,
the FIFO, or an in-memory stream in tests). The slow-client test uses a fake stream that delivers bytes slowly; it is
a test helper, not a port. `store` takes paths and `Read`/`Write`; its tests use a temporary directory.

## Error handling

- **Typed errors.** Domain functions return typed errors: one enum per module (`CmdError`, `AcqError`,
  `StoreError`), with `Display` for the UI and remote.
- **Nothing swallowed.** No error is swallowed. No `unwrap` or `expect` on input or IO.
- **Errors go back to their sender:** the SCPI error queue, the HTTP status, a toast.
- **Hardware errors.** The acquisition thread retries them with bounded backoff, reports each in `Readings::status`,
  and never blocks the main thread.
- **Broken invariants.** They panic in debug builds and tests. In release they log one line and recover to a safe
  state. Each stage has `reset()`, which returns it to its start state (the accumulator: empty maps; records: empty
  store; the acquisition client: re-publish `AcqWant`), and a test that a stage after `reset()` behaves as a new one.
- **Locks.** One helper takes a guard with `lock().unwrap_or_else(PoisonError::into_inner)` (`adr/0012`).
- **Log format.** One line per event, `oos-ui <area>: <what> <values>`, to stderr and `/tmp/oos-ui.log`.

## Testing

| Level | What | Where |
|---|---|---|
| unit | settings groups, commands, `classify`, kernels, measurement functions | in the module, pure |
| host | stages and `Session` with fakes; the step pipeline end to end on the Demo source | `ui/tests/` |
| golden pictures | the composed picture per display mode and time base against reference PNGs, with a tolerance | `ui/tests/golden/` |
| perf guards | ratios against reference paths | `tools/perf-guard.sh` |
| GPU | the CPU picture against the GPU picture | `gpu/test-host.sh` |
| unit lab | the acceptance set on the scope | `tools/oos-stress/`; criteria in `fork/draw-architecture.md` |

Every bug fix comes with a test that fails before the fix. Tests pin behaviour, not internals.

## Performance budgets

| Budget | Value |
|---|---|
| screen step on the first A72 | p99 <= 15 ms; merge and window <= 2 ms; render and arena copy <= 6 ms |
| frame rate | >= 29.5 pictures/s in every acceptance mode |
| worker time per frame on an A53 | <= 2.7 ms; load <= 70 % per little core in the worst mode, deposit, off-path decoding and `to_record`, and measurement jobs together |
| records stage on the first A72 | <= 1.5 ms per step off the fast path (adding a `RecordBatch` to the store and building the pseudo-batch); records are built on the workers |
| input to photon | the preview at the next step; the first batch under the new mapping <= 70 ms |
| memory | RSS growth <= 60 MB against dho804 32041d0 |

Measured by the pacing and cost counters in `:SYSTem:STATe?` and by unitlab.

## Enforcement

`cargo xtask check` runs in the gate (phase 1). It checks:
- **crate dependencies** from cargo metadata. The domain's direction is enforced here, by the crate split:
  ```
  crate oos-dsp ->
  crate oos-core -> oos-dsp
  crate oos-hal -> libc
  crate oos-ui -> libc oos-hal oos-dsp oos-core
  crate oos-fpgad -> libc oos-hal
  ```
- **module rules** in `xtask/data/architecture.txt`, over top-level modules of a crate (`layer ui <module> -> <allowed>`,
  `forbid ui <from> <to>`). They find every `crate::X` path and `use crate::{X, ..}` tree. They have no ratchet, so
  each rule is added by the migration step that makes the tree meet it (`overhaul-migration.md`). Target rules:
  - adapters do not name each other: `forbid ui <adapter> <adapter>` for each pair of `display`, `input`, `remote`,
    `store`, `system`, `instruments`, `protocols`;
  - stages do not name adapters: `forbid ui <stage> <adapter>` for `records`, `picture`, `compose`, `views`, `menus`;
  - views and menus name only the stages whose outputs they read: `layer ui views -> compose records picture`,
    `layer ui menus -> compose records`.
- **size limits**, below.
- **statics**: a `static` with interior mutability only on the metrics allowlist.

What the module rules cannot see, the code enforces:
- ownership, by visibility ([Enforced by visibility](#enforced-by-visibility));
- `oos_hal::dho` only inside `board/dho`, by the existing `board::boundary` lint.

## Code rules

### Formatting and lint

`rustfmt.toml` at the root and `cargo fmt --check` in the gate. Clippy with `-D warnings` and the lint set of
`adr/0009-formatting-and-lint.md`.

### Size limits

Hard limits for new code, as the checker counts them (`adr/0010-size-limits.md`):
- 400 physical lines per file under `src/`, minus items marked `#[cfg(test)]` or `#[test]`;
- 60 physical lines per function, comments included; test functions exempt;
- 12 fields per struct, named or tuple;
- a type's inherent `impl` blocks in at most 3 files of its crate.

Existing violations are on an allowlist that can only shrink (`cargo xtask ratchet`).

### Idioms

- Traits at boundaries.
- Enums for state.
- No stringly-typed settings inside the crate.
- No god objects.
- Composition over inheritance.
- Value semantics where they fit.
