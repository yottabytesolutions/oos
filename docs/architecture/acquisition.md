# Acquisition

Target architecture of the acquisition thread and its port to the UI. Part of [architecture.md](../architecture.md).
Drawing is in [picture.md](picture.md), records and measurements in [records.md](records.md). Today's code is
referenced as of dho804 32041d0.

## Role

The acquisition thread (`acq`, second A72 with `perf.place`) owns the acquisition hardware: BAR1, the C2H stream,
the AFE lines, the FPGA ring, the DMA buffers and the configuration shadow (`seq::State`). Nothing else touches them
while oos-ui runs. It never draws, never reads `Settings` and never decides what the screen shows. It turns a settings
snapshot into acquired data, tagged with the generation it was acquired under, and hands that data out through typed
outputs.

## Port: `board::Acq`

Today the UI and the thread share one `Shared` struct of about 40 fields under one mutex: settings, one-shot
requests, outputs and readings side by side, each with its own counter convention (`stamp`, `avg_clear`,
`final_req`/`final_ack`, `levels_want` ids, `rearm`, `force`). The target splits it by kind. Each kind has one writer.

| Kind | Writer | Reader | Shape |
|---|---|---|---|
| `AcqWant` | UI (`app/acq_client`, from the change point's `AcqIntent`) | acq | latest value plus `gen`, replaced whole |
| `AcqCmd` | UI | acq | queue of one-shot commands, each with an id; acq acknowledges the id |
| `Batch` | acq | deposit pipe | owned buffer plus `BatchMeta`, moved, never shared |
| `Frame` | acq | UI record path | decoded volts plus `BatchMeta` |
| `MeasRead` | acq | measurement path | decoded volts plus `BatchMeta`; a separate type that no drawing kernel accepts |
| `Readings` | acq | UI | latest value: status, unit counters, board readings, acknowledged ids, the applied `(gen, epoch)` |

The change point produces a board-neutral `AcqIntent` (architecture.md, [Acquisition
intent](../architecture.md#acquisition-intent)). `app/acq_client` maps it to this board's `AcqWant` through
`Board::want`, so the domain never names `Want` or `seq::Settings`. `board/mod.rs` defines these types for every board. `board/dho` implements the thread. The `board::boundary` lint
stays.

### `AcqWant` and generations

`AcqWant` holds everything the UI wants the thread to apply:
- `Want` (`seq::Settings` and the hz parameters);
- `slow` (roll, scan);
- the generator request;
- `needs_raw`;
- `deep_raw`;
- the counter, DVM, mask and zone wants.

Today these are separate `Shared` fields, so a change of `needs_raw` or a mask want raises no generation
(`pacing::WANTED` covers `want` only). One struct with one `gen` makes every acquisition-relevant change visible
in one number.

- `gen` is raised by the change point ([architecture.md](../architecture.md#generations)) whenever the
  `ChangeSet` contains `acq` (`classify` calls `new_gen`). It is the only writer.
- The thread stores `applied` = the `gen` of the want it last applied (configured, or found equal). Every output it
  produces from then on carries `applied` in its `BatchMeta`.
- `epoch` is the second number in `BatchMeta`. It is raised by explicit requests whose settings may not differ:
  SINGLE armed, CLEAR, RUN after STOP, an autoset or Bode stage, including a retry with identical settings. It
  replaces `hw_stamp`. Request resolution in the change point calls `new_epoch`. Autoset and Bode return
  `Request::Epoch(token)` and read their epoch from the next step's `ChangeSet` (today `autoset.rs` 180 stores the
  stamp in the same call; the token replaces that, adr/0003). An epoch change is published as part of `AcqIntent`
  and so of `AcqWant`, so the pair (`gen`, `epoch`) is always consistent.
- Consumers compare the pair:
  - the picture uses the mapping key, not `gen` (picture.md);
  - records and measurements accept only the current `epoch` and, after a hardware reconfiguration, only the current
    `gen` (records.md).

`BatchMeta`: `id` (batch number), `gen`, `epoch`, `frames`, the acquisition snapshot (sample interval, pre-trigger
samples, per-channel scale, offset, invert, coupling, bandwidth, trigger source, level, slope, coupling and noise
reject), `mode` (normal, pairs with k, deep pairs, roll sweep, scan sweep, segment, hardware average) and `t_read`
(when the read finished). A frame is converted and refined only from its own `BatchMeta`, never from `Settings`.

### `AcqCmd`

| Command | Today |
|---|---|
| `Force` | `Shared::force` |
| `Rearm` (a SINGLE scan complete, SINGLE pressed again) | `halted`/`rearm` |
| `Hold(bool)` and `Final(id)` (STOP exact, stop-exact.md) | `hold`, `final_req`/`final_ack` |
| `AvgClear(id)` | `avg_clear` counter |
| `Segments(run, want)`, `SegRead(k)` | `seg` |
| `SelfCal(..)`, `CalCancel` | `cal_req`, `cal_cancel` |
| `Levels(id, ..)`, `Slots(id)` (autoset, debug) | `levels_want`, `slots_want` |

Each command is taken once. The thread puts its id into `Readings::acked` when done. The answer to a request
(levels, slots, cal result) is in `Readings` under the same id. No counter conventions per field.

### Locking

- `AcqWant` and `Readings` are latest-value cells (a mutex held only for a copy or a swap, never across IO).
- `AcqCmd` and `Frame`/`MeasRead` go through bounded channels.
- `Batch` moves into the pipe with its buffer; the buffer comes back through the pipe's free list (picture.md).
- The thread never holds a lock across a register access, a C2H read or a decode. Today it does not either, but the
  rule now holds by construction: no `Shared` guard lives in the loop.

## Thread structure

Today: one closure of about 890 lines (`hw.rs` 1263-2154) with about 30 loop-local variables. Target: `AcqLoop`, a
struct that owns the state, and one function per phase. One pass:

```
loop {
    service()            board readings, LA probe, self-calibration, generator
    apply()              take AcqWant; configure on change; drop what the old gen left
    units()              DVM, measure-unit levels, core trim, slot dump: only with nothing armed
    match mode {
        Slow::Roll | Slow::Scan => stream(),
        _ if segments    => segments(),
        _                => batch(),
    }
}
```

`AcqLoop` groups its state by phase, so each function borrows only its part:

| Part | Contents (today's locals) |
|---|---|
| `Io` | `bar`, `c2h`, `afe_bus`, `state`, `current`, cal tables |
| `Buffers` | `dma`, `raw_dma`, `pool_made` |
| `Stream` | `rolling`, `ring`, `ring_la`, `sweep`, `sweep_la`, `cursor`, `last_info`, `halted`, `done_seen`, `published` |
| `BatchState` | `armed`, `paused`, `fresh`, `single_done`, `fill`, `cap`, `helper`, `help_cost`, `comp_last`, `deep_last`, `la_fails` |
| `Units` | `avg`, `counting`, `dvm_run`, `guard`, `levels_done`, `slots_done`, trim state |
| `Service` | `afg_io`, `afg_done`, `xadc_at` |

### `apply`

1. Take the newest `AcqWant`. Unchanged `gen` and `epoch`: nothing to do.
2. Changed `gen`: disarm a batch armed ahead (its settings are the old ones), configure if `Want` or `slow` differ,
   restart the units whose wants changed, set `applied = gen`, and publish `Readings::raw_planned { gen, .. }` before
   any output of that `gen` (records.md).
   A route-only change (`PipeView.route`) is `acq` too, so it raises `gen` with an unchanged `Want`: no
   reconfiguration, but the batch armed ahead is disarmed. That costs one batch per route flip and is intended. A
   batch armed under the old route would otherwise be routed by a stale `gen`.
3. Changed `epoch` alone (a retry with identical settings, CLEAR, RUN after STOP): no reconfiguration, but the same
   boundary. Disarm a batch armed ahead under the older epoch.
4. Drop what the older `gen` or `epoch` left in flight: the queued `Frame`s and a fill that `NEW_SETTINGS` ended.
   Today both are still read and queued (draw-architecture.md 1). The ended fill is not read at all.

The invariant: after `apply` returns, nothing the thread publishes carries a (`gen`, `epoch`) older than the applied
pair. The thread publishes the applied pair in `Readings::applied`; `*OPC?` and remote waits read it.

`Board::want` is pure: the same `AcqIntent` gives the same `AcqWant`, so `gen` and `epoch` stay the only change
signals.

### `batch`

One batch: arm, fill, read, post-read reads, publish. In order:

1. STOP (`Hold`): finish the batch in progress, do the final raw read where the measurements want it (stop-exact.md),
   then no arming until `Hold(false)`.
2. SINGLE: one batch per epoch (`single_wants_more`).
3. Hardware average: clear on `AvgClear`.
4. Arm, unless armed ahead under the same `gen` and `epoch`.
5. `Force`: one force pulse into the armed batch.
6. Fill: the Fill plan (`pacing::Fill`) ends the ring fill before a screen step. Meanwhile the thread draws shares of
   the previous batch as the helper (picture.md).
7. Read the ring (one C2H read, chunked).
8. Post-read reads, at most one raw re-read per batch, before anything arms again (the ring must stay):
   - LA words of the same record;
   - live compression: the newest frame raw, at most every `RAW_PERIOD`;
   - deep record: the screen window raw, at most every `deepmeas::PERIOD`.

   The two raw reads are `MeasRead`s (records.md).
9. Unit counters of this batch: trigger status, mask, zone, average progress. They go into `Readings`.
10. Arm ahead (`prearm`) where nothing needs this ring any more.
11. Publish: the buffer with every frame to the pipe as one `Batch` (the pipe decides between deposit and records,
    picture.md). The newest frame decoded as a `Frame` to the
    record path, with the LA record. `MeasRead`s to the measurement path.

What changes against today:
- The newest frame is in the `Batch` like the others. The decoded newest `Frame` is for records only and is never
  drawn (route C goes away, picture.md).
- No `Deliver` closure: the pipe tells the UI when a batch is whole.
- Frames mode (`acq.pipeline = frames`, the queue of every frame) and the no-buffer fallback both become a `Batch`
  drawn by the UI thread's own deposit (picture.md). There is one batch path, not two compositions.

### `stream` and `segments`

- `stream` covers roll and scan together. They share the chunk reads, the LA words and the 33 ms publish; scan adds
  the sweep writer and the SINGLE sweep stop. A published sweep is a `Frame` with `mode` roll or scan and the current
  `gen`. It replaces the picture (window of 1, picture.md), so a sweep never mixes with batches.
- `segments` stays `seg_step`. Read-back segments are `Frame`s with `mode = segment`. They go to the record path,
  not to the pipe.

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

1. Split the closure into `AcqLoop` and the phase functions, with no behaviour change (draw plan step 1b). The
   existing hw tests and the counters stay equal.
2. `AcqWant` with `gen`, `epoch` in `BatchMeta`, `apply` drops the old generation (with draw plan 1c).
3. `AcqCmd` and `Readings` replace the remaining `Shared` fields, one group per merge (requests with ids first:
   levels, slots, final; then the unit readings).
4. `MeasRead` as its own type (with draw plan step 2).
5. The `Device` trait and the host tests of the loop.
