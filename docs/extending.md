# Extending OOS

How a feature crosses the layers of the target architecture (`architecture.md`). Each recipe lists the files to touch,
in order, and the tests that must exist before the change merges. Paths are the target layout. Until the migration
step named in `overhaul-migration.md` has landed, the old location applies; the step says where.

Every recipe ends with the overhaul gate (phase 1, `docs/development.md`): `cargo xtask check` and the test runs (fmt, clippy, tests in release and debug, arm64 check, size
limits, boundary rules, perf-guard, `gpu/test-host.sh`).

## Add a setting

1. **Group.** Pick the group in `settings/` that owns the value. Add the private field, its default and an accessor.
   If no group fits, the setting is probably UI state (menus, views), not a setting.
2. **Invariants.** Add the range or combination check within the group to its `apply`. A constraint across groups
   (a clamp by another group's value) goes into `Settings::normalize()`
   ([Normalize](architecture.md#normalize)), never into `apply`.
   If the group passes about 12 fields because it holds variants with their own parameters, split it into an enum of
   per-variant structs: `TriggerKind::Edge(EdgeCfg)`, `TriggerKind::Pulse(PulseCfg)`.
3. **Command.** Add a variant to the group's command enum, for example `AcquireCmd::Foo(Foo)`.
4. **Class.** Add the field to the group's `classify` function. It does not compile until you do. Choose the classes from
   [Change detection](architecture.md#change-detection). If the setting changes what the hardware acquires, it is
   `acq`. If it changes where samples land on screen, it is `mapping`. If it changes nothing that holds state, it is
   `Unrelated`.
5. **Store.** Add the `key = value` name to `store/setup.rs` (read and write). Old setup files without the key load
   the default.
6. **Edges.** Expose it: a menu item ([Add a menu item](#add-a-menu-item)), an SCPI command
   ([Add an SCPI command](#add-an-scpi-command)). Both produce the new command.
7. **Consumer.** If the acquisition needs it, add it to `AcqIntent` in `change/` and map it in `Board::want` in
   `board/dho`. The domain does not see board or hal types.

Tests:
- group unit tests for the invariants and the command;
- a `classify` test: changing the field yields exactly the chosen classes;
- a setup round trip: write, read, equal.

## Add an SCPI command

1. **Parse.** Add the header to the table in `remote/scpi/`. It maps to a `Command` (set) or a `Query` (read). No
   settings logic in the parser.
2. **Query.** A query reads from the step's state in the `reply` stage. If it needs a value that does not exist yet,
   add an accessor to the owning stage, not a field read through it.
3. **Errors.** A `CmdError` from `apply` becomes the SCPI error queue entry. The parser returns `-113` or `-224` for
   text it cannot map.
4. **Other transports.** HTTP JSON and MCP use the same `Command` and `Query` types (`remote/http`, `remote/mcp`). Add
   the name there if the command belongs in the web UI or the tool list.
5. **Docs.** Add the command to the SCPI reference in `docs/` with its range and unit.

Tests:
- parser test: text to command, text to query, malformed text to the right error;
- a host test over an in-memory `Read + Write` stream: send, step, read the answer;
- a set followed by a query on the same connection answers after the set (`After { gen, epoch }`).

## Add a measurement

1. **Function.** The measurement is a pure function in `dsp/src/measure.rs` over volts and `dt`. Add the variant to
   `dsp::measure::Meas` with its unit.
2. **Records.** `records/measure` runs the enabled measurements on each record of the current generation. A new
   variant needs no code there unless it needs more than one channel or the deep raw window
   (`architecture/records.md`).
3. **Statistics.** Statistics apply to every measurement. Nothing to add.
4. **Edges.** Menu entry in `menus/measure`, SCPI name in `remote/scpi/`, store name if it can be saved in a slot.

Tests:
- `dsp` unit tests on synthetic signals with a known answer, including edge cases (no edge, clipped, one period);
- a records test that the value is reset on a `stats` class change.

## Add a menu item

1. **Item.** Add the item to its page in `menus/`. An item has a label, a value reader (from settings or a component's
   state) and an action that returns a `Command` or a component event. It writes nothing itself. A knob on the item
   returns a relative command (`Step(i32)`).
2. **Layout.** The menu layout lint (`menus/lint`) runs in the tests. It checks label width and page height.
3. **Script access.** If the item should be reachable from scripts and unitlab, give it a stable id in `menus/script`.

Tests:
- the layout lint;
- an input test: select the item, turn the knob, expect the command.

## Add a trigger type

A trigger type is a setting with a hardware side and a software side.

1. **Setting.** Add the variant to `settings/trigger`'s type enum with its parameters as fields of the trigger group
   ([Add a setting](#add-a-setting)). Class: `acq`, and `mapping` only if it moves the trigger point on screen.
2. **Software side.** If the type is evaluated in software (refinement, sparse fill, records), add it to
   `dsp::trigger::Trigger` as a pure function with tests.
3. **Hardware side.** Map it to the FPGA trigger in `board/dho` through `oos_hal::dho` register sequences. If the FPGA
   cannot do it, the board reports the type as unsupported in its capabilities, and the menu hides it.
4. **Edges.** Menu page, SCPI tree under `:TRIGger:<type>`, store name.

Tests:
- `dsp` tests for the software condition;
- a hal `Recorder` test that pins the register writes for the new type;
- a `board::fake` test that an unsupported type is hidden and rejected by `apply`.

## Add a decoder

Decoders are `protocols::Protocol` modules (`protocol-modules.md`).

1. **Module.** Copy `protocols/example` to `protocols/<name>`. Implement `Protocol`: its settings, decode over a
   record, annotation rows.
2. **Register.** Add it to the registry in `protocols/mod.rs`. The decoder menu and the SCPI `:BUS<n>:MODE` names come
   from the registry.
3. **Cache.** The decode cache is keyed by record and decoder settings. Nothing to add for change detection.

Tests:
- decode tests on recorded or synthetic records with known frames;
- the registry test that names and ids are unique.

## Add a board

A board is a `board::Board` adapter. `board/dho` is the reference.

1. **Adapter.** Create `board/<name>/`. Implement `Board`: capabilities, apply an acquisition request, the
   acquisition thread with `AcqWant` and `AcqCmd` in and `Batch`, `Frame`, `MeasRead`, `Readings` out
   (`architecture/acquisition.md`).
2. **Boundary.** Only `board/<name>` may use its hardware crate. Add the crate line in `xtask/data/architecture.txt`
   and extend the `board::boundary` lint to the new crate.
3. **Selection.** `oos_hal::model::platform()` detects the model (`docs/models.md`). `board::select` (target; today
   the type alias `board::Active`) picks the adapter from the platform. Add the new model there.
4. **Fake first.** Run the full host suite against `board::fake` with the new board's capabilities before touching
   hardware.

Tests:
- the board's own register or protocol tests with its bus fake;
- the shared `Board` contract tests in `board/contract.rs` (new with the typed channels, migration step M4), run against
  the fake and the new board's host build.
