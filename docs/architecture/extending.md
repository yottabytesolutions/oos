# Extending OOS

How a feature crosses the layers of the target architecture (`README.md`). Each recipe lists the files to touch,
in order, and the tests that must exist before the change merges. Paths are the target layout. Each recipe starts with
an "Until" line: what to do while the migration step it depends on has not landed.

Every recipe ends with the coordinator's default `cargo xtask check` on the integrated candidate
([check inventory](../development.md#executable-inventory-and-completion-evidence)). Workers run
`--fast` and focused tests. The coordinator adds `--image` before unit rounds and releases.
Repository CI is off until 0.2.0; partial runs cannot establish completion.

## Add a setting

Until M13 (settings groups) and M14 (commands): add the field to `Model` in `oos-core`, classify it in its future group's `classify` function, and write it only from the change point.

1. **Group.** Pick the group in `oos-core`'s `change/settings/` that owns the value. Add the private field, its default and an accessor.
   If no group fits, the setting is probably UI state (menus, views), not a setting.
2. **Invariants.** Add the range or combination check within the group to its `apply`. A constraint across groups
   (a clamp by another group's value) goes into `Settings::normalize()`
   ([Normalize](README.md#normalize)), never into `apply`. `apply` may reject a command through `Context` (the
   source channel does not exist on this model) but never clamps another group.
   If the group passes 12 fields because it holds variants with their own parameters, split it into an enum of
   per-variant structs: `TriggerKind::Edge(EdgeCfg)`, `TriggerKind::Pulse(PulseCfg)`.
3. **Command.** Add a variant to the group's command enum, for example `AcquireCmd::Foo(Foo)`.
4. **Class.** Add the field to the group's `classify` function. It does not compile until you do. Choose the classes from
   [Change detection](README.md#change-detection). If the setting changes what the hardware acquires, it is
   `acq`. If it changes where samples land on screen, it is `mapping`. If it changes nothing that holds state, it is
   `Unrelated`. A field can be in several classes: V/div is `acq` (the AFE gain changes) and `mapping` (traces move).
   A `mapping` field enters `MappingKey` automatically, because `classify` builds the key from the same destructure.
5. **Store.** Add the `key = value` name to `store/setup.rs` (read and write). Old setup files without the key load
   the default.
6. **Edges.** Expose it: a menu item ([Add a menu item](#add-a-menu-item)), an SCPI command
   ([Add an SCPI command](#add-an-scpi-command)). Both produce the new command.
7. **Consumer.** If the acquisition needs it, add it to `AcqIntent` in `oos-core`'s `change/` and map it in
   `Board::want` in `board/dho`. The domain does not see board or hal types.
8. **Snapshot.** If decoding, conversion or trigger refinement of a frame depends on it (scale, offset, invert,
   coupling, a trigger parameter), add it to the acquisition snapshot in `BatchMeta`. Frames convert from their batch,
   never from `Settings` (`acquisition.md`).

Tests:
- group unit tests for the invariants and the command;
- a `classify` test: changing the field yields exactly the chosen classes, and changes `MappingKey` if and only if
  `mapping` is one of them;
- if step 8 applies: a conversion test that a batch converts with its own snapshot after the setting changed;
- a setup round trip: write, read, equal.

## Add an SCPI command

Until M14 (commands) and M15 (remote on commands): add the header to `scpi.rs` and write through today's setter, which
M2.3 routes to the change point; the command variant comes with M14 and the wait condition with M15.

1. **Parse.** Add the header to the table in `remote/scpi/`. It maps to a `Command` (set) or a `Query` (read). No
   settings logic in the parser.
2. **Query.** A query reads from the step's state in the `reply` stage. If it needs a value that does not exist yet,
   add an accessor to the owning stage, not a field read through it.
3. **Errors.** A `CmdError` from `apply` becomes the SCPI error queue entry. The parser returns `-113` or `-224` for
   text it cannot map.
4. **Other transports.** HTTP JSON and MCP use the same `Command` and `Query` types (`remote/http`, `remote/mcp`). Add
   the name there if the command belongs in the web UI or the tool list.
5. **Waiting.** A query that reads acquired data gets the connection's wait condition, `After { gen, epoch }`, from the
   commands before it (README.md, [Step pipeline](README.md#step-pipeline)). `:SINGle` then
   `:WAVeform:DATA?` waits for the SINGLE's record; `*OPC?` answers when the connection's last `gen` and `epoch` are
   applied. Queries of settings answer at once. Encode large answers on the listener thread from an `Arc` snapshot.
6. **Docs.** Add the command to the SCPI reference in `docs/` with its range and unit.

Tests:
- parser test: text to command, text to query, malformed text to the right error;
- a host test over an in-memory `Read + Write` stream: send, step, read the answer;
- a set followed by a query on the same connection answers after the set (`After { gen, epoch }`).

## Add a measurement

Until M16 (stages): steps 1 to 3 apply as written; the records side is in `ui.rs` and `framestats.rs`.

1. **Function.** The measurement is a pure function in `dsp/src/measure.rs` over volts and `dt`. Add the variant to
   `dsp::measure::Meas` with its unit.
2. **Records.** `records/measure` runs the enabled measurements on each record of the current generation. A new
   variant needs no code there unless it needs more than one channel or the deep raw window
   (`records.md`).
3. **Cost class.** Declare whether it is cheap and foldable: computable per frame by the deposit workers from a
   running fold (min, max, sums), like the kinds in `framestats::CHEAP`. A foldable measurement counts every raw frame
   on the fast path; any other counts records only. Most are not foldable; say so explicitly.
4. **Statistics.** Statistics apply to every measurement. The count rule per route is in `records.md`.
5. **Edges.** Menu entry in `menus/measure`, SCPI name in `remote/scpi/`, store name if it can be saved in a slot.

Tests:
- `dsp` unit tests on synthetic signals with a known answer, including edge cases (no edge, clipped, one period);
- a records test that the value is reset on a `stats` class change;
- if foldable: the fold over frames equals the function over the record (as `cheap_results_equal_the_analysis_for_the_cheap_kinds`).

## Add a menu item

Until M14 (commands) and M17 (menus as components): add the item in `ui.rs`'s menu tables; its action writes through
today's setter, which M2.3 routes to the change point. It returns a command from M14.

1. **Item.** Add the item to its page in `menus/`. An item has a label, a value reader (from settings or a component's
   state) and an action that returns a `Command` or a component event. It writes nothing itself. A knob on the item
   returns a relative command (`Step(i32)`).
   - A `Command` goes to the change stage with the step's input commands.
   - A component event (open a view, move a cursor, select a page) goes to the component that owns the state, in the
     `views` stage of the same step. If the component must change a setting in response, it returns a command into
     the outbox.
2. **Layout.** The menu layout lint (`menus/lint`) runs in the tests. It checks label width and page height.
3. **Script access.** If the item should be reachable from scripts and unitlab, give it a stable id in `menus/script`.

Tests:
- the layout lint;
- an input test: select the item, turn the knob, expect the command.

## Add a trigger type

Until M13: the type is a `Model` field; steps 2 to 5 apply as written from M4 (BatchMeta snapshot) on.

A trigger type is a setting with a hardware side and a software side.

1. **Setting.** Add a variant with its own config struct to `change/settings/trigger`'s `TriggerKind`
   (`TriggerKind::Pulse(PulseCfg)`), following [Add a setting](#add-a-setting). Class: `acq`, and `mapping` only if
   it moves the trigger point on screen.
2. **Snapshot.** Put the parameters that refinement or conversion needs into the trigger part of `BatchMeta`.
3. **Fast-path refinement.** Decide whether the type gets software refinement of the trigger point on the fast path
   (as edge does, `trignear::edge_near`). If not, the hardware trigger point is used as is; say so in the variant's
   doc comment. Types that need the refinement but cannot do it near the hardware point force the records route.
4. **Software side.** If the type is evaluated in software (refinement, sparse fill, records), add it to
   `dsp::trigger::Trigger` as a pure function with tests.
5. **Hardware side.** Map it to the FPGA trigger in `board/dho` through `oos_hal::dho` register sequences. The
   trigger unit's registers are documented in `hal/src/dho/tpu.rs` and `seq.rs`; serial triggers in
   `../fork/specs/07-serial-triggers.md`. If the FPGA cannot do it, the board reports the type as unsupported in its
   capabilities, and the menu hides it.
6. **Edges.** Menu page, SCPI tree under `:TRIGger:<type>`, store name.

Tests:
- `dsp` tests for the software condition;
- a hal `Recorder` test that pins the register writes for the new type;
- a `board::fake` test that an unsupported type is hidden and rejected by `apply`;
- a conversion test that a batch refines with its own trigger snapshot after the trigger changed.

## Add a decoder

Applies now: `protocols/` is already in its target shape.

Decoders are `protocols::Protocol` modules (`../protocol-modules.md`).

1. **Module.** Copy `protocols/example` to `protocols/<name>`. Implement `Protocol`: its settings, decode over a
   record, annotation rows.
2. **Register.** Add it to the registry in `protocols/mod.rs`. The decoder menu and the SCPI `:BUS<n>:MODE` names come
   from the registry.
3. **Cache.** The decode cache is keyed by record and decoder settings. Nothing to add for change detection.

Tests:
- decode tests on recorded or synthetic records with known frames;
- the registry test that names and ids are unique.

## Add a board

Until M5 (typed channels): the port is `board::Board` with `Shared`; a second board is not supported before M5.

A board is a `board::Board` adapter. `board/dho` is the reference. The port is DHO-family: `AcqIntent`, `BatchMeta`
and the batch, pairs and roll model describe the DHO800/900 acquisition. A board of that family (another FPGA image,
another channel count) fits. A board with a different acquisition model needs an ADR that generalises `AcqIntent`
first; this recipe does not cover it.

1. **Adapter.** Create `board/<name>/`. Implement `Board`: capabilities, apply an acquisition request, the
   acquisition thread with `AcqWant` and `AcqCmd` in and `Batch`, `Frame`, `MeasRead`, `Readings` out
   (`acquisition.md`).
2. **Boundary.** Only `board/<name>` may use its hardware crate. Add the crate line in `xtask/data/architecture.txt`
   and two `confine` rules: the hardware crate's paths only from `board::<name>`, and `board::<name>` only from
   `board`.
3. **Selection.** `oos_hal::model::platform()` detects the model (`docs/models.md`). `board::select` (target; today
   the type alias `board::Active`) picks the adapter from the platform. Add the new model there. Dispatch is static
   today: one board per build. A second board makes `board::select` an enum of the boards, chosen once at start and
   matched at the few call sites of `Board`, not a `dyn Board`, so the acquisition hot path stays monomorphic.
4. **Fake first.** Run the full host suite against `board::fake` with the new board's capabilities before touching
   hardware.

Tests:
- the board's own register or protocol tests with its bus fake;
- the shared `Board` contract tests in `board/contract.rs` (new with the typed channels, migration step M5), run against
  the fake and the new board's host build.

## Add an analysis view

Until M17: views live in `ui/`; declare `RESETS_ON` from M2.2 on.

A view (spectrum, eye, Bode, power, a new one) is a component in `views/` with its own state.

1. **Component.** `views/<name>.rs`: its state, `step(&mut self, &Records, &ChangeSet, events) -> Outbox`, and
   `draw(&self) -> Layer`. It reads records and results; it never writes settings, only returns commands.
2. **Classes.** Declare `const RESETS_ON: Classes` with the classes its state depends on (a trend resets on `stats`
   and `acq`, an eye on `mapping`), and reset in `step` on exactly those.
3. **Settings.** Its options are a settings group or a part of `analysis` ([Add a setting](#add-a-setting)).
4. **Raw records.** If it needs raw records, it says so in `UiAcq::of`, which turns compression off for it.
5. **Menu and remote.** Menu page, SCPI tree, store names.

Tests:
- its `step` over scripted records; its picture against a golden reference;
- for each declared class, a change resets it; for each other class, it does not.

## Add a display mode or kernel

Until M12 (old paths removed): add the kernel to the new path behind `draw.model` only.

A display mode changes how frames are deposited (lines, dots, a new one).

1. **Kernel.** Add the kernel in `picture/deposit/kernel.rs`. One weight model per mode: define the weight per frame
   per column (`picture.md`, Weights) so that brightness is independent of the source.
2. **Sources.** The kernel takes every source of the mode: raw, codes, pairs, deep pairs and records. A source it
   cannot take forces the records route for that mode; state it.
3. **Mapping key.** The mode is a `mapping` field, so units of the old mode are dropped and a preview is drawn.
4. **GPU.** The GPU helper reads W as before; a mode that needs a different grade adds it in `picture/grade.rs` and
   in `gpu/oos-gpu.c`, both checked by `gpu/test-host.sh`.

Tests:
- brightness within 3 % across sources at equal content (the `pair_raw_mix_report` pattern);
- golden pictures for the mode; perf-guard for the kernel against lines.

## Add a math function

Applies now for the `dsp` part; the state part from M17.

1. **Function.** A pure function in `dsp/src/math.rs` (`Op1` or `Op2`) or an expression form in `dsp/src/expr.rs`,
   over volts and `dt`.
2. **Setting.** The selection is part of the `analysis` group ([Add a setting](#add-a-setting)); class `math`, and
   `mapping` if the math trace's scale follows from it.
3. **State.** A function that holds or averages over records keeps its state in the math component, reset on `math`.

Tests: `dsp` tests with known answers; a records test that it reads only current-`gen` records.

## Add a system.conf flag

Until M5 (`Config`): read the key in the module that uses it, once at start, and note it for M5.

1. **Parse.** Add the key to `Config` in `oos-core` with its default and range. `Config` is read once at start.
2. **Use.** Pass the value to the code that needs it. No code reads `system.conf` or the environment elsewhere.
3. **Runtime change.** A flag that changes at runtime is not a flag: it is a setting in `interfaces`, written through
   a command, and saved by `store::sysconf`.
4. **Docs.** List it in the `system.conf` reference with its default.

Tests: parse with and without the key; two `Config` values give two behaviours in one test process.

## Add a system service

Until M18 (target layout): the service lives in `ui/src/<name>.rs`; steps 1 to 4 apply.

A network or unit service (ntp, ssh, mdns, a new one) lives in `system/`.

1. **Thread.** Its own thread on an A53 with SCHED_OTHER, blocked on its IO (README.md,
   [Threads and messages](README.md#threads-and-messages)). Bounded timeouts and retries on every network call.
2. **Settings.** Its on/off and options are `interfaces` settings. It reads them as values sent on change, not
   through shared state.
3. **Status.** It publishes its status as a latest-value cell or a channel that its menu page reads.
4. **Firewall.** If it listens, add its port to the nftables rules and to `netacl` (`docs/network.md`).

Tests: its protocol logic over in-memory streams; start and stop through settings changes.
