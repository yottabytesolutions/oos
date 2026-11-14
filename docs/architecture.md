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

Hexagonal where it pays, plain modules where it does not.

| Layer | Contents | May depend on |
|---|---|---|
| domain | settings and their commands, change detection, `AcqIntent` (settings to a board-neutral acquisition request), the picture window and weights (`picture::window`), measurement and trigger definitions from `dsp` | `dsp`, std; never `board` |
| application | the step pipeline: stages with typed inputs and outputs, wired by `Session` | domain, port traits |
| ports | traits the application calls: board, display, input, remote transport, files, clock | domain types |
| adapters | `board/dho` (hal), DRM, GPU helper, VPU, panel UART and touch, SCPI, HTTP, MCP, VXI-11, USBTMC, setups and `system.conf`, network services | the port they implement, domain types, their device or protocol |

Rules (enforced by the boundary checker, `arch-rules.toml`; a module belongs to the layer of its longest matching
prefix, so `picture::window` is domain and the rest of `picture` is application):
- **Domain is pure.** Domain code does no IO, starts no thread, reads no clock and holds no global state. Time enters
  as an argument: the window's 67 ms age cap and pacing take `now` from the caller, which reads `clock::Clock`.
- **Adapters stay apart.** Adapters do not call each other. Two adapters that must cooperate do it through the
  application.
- **One user per outside interface.** Only `board/dho` uses `oos_hal::dho`. Only `display/` uses DRM, the GPU helper
  and the VPU.
- **Views and menus don't write settings.** They read settings and results, and change settings only by returning
  commands.

Small pure helpers (`clock`, `font`, `layout`, `json`) are plain modules; a port would cost more than it gives.

## Module map

The target layout of `ui/src`. One responsibility per module; size limits as in [Size limits](#size-limits). Today's
files are in brackets where they are split or moved.

| Module | Responsibility |
|---|---|
| `app/session.rs` | creates the stages and channels, runs one step: calls the stages in order and passes their outputs. Holds no domain state |
| `app/intake.rs` | collects the inputs of one step (input events, remote requests, acquisition events) and turns them into commands and requests |
| `app/acq_client.rs` | the UI side of the acquisition: maps `AcqIntent` to the board's `AcqWant` and publishes it, sends `AcqCmd`, takes `Frame`, `MeasRead` and `Readings` (`architecture/acquisition.md`) |
| `settings/` | `Settings` and one module per group: `channels`, `timebase`, `trigger`, `acquire`, `display`, `measure`, `analysis`, `interfaces`. Each: struct, invariants, command enum, `apply`, its part of `classify`, tests [`model.rs`] |
| `command.rs` | `Command` (top-level enum of the group enums), `CmdError`, the reply type |
| `change/` | the change stage: owns `Settings` privately and hands out `&Settings`; applies commands, normalizes; `classify`, `Request`, `ChangeSet`, `Generations`, `AcqIntent` |
| `picture/` | accumulator, grade, window and decay; `picture/deposit/`: the batch pipe, `BatchUnit`, kernels (spans, dots), workers (`architecture/picture.md`) [`drawpipe.rs`, `fastdraw.rs`, `sparse.rs`, drawing parts of `compress.rs`, `wave::Phosphor`] |
| `records/` | the record store (`last`, history, segments), measurement jobs, statistics, compression and deep-read policy (`architecture/records.md`) [`wave.rs`, `framestats.rs`, `deepmeas.rs`, parts of `ui.rs`] |
| `compose/` | the screen from layers: plot, traces from the accumulator, overlays, top bar; `gfx`, `font`, `layout` [`ui.rs` rendering] |
| `views/` | scope, windows, spectrum, eye, Bode, power, LA, XY, bus view: one component each, with its own state, inputs and drawing [`ui/analysis.rs`, `ui/eyepage.rs`, `ui/bode.rs`, `ui/la.rs`, ...] |
| `menus/` | the menu tree, items, dialogs, the layout lint, script access [`ui.rs` menus, `ui/lint.rs`, `ui/script.rs`] |
| `input/` | panel keys and knobs, touch, mapping to commands [`input.rs`, `ui.rs` input handling] |
| `remote/` | SCPI, HTTP, MCP, VXI-11, the control FIFO, `netacl`, `net6`: transports and parsers to commands and queries [`scpi.rs`, `http.rs`, `json.rs`, `vxi11.rs`, `cmd.rs`, `net.rs`] |
| `store/` | setup files, CSV, PNG, `system.conf` [`store.rs`, `sysconf.rs`] |
| `display/` | DRM output, the GPU helper, the VPU encoder [`drm.rs`, `gpu.rs`, `vpu.rs`] |
| `system/` | network services and the unit: `mdns`, `sntp`, `ssh`, `wlan`, `ethlink`, `hostname`, `time`, `update`, `backlight`, `placement` |
| `instruments/` | external instruments: the USBTMC client for an external generator [`usbtmc.rs`] |
| `board/` | the `Board` port, `Frame`, `to_record`; `board/dho/`, the DHO800/900 adapter, its acquisition thread split as in `architecture/acquisition.md` |
| `protocols/` | decoder modules behind `Protocol` (unchanged) |

`Ui` (today 130 fields in `ui.rs`) does not move into `Session`. Its state is divided among the stages and components
above, each the single writer of its part.

## State and ownership

Every piece of state has one writer. Readers get it by value, by `&` within the step, or as a message to another
thread.

| State | Writer | Readers | Lifetime |
|---|---|---|---|
| `Settings` | the `settings` groups, through `Command`s applied in the change stage | every stage, by `&` | process; saved by `store` |
| `ChangeSet` | the change stage | stages after it in the same step | one step |
| `Generations` (`gen`, `epoch`) | `change` (`new_gen` from `classify`, `new_epoch` from request resolution) | acquisition client, records, accumulator | process |
| `AcqIntent` with `gen` and `epoch` | the change stage | `app/acq_client` | one step |
| `AcqWant` (board-specific) | `app/acq_client`, through `Board::want` | acquisition thread, latest value | until the next change |
| hardware, batch buffers, `AcqLoop` state | acquisition thread (`board/dho`) | nobody; results leave as `Batch`, `Frame`, `MeasRead`, `Readings` | process |
| `Readings` (status, counters, acknowledged ids, the read plan) | acquisition thread | UI, latest value | process |
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

`arch-rules.toml` covers what visibility cannot: which modules may import which (layers, adapters, exclusive users).
The checker scans every path in the source, including fully qualified paths inside expressions, not only `use`
lines.

## Settings and commands

`Settings` is one owned struct of grouped sub-structs. Change detection needs one snapshot of everything; ownership
per area comes from the group modules.

Each group module (`settings/trigger.rs`, ...) holds:
- the group struct, private fields, `pub` accessors, setters visible only to `crate::change`;
- its invariants within the group, checked in `apply` (ranges, combinations of its own fields);
- its command enum (`TriggerCmd::Level(f32)`, `TriggerCmd::Slope(Slope)`, ...);
- `apply(&mut self, cmd, &Context) -> Result<(), CmdError>`. `Context` is read-only: the model's limits and the values
  of other groups the check needs;
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

The change stage applies one step's commands in a fixed order of sources: store, then remote, then input. Within a
source, arrival order. A setup load is the base; remote writes apply on top; the panel, where a person acts, applies
last and wins a conflict between two absolute values of the same field. Relative commands never conflict.

### Normalize

Cross-group constraints have one model: `Settings::normalize()`, run by the change stage after the step's commands and
before `classify`. It is the only code with `&mut` access to more than one group. It holds today's per-step rewrites:
- `clamp_offsets` (offsets within the range of the current scale);
- `fit_platform` (what the board lacks: CH3 and CH4 off on a 2-channel model, an unsupported coupling to DC, LA off
  without the LA port, sources that point at a missing input);
- clamps across groups, such as the trigger level within the source channel's range after a scale change.

Normalize runs in a fixed order and is idempotent: running it twice gives the same `Settings`. A value it changes is
part of the diff like any other, so `fit_platform` switching a channel off is an `acq` change.

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
  `Request::Epoch(token)` from an autoset or Bode stage (also a retry with the same settings). Stages return requests
  as values; the change stage takes them at the next step.
- **`UiAcq`:** UI state that changes what is acquired or drawn but is not a setting: the XY view, the protocol watch
  (both need raw records), the mask reference, autoset or Bode being active. It is not stored. Each step derives it as
  a value from the views' and menus' state (`UiAcq::of`), and the change stage diffs it against the previous step's
  value.

Classes and their consumers:

| Class | Consumers |
|---|---|
| `acq` | `AcqIntent` and `gen`; records of an older `gen` after a reconfiguration are not measured |
| `mapping` | accumulator: preview, drop maps of the old mapping key (`architecture/picture.md`) |
| `phosphor` | accumulator: maps and weights reset |
| `stats` | measurement statistics and results |
| `mask` | mask counts |
| `zone` | zone drops of the hardware zone |
| `hw_average` | the hardware average restarts |
| `history` | history and segments |
| `math` | math and FFT state that holds or averages |

A keyed cache needs no class. The decode cache is keyed by record and decoder settings, so its key decides.

### Generations

Two numbers, private to `change` (`Generations`):
- **`gen`** rises when the `ChangeSet` contains `acq`. `classify` calls `new_gen`. The acquisition thread tags every
  output with the `gen` it applied.
- **`epoch`** rises on requests whose settings may not differ: SINGLE armed, CLEAR, RUN after STOP, an autoset or Bode
  stage. Request resolution calls `new_epoch`. It replaces `hw_stamp`.
- **Tokens instead of synchronous returns.** An autoset or Bode stage runs in `views`, after `change`, and cannot
  write generations without breaking the stage order (adr/0004). It returns `Request::Epoch(token)`. The next step's
  change stage resolves it, and `ChangeSet::epochs` carries `(token, epoch)`. The stage then accepts only records of
  that epoch. Cost: one step (33 ms) before its acquisition starts, against stages that wait several batches anyway.
- The pair is published inside `AcqIntent`, so it is always consistent.
- Readers filter on the pair: the picture by its mapping key, records and measurements by `epoch` and `gen`
  (`architecture/records.md`).

### Acquisition intent

The change stage produces `AcqIntent`: what to acquire, in the domain's terms (channels, time base, depth, trigger,
modes, the raw and deep-read needs, the unit wants), with `gen` and `epoch`. It is board-neutral. `app/acq_client`
maps it to the board's `AcqWant` through `Board::want(&AcqIntent)` (`architecture/acquisition.md`). The domain never
names a board type.

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
intake     (input, remote requests, BatchUnits, RecordBatches, Frames, MeasReads, Readings)
                                                       -> commands, requests, acquired data
change     (commands, requests, UiAcq)                 -> Settings', ChangeSet, AcqIntent, AcqCmds
acquire    (AcqIntent, AcqCmds)                        -> AcqWant through Board::want, published to the acquisition thread
records    (RecordBatches, Frames, MeasReads, ChangeSet) -> records, record store, the step's pseudo-batch
accumulate (BatchUnits, pseudo-batch, ChangeSet, now)  -> picture maps and weights
measure    (records, ChangeSet)                        -> results, statistics
views      (results, records, ChangeSet)               -> view state
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
the query waits.

## Threads and messages

| Thread | Owns | Talks through |
|---|---|---|
| main | `Session` and every stage | the channels below; never blocks on IO |
| acquisition (`board/dho`, second A72) | hardware, batch buffers, `AcqLoop`, the helper's share | `AcqWant` and `AcqCmd` in; `Batch`, `Frame`, `MeasRead`, `Readings` out |
| deposit workers (A53) | their share maps; the route per batch | `BatchUnit` (fast path) or `RecordBatch` (decoded frames, off it) to main (`architecture/picture.md`) |
| measure (A53, scoped per channel) | one measurement job | job and result channels (`architecture/records.md`) |
| input | the panel UART, the touch device | `InputEvent` channel to main |
| remote listeners | sockets of SCPI, HTTP, MCP, VXI-11, the control FIFO | `RemoteRequest { command or query, reply sender }` to main |
| web PNG / send | encoding and sending pictures | the composed-picture snapshot |
| ntp, ssh, mdns, wlan | their service | status values the menus read; settings by command |

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
the FIFO, or an in-memory `Cursor` pair in tests). `store` takes paths and `Read`/`Write`; its tests use a temporary
directory.

## Error handling

- **Typed errors.** Domain functions return typed errors: one enum per module (`CmdError`, `AcqError`,
  `StoreError`), with `Display` for the UI and remote.
- **Nothing swallowed.** No error is swallowed. No `unwrap` or `expect` on input or IO.
- **Errors go back to their sender:** the SCPI error queue, the HTTP status, a toast.
- **Hardware errors.** The acquisition thread retries them with bounded backoff, reports each in `Readings::status`,
  and never blocks the main thread.
- **Broken invariants.** They panic in debug builds and tests. In release they log one line and recover to a safe
  state (picture reset, acquisition re-armed).
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
| worker time per frame on an A53 | <= 2.7 ms; load <= 70 % per little core in the worst mode |
| input to photon | the preview at the next step; the first batch under the new mapping <= 70 ms |
| memory | RSS growth <= 60 MB against dho804 32041d0 |

Measured by the pacing and cost counters in `:SYSTem:STATe?` and by unitlab.

## Code rules

### Formatting and lint

`rustfmt.toml` at the root and `cargo fmt --check` in the gate. Clippy with `-D warnings` and the lint set of
`adr/0009-formatting-and-lint.md`.

### Size limits

New code stays within:
- 400 lines per file, test modules excluded;
- 60 lines per function;
- about 12 fields per struct.

Existing oversized files and functions are on an allowlist that can only shrink (`adr/0010-size-limits.md`).

### Idioms

- Traits at boundaries.
- Enums for state.
- No stringly-typed settings inside the crate.
- No god objects.
- Composition over inheritance.
- Value semantics where they fit.
