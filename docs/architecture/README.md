# OOS architecture (target)

This is the architecture OOS is moving to on the `overhaul` branch. Where the code still differs, the migration plan
(`migration.md`) says which step closes the gap. Decisions and their reasons are in `adr/`. How to add things
is in `extending.md`. Rules for humans and agents cite the headings of this file by anchor.

Reading order: [Layers](#layers), [State and ownership](#state-and-ownership), [Change detection](#change-detection),
[Threads and messages](#threads-and-messages), [Data flow](#data-flow). The acquisition thread, the picture and the
record path have their own files: `acquisition.md`, `picture.md`,
`records.md`. Boot, kernel and the other crates: `system.md`.

## Why

Display bugs shared structural causes. Frames reached the picture through six routes with two weight models.
Batches split across pictures. Old settings reached new pictures. Measured at the start (fork commit 32041d0):

- `Model` had 178 public fields, about 103 direct writes and a string-keyed setter for SCPI and HTTP. Three separate
  mechanisms decided what reacts to a change.
- `Ui` had 130 fields over 28 files. `Ui::step` was a 289-line loop whose correctness depended on line order. The
  acquisition thread was one closure of about 890 lines.
- 47 functions exceeded 100 lines. No formatter or linter was enforced.

Goals: a fast, responsive UI without hiccups or flicker; a documented target that others can extend; rules that
tools enforce; idiomatic Rust with small units and good tests. The fork's unit branch stays frozen at 32041d0 while
the overhaul is in progress; fixes the unit needs go through the hotfix lane.

Keep the crate split, the Board port and pure DSP and HAL code. Give settings and the picture one owner each.
Use small stages, host tests and enforced boundaries to keep the UI responsive and safe to extend.
Formatting and restructuring increase upstream divergence. Future upstream PRs are self-contained proposals.
Acceptance is defined in [the drawing design](drawing.md#acceptance-criteria-unitlab).
The [task tracker](../tasks/README.md) carries guardrails, architecture review and migration work.

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
- **The `Board` port.** The UI speaks in its own terms and gets frames in volts. Two `confine` rules in
  `xtask/data/architecture.txt` keep `oos_hal::dho` inside `board/dho` and `board::dho` inside `board`.
- **Decoder modules.** The `Protocol` trait for decoder modules (`../protocol-modules.md`).
- **The rest of the system.** Boot chain, kernel, update and the network services are unchanged
  (`system.md`).
- **One dependency:** `libc`.

What changes is the inside of `oos-ui`.

M2.1 intermediate state: `oos-core` now holds the existing flat `model::Model`, its defaults and pure
methods, plus math, decoder, zone, LA and protocol settings values. UI compatibility exports and extension
traits keep existing consumers on the same behavior. The application constructs capability `Config`
values and a read-only protocol `Context`; decoder objects, registry callbacks, palette/rendering and
board access stay in `oos-ui`. Core has no callbacks into adapters, unsafe code or platform detection.
Config and Context borrow constant tables and own no heap buffers; they live for a query. The UI owns an
immutable metadata vector for the compiled protocol registry for the process lifetime (four production
entries, below 1 KiB on the host); no queue or cache is introduced. Generator-limit and comparator-hysteresis
queries remain UI adapter helpers. Its seven existing modules have layer rules and crate-wide purity lints. The `change` module and its
rule arrive in M2.2. Flat public fields and legacy edge setters remain until their planned migration;
this move does not activate normalize/classify, generations, outboxes or new reset consumers.

M2.2 intermediate state: `core/src/change/` now classifies the flat model and derived `UiAcq`,
normalizes a shadow scratch in the fixed group order, and resolves bounded value requests and stage tokens.
`ui/src/ui/change.rs` observes the legacy pre-acquisition seams and internal proposals. Its generations,
intents, mapping keys and proposed commands remain private observations; acting settings, publication,
record acceptance, clamps, marks and resets retain the legacy path. `drawpipe::Key` still refreshes workers
for trigger-only changes. The new worker key retains that complete coverage for the later switch.

Each shadow owns one previous normalized Model and one normalization scratch, each strictly below 64 KiB
including retained String/Vec capacities. Refresh uses explicit field copies and capacity-reusing clones;
capacity growth is checked before refresh. Each outbox retains at most 64 command slots and strictly less
than 64 KiB including payload capacity. Cached payloads survive changes in the active field set; overflow
returns an error. Generations occupy 24 bytes, ChangeSet less than 4 KiB, and UiAcq/DepositView less than
1 KiB each. The observer additionally retains fixed diagnostic tables for 243 classified field slots,
18 oracle rows and ten counters, plus 256 paired intake/change/shadow timing samples. A prepared
comparison holds one ChangeSet and a fixed field table below 16 KiB. The shadow point precedes legacy
auto_step; settings and tokens emitted later resolve together at the next point. Failed resolutions
retain their queued requests. No unbounded log
collection, new global state or production allocator is introduced.

`stat` and `:SYSTem:STATe?` expose all ten raw shadow counters, normalization/outbox disagreements, errors,
unreviewed differences and timing samples. Legacy intake/change budgets exclude separately reported shadow time. First-field/row traces go to stderr. The oracle distinguishes
legacy dirty clears serving mapping and persistence consumers using the effective field contract; it
never treats every broad mark as all class bits. Proposed D1-D6 differences remain review inputs, not
suppression rules. Exact host fixtures and negative controls accompany the observer. The required shadow
unit round and oracle/difference acceptance still precede any activation in M2.3.
The warmed host census covers 256 real UI steps on fake Shared without producer threads, plus
256 explicit stage captures. It measures keys/settings, changed keys, normalize/classify,
comparison, finish capture/proposal comparison and stage capture separately. The existing
core snapshot census additionally varies warmed heap payloads. Neither census establishes
zero allocations for all legacy UI work or for every input storm.

M2.3a intermediate state: the fixed-order pure normalizer now repairs the live Model before
shadow classification. Both legacy per-step clamps and the cross-group zoom rewrites are
removed from their old seams.
Analyzer source snaps remain at intake ahead of job submission through M2.3e, so an edit
that selects an off channel submits only the repaired source and does not reach the broad bridge.
`normalize` reports an effective repair without a
new retained snapshot or allocation; one `mark_changed` bridges that pass to the remaining legacy consumers.
The existing shadow scratch captures pre-normalize values to restrict D3 diagnostics to the
classes of fields actually repaired; a row alone grants no allowance.
It marks the picture only in RUN and clears statistics in RUN and STOP. Idempotent passes do
neither. Keep the bridge through M2.3d; its picture side stops acting in d and its statistics
side remains until e. Classification, generations, publication, outboxes and the remaining reset
consumers still use the M2.2 observation/legacy arrangement. Normalization equality is checked
at the pre-stage change point, including zoom and analyzer sources, before later internal writes.
DEFAULT deliberately clears previous-setup statistics, including repeated equal-value defaults.
Cursor mode is display-only; it does not reset temporal views. Mask enable/reference publication
alone does not change acquisition identity; mask source is an acquisition field because it
selects the hardware mask channel. The owner decided D9: advance the generation once,
drop in-flight records mask-tested against the old channel, and reset mask statistics.
Legacy does not advance its generation on this edit. The observer permits only extra new
ACQ and MASK at an independently observed mask-source change, including demo sources;
acting generation/filter and reset consumers still switch in M2.3b/e. Effective worker-route
or raw/pair read-format changes also change acquisition identity. A hardware mask can leave
workers enabled while requiring raw input,
so the adapter supplies the effective compression factor as a small value in UiAcq.
The existing analyzer average/waterfall reset reports MATH|VIEW: held FFT averages belong
to math and waterfall rows to view state. Spectrum state settings classify MATH|VIEW, eye and power
settings VIEW; these analyzer edits neither classify STATS nor clear measurement statistics.
The observer retains 18 fixed oracle rows.
The allocation census includes the live normalizer and bridge as well as shadow work.

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
- **No hidden global state.** A `static` with interior mutability (atomics, `Mutex`, `Cell`, `RefCell`) is allowed
  only for metrics, on an allowlist the checker enforces. A lazy `static` (`OnceLock`, `LazyLock`) whose value type has
  no interior mutability, such as the demo signal caches in `wave.rs`, is exempt: it is a constant computed late. Today's `pacing` state and `deepmeas::FORCE` move into the
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
| `change/settings/` | `Settings`, `normalize`, and one module per group: `channels`, `timebase`, `trigger`, `acquire`, `display`, `measure`, `analysis`, `interfaces`. Each: struct, invariants, command enum, `apply`, its `classify` function, tests [`model.rs`] |
| `command.rs` | `Command` (top-level enum of the group enums), `CmdError`, `Query`, the reply type |
| `change/` | the change stage, with `settings/` inside it so visibility can name it: owns `Settings` privately and hands out `&Settings`; applies commands in merge order, normalizes, classifies; `Request`, `ChangeSet`, `Generations`, `MappingKey` construction, `AcqIntent` with the `route` tag, `DepositView` |
| `acq.rs` | the acquisition data types: `BatchMeta`, `Frame`, `MeasRead`, `Record`, `RecordBatch` headers (`acquisition.md`) |
| `memplan.rs` | `memplan(&Settings, &Caps) -> MemPlan`: bytes per acquisition buffer class; pure |
| `window.rs` | the picture model: window bounds (N, the 3-batch cap, the age cap), decay in 16.16, weights; pure, `now` as argument |
| `config.rs` | `Config` and `Config::parse(&str, env pairs, &Platform)`: pure. `oos-ui` reads `system.conf`, the environment and the platform once at start and passes them in |

`oos-ui`:

| Module | Responsibility |
|---|---|
| `app/` | `session.rs`: creates the stages, the channels and the deposit pipe, runs one step, holds no domain state. `intake.rs`: collects one step's inputs into commands, requests and acquired data, and owns `Layout`. `acq_client.rs`: maps `AcqIntent` to the board's `AcqWant` through `Board::want`, publishes it and `DepositView`, sends `AcqCmd`, takes `Frame`, `MeasRead`, `Readings`. `outbox.rs`: internal commands and requests for the next step |
| `picture/` | accumulator (`acc.rs`, using `oos_core::window`), grade; `picture/deposit/`: the batch pipe, `BatchUnit`, kernels (spans, dots), workers (`picture.md`) [`drawpipe.rs`, `fastdraw.rs`, `sparse.rs`, drawing parts of `compress.rs`, `wave::Phosphor`] |
| `records/` | the records stage: record store (`last`, history, segments), measurement jobs, statistics, compression and deep-read policy (`records.md`) [`wave.rs`, `framestats.rs`, `deepmeas.rs`, parts of `ui.rs`] |
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
| `board/` | the `Board` port (`want`, capabilities, the thread's channel ends), `to_record`; `board/dho/`, the DHO800/900 adapter, its acquisition thread split as in `acquisition.md` |
| `protocols/` | decoder modules behind `Protocol` (unchanged) |

`Ui` (today 130 fields in `ui.rs`) does not move into `Session`. Its state is divided among the stages and components
above, each the single writer of its part.

## State and ownership

Every piece of state has one writer. Readers get it by value, by `&` within the step, or as a message to another
thread. Every piece of state has a bound in bytes. A change that adds state without a bound in this table does not
merge ([Memory and queues](#memory-and-queues)).

| State | Writer | Readers | Lifetime | Bound (bytes) |
|---|---|---|---|---|
| `Settings` | the change stage, through `Command`s and `normalize` | every stage, by `&` | process; saved by `store` | < 64 kB |
| `Config` | read once at start | every stage, by `&` | process | < 16 kB |
| `ChangeSet` | the change stage | stages after it in the same step | one step | < 4 kB |
| pending `ChangeSet` (light steps since the last full step) | the change stage | the next full step, which clears it | until the next full step | < 4 kB |
| `Pacing { next_step, period }` | main | acquisition (`pacing::Fill`), latest value | process | < 64 B |
| `Generations` (`gen`, `epoch`, `stats_gen`) | `change` (`new_gen` from `classify`, `new_epoch` from request resolution, `stats_gen` on the `stats` class) | every stage, as values in `ChangeSet` | process | 24 B |
| outbox of internal commands and requests | the stage that emits them | the change stage, next step | one step | 64 entries |
| `AcqIntent` with `gen` and `epoch` | the change stage | `app/acq_client` | one step | < 4 kB |
| `AcqWant` (board-specific) | `app/acq_client`, through `Board::want` | acquisition thread, latest value | until the next change | < 4 kB |
| hardware, batch buffers, `AcqLoop` state | acquisition thread (`board/dho`) | nobody; results leave as `Batch`, `Frame`, `MeasRead`, `Readings` | process | `memplan` classes `dma`, `raw`, `ring`, `segments` |
| `DepositView { mapping }` | the change stage, through `app/acq_client` | deposit workers, at first claim of a batch | until the next mapping change | < 1 kB |
| `Readings` (status, counters, acknowledged ids, the read plan, `k`, the applied `(gen, epoch)`) | acquisition thread | UI, latest value | process | < 4 kB |
| the deposit pipe | `Session` (created and owned) | workers by claim, main by `BatchUnit` and `RecordBatch` | process | its buffers are the `dma` class; queue count = pool size |
| a batch's share maps | the worker or helper drawing that share | the accumulator, after the batch is published | one batch | workers x 2 maps x plot pixels x 4 B |
| accumulator maps (`ph`, `phz`), weights, window | `picture` accumulator | `compose`, by `&` | until a `ChangeSet` resets them | window <= 3 units of maps, plus 2 persistence maps |
| records, the last record, history, segments | `records` | measurement, views, store, remote queries | per class | `HISTORY_MAX` x screen record; raw `Arc`s as in [Retaining owners](#retaining-owners) |
| measurement results and statistics | `records` measurement | views, remote queries | per class | < 64 kB |
| view state (spectrum, eye, Bode, LA) | its view component | `compose` | until the view closes or its class resets | per view, its own constant (`EYE_MAX`, `SPECTRUM_MAX`) |
| `Layout` (panes, plot areas, HDMI, menu width) | intake, from layout events and requests | change (`UiAcq::of`), menus, views, compose, by `&` | process | < 4 kB |
| menu state (open menu, focus, dialogs) | `menus` | `compose` | process | < 64 kB |
| system settings (`system.conf`) | `store::sysconf`, through `interfaces` commands | `system` services | file | file, < 16 kB |
| a network service's state | that `system` service | its menu page | process | per service, < 1 MB |
| picture snapshot leases | `present` (the helper's readback, on demand) | web PNG thread, by lease | until returned, at most 1 s | 2 pictures |

### Enforced by visibility

Ownership is enforced by the compiler, not by a checker rule:
- The change stage holds `Settings` in a private field and hands out `&Settings`. The settings groups live inside
  the change module (`change::settings`), so their setters and `apply` are `pub(in crate::change)`: nothing outside the
  change stage can call them, in `oos-core` or in `oos-ui`. `oos-core` re-exports the read-only types
  (`pub use change::settings::Settings`).
- `Generations` is private to `change`; other stages see `gen` and `epoch` only as values in `ChangeSet` and
  `AcqIntent`.
- `ph`, `phz`, W and the window are private fields of the accumulator, exposed only as `&Picture`.
- Owned types have no `pub` fields.

The checker covers what visibility cannot: which modules may name which ([Enforcement](#enforcement)). It finds
fully qualified paths inside expressions as well as `use` lines.

Rules that neither visibility nor the module rules can see get a mechanical check, so none is left to review alone:
- `oos-core` has clippy `disallowed_methods` and `disallowed_types` for `std::env`, `std::fs`, `std::net`,
  `Instant::now`, `SystemTime::now` and `thread::sleep`, crate-wide. In `oos-ui` the same list applies to the stages;
  only the adapters and `clock` may call them, by an `#[allow]` with a reason at module level. The gate counts these
  allows per crate, shrink-only (G1), so a new one shows in review.
- `Config` and `OnceLock` reads of `system.conf` in policy code (today `hw.rs` 604, 718, 1141) are caught by the same
  `std::fs` rule once they move behind `Config` (M5).
- The module rules cover external crate paths. Two `confine` rules hold the board boundary: `oos_hal::dho` only in
  `board::dho`, `board::dho` only in `board` (G1, `migration.md`, `docs/development.md`).
- A snapshot test lists every field classified `Unrelated`, so adding one shows in review.

## Settings and commands

`Settings` is one owned struct of grouped sub-structs. Change detection needs one snapshot of everything; ownership
per area comes from the group modules.

Each group module (`change/settings/trigger.rs`, ...) holds:
- the group struct, private fields, `pub` accessors, `pub(in crate::change)` setters;
- its invariants within the group, checked in `apply` (ranges, combinations of its own fields);
- its command enum (`TriggerCmd::Level(f32)`, `TriggerCmd::Slope(Slope)`, ...);
- `apply(&mut self, cmd, &Context) -> Result<(), CmdError>`. `Context` is read-only and narrow: the model's limits
  and, per group, a few named accessors of other groups' values (`ctx.channel_exists(ch)`), never a whole `&Settings`.
  `Context` is only for rejecting a command (the source channel does not exist on this model). Clamping is
  `normalize`'s job, so a command never changes another group;
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
last and wins a conflict between two absolute values of the same field. Relative commands never conflict. Four
sources stay, because each has its own reply path and error semantics; collapsing them would lose who to answer. A remote
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
- clamps across groups, such as the trigger level within the source channel's range after a scale change;
- the memory depth, Auto included, clamped so that `memplan` stays within `Config.mem_budget`, if the `memplan`
  sweep shows any setting can exceed it ([Memory plan](#memory-plan)).

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
  destructure as the settings fields. It is not stored. Each step derives it as a value from the views' and menus'
  state (`UiAcq::of`), and the change stage diffs it against the previous step's value.
- **Layout.** The screen layout (panes, plot areas, HDMI output, the menu's width) is its own value, `Layout`, owned
  by intake. Intake applies this step's layout events before the change stage runs. `UiAcq::of`, menus, views and
  compose read `&Layout`; none of them writes it. A menu that wants a wider area returns a layout request to intake
  for the next step. So `UiAcq::of` sees this step's plot sizes, and no component derives its own copy.

Classes and their consumers. A setting can be in more than one class: V/div is `acq` (the AFE gain changes) and
`mapping` (traces move on screen). Each consumer, views included, declares the classes it reacts to next to its state
(`const RESETS_ON: Classes`), and a test checks every view against it.

| Class | Consumers |
|---|---|
| `acq` | `AcqIntent` and `gen`; records of an older `gen` after a reconfiguration are not measured |
| `mapping` | accumulator: preview, drop maps of the old mapping key (`picture.md`) |
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

Three numbers, private to `change` (`Generations`):
- **`gen`** rises when the `ChangeSet` contains `acq`. `classify` calls `new_gen`. The acquisition thread tags every
  output with the `gen` it applied.
- **`epoch`** rises on requests whose settings may not differ: SINGLE armed, CLEAR, RUN after STOP, an autoset or Bode
  stage. Request resolution calls `new_epoch`. It replaces `hw_stamp`.
- **`stats_gen`** rises with the `stats` class. Measurement jobs carry it; a late result of an older `stats_gen` is
  dropped (`records.md`).
- **Tokens instead of synchronous returns.** An autoset or Bode stage runs in `views`, after `change`, and cannot
  write generations without breaking the stage order (adr/0004). It returns `Request::Epoch(token)` through the outbox.
  The next step's change stage resolves it, and `ChangeSet::epochs` carries `(token, epoch)`. Cost: one step (33 ms)
  before its acquisition starts, against stages that wait several batches anyway. Rules:
  - **Commands first, same step.** The settings commands a stage emits with its token (new time base, generator
    frequency) are applied in the same change step, before the epoch. `AcqIntent` carries the new `gen` and the new
    `epoch` together, so no record of the old settings carries the new epoch.
  - **An epoch-only change is an acquisition boundary.** The acquisition thread drops a batch armed ahead and queued
    frames of the older epoch, as for `gen` (`acquisition.md`, `apply`).
  - **A token without an epoch accepts nothing.** Until its `ChangeSet` arrives, the stage takes no records.
  - Tokens are unique per request (requester, run id, stage). Several requests in one step may share one epoch.
- The pair is published inside `AcqIntent`, so it is always consistent.
- Readers filter on the pair: the picture by its mapping key and `epoch`, records and measurements by `epoch` and `gen`
  (`records.md`).

### Acquisition intent

The change stage produces `AcqIntent`: what to acquire, in the domain's terms (channels, time base, depth, trigger,
modes, the raw and deep-read needs, the unit wants), with `gen` and `epoch`. It names no board type, but it describes the DHO800/900 family's acquisition model
(batches, pairs, roll); another family needs an ADR that generalises it. `app/acq_client`
maps it to the board's `AcqWant` through `Board::want(&AcqIntent)` (`acquisition.md`). `Board::want` is
pure: the same intent gives the same `AcqWant`. The domain never names a board type. `AcqIntent`
also carries the `route` tag for the deposit pipe (fast path or records). A route change is an `acq` change, so it
always comes with a new `gen`. The thread does not interpret the tag; it copies it into the `BatchMeta` of every batch
it arms, so a batch's route, `gen` and `epoch` come from one cell written once. `gen` and `epoch` stay the only change
signals.

The mapping is not a tag. The change stage publishes `DepositView { mapping }`, a latest-value cell with the current
`MappingKey` and its values, through `app/acq_client`. A worker reads it when it first claims a batch, and all shares of
that batch use that mapping (`picture.md`). It carries no change signal: a batch drawn under a newer
mapping than it was armed with is what the user wants to see, and the accumulator drops units whose key is not the
current one at merge. This cell is why `PipeView` is not needed: the route went into `AcqWant`, and the mapping alone
needs no consistency with `gen`.

### Run state

Run state goes by transition, not by value:
- `run` and the trigger sweep together with `run` are not settings changes.
- Each consumer acts on the run-state transition of the step. RUN to STOP keeps the picture; STOP to RUN restarts the
  average.
- A mapping change in STOP redraws from the last record and never blanks.

## Step pipeline

`Session::step` runs fixed stages. Each stage owns its state, takes typed inputs and returns typed outputs. The order
is in the code; no stage relies on a side effect of another.

Two kinds of step run the same stages in the same order ([Main loop and pacing](#main-loop-and-pacing)): the full
step on the vblank clock runs all of them; the light step on an input or remote event runs intake (input and remote
only), change, acquire and reply. The change stage keeps the light steps' `ChangeSet`s pending until the next full
step consumes them.

```
intake     (input, remote requests, outbox, BatchUnits, RecordBatches, Frames, MeasReads, Readings)
                                                       -> commands, requests, acquired data
change     (commands, requests, UiAcq)                 -> Settings', ChangeSet, AcqIntent, AcqCmds
acquire    (AcqIntent, AcqCmds)                        -> AcqWant through Board::want, published
records    (RecordBatches, Frames, MeasReads, ChangeSet) -> records, record store, pseudo-batch, Replace
accumulate (BatchUnits, pseudo-batch, Replace, ChangeSet, now) -> &Picture
measure    (records, ChangeSet)                        -> results, statistics
views      (results, records, events, ChangeSet)       -> view state, outbox (commands, requests for the next step)
compose    (picture, results, views, menus)            -> layers and trace jobs
present    (layers)                                    -> DRM or the GPU helper; snapshot for remote clients
reply      (results)                                   -> answers to remote queries
```

`records` runs before `accumulate` because off the fast path the picture is drawn from records: the step's
records form one pseudo-batch (`picture.md`). History and segments are part of the record store, so
`records` updates them, not `measure`.

A remote query is answered in `reply` from that step's state. Each query carries a typed wait condition: `After {
gen, epoch }`. Each connection keeps its own `(gen, epoch)`: the values the change stage returned with the reply to
that connection's last command. A query copies it. Another connection's commands never delay it. `:SINGLE; :WAV:DATA?` therefore waits
for a record of the SINGLE's epoch and never returns the old record. `*OPC?` uses the same condition: it answers when
`Readings::applied` reaches the connection's last `gen` and `epoch`. The transport thread holds the connection while
the query waits.

A waiting query never hangs silently, but it has no fixed server-side timeout: a SINGLE may legitimately wait hours for
its trigger. It ends on the first of:
- the condition is met: the answer is sent;
- the connection closes: dropped, nothing queued (a client-side timeout that closes the socket ends here);
- a device clear (VXI-11 `device_clear`, `*CLS`), or `:STOP` or `:RUN` from any source that makes the condition
  unreachable: cancelled, `-410` (query interrupted) queued;
- a new command on the same connection before the answer was read: cancelled, `-410` queued, as SCPI prescribes;
- a VXI-11 read: its own `io_timeout` from the call, then `-410`.

## Threads and messages

| Thread | Core | Policy | Owns | Talks through |
|---|---|---|---|---|
| main | first A72 | SCHED_OTHER | `Session` and every stage | the channels below; never blocks on IO |
| GPU helper (process) | first A72, with main, within its budget; the unit round of M5c tries an A53 | SCHED_OTHER | nothing; reads the arena | the arena, its socket and fences |
| acquisition (`board/dho`) | second A72 | SCHED_OTHER | hardware, batch buffers, `AcqLoop`, the helper's share | `AcqWant` and `AcqCmd` in; `Batch`, `Frame`, `MeasRead`, `Readings` out |
| deposit workers | A53, one per little core | SCHED_OTHER | their share maps; the route per batch from its `BatchMeta` tag, the mapping from `DepositView` at first claim | `BatchUnit` (fast path) or `RecordBatch` (records, off it) to main (`picture.md`) |
| measure | A53, persistent, one helper per channel | SCHED_OTHER | one measurement job | job and result channels (`records.md`) |
| input | A53 | SCHED_OTHER, blocked on read | the panel UART, the touch device | `InputEvent` channel to main |
| remote listeners | A53 | SCHED_OTHER, in `epoll_wait` | sockets of SCPI, HTTP, MCP, VXI-11, the control FIFO; encoding of large answers | `RemoteRequest { command or query, wait, reply sender }` to main |
| web PNG / send | A53 | SCHED_IDLE (nice 10 where refused) | encoding and sending pictures | the composed-picture snapshot |
| ntp, ssh, mdns, wlan | A53 | SCHED_OTHER | their service | status values the menus read; settings by command |

### Waiting

Every thread blocks in exactly one call, on its event sources and one deadline (adr/0016). No thread sleeps in a loop.
A hardware poll remains only where the FPGA raises no event; it runs at the hardware's period inside the same call.

| Thread | The one blocking call | Wake sources | Worst-case wake latency | Hardware poll (period, cost) |
|---|---|---|---|---|
| main | `epoll_pwait2` | vblank sequence event and page-flip event (DRM fd); wake `eventfd` (a `BatchUnit`, a `RecordBatch`, a remote request, `Readings` changed, the outbox of a worker); input fds; GPU helper reply socket; a `timerfd` deadline only without a display (host, headless) | an input or remote event: under 1 ms to the light step; a picture: the next flip | none |
| acquisition | `ppoll` on its command `eventfd` with a timeout of the hardware period (sub-millisecond, so `ppoll`, not `poll`) | `AcqWant` changed, `AcqCmd` queued, cancel | notification under 1 ms outside a C2H read; inside one, the rest of that read chunk ([Acquisition reaction](#acquisition-reaction)) | trigger done (BAR1 `0x4080` bit 30): every 500 us while armed; ring count during a fill: every 200 us, or helper work between reads; each read is one MMIO load |
| deposit workers | channel receive (`sync_channel`) | a claimable batch, shutdown | under 50 us | none |
| measure | channel receive | a job, shutdown | under 50 us | none |
| input | `ppoll` | panel UART, touch evdev, shutdown `eventfd` | under 1 ms | none |
| remote listeners | `epoll_wait` per listener thread | accept, readable, writable (`EPOLLOUT` while an output queue is non-empty), reply ready `eventfd`, shutdown | under 1 ms | none |
| web PNG / send | channel receive | a snapshot ready, shutdown | not latency-bound | none |
| GPU helper | `poll` on its socket and the GPU fences | a submission, a fence signalled, shutdown | under 1 ms | none |
| ntp, mdns, wlan | `poll` | their sockets, rtnetlink link events, supplicant events, config `eventfd`, protocol deadlines (`timerfd`) | protocol-bound | none |

Measured on the unit and shown in `:SYSTem:STATe?`: per thread its wakes per second and the p99 time from wake source
to handling. unitlab asserts the latency column; an idle scope in STOP wakes main only on input and the vblanks it
needs. The latency column is provisional until [Platform tuning](system.md#platform-tuning) records the idle states,
governor and IRQ affinity it depends on.

The FPGA raises no usable user interrupt for trigger done, ring count or roll chunks: stock firmware polls BAR1
(`0x4080` bit 30) and then does one blocking C2H read, with the driver on legacy INTx and MSI off (RE, `regmap`;
the same INTx mode fails under OOS's kernel, [DMA completion](#dma-completion)).
So those two polls stay, but inside `ppoll(eventfd, period)` (`poll` takes milliseconds; the 200 and 500 us periods
need `ppoll`, `epoll_pwait2` or a `timerfd`): a command interrupts them at once, and an armed scope in
STOP costs nothing. The 10 us sleep after every BAR write (`bus.rs` 167) lasts 60 to 70 us today, because a
SCHED_OTHER thread has 50 us of timer slack. `oos-ui` sets `prctl(PR_SET_TIMERSLACK, 1)` at startup, before it starts
any thread, so every sleep and timeout is as long as asked. M3c measures the minimum spacing the FPGA needs and sets
the gap to it, per write list where the hardware allows it.

### Acquisition reaction

Four different things, with different bounds:

| What | Bound | Why |
|---|---|---|
| notification: a command reaches the thread | under 1 ms outside a C2H read | `ppoll` on the command `eventfd` with a timespec computed from the next absolute deadline (`CLOCK_MONOTONIC`), recomputed after every command and after `EINTR`, so waits do not drift |
| application: the new `AcqWant` is applied | the end of the current read chunk, or the next hardware check | a C2H read cannot be interrupted: the driver's character device has no `.poll`, and a read blocks until the transfer completes or the driver times out (10 s) |
| STOP completion: the batch in progress is finished | its fill plus its read, plus the final raw read if a `Final` asks | STOP is a level: the batch armed when it arrives completes, as the stop-exact design requires |
| shutdown cancellation | one read chunk: 4 MiB (`MAX_READ`) at the measured rate, 17 ms at 240 MB/s; a stalled read ends at the driver timeout | the loop checks the cancel flag between chunks; shutdown's 2 s join bound covers a chunk, and a stalled read is left to process exit ([Cancellation](#cancellation)) |

- Helper work between hardware checks is bounded: the helper deposits one frame at a time and checks the ring count
  between frames, so a check comes at least every frame deposit (budget 2.7 ms).
- The chunk time is a counter (`acq chunk`, p99) with a budget, so a slower link or a larger chunk shows as a failed
  budget, not as a slow STOP. A smaller `MAX_READ` lowers the bound at a cost per read that M5c measures.

### DMA completion

| Boot | C2H completion | Where | Shown as |
|---|---|---|---|
| cold boot | MSI (`poll_mode=0`, today's unit config) | on an A53: IRQ 78 on cpu1, measured | `xdma=msi cpu1` |
| kexec start | `poll_mode=1`, unless M5c finds a working alternative | the completion threads, bound by the driver to cpu0 and cpu1 (A53s) | `xdma=poll cpu0,1` |

- **IRQ placement is explicit.** GICv3 routes a wired device IRQ to cpu0 by default, so rcS writes
  `/proc/irq/<n>/smp_affinity_list` for the DRM, GPU, network and USB IRQs and checks the xdma MSI ([Platform
  tuning](system.md#platform-tuning)). Measured 2026-11-15: the xdma MSI is IRQ 78 on cpu1, at 31 interrupts/s at setup
  B and 8/s at 4 channels 10 M.
- **Why not MSI after kexec.** The GIC-500 cannot disable LPIs once enabled, and without an EFI boot the next kernel
  cannot reuse the old LPI tables, so MSIs never arrive after kexec (inferred; the symptom is verified).
- **Why not INTx.** INTx (`poll_mode=0 interrupt_mode=2`) failed on the unit on 2026-11-15: the legacy line came up as
  IRQ 76 on chip `dummy` with about 230,000 interrupts/s on cpu0, 10 C2H time-outs in 2 min, 3.8 fps (log:
  `oos_tuning_20261115.md` in the unit repo). The RK3399 PCIe legacy interrupt routing in this kernel does not deliver
  a usable line. INTx is rejected as configured; it needs a PCIe legacy IRQ fix first.
- **Why not poll.** The poll threads are bound to cpu0 and cpu1 and burn an A53 per transfer, competing with the
  deposit workers.
- A unit round (M5c) measures CPU per read, read latency and time-outs for MSI on cold boot and poll mode after
  kexec, and looks for a working kexec alternative to poll mode. The poll threads cost an A53 per transfer until one
  is found.

### Main loop and pacing

The display paces the steps (adr/0016):
- **Vblank, not flips.** Main queues a CRTC sequence event (`drmCrtcQueueSequence`) for every vblank and receives it in
  its `epoll_wait`. The vblank clock is independent of whether a flip completed, so a late composition never shifts the
  step clock. At a 60 Hz refresh a full step starts on every second vblank (30 Hz); at 30 Hz on every vblank.
- **First picture after idle.** When main was idle (STOP, nothing changed) and something changes, the full step runs
  at the next vblank, not the next even one; the 30 Hz phase restarts from there.
- **Without vblanks.** With the display off, HDMI unplugged as the only output, or on the host, a `timerfd` with
  absolute 33.3 ms deadlines stands in. Switching between the two keeps the step count continuous.
- **The phase goes to the acquisition.** Main publishes `Pacing { next_step, period }` (the time of the next full
  step and the step period) in a latest-value cell. `pacing::Fill` in the acquisition thread ends a ring fill so that
  the batch is ready before the next step.
- **Outputs.** The panel and HDMI share one DRM fd. Each output has its own id (CRTC), its own vblank subscription,
  its own buffer states and its own pending flip; events carry the CRTC id and are accounted per output. The step
  clock follows the panel. With HDMI as the only output, it follows HDMI's vblank.
- **Startup.** Main sets the mode, submits a first picture (the splash or a blank), and queues the first vblank
  sequence event before it enters `epoll_wait`, so the clock runs from the first iteration.
- **Liveness.** If no vblank event arrives within 3 periods (a disconnect, a disabled CRTC, a missing event), main
  switches that output to the `timerfd` clock and counts it; the next vblank event switches it back. If the GPU
  helper dies, presentation falls back to the CPU path ([GPU helper death](#gpu-helper-death)) and the clock keeps
  running on vblanks. A failed flip releases its buffer and is counted; it never stops the clock.
- **A flip never blocks main.** Main submits a picture and returns to `epoll_wait`. It composes the next only when a
  free buffer exists (double buffering with the GPU helper's fenced pool, triple where the budget allows); otherwise it
  skips that step's compose and counts it. Per output a buffer is in one of four states: free, being rendered, pending
  flip, scanned out ([GPU work proportional to change](#gpu-work-proportional-to-change)). Today's `wait_flip` with a 100 ms timeout (`drm.rs` 390) goes.
- **Light step between full steps.** An input event or remote request wakes main at once and runs a light step:
  intake of input and remote requests only, change, acquire, reply. It does not drain `BatchUnit`s or `RecordBatch`es,
  and it runs no records, accumulation or compose. A settings round trip (SCPI write, reply) stays under 2 ms; the
  picture follows at the next full step.
- **Pending changes.** The change stage keeps a pending `ChangeSet`. A light step merges its own into it: the union of
  the classes, the newest generations, all `(token, epoch)` pairs, and the run-state transitions as an ordered list
  (STOP then RUN stays two transitions, not none). The next full step consumes the pending set and clears it, so every
  consumer that runs only in full steps (records, accumulator, views) sees every change since its last step.
- **Generation barrier.** A full step applies the pending set to its consumers before it takes any unit, record or
  result, so data of a generation older than the pending `gen` or `epoch` is filtered as stale, not drawn first and
  reset after. Host tests: a mapping change in a light step gives a preview at the next full step; four light steps
  (a mapping change, CLEAR, STOP, RUN) before one full step deliver both classes, both transitions in order and the
  newest generations.
- **Service points.** A full step can take up to 15 ms, so main does not wait for its end to serve requests. Between
  stages it checks its reactor without blocking (`epoll_pwait2` with a zero timeout). Pending input and remote
  requests get a light-step slice within a quota (at most 8 requests, 64 kB of input and 0.5 ms per service point),
  then the full step continues where it was. No stage runs longer than 2 ms between two service points; a stage that
  would (accumulate on HDMI, compose) has service points inside it, at row-band boundaries.
- **One snapshot per step.** A full step takes a snapshot of `Settings` (and `Layout`) at its change stage, and every
  stage of that step reads the snapshot, not the live value. A light slice at a service point changes the live
  `Settings` and the pending `ChangeSet` only; the running full step does not see it, and the next full step does.
  A query answered in a slice reads the live value. So a picture's labels always match its trace. The snapshot is the
  `&Settings` the change stage hands out for the step (adr/0001, adr/0004); a clone of a few kB.
- **Fairness.** A full step always finishes before the next one starts, whatever the remote load: light work per full
  step is capped by the quotas. Requests beyond a quota wait for the next service point, in arrival order. A full step
  never delays a request by more than one service interval (2 ms) plus its slice.
- **No busy wait.** The last-millisecond loop of today's pacing (`main.rs` 563, 610) goes with the timer.

### Hand-offs

The default is the standard library: a bounded `sync_channel`, or a `Mutex` cell around a small copy, with an
`eventfd` wake where the reader must react within a step. Each hand-off has a contention counter (lock waits and wait
time, per thread). A hand-off moves to a lock-free primitive only when its counter shows contention on the unit, and
the lock-free primitives (SPSC ring, triple buffer, seqlock) live in exactly one reviewed module, `oos_hal::sync`, with
their own tests, Miri runs (task 0034) and the same contention counter (adr/0016).

| Hand-off | Producers -> consumers | Default | Return path | Lock-free candidate if contended |
|---|---|---|---|---|
| `Batch` into the pipe | acquisition -> workers (each claims frames or a channel) | bounded `sync_channel` of `Arc<SampleStore>` | the buffer returns to the acquisition's free pool when the last `Arc` drops: an MPSC return channel, any thread may drop last | per-producer SPSC ring for the batch; the return stays MPSC |
| a share between deposit and reduction | worker -> the workers reducing its batch | the share is published into the batch's slot and becomes immutable; reduction reads it only after the batch's barrier (all shares in) | shares go back to their owner worker's free list | none needed |
| `BatchUnit`, `RecordBatch` | workers -> main | bounded `sync_channel`, plus main's wake `eventfd` | maps return to their worker through a bounded channel per worker | SPSC ring per worker |
| `AcqWant`, `DepositView`, `Pacing` | main -> acquisition, workers | `Mutex` cell, held for a copy or an `Arc` swap, plus the reader's wake `eventfd` | none | seqlock only over `Copy` fields stored as atomics |
| `Readings` | acquisition -> main | `Mutex` cell plus main's wake `eventfd` | none | as above |
| `AcqCmd` | main -> acquisition | bounded `sync_channel` plus the acquisition `eventfd` | acknowledgements in `Readings` | none needed: rare |
| picture snapshot | helper -> encoder | bounded `sync_channel` of leases (readback buffers, at most 2 out) | the lease returns through a channel to the helper | none needed: at most 30 per s |
| `RemoteRequest` | listeners -> main | bounded `sync_channel` plus main's wake `eventfd` | the reply goes to the connection's output queue | none needed: rare |
| replies | main -> a listener | bounded output queue per connection plus the listener's wake `eventfd`; written on `EPOLLOUT` | none | none needed: per connection |
| metrics | each thread -> the status reader | per-thread counters, cache-line aligned, summed by the reader | none | already lock-free |

Wake protocol, for every `eventfd` pair:
- **Enqueue before notify.** A producer puts the item into the channel or cell first and writes the `eventfd` after,
  so a woken consumer always finds it.
- **Drain, then recheck.** Reading an `eventfd` resets its counter. A consumer that stops early (a quota, a service
  point) writes the `eventfd` again before it returns, so the remaining items are not lost; a consumer that drains
  everything rechecks the channel once after reading the `eventfd`.
- **Seqlocks** only over fields that are atomics themselves; never over ordinary `Copy` fields, which is undefined
  behaviour in Rust.
- **No ring crates.** Every primitive is in `oos_hal::sync`, on `std` and `libc` only (the one dependency).

No lock is held across a copy of sample data, an allocation, a format or a sort. Summaries of metrics (`pacing.rs`
362 sorts histories under its lock today) take a snapshot first and work outside the lock. The contention counters
show in `:SYSTem:STATe?` (`lock <name> waits=N p99=us`).

### Placement

Placement comes from `system/placement` (`perf.place`), with the cores read from `cpu_capacity`. The xdma interrupt
goes to a little core from the rootfs. No thread runs a real-time policy: a stuck real-time thread on this SoC stalls
the panel, and the budgets hold without it.

Large answers are encoded off main: a deep `:WAVeform:DATA?` gets an `Arc` snapshot of the record in its reply, and
the listener thread encodes and sends it. Main never writes to a socket: every reply goes into the connection's output
queue (today `scpi.rs` 46 writes from the UI thread with a 2 s stall timeout).

Typed outputs replace `board::HwShared`, today an `Arc<Mutex<Shared>>` of about 40 fields with settings, one-shot
requests, outputs and readings under one lock. Each kind gets one writer (`acquisition.md`, Port):
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
(`migration.md`, M5). Until then `Shared` stays.

## Memory and queues

The unit has 3.9 GB of RAM, about 3.6 GB available after boot, and no swap ([Budget arithmetic](#budget-arithmetic)). An unbounded buffer ends in an OOM kill. With `panic = abort` and
the restart in rcS, an OOM kill becomes a restart loop ([Safe start and exit](#safe-start-and-exit)). So every buffer
and every queue has a bound in bytes, not only in count.

### Memory plan

`oos_core::memplan(&Settings, &Caps) -> MemPlan` computes the bytes of each acquisition buffer class:

| Class | Holds | Size |
|---|---|---|
| `dma` | the read-out buffers of the batch pool | (in-flight batches + a fixed pin allowance of 1) x batch bytes; in flight is at most the pool of 3 |
| `raw` | raw re-reads (deep window, `RAW_PERIOD`, STOP final) and the holders' copies of kept frame ranges (`last[0]`, measure jobs, remote snapshots, export) | one window per channel for rereads, plus each holder's limit in [Retaining owners](#retaining-owners) |
| `ring` | the roll and scan ring, immutable chunks (`Arc<[Chunk]>`) | one screen: 10 x tdiv / dt points per channel, x 4 B (`f32`), plus the chunks a reader still pins |
| `segments` | read-back segments in flight | the segment channel's bound |

The other buffers do not depend on the acquisition and are bounded by their own constants, each next to its owner:
the share maps (workers x 2 maps x plot pixels x 4 B), the accumulator (3 units of maps plus 2 persistence maps), the
record store (`HISTORY_MAX` x screen record), and each view's maps (`EYE_MAX`, `SPECTRUM_MAX`). The memory budget
in [Performance budgets](#performance-budgets) is `memplan`'s total plus these constants.

- Ring snapshot protocol: the writer appends chunks and never changes a published one. A sweep is a list of chunk
  `Arc`s. Only in-flight work (the records stage, a measure job's copy step) pins a sweep's chunks, for at most one
  step; a holder that keeps the sweep longer copies it once, off main, into its own bounded buffer. The `ring` class
  is one screen plus one pinned sweep, 2 screens at most. A chunk is freed when the writer has passed it and no
  reader pins it.
- The acquisition thread and the pipe allocate from the plan, not from their own estimates.
- A host test sweeps `memplan` over mode x channels x depth x time base on every model's `Caps`. If no reachable
  setting exceeds `Config.mem_budget`, the test asserts that, and there is no depth clamp. If one does, `normalize`
  clamps the depth (Auto included) so that the total stays within the budget, and the test covers the clamp. The
  sweep, not a guess, decides whether the clamp exists.

### Budget arithmetic

Memory available to `oos-ui`: the RK3399 has 3.9 GB of RAM, and about 3.6 GB is available after boot (measured
2026-11-14, `MemAvailable`). The Zynq-side DDR belongs to the FPGA and is not visible to Linux; it is not part of any
budget. There is no swap.

Peak use is the sum of four terms. Each factor is a named constant or a `Caps` value, and the `memplan` sweep computes
the sum for every reachable setting:

| Term | Formula | Factors |
|---|---|---|
| base | process, fonts, layout, GPU arena | measured: 73 MB RSS with 4 channels in RUN on the panel (2026-11-14) |
| maps | (workers + helper) x in-flight shares x map set, plus the accumulator's window (3 units) and 2 persistence maps; map set = 2 maps x channels x plot pixels x bytes per count | plot pixels: 1024 x 486 on the panel, up to 1920 x 1080 on HDMI; 2 B per count in the window, 4 B with decay |
| acquisition | `dma` + `raw` + `ring` + `segments` + decoded copies | `dma` = pool (3) x batch bytes; decoded = 2 x code bytes (u16 codes to f32 volts) for every frame decoded at once; `raw` <= `deepmeas::MAX_BYTES` (24 MiB) per window |
| retained | every retaining owner below at its limit | [Retaining owners](#retaining-owners) |

- **Buffer return.** A DMA buffer goes back to the pool only when its last reader drops it: the workers have
  deposited, the records stage has built its records, and every holder has made its copy. An `Arc` pins the whole
  allocation, not one frame, so the in-flight readers are kept to one step ([One ownership path per
  batch](#one-ownership-path-per-batch)). When no buffer is free, the acquisition drops the next batch and counts it
  (`drops batch`); it never overwrites a buffer a reader holds.
- **Reconfiguration overlap.** The acquisition thread allocates the new plan's buffers when the old plan's are back,
  while nothing is armed. Old buffers still in flight at the switch (at most the pool of 3, for at most one step) are
  counted in the `dma` class of both plans for that step; `memplan` adds the old pool to the new plan's total when it
  checks a reconfiguration against the budget.
- **The two figures.** The baseline in [Performance budgets](#performance-budgets) (120 MB) is the measured base plus
  the maps term at HDMI size. `Config.mem_budget` (default 512 MB) bounds the acquisition and retained terms. M5b
  replaces both with the values the sweep computes, and the sweep test fails if a later change moves them by more
  than 10 % without a doc change.
- **Rejection.** If even the smallest plan for a setting (minimum depth, fewest buffers) does not fit
  `Config.mem_budget`, `apply` rejects the command with a `CmdError`, and a setup file that holds it loads without
  that key. The depth clamp in `normalize` applies only when a smaller depth fits.

### Retaining owners

Every owner that keeps acquired data past its step has a byte limit and a release rule:

| Owner | Limit | Released when |
|---|---|---|
| `last[0]` and its raw copy | one record, one raw frame range in its own buffer (`raw` class) | the next record replaces it |
| running measure job | its own copy of one raw frame range per channel | the job ends |
| pending measure job | its own copy of one raw frame range per channel | a newer record replaces it, or a `stats_gen` or epoch change discards it |
| ring chunk pins | 1 sweep, in-flight work only, at most one step | the step ends; longer holders copy |
| history and segments | `HISTORY_MAX` screen records, no raw | the oldest entry drops out; the `history` class clears it |
| waveform snapshots held by remote listeners | its own copy, one per connection, 2 over all connections | the answer is sent, the connection closes, or the send times out |
| picture snapshot leases (readback buffers at the encoder) | 2 x one picture (2.46 MB on the panel, 8.3 MB on HDMI) | the encoder returns the buffer; after 1 s the next snapshot is refused |
| remote output queues | 256 kB per connection, 1 MB over all | see [Queues](#queues) |
| remote connections | 8 over SCPI, VXI-11, HTTP and MCP together | a ninth is refused at accept |
| `RemoteRequest` | 8 per connection, 32 over all | see [Queues](#queues) |
| export (CSV, PNG, setup) | one at a time, from its own copy | the file is written |

- A slow waveform client gets 10 s per send. A send that does not progress in that time closes the connection and
  releases the snapshot. The snapshot is a copy, so even a stalled client holds no DMA buffer.
- The sum of these limits is the retained term of the budget arithmetic.

### Queues

Every channel between threads is bounded in count and in bytes, and has a stated overload policy. The acquisition
thread never blocks on a full queue.

| Channel | Bound | When full |
|---|---|---|
| `Batch` into the pipe | the pool (count) and the `dma` class (bytes) | the acquisition thread drops the batch and counts it; deposit never moves to main |
| `BatchUnit` to main | pool size | cannot fill: there are never more units than buffers in flight |
| `RecordBatch` to main | 2 per step, `records` class | thin to the newest batch and count; a reader that needs every frame (mask, segments) counts the loss |
| `Frame` (newest frame, live roll and scan sweeps) | latest-value cell: one frame | replaced; a sweep is screen-resolution min/max plus the `Arc`s of its ring chunks, never a copy |
| segment read-back | 2 batches of segments, `segments` class | back-pressure: the read-back waits for space. Safe because acquisition is stopped during read-back (today `seg_step` waits the same way, `hw.rs` 2252) |
| `MeasRead` | latest-value cell per kind | replaced by the newer read |
| `AcqCmd` | 16 | the sender gets an error; commands are never dropped silently; a superseded command is acknowledged as `Superseded` |
| `RemoteRequest` | 8 per connection, 32 over all connections | the listener answers SCPI `-350` (queue overflow) or HTTP 503 |
| remote output queue | 256 kB per connection, 1 MB over all connections | main never blocks: a reply that does not fit is refused with `-350` (SCPI) or 503 (HTTP) if the connection is idle, and the connection is closed if it is already behind on an earlier answer. Waveform answers are encoded in 64 kB chunks into a reusable buffer, one chunk at a time as the queue drains; partial writes and `EAGAIN` keep the order; the 10 s per-send rule closes a stalled client |
| `InputEvent` | 64 | knob steps coalesce (relative steps add up); keys are never dropped |
| measure jobs | one running, one pending | the pending job is replaced by a newer record |

The roll OOM of 2026-11-14 is the case these bounds close. The ring itself is one screen. The frame queue kept every
complete roll sweep after SINGLE, each a full copy of the ring: `hw.rs` cleared the queue only when its last entry was
partial, and roll's `continue` skipped the hold check. With a latest-value cell and immutable ring chunks, roll holds
one ring and the chunks a reader pins. After a complete SINGLE sweep, `stream` publishes nothing until `Rearm`
(`acquisition.md`).

### Degradation order

Under overload the step sheds work in this order, and counts each shed:
1. measurement reads (`MeasRead`): the newest wins;
2. records beyond the newest per step, where no reader needs every frame;
3. records that a reader needs (mask, zones, segments): counted as lost frames in that reader's statistics;
4. whole batches at the pipe intake.

The picture's newest batch and input handling are never shed.

### Watchdog

The protection is in this order. The watchdog is the last line, never the design:
1. **Bounded allocation.** `memplan` and the byte bounds above keep use within the budget in every reachable setting.
   A setting that cannot fit is rejected.
2. **STOP releases.** STOP (from the user or the watchdog) releases what is in flight: queued `Frame`s and
   `RecordBatch`es, the pending measure job, ring pins beyond the current sweep, and remote waveform snapshots that
   are not being sent. It keeps `last[0]` and the allocated pool, so RUN needs no allocation. At the watchdog's hard
   limit memory wins: its `reset()` drops `last[0]` too, and the stopped picture keeps only the maps.
3. **Watchdog.** `system/` reads the process RSS once a second. A once-a-second check cannot catch a fast runaway on
   its own; it catches a slow leak that the bounds missed. Above the soft limit (`Config.mem_budget` + 256 MB) it
   logs and shows a warning. Above the hard limit (1 GB) it forces STOP through the outbox without a `Final` command, so the
   watchdog's STOP never does the allocating final raw read, calls `reset()` on the stages that hold buffers (records,
   accumulator, views), and shows a toast. If RSS is still above the hard limit 5 s later, `oos-ui` exits with code
   75, and rcS starts safe at once. The limits are `Config` values, tuned on the unit.

### Observability

Each channel and each shed above has a drop counter. `:SYSTem:STATe?` prints them on one line,
`drops batch=N recbatch=N frame=N measread=N acqcmd=N remote=N input=N measjob=N frozen=N`, next to RSS, the `memplan` total
and the watchdog state. unitlab fails a run where a counter grows that must stay 0 in that mode.

## Safe start and exit

rcS restarts `oos-ui` after every exit, and `oos-ui` restores the last saved setup at every start. The release build
has `panic = abort`. Together that turns any panic or OOM kill that a setting provokes into a restart loop, and the
loop survives a power cycle because the setup is on `/oos`. The design breaks that loop (adr/0015).

### Exit contract

| Exit | Meaning | rcS counts it |
|---|---|---|
| 0 | clean: stop requested, update, shutdown | no |
| 75 | memory: the watchdog's hard limit was reached | yes, and rcS starts safe at once |
| other non-zero, or a signal (SIGABRT from a panic, SIGKILL from the OOM killer) | abnormal | yes |

- At start `oos-ui` writes `/oos/oos-ui.running`. A clean exit removes it. After `Config.stable_s` (60 s) of running
  it is replaced by `/oos/oos-ui.stable`. So a start that finds `oos-ui.running` knows the last run ended abnormally
  before it was stable, even across a power cycle.
- The rcS loop keeps `/oos/oos-ui.exits`: the times of abnormal exits within the last hour. A slow loop (a crash
  every 10 minutes) is counted as well as a fast one. Entries older than an hour drop out; a run that stays up for an
  hour clears the list.

### Safe start

- When `oos-ui.exits` holds 3 entries, or the last exit was 75, rcS starts `oos-ui --safe`. Safe start uses default settings and the default
  acquisition, renames the saved setup to `/oos/oos-ui.setup.bad`, and shows a toast that says so and where the file
  is.
- Two setup files exist. `/tmp/oos-ui.setup` is what a restart within the same boot restores; `/oos/oos-ui.setup` is
  what a power-on restores (rcS copies it to `/tmp`). Today both are written every 2 s with the current settings
  (`main.rs`, `SETUP_FILE`). In the target both are written only with settings that ran `stable_s` without an
  abnormal exit, so neither a same-boot restart nor a power cycle restores a setting that crashed the scope within a
  minute. The timer belongs to the candidate settings, not to the process: it restarts at every settings change, and
  the candidate is promoted to both files after `stable_s` without a change and without an abnormal exit. Each
  candidate that stays unchanged for `stable_s` is persisted at that moment, so a client that writes settings every
  few seconds still leaves the last setup that held for a minute; only settings that never rest for a minute are not
  persisted, and the previous persisted setup stays.
- A clean power-off from the panel or SCPI writes the current settings to both files before the power goes, even
  within `stable_s` of a change: a clean power-off is no evidence against them.
- The RSS watchdog ([Watchdog](#watchdog)) forces STOP before the OOM killer acts, and exits with 75 if memory does not
  come back, so a memory runaway ends in a stopped scope or in safe start, never in an OOM kill loop.
- On an image in its trial start, safe start is off. The update watcher's crash-loop rule writes the previous image
  back instead (`../update.md`), because there the image, not the setup, is the suspect.

The image supervisor also owns kexec undo: the updater starts `/bin/oos-ui-loop` only when an exact-name
process probe reports no loop. Ledger `mode` remains read-only. Only a parse failure permits recovery;
timeouts select safe once without clearing healthy history. Recovery retains at most two damaged ledgers.
Writers share a BusyBox-compatible mkdir lock with PID claims; a timed-out archive's surviving counter
must finish before another writer can commit. Dead PID claims and empty interrupted locks are reclaimed.

### Kernel panics

- **Reboot instead of halt.** The OOS kernel command line carries `panic=10`, so a panic reboots after 10 s instead
  of halting until someone pulls the power cable. It is set in `OOS_BOOTARGS` (`rootfs/board/dho/etc/oos-board`) and
  the image's own boot config, never in p4, p5 or U-Boot. `panic_on_oops` stays 0.
- **Trial starts.** A panic after rcS's first commands (`oos-oneshot` and the start count) reboots into the fallback
  that those commands set up, as a hang there does today (`../update.md`). A panic before rcS leaves `OOSK` armed, so
  `panic=10` restarts the same image in a loop. That is the documented limitation of `../update.md` ("nothing counts
  the starts before rcS"), now automatic instead of at each power-on; recovery is the card in a reader. The trial
  fallback is covered by tests for panics after rcS's first commands. In rcS, `oos-oneshot` and the trial bookkeeping
  run before the crash archive below.
- **Keep the text.** ramoops (at 0x110000, 896 kB: console 512 kB, dmesg 128 kB for oops and panic, pmsg 256 kB)
  survives a warm reset but not a power cut. At the next boot rcS archives the pstore records to `/oos/crash/`:
  - **Byte budget.** `/oos` is 16 MiB and holds the counters, the setup quarantine and the update state, so the crash
    archive has a budget of 1 MiB in total. rcS prunes the oldest boots before it writes, copies each record bounded
    (the dmesg record whole, the console record's last 64 kB), and never writes when less than 4 MiB of `/oos` would
    stay free. A failed archive is logged and skipped; it never blocks the boot or recovery.
  - **Order.** First the archive, durably ([Durable writes](#durable-writes)); then the counter, durably; then pstore
    is cleared. Each record is named by its pstore id and time, so a boot interrupted at any step repeats the steps
    without archiving or counting a record twice.
- **What counts.** Console and pmsg records are present after every warm reboot, clean ones too, so they do not count.
  A boot counts one abnormal exit for safe start ([Safe start](#safe-start)) only if pstore holds a dmesg record whose
  reason is a panic or an oops. A stale `oos-ui.running` marker from the same boot is the same event and is not counted
  again. `:SYSTem:STATe?` and the System page show the last crash's time and first line.

### Shutdown order

On a clean exit (SIGTERM, an update, the stop request), the current M1b implementation stops main intake,
then uses deadlines measured from shutdown entry:

1. remote listeners stop accepting and close connections, through 500 ms;
2. the pipe closes and deposit/measurement workers join, through 1000 ms;
3. the eligible setup is persisted;
4. the GPU helper receives EOF and waits through 2100 ms, then is killed with at most 100 ms to reap;
5. DRM is released after helper termination; otherwise process exit releases its buffers.

These waits share a 2.2 s budget, below the updater's 3 s SIGKILL threshold and the 2.5 s healthy-exit
limit. Storage/kernel IO can still block beyond a process deadline. A timed-out worker keeps its own
buffers until it finishes. Acquisition cancellation remains M3.0/M5 and follows the target contract below.

Memory STOP skips acquisition work while the UI clock, toast expiry and update/NTP/USB/SSH polls continue.
A helper restart restores its retry budget only after acknowledged pictures span ten seconds; shorter
runs retain the 1/5/30 s backoff and exhaust after three restart attempts.

### Cancellation

STOP (`hold`) finishes the acquisition in progress, which may wait for a trigger for ever. Shutdown needs a path that ends
every wait:
- **One cancel flag** per thread, an `AtomicBool` owned by `Session` and shared by `Arc`. Setting it is the request.
- **Every blocking wait checks it.** Trigger and ring waits are in `ppoll` on the command `eventfd` and return
  `Cancelled` at once. A C2H read is split into `MAX_READ` pieces; the loop checks the flag between pieces, so
  cancellation inside a read takes up to one chunk ([Acquisition reaction](#acquisition-reaction)). A read already in
  the driver cannot be interrupted and ends within the xdma C2H timeout (10 s), owned by `hal`.
- **No buffer is freed while a thread can still use it.** The thread that owns a buffer frees it, after its last
  read returned. Shutdown waits for the join; if the join times out, `oos-ui` exits without freeing that buffer
  (the process ends anyway) rather than freeing memory a live read may still write to.
- Host tests with the scripted `Device`: cancel during a trigger wait, during a split read, and during a
  reconfiguration, each ending within the bound.

### Durable writes

Every file the next start depends on (both setup files, `oos-ui.running`, `oos-ui.stable`, `oos-ui.exits`) is
written the same way, in this order:
1. write the new content to `<name>.new` in the same directory;
2. `fsync` the file;
3. `rename` it over `<name>`;
4. `fsync` the directory.

The markers change in a fixed order: `oos-ui.running` is written before any setting is applied; `oos-ui.stable`
replaces it only after the stable setup has been written durably. rcS updates `oos-ui.exits` durably before it starts
the next `oos-ui`. Host tests cut each sequence after every step (a file system fake that drops what was not synced)
and check that the next start sees a consistent state: the old file or the new one, and a marker that never claims
stable for a setup that was not written.

### GPU helper death

The GPU helper is a separate process. When it dies (pipe EOF or `waitpid`), `present` switches to the CPU path at the
next step, counts the event, and restarts the helper with a backoff of 1, 5 and 30 s. After 3 failed restarts it stays
on the CPU path until the next start of `oos-ui`, and `:SYSTem:STATe?` says so. A helper death never ends `oos-ui`.

## Setup files

A setup file is `key = value` lines under a header line, `# OOS setup v<N>: key = value (oos-ui)`. Today `N` is 1.
`store` owns the format (adr/0002: a setup loads as a list of commands).

- **Unknown keys** are ignored with one log line, so an older build reads a newer file.
- **Missing keys** keep their default, so a newer build reads an older file.
- **A key never changes its meaning.** A value with an incompatible meaning (another unit, another range, another
  set of names) gets a new key name; the old key is read by a migration step and never written again. An older
  build then ignores the new key instead of applying it with the old meaning.
- **Renamed keys** raise `N`. `store` keeps one migration function per step,
  `migrate_v<N>_to_v<N+1>(lines) -> lines`, applied in order before parsing. Migration functions are never edited
  after release; a fix is a new step.
- **A value out of range** is a `CmdError` from `apply` like any remote write: the key is skipped, the rest of the file
  loads, and the toast names the key.
- **A file from a newer `N`** loads as far as the keys are known, with a toast that the file is newer than the build.
- Setup files, the safe-start markers and the exit counter are written durably ([Durable writes](#durable-writes)),
  so a power loss leaves the old or the new file, never half of one, and never an old one after a reported write.

Tests: a round trip per `N`; every migration step on a fixture from that version; a file with an unknown key, a missing
key, an out-of-range value and a newer `N`; a truncated file.

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

Details: `acquisition.md`, `picture.md`, and `drawing.md`.

Settings:

```
knob, key, touch           -> input  -> Command --+
SCPI, HTTP, MCP, VXI-11    -> remote -> Command --+-> change: apply -> Settings' -> classify | requests -> ChangeSet
setup file                 -> store  -> Commands -+                                 -> consumers
                                                                                     -> AcqWant -> acquisition thread
```

Records and measurements: `records.md`.

## Data path cost

How samples travel from the FPGA to pixels without needless copies, conversions or sweeps (adr/0016). The figures are
for 4 channels on the panel plot (970 x 448 in the benchmarks, map set M = 3.48 MB of `u16`).

### One ownership path per batch

- **Immutable storage.** The DMA buffer of a batch becomes immutable after the read: `Arc<SampleStore>` with the
  codes, the `BatchMeta` and the frame offsets. In-flight work (deposit, the records stage, `to_record`, a measure
  job's copy step) holds an `Arc` and a range, never a copy.
- **Holders copy once, off main.** Anything that keeps sample data longer than one step (`last[0]`'s raw range, a
  pending or running measure job, a remote waveform snapshot, an export) copies its frame range once, on its own
  thread or a worker, into its own bounded buffer, and drops the `Arc`. Only in-flight work pins the DMA store, so a
  slow client or a long measurement never holds a DMA buffer and never starves the acquisition.
- **The newest frame is a view.** It is an index into its batch, not a clone under a lock (today `hw.rs` 2173 clones
  it into `Shared`, and the fast path clones it again, `ui.rs` 1945: two 10 MB copies at 4 x 10 M).
- **Each record is built once.** `to_record` runs once per frame that needs a record. The record holds its screen
  points; its raw range stays an `Arc` plus range only while in flight, and becomes its holder's copy if it is kept.
  History keeps the screen record only.
- **Measurement reads are separate.** A `MeasRead` decodes only the channels and window its measurements need, into a
  reusable buffer of the measure thread. Its reread cost is scheduled: at most one reread per `RAW_PERIOD`, and the
  acquisition counts the time it loses (`cost reread=`).

### Codes until volts

- Samples stay `u16` codes through transfer, deposit and the cheap measurements. Deposit maps codes to rows with the
  batch's code-to-row table, for every channel count. At 3 and 4 channels each worker deposits one channel with a
  per-channel kernel that reads that channel's codes from the interleaved frame, never a decoded `f32` vector. A unit
  benchmark against today's `f32` path decides when it becomes the default.
- The cheap measurements (min, max, mean, RMS, peak-peak) run on codes in the deposit pass and convert their results.
- A measurement that needs volts (rise time, frequency, FFT input) converts its own window, once, into the measure
  thread's reusable buffer.

### Measurements

- One pass per record computes the shared statistics (min, max, sum, sum of squares, the edge list), and each
  requested measurement reads from them. Only requested products are computed.
- The per-frame folds reuse the trigger position and the extrema of the deposit pass instead of scanning again
  (today `framestats.rs` 139, 222).
- The measure thread and its per-channel helpers are persistent; no thread is spawned per job.

### Maps: workers pre-sum

A channel's map pair (`ph`, `phz`) on the panel plot is 970 x 448 x 2 maps x 2 B = 1.74 MB. How the work is split
depends on the channel count, so that no batch needs more than one reduction pass:

| Channels | Split | Reduction |
|---|---|---|
| 3 or 4 | two phases. First, the trigger table: the workers split the batch's frames into ranges and refine the trigger point of each frame once, written into the batch's per-frame trigger table, immutable after. Then by channel: each worker owns one channel's maps and deposits that channel of every frame, reading the table; the helper does not deposit | none: the unit is the workers' channel maps side by side |
| 2 | by channel and frame: two workers per channel claim frames; no helper share | per channel, 2 shares: each of the two workers sums one row band |
| 1 | by frame: four workers and the helper claim frames | 5 shares, each of the four workers sums one row band |

- **Trigger table.** At 3 and 4 channels the trigger is refined once per frame, in parallel, before any channel is
  deposited. The worker that owns the trigger channel does not refine it for the others, so no channel waits on one
  worker. The table holds each frame's trigger offset and the extrema the folds reuse. At 1 and 2 channels the worker
  that claims a frame refines it, as today.
- Each share records the rows it touched; reduction and the later adds cover touched rows only.
- Reduction cost, worst case (every row touched, 2 GB/s copy bandwidth): at 1 channel, 5 shares x 1.74 MB read plus
  1.74 MB written, over 4 workers, is 2.6 MB per worker, 1.3 ms; at 2 channels, 2 x 1.74 MB read plus 1.74 MB written
  per channel over 2 workers is the same 2.6 MB, 1.3 ms. Budget: p99 <= 1.5 ms per worker.
- **Who waits for whom.** Nobody blocks on a reduction. A worker that finishes its frames claims the next reduction
  band of a batch whose shares are all in, as it claims frames. The unit is published when its last band is done. Main
  never waits: it takes the units that are published at its step. The acquisition never waits for the workers: a full
  pool drops the batch ([Queues](#queues)). Worst-case batch latency is the slowest share plus one band.
- Main adds at most 3 map sets per step (the window's units at Min persistence, or one unit into the decay maps), over
  touched rows. This removes today's per-share adds on main (about 52 MB of merge traffic per batch with five shares).
- No arithmetic under a mailbox lock: a share moves by ownership through its hand-off.

### Fused accumulation

- **No maxima.** The grade is count / W through the LUT, with W the weight sum ([picture.md](picture.md), Window
  and grade). It needs no per-column or per-map maximum, so nothing has to find or invalidate one. Today's maxima
  scans (`ui.rs` 2558) go with the `vmax` follow.
- **Min persistence.** The window holds up to 3 units. A step rebuilds the rows that changed: rows touched by the unit
  that entered and rows touched by the unit that left the window. Each unit keeps its touched-row range, so the
  rebuild knows both. Rows that only the leaving unit touched are cleared in the same pass, so its trace does not stay
  as a ghost.
- **Above Min.** One fused pass over the whole map per step applies the decay and adds the step's unit (1 to 2 ms,
  [Performance budgets](#performance-budgets)). Every row decays every step, so replay equals live and the CPU picture
  equals the GPU picture. There is no lazy decay.
- **Dirty rows.** The rows the step changed are dirty: at Min the union of the entering and the leaving unit's rows; a
  reset or a `Replace` makes every row dirty; above Min every row is dirty every step (the decay). The dirty range goes
  to the upload.
- A frozen picture (STOP, SINGLE done, replay) does no accumulation work and has no dirty rows.

### GPU work proportional to change

- The accumulator hands the helper the dirty row ranges of the maps ([Fused accumulation](#fused-accumulation)); the
  helper uploads those rows only. The row compare against the arena copy (`gpu.rs` 460) goes. Under persistence above
  Min every row is dirty every step, so the whole map is uploaded; dirty rows pay off at Min persistence and in a
  frozen picture.
- **Buffer ownership.** Each output has a pool of 3 buffers. A buffer is in exactly one state: free, uploading,
  rendering, pending flip, or scanned out. A GPU fence ends rendering, not scan-out: a buffer becomes free only after
  the flip that replaced it on screen has completed. Main tracks the states per output from the helper's completion
  messages and the flip events.
- **Fences.** The helper flushes each submission (`glFlush`) and attaches a fence. At startup it detects its
  capabilities. With `EGL_ANDROID_native_fence_sync` it exports a native fence fd and sends it to main over the
  socket, so main can `epoll` on it. Without it, the helper waits on an `EGL_KHR_fence_sync` object in its own thread
  and sends a completion message on the socket. An EGL sync object is not an fd and never crosses the process
  boundary. `glFinish` stays only on a fallback readback path. Panfrost's support for native fence fds and its
  asynchronous behaviour on this board are not verified; M9b measures them before any default is set.
- **Snapshots.** Read back only on demand (a screenshot, a web viewer that asked), into a pool of 2 pixel-pack buffers
  (GLES 3 PBO): the helper starts the readback after a composition, fences it, and hands the buffer to the encoder when
  the fence signals, by ownership, not copied. Without PBO support, a synchronous `glReadPixels` into one buffer is the
  fallback, run only on demand. The periodic snapshot every 15 frames goes.
- **Snapshot leases.** A readback buffer handed to the encoder is a lease: at most 2 out at a time, counted in the
  retained memory ([Retaining owners](#retaining-owners)). An encoder running at `SCHED_IDLE` that keeps its lease
  longer than 1 s loses the next snapshot (busy), never a buffer that presentation needs; presentation uses its own
  pool.
- HDMI: its maps scale with its plot pixels (up to 4.3 x the panel at 1920 x 1080). It has its own budget
  ([Performance budgets](#performance-budgets)); when both outputs run and the budget is exceeded, HDMI composes at
  half rate and counts it.

### Allocations

Zero allocations per frame after warm-up on every hot path. Each has a counting-allocator guard in the tests (gate step
in [Performance budgets](#performance-budgets)):
- the acquisition read, publish and reread;
- the deposit and the row-band reduction;
- the records stage and `to_record`;
- the accumulator merge and grade;
- a measure job;
- compose and present, including the GPU submission;
- a remote reply and a waveform encode.

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
- **Nothing swallowed.** No error is swallowed. No `unwrap` outside tests. `expect` only for an invariant the code
  itself just established (a value checked a line above, a non-empty array by construction), with the invariant as
  its message. Never on input or IO.
- **Errors go back to their sender:** the SCPI error queue, the HTTP status, a toast.
- **Hardware errors.** The acquisition thread retries them with bounded backoff, reports each in `Readings::status`,
  and never blocks the main thread.
- **Inconsistent state.** A condition that can be detected at run time (a count that does not match, a unit of an
  unknown key) is not an invariant: it is an error. It is a `debug_assert!` in debug builds and tests. In release it
  logs one line and recovers to a safe state. A violated `expect` aborts in release, and safe start
  ([Safe start and exit](#safe-start-and-exit)) handles a repeated abort. Each stage has `reset()`, which returns it to its start state (the accumulator: empty maps; records: empty
  store; the acquisition client: re-publish `AcqWant`), and a test that a stage after `reset()` behaves as a new one.
- **Locks.** One helper, `oos_hal::sync::lock`, takes a guard with `lock().unwrap_or_else(PoisonError::into_inner)`
  (`adr/0012`).
- **Log format.** One line per event, `oos-ui <area>: <what> <values>`, to stderr and `/tmp/oos-ui.log`.

## Testing

| Level | What | Where |
|---|---|---|
| unit | settings groups, commands, `classify`, kernels, measurement functions | in the module, pure |
| host | stages and `Session` with fakes; the step pipeline end to end on the Demo source | `ui/tests/` |
| golden pictures | the composed picture per display mode and time base against reference PNGs, with a tolerance | `ui/tests/golden/` |
| memory | the `memplan` sweep over every model's `Caps`; every queue's bound and overload policy; a SINGLE roll sweep publishes once | `oos-core` tests, `ui/tests/` |
| host soak | the step loop on a scripted source (today's `Ui` until M16, then `Session` on `board::fake`), 10 000 steps per acquisition mode (normal, pairs, deep pairs, average from M1; roll, scan, segments through the real acquisition loop from M3.0) with a scripted change storm; heap allocation reaches a plateau after the first 1 000 steps and stays within 1 % of it, and every drop counter matches the mode | `ui/tests/soak.rs`, in the full gate |
| perf guards | ratios against reference paths | `tools/perf-guard.sh` |
| GPU | the CPU picture against the GPU picture | `gpu/test-host.sh` |
| unit lab | the acceptance set on the scope | `tools/oos-stress/`; criteria in `drawing.md` |

Every bug fix comes with a test that fails before the fix. Tests pin behaviour, not internals.

The host soak finds the class of the roll OOM on the host: a buffer that grows per step shows as a missing
plateau within seconds, without the unit. It counts allocations with a counting global allocator in the test binary
only.

## Performance budgets

Every budget is a counter in `:SYSTem:STATe?` (p50 and p99 over the last 10 s, one line per thread) and a gate
(adr/0016). Host perf guards check ratios and allocation counts on every commit; unitlab asserts the absolute values on
the unit in each round. The values are for 4 channels on the panel unless a row says otherwise; the first unit round
after each step may tighten them, never loosen them without a doc change.

| Thread or stage | Budget | Counter | Asserted by | Guard lands in |
|---|---|---|---|---|
| main, full step (first A72) | p50 <= 8 ms, p99 <= 15 ms | `step` | unitlab | M3d (counter), asserted from round 1 |
| intake + change | p99 <= 0.5 ms | `stage intake`, `stage change` | host timing guard, unitlab | M2.3 |
| records stage | p99 <= 1.5 ms off the fast path; records are built on the workers | `stage records` | host timing guard, unitlab | M6 |
| accumulate (at most 3 map sets, touched rows, fused grade) | p99 <= 2 ms; <= 4 ms on HDMI | `stage accumulate` | host timing guard, unitlab | M9, M10 |
| compose + present submission (sum) | p99 <= 4 ms; <= 6 ms on HDMI | `stage compose`, `stage present` | unitlab | M3d (counter), asserted from M9b |
| light step (input or remote) | settings round trip p99 <= 2 ms when main is idle | `light` | host test, unitlab | M3d |
| remote arrival to reply during full steps | p99 <= 3 ms, measured from the listener's read timestamp to the reply in the output queue | `remote latency` | unitlab (`ifload.py` with pictures running) | M3d |
| remote throughput | >= 500 query/answer exchanges per s on one SCPI connection, picture rate within 10 % | `remote` | unitlab (`ifload.py`) | M3d |
| acquisition notification | under 1 ms outside a C2H read (provisional until Platform tuning) | `acq wake` | host test with the scripted `Device`, unitlab | M3c |
| acquisition read chunk | p99 <= 20 ms per 4 MiB chunk; this bounds application and cancellation inside a read | `acq chunk` | unitlab | M3c |
| acquisition overhead per batch (excluding DMA and rereads) | p99 <= 1 ms | `acq batch` | unitlab | M3c |
| DMA completion | completion CPU <= 5 % of one A53; overhead <= 1 ms per 4 MiB beyond link time. Exception: a kexec session runs in poll mode, which burns an A53 per transfer, and is exempt; it normally ends with a warm reboot after the install | `xdma` | unitlab | M5c |
| deposit worker, per frame | p99 <= 2.7 ms; load <= 70 % per little core with measurements | `worker` | perf guard (ratio), unitlab | M7 |
| share reduction, per batch (1 and 2 channels; none at 3 and 4) | p99 <= 1.5 ms per worker | `reduce` | perf guard, unitlab | M9 |
| measure job | cheap set p99 <= 2 ms; full set on 4 x 625 k p99 <= 40 ms; deep work per [Depth baselines](#depth-baselines) | `measure` | perf guard, unitlab | M6 |
| GPU helper | CPU per composition <= 3 ms on the panel, <= 5 ms on HDMI; GPU time <= 10 ms | `gpu cpu`, `gpu time` | `gpu/test-host.sh` ratio, unitlab | M9b |
| HDMI | >= 29.5 pictures/s alone; >= 15 with the panel, counted when halved | `hdmi` | unitlab | M9b |
| picture rate | >= 29.5 pictures/s in every acceptance mode | `fps` | unitlab | exists |
| input to photon | the preview at the next flip; the first batch under the new mapping <= 70 ms | `latency` | unitlab | M4 |
| wake latency | as in [Waiting](#waiting), provisional until [Platform tuning](system.md#platform-tuning) | `wake <thread>` | unitlab | M3c, M3d |
| idle in STOP | total CPU < 3 % of one A53; main wakes only on flips and input | `cpu`, `wakes` | unitlab | M3d |
| allocations | 0 per frame after warm-up on every hot path ([Allocations](#allocations)) | `alloc` | counting-allocator tests | each step that touches a hot path; harness in G2 |
| memory | RSS <= 120 MB + `memplan` total, flat over a 2 h soak ([Memory and queues](#memory-and-queues)) | `rss` | host soak, unitlab | M5b |

### Host budget rows

`tools/perf-budgets.py` reads this table and checks every observation emitted by `tools/perf-guard.sh`.
Ratios are candidate / reference, measured in the same process. Both paths alternate in 101 rounds,
reversing the starting side each round. The exact nearest-rank p99 is computed separately for each path;
the tail ratio is p99(candidate) / p99(reference). It is never a tail divided by a minimum, nor a
percentile of per-pair ratios. Each sample is the mean call time of a fixed run (1 to 330 calls), so these
are host run distributions, not runtime frame-event quantiles. The best ratio keeps its existing limit;
the tail limit adds 50 % contention headroom. Best ratios use elapsed time; host tails use POSIX
process CPU (including worker threads), excluding time lost to other host processes. The worker observer
blocks while waiting, so its spin is excluded. Unit event counters retain their elapsed/CPU definitions
below. No host absolute-time threshold is used.

| Guard ID | Best ratio <= | p99 CPU ratio <= | Workload / reference | Budget covered |
|---|---|---|---|---|
| batch_read | 1.0 | 1.5 | five approximately 4 MB batches; retained DMA buffer / allocating read | acquisition read primitive |
| batch_decode | 0.5 | 0.75 | five channel/depth cases; fused decode / two-pass decode | acquisition decode primitive |
| decode_f32 | 0.8 | 1.2 | same batches; f32 / f64 fused decode | acquisition decode primitive |
| perf_guard_deposit_spans | 1.25 | 1.875 | nine signals, 100 k and 12.5 k points; spans / pixel writes | deposit kernel |
| perf_guard_dots | 1.25 | 1.875 | nine signals, 100 k points; runs / per-sample adds | deposit kernel |
| perf_guard_fast_interpolation | 1.1 | 1.65 | seven sparse timebase/channel cases; fast / exact | deposit kernel |
| perf_guard_compressed_frame | 0.5 | 0.75 | 100 k point square, k=15; compressed / raw decode and draw | deposit kernel |
| perf_guard_codes_per_frame | 0.9 | 1.35 | five channel/trigger cases; codes / decode and draw | deposit kernel |
| perf_guard_deposit_worker_per_frame | 1.5 | 2.25 | three depths, measurements off; one worker / frames mode | worker orchestration |
| perf_guard_framestats | 0.7 | 1.05 | 125 k points, cheap set; fused / reference statistics | measure primitive |

The same checker requires 64 allocation-free post-warm-up frames for each UI probe (`deposit_spans`,
`deposit_dots`, `deposit_codes`, `cheap_measurements`). The process-wide counting allocator includes
allocations, zeroed allocations, reallocations and freed transients; each probe warms up eight frames in
an isolated child with no workers. HAL additionally guards retained DMA reads, read steps and kept-vector
decode in `hal/tests/alloc_batch.rs`. The existing soak guards retained memory, not zero allocations.

These rows gate existing host primitives. They do not stand in for complete future stages. Intake/change,
light steps and acquisition wake tests land with M2.3/M3c/M3d; records, reductions, accumulator and full
measurement jobs extend the harness in M6/M9/M10. Compose/present, GPU submission and remote encode remain
with their owning steps. Unit-only rows in the table above, including absolute CPU, latency, FPS and RSS,
remain for the next applicable unit round. No unitlab runner is present in this tree; its future adapter
can read these tables without treating a host ratio as unit evidence.

### How each budget is measured

Two kinds of distribution are kept apart:
- **Runtime events** on the unit: every occurrence in the window is a sample, timestamped where it starts.
- **Host benchmarks**: repeated runs of a fixed workload; the perf guard compares the p99 of the runs with a reference
  path measured in the same process.

For runtime counters the window is 10 s, or 60 s for events slower than 5 per second (measure jobs, rereads); a
quantile is reported only with at least 50 samples, otherwise the line says `n<50`. Quantiles are exact (sorted
samples of the window, sorted outside any lock). CPU counts are per process and per thread from `/proc/self/task`.

| Counter | Start | End | Time | Workload for the assertion |
|---|---|---|---|---|
| `step` | the vblank event that starts the full step | the last stage returns | elapsed | each acceptance mode for 60 s |
| `stage <name>` | stage entry | stage return, service-point slices excluded | elapsed | as `step` |
| `light` | the request's arrival in main (its listener timestamp) | its reply in the output queue | elapsed | main idle, 1000 settings writes |
| `remote latency` | the listener's read of the request (`CLOCK_MONOTONIC` taken in the listener) | the reply in the output queue | elapsed | `ifload.py` with pictures running |
| `acq wake`, `wake <thread>` | the source's timestamp: the writer stamps the item before it writes the `eventfd`; the kernel timestamp for vblank and flip events | the consumer starts handling the item | elapsed | change storm and remote load |
| `acq chunk` | `read()` call | `read()` return | elapsed | deep raw window and 4 channels at 10 M |
| `acq batch` | the batch's ring read returns | its publish to the pipe returns; DMA and rereads excluded | elapsed | every acceptance mode |
| `xdma` | per read | per read | CPU of the completion path (IRQ thread or poll thread) and elapsed read time minus bytes / link rate | 10 min of 4-channel acquisition after a normal boot; pass: CPU <= 5 % of one A53, overhead <= 1 ms per 4 MiB. Not asserted in a kexec session (poll mode, exempt) |
| `worker`, `reduce` | claim | publish | CPU of the worker thread | 4 channels and 1 channel, deep pairs |
| `measure` | job taken | result sent | elapsed and CPU | cheap set and full set per depth class ([Depth baselines](#depth-baselines)) |
| `stage compose` + `stage present` | compose entry | submission to the helper returns | elapsed; the sum is asserted, not each part | every acceptance mode, panel and HDMI |
| `gpu time` | GPU timer query (`EXT_disjoint_timer_query`) around the composition; without it, the helper's wall time from submit to fence | as start | GPU elapsed | as `step` |
| `gpu cpu` | helper receives a submission | helper flushes it | CPU of the helper | as `step` |
| `cpu`, `wakes` | window start | window end | process CPU over the window; wakes per thread | STOP, 60 s, no input |

### Depth baselines

The earlier deep-raw measurements (`../fork/deep-meas.md`, 1 ms/div, 10 M, CH1: a window of 12.5 MB) are the
reference for deep work. They measured: DMA 52 ms, decode 51 ms, screen record 5.5 ms, cheap measurement 58 ms. The
budgets above are for normal depths (up to 1 M points per channel on screen); deep work has its own targets:

| Work | Normal depth | Deep raw window (12.5 MB) | Basis |
|---|---|---|---|
| decode | none: codes stay codes | none for drawing; the measure thread converts only what a non-cheap measurement needs | adr/0016, codes until volts |
| screen record | inside the records stage budget | <= 5.5 ms, off main | measured |
| cheap measurement set | p99 <= 2 ms | p99 <= 10 ms: one pass over 12.5 MB of codes at >= 2 GB/s, plus margin | from 58 ms today, which included the decode to `f32` |
| full measurement set | p99 <= 40 ms on 4 x 625 k | measured in M6, recorded here | no baseline yet |
| reread DMA | not used | 52 ms per window, at most once per `RAW_PERIOD`, outside `acq batch` | measured |

Every guard of a shipping budget lands by M12, so the stable stop point ships with its budgets enforced.

Gate step G2 ([migration](migration.md)) adds the tools these guards need: tail-aware perf guards (p99 over repeated
runs against a reference, not only the best of rounds), the counting-allocator harness, and the unitlab budget table
read from this section.

## Enforcement

`cargo xtask check` is the gate (`../development.md`: the steps, the ratchets and how to change a rule). For the
architecture it checks:
- **crate dependencies** from cargo metadata. The domain's direction is enforced here, by the crate split. Target
  lines in `xtask/data/architecture.txt`:
  ```
  crate oos-dsp ->
  crate oos-core -> oos-dsp
  crate oos-hal -> libc
  crate oos-ui -> libc oos-hal oos-dsp oos-core
  crate oos-fpgad -> libc oos-hal
  ```
- **module rules** in the same file, over module paths (`layer ui <module> -> <allowed...>`, `forbid ui <from> <to>`).
  Every path in the code counts, with `self::` and `super::` resolved against the file's module. They have no
  ratchet, so each rule is added by the migration step that makes the tree meet it (`migration.md`). Target
  rules:
  - adapters do not name each other: `forbid ui <adapter> <adapter>` for each pair of `display`, `input`, `remote`,
    `store`, `system`, `instruments`, `protocols`;
  - stages do not name adapters: `forbid ui <stage> <adapter>` for `records`, `picture`, `compose`, `views`, `menus`;
  - views and menus name only the stages whose outputs they read: `layer ui views -> compose records picture`,
    `layer ui menus -> compose records`;
- **size limits**, below.
- **statics**: a `static` with interior mutability only as a listed metrics counter; lazy statics of plain data are
  allowed.

`unsafe` placement is checked: the allowed path prefixes are `unsafe <path prefix>` lines in
`xtask/data/architecture.txt`, and today's `unsafe` outside them is in the shrink-only `xtask/data/unsafe.txt`.
Known gap: the `// SAFETY:` comments (`../../AGENTS.md`, Code) are not checked yet. That needs clippy
`undocumented_unsafe_blocks` in the lint set (auxout). Until then, review enforces it.

What the rules cannot see, the code enforces: ownership, by visibility ([Enforced by visibility](#enforced-by-visibility)).
`oos_hal::dho` only inside `board/dho` is a module rule (`confine`) since G1.

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
