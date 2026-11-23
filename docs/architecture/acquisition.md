# Acquisition

Target architecture of the acquisition thread and its port to the UI. Part of [README.md](README.md).
Drawing is in [picture.md](picture.md). Records and measurements are in [records.md](records.md).
Historical comparisons use dho804 32041d0. See [frame pipeline status](frames.md) for the current
deposit path, source admission and decoded-record hand-off.

## Role

The acquisition thread (`acq`, second A72 with `perf.place`) owns the acquisition hardware: BAR1, the C2H stream,
the AFE lines, the FPGA ring, the DMA buffers and the configuration shadow (`seq::State`). Nothing else touches them
while oos-ui runs. It never draws, never reads `Settings` and never decides what the screen shows. It turns a settings
snapshot into acquired data, tagged with the generation it was acquired under, and hands that data out through typed
outputs.

## Port: `board::Acq`

Today the UI and the thread share one `Shared` struct of about 40 fields under one mutex: settings, one-shot
requests, outputs and readings side by side, each with its own counter convention (`stamp`, `avg_clear`,
`final_req`/`final_ack`, `rearm`, `force`). The target splits it by kind. Each kind has one writer.

| Kind | Writer | Reader | Shape |
|---|---|---|---|
| `AcqWant` | UI (`app/acq_client`, from the change point's `AcqIntent`) | acq | latest value plus `gen`, replaced whole |
| `AcqCmd` | UI | acq | queue of one-shot commands, each with an id; acq acknowledges the id |
| `Batch` | acq | deposit pipe | `Arc<SampleStore>`: the codes, immutable after the read, plus `BatchMeta`; shared read-only by in-flight work; the buffer returns to the pool after its last reader (README.md, [Budget arithmetic](README.md#budget-arithmetic)) |
| `Frame` | acq | UI record path | a view into its batch's store (codes) plus `BatchMeta`; its consumer converts to volts; a roll or scan sweep is screen-resolution min/max plus ring chunks |
| `MeasRead` | acq | measurement path | the codes of the window the measurements need, in the reread buffer, plus `BatchMeta`; the measure thread converts what it needs; a separate type that no drawing kernel accepts |
| `Readings` | acq | UI | latest value: status, unit counters, board readings, acknowledged ids, the applied `(gen, epoch)` |

The change point produces a board-neutral `AcqIntent` (README.md, [Acquisition
intent](README.md#acquisition-intent)). `app/acq_client` maps it to this board's `AcqWant` through
`Board::want`, so the domain never names `Want` or `seq::Settings`. `board/mod.rs` defines these types for every board. `board/dho` implements the thread. The `confine` rules in
`xtask/data/architecture.txt` keep `oos_hal::dho` inside `board::dho` and `board::dho` inside `board`.

### `AcqWant` and generations

`AcqWant` holds everything the UI wants the thread to apply:
- `Want` (`seq::Settings` and the hz parameters);
- `slow` (roll, scan);
- the generator request;
- `needs_raw`;
- `deep_raw`;
- the counter, DVM, mask and zone wants;
- `hold`: the run state as a level (STOP holds, RUN does not). Run state is state, not an event, so it lives in the
  want and is never superseded.

Today these are separate `Shared` fields, so a change of `needs_raw` or a mask want raises no generation
(`pacing::WANTED` covers `want` only). One struct with one `gen` makes every acquisition-relevant change visible
in one number.

- `gen` is raised by the change point ([README.md](README.md#generations)) whenever the
  `ChangeSet` contains `acq` (`classify` calls `new_gen`). It is the only writer.
- The thread stores `applied` = the `gen` of the want it last applied (configured, or found equal). Every output it
  produces from then on carries `applied` in its `BatchMeta`.
- `epoch` is the second number in `BatchMeta`. It is raised by explicit requests whose settings may not differ:
  SINGLE armed, CLEAR, RUN after STOP, an autoset or Bode stage, including a retry with identical settings. It
  replaces `hw_stamp`. Request resolution in the change point calls `new_epoch`. Autoset and Bode return
  `Request::Epoch(token)` and read their epoch from the next step's `ChangeSet` (today `autoset.rs` 180 stores the
  stamp in the same call; the token replaces that, ADR 0033). An epoch change is published as part of `AcqIntent`
  and so of `AcqWant`, so the pair (`gen`, `epoch`) is always consistent.
- Consumers compare the pair:
  - the picture uses the mapping key, not `gen` (picture.md);
  - records and measurements accept only the current `epoch` and, after a hardware reconfiguration, only the current
    `gen` (records.md).

`BatchMeta`: `id` (batch number), `gen`, `epoch`, `frames`, the acquisition snapshot (sample interval, pre-trigger
samples, per-channel scale, offset, invert, coupling, bandwidth, trigger source, level, slope, coupling and noise
reject), `mode` (normal, pairs with k, deep pairs, roll sweep, scan sweep, segment, hardware average) and `t_read`
(when the read finished), and the `route` tag copied from `AcqWant` at arm. A frame is converted and
refined only from its own `BatchMeta`, never from `Settings`.

### `AcqCmd`

Bound to a `(gen, epoch)`, may be superseded:

| Command | Today |
|---|---|
| `Single { epoch }` | `run`/`single` edges via `Shared` |
| `Force` | `Shared::force` |
| `Rearm` (a SINGLE scan complete, SINGLE pressed again) | `halted`/`rearm` |
| `Final(id)` (the STOP final raw read, stop-exact.md) | `final_req`/`final_ack` |
| `Segments(run, want)`, `SegRead(k)` | `seg` |
| `Slots(id)` (debug) | `slots_want` |

Unbound, always served in order:

| Command | Today |
|---|---|
| `AvgClear(id)` | `avg_clear` counter |
| `SelfCal(..)`, `CalCancel` | `cal_req`, `cal_cancel` |

STOP and RUN are not commands: they are the `hold` level in `AcqWant` (today `Shared::hold`).

Each command is taken once. The thread puts its id into `Readings::acked` when done. The answer to a request
(slots, cal result) is in `Readings` under the same id. No counter conventions per field.

Commands and the want are separate channels, so their order needs a rule:
- **Bound to a pair.** Every bound command carries the `(gen, epoch)` it belongs to, taken from the `AcqIntent` of the step
  that sent it. The thread serves a command only once it has applied that pair.
- **Superseded.** A command whose pair is older than the applied pair is not executed. The thread acknowledges it
  as `Superseded(id)` in `Readings` and counts it (`drops acqcmd`). The sender sees the outcome; a `Force` or `Final`
  never reaches a newer acquisition than it was meant for.
- **Ahead.** A command whose pair is newer than the applied pair waits in the queue until `apply` has taken that want.
- **SINGLE cannot vanish.** `AcqWant` is a latest-value cell, so a SINGLE epoch followed by another epoch in quick
  succession could be overwritten before the thread reads it. A SINGLE arm is therefore also a queued command,
  `Single { epoch }`. The thread serves each in order: it arms that epoch's batch, or, if a newer epoch was applied
  first, acknowledges it as `Superseded`, so the UI ends that SINGLE explicitly instead of waiting for it.
- **Unbound commands** carry no pair and are served in order whatever the want: a calibration cancel or an average
  clear must never be lost to a settings change.
- Host tests: a `Force` sent before a reconfiguration is superseded, not applied to the new settings; two SINGLE
  presses within one loop pass give two acknowledgements; a `Final` waits for its want; STOP and a `gen` change in
  one loop pass end held; a `CalCancel` sent before a reconfiguration is served.

### Locking

The DHO worker copies the ordinary batch inputs from `Shared` in one snapshot. That includes settings,
identity, route, ADC core matching and raw measurement requests. Its result publishes trigger, mask and zone
readings, raw-window availability and decoded counts together. On the deposit route those fields are visible
before dispatch. On the record route the same transaction enqueues frames after checking their identity.

Shutdown, force, calibration, segment controls and wait cancellation still use live checks. No `Shared` guard
spans register IO, DMA, decode, admission waits or diagnostics recording.

## Thread structure

`board/dho/hw/plan.rs` translates the model and plans the ring read. `window.rs` owns live and replay windows.
`trigger.rs` maps trigger settings. `decode.rs` converts a captured record using its own metadata.

The thread starts `AcqLoop`. Each pass services readings, self-calibration and the generator, then applies
settings and STOP. It polls the DVM, slot dump and core trim when segments are idle. The worker then streams
roll/scan, handles segments or runs an ordinary batch. `hw/acq/roll.rs` owns the stream and sweep.

| State | Owner |
|---|---|
| Register bus, C2H, AFE, sequencer and calibration tables | `Io` |
| Applied pair, settings and mode | `Configured` |
| DMA buffers, arm, fill, average and first-record selection | `BatchState` |
| Last capture and STOP replay | `Replay` |
| Roll ring, scan sweep and publish cursor | `Stream` |
| Generator, board readings and frequency counter | `Service` |
| DVM, slot dump and trim | `Units` |

The following apply, batch and stream contracts also describe later port migration.
The current worker uses `Shared` for its snapshots and bounded hand-offs.

### `apply`

1. Take the newest `AcqWant`. Unchanged `gen` and `epoch`: nothing to do.
2. Changed `gen`: disarm a batch armed ahead (its settings are the old ones), configure if `Want` or `slow` differ,
   restart the units whose wants changed, set `applied = gen`, and publish `Readings::raw_planned { gen, .. }` before
   any output of that `gen` (records.md).
   A route-only change (the `route` tag) is `acq` too, so it raises `gen` with an unchanged `Want`: no
   reconfiguration, but the batch armed ahead is disarmed. That costs one batch per route flip and is intended. A
   batch armed under the old route would otherwise be routed by a stale `gen`.
3. Changed `epoch` alone (a retry with identical settings, CLEAR, RUN after STOP): no reconfiguration, but the same
   boundary. Disarm a batch armed ahead under the older epoch.
4. Drop what the older `gen` or `epoch` left in flight: the queued `Frame`s and a fill that `NEW_SETTINGS` ended.
   Today both are still read and queued ([current data flow](drawing.md#current-data-flow-dho804-32041d0)). The ended fill is not read at all.

The target invariant is that after `apply` returns, nothing the thread publishes carries a (`gen`, `epoch`) older
than the applied pair. The target publishes the pair in `Readings::applied` for `*OPC?` and remote waits.
The current operation barrier is described in [SCPI operation completion](../scpi.md#operation-completion).

`Board::want` is pure: the same `AcqIntent` gives the same `AcqWant`, so `gen` and `epoch` stay the only change
signals.

### `batch`

One batch: arm, fill, read, post-read reads, publish. In order:

1. STOP: hold prevents normal batch arming. A serial capture already armed can finish, and an eligible
   last batch can supply a final raw reread. A stopped-view request takes the replay path. Without that
   view, the pass can still reach configuration before batch intake checks the hold. RUN releases the
   hold before publishing the new epoch. See [STOP and RUN](frames.md#stop-and-run).
2. SINGLE: one batch per epoch (`single_wants_more`).
3. Hardware average: clear on `AvgClear`.
4. Cap the plan by measured cost, hardware, read time and plane capacity. Reserve a DMA destination and any
   funded tap before arm. An already armed successor keeps its original read geometry. A record-route read
   waits for the previous record hand-off to drain. Source waits also wake on control or settings changes.
5. `Force`: one force pulse into the armed batch.
6. The fill phase uses `pacing::Fill` to aim at the next frame deadline. By default DRM flip completion anchors
   the deadline at every third vblank. The HAL sequences fill and read
   together so cancellation preserves the register order.
7. Read the ring (one C2H read, chunked).
8. Post-read reads, at most one raw re-read per batch, before anything arms again (the ring must stay):
   - LA words of the same record;
   - live compression: the newest frame raw, at most every `RAW_PERIOD`;
   - deep record: the screen window raw, at most every `deepmeas::PERIOD`.

   The two raw reads are `MeasRead`s (records.md).
9. Read trigger, mask and zone status for the batch result. Log hardware average progress when enabled.
10. With `acq.prearm = on`, arm ahead where nothing needs this ring any more. The deposit route waits for the
    prior publication before submitting the completed read. See [admission and telemetry](frames.md#admission).
11. Publish the buffer with every kept record as one `Batch`. The pool routes by BatchMeta to Deposit or
    RecordBatch and decodes the newest record for readers, not drawing. RecordBatch already contains the newest
    record, so it is not decoded twice. `MeasRead`s go to the measurement path.

What changes against today:
- The newest frame is in the `Batch` like the others. The decoded newest `Frame` is for records only and is never
  drawn (route C goes away, picture.md).
- No `Deliver` closure: the pipe tells the UI when a batch is whole.
- F4 removes Frames mode (`acq.pipeline = frames`) and its selectors after M12a acceptance.
  There is no no-buffer fallback: F3 waits before read. There is one deposit composition path.

### `stream` and `segments`

- `stream` covers roll and scan together. They share the chunk reads, the LA words and the 33 ms publish; scan adds
  the sweep writer and the SINGLE sweep stop. A published sweep is a `Frame` with `mode` roll or scan and the current
  `gen`. It replaces the picture (window of 1, picture.md), so a sweep never mixes with batches.
- The ring is immutable chunks (`Arc<[Chunk]>`, `f32` points). The writer appends new chunks and drops old ones; it
  never changes a published chunk. A sweep is published as screen-resolution min/max per column plus the `Arc`s of
  its chunks for measurements, never a copy. It goes into the latest-value `Frame` cell, so a newer sweep replaces an
  unread one. The ring is one screen (10 x tdiv / dt points per channel) plus the chunks a reader still pins, the
  `ring` class of `memplan` (README.md, [Memory plan](README.md#memory-plan)).
- After a complete SINGLE sweep, `stream` publishes nothing until `Rearm`. Host test: a SINGLE roll sweep publishes
  exactly one complete sweep, and nothing more over 1000 passes. (`fix/roll-queue` does the same in the old code on
  `dho804`.)
- `segments` stays `seg_step`. Read-back segments are `Frame`s with `mode = segment`. They go to the record path,
  not to the pipe, through their own bounded channel with back-pressure, not through the `Frame` cell. Acquisition is
  stopped during read-back, so waiting for space there blocks nothing else.

### Source switches

`acq.write_gap` defaults to `0`, grouping writes for the ordinary batch. Set it to `10` for the per-write delay.
`acq.sched` defaults to `fifo` for real-time acquisition scheduling. Set it to `other` for normal scheduling.

See the [configuration inventory](../system-conf-keys.md#acquisition-and-rendering-switches) for
the other startup switches and their unit evidence.
See [acquisition source overhead](system.md#acquisition-source-overhead).

## Rules

- One writer per output. The thread writes `Readings`, `Batch`, `Frame`, `MeasRead`. The UI writes `AcqWant` and
  `AcqCmd`.
- Every output carries `BatchMeta`. Nothing downstream reads `Settings` to interpret acquired data.
- A measurement read is a `MeasRead`, never a `Frame` and never in a `Batch`.
- No `std::thread::sleep` in the loop except the error back-off and the start-up wait. Waiting for a trigger ends on
  a new `AcqWant` or an `AcqCmd` (today `acquire_batch_until` with `changed()`).
- Errors go to `Readings::status` and the log, with a bounded back-off (200 ms today). The mask/zone stall guard
  stays.
- Register access only through `hal::dho::seq` steps and the documented reads. No FPGA flash, no 0x205c reads.

## Tests

Today the loop is `cfg(target_os = "linux")` and has no host test. Target:
- The decisions are pure functions of (`AcqWant`, `BatchState`, readings), tested on the host:
  - reconfigure or not;
  - disarm or keep the batch armed ahead;
  - which post-read read is due;
  - prearm allowed;
  - SINGLE gating;
  - what to drop on a `gen` change.
- The IO functions stay thin. `AcqLoop` is generic over a `Device` trait (configure, arm, wait, read, read again,
  disarm), implemented by the DHO hardware and by a scripted fake for host tests. Host tests cover one pass per
  phase, the `gen`/`epoch` invariant, STOP and SINGLE.
- perf-guard covers the decode and deposit kernels the thread calls (best-of timing, rc checked in the gate).

## Migration

1. The worker uses `AcqLoop` and phase functions. The same hardware tests and counter checks cover the split.
2. `AcqWant` with `gen`, `epoch` in `BatchMeta`, `apply` drops the old generation (with draw plan 1c).
3. `AcqCmd` and `Readings` replace the remaining `Shared` fields, one group per merge (requests with ids first:
   slots, final; then the unit readings).
4. `MeasRead` as its own type (with draw plan step 2).
5. The `Device` trait and the host tests of the loop.
