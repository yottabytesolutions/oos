# Picture

Target architecture of the path from acquired frames to the phosphor maps the grade and the GPU read. Part of
[README.md](README.md). Derivations, measurements and the step plan are in
[drawing.md](drawing.md). The owner-approved [frame pipeline](frames.md) supersedes the worker-share kernel,
publication and window unit. Its F0-F7 are target work, not completed implementation. This page keeps the
picture lifetime and record-route rules consistent with that target.

## Owners

| Owner | Thread | Owns | Never |
|---|---|---|---|
| Accumulator | UI (first A72) | the deposit window, `ph`, `phz`, the weight sums W, the persistence maps, the grade, the arena copy | draws a frame itself, except a `Replace` record and the record pseudo-batch |
| Deposit pool | coordinator and three workers, 4 x A53 | one open Deposit and Placed arena; exclusive column bands during deposit | touch Picture or read live Settings/Model |
| Acquisition | second A72 | hardware, DMA buffer pool and ReadPlan | decode or deposit; the helper role is removed in F2 |
| GPU helper | own process | nothing; reads the arena | |

The accumulator is the only writer of `ph` and `phz`. Today eight `ph.clear()` sites and six routes write them
([current data flow](drawing.md#current-data-flow-dho804-32041d0)). In the target `ph`, `phz`, W and the window are private fields of the accumulator, and
readers get only `&Picture`, so the compiler enforces the single writer. Every write goes through the accumulator's
methods, driven by these inputs:
- the `ChangeSet` of the change point ([README.md](README.md));
- whole-batch deposits from the pool (`Deposit`);
- the step's record pseudo-batch, off the fast path;
- `Replace(record)`: one record that replaces the picture.

## Replace

`Replace { record, reason }` is the route for every picture that is one record, not a stream of batches. The reason
sets its lifetime:

| Reason | Record | Lifetime |
|---|---|---|
| `Preview` (a mapping change while running) | `last[0]` under the new key | live: ends with the first deposit of the current mapping key and epoch |
| `Sweep` (roll, scan) | the newest sweep `Waveform` (screen-resolution min/max) | live: the next sweep replaces it |
| `Stopped` (only on a mapping change while stopped, or when the STOP final `MeasRead` must be shown) | `last[0]` | frozen |
| `SingleDone` | the SINGLE's record | frozen |
| `Replay` (history, segments) | the selected record | frozen |

- Only a live `Replace` is ended by acquired data. A frozen one stays until an explicit change: RUN, SINGLE, CLEAR, the
  end of replay, or a mapping change, which issues a new frozen `Replace` of the same record under the new key. Deposits
  that arrive while a frozen `Replace` holds are counted as `rejected.frozen`, so an already queued batch
  cannot overwrite STOP, SINGLE done or a replay.
- Same step, both inputs: a frozen `Replace` wins over deposits. A `Preview` loses to deposits of the current key
  and epoch (they are the newer picture), and is not drawn. A `Sweep` excludes deposits: roll and scan produce no
  batches, and a deposit that arrives anyway is counted as `rejected.stale_key` or `rejected.epoch` by its mismatch.

- The accumulator clears the window and draws the record with the same mode kernel and the same weight as a deposited
  frame of the mode (w per column, or W_frame for dots), so a replaced picture has the brightness of a live one.
- The picture then holds this one record (a window of 1) for the lifetime its reason gives.
- The records stage decides `Replace`: it is a value in its output, never a side effect.
- This is the only way a reader `Waveform` or `MeasRead` reaches the picture. F7 replaces the legacy Frame name.

## The batch path

```text
acq: reserve buffer -> read Batch (buffer + BatchMeta) -> bounded pool
pool: place slices -> deposit exclusive tiles -> coalesce whole batches -> Deposit slot
UI: ChangeSet -> accept -> fold window or decay/add -> W grade -> publish dirty rows
```

- **`Deposit`.** The pool coalesces whole batches of one key/identity into one difference-plane set. Workers
  own tiles for a call, not full maps. A full UI slot causes coalescing within per-plane hit bounds, then waiting;
  acquisition waits for buffer return before arming. It never reads just to drop under load.
- **Mapping key.** At first claim the pool takes projections only from a DepositView whose `(gen, epoch)` matches
  BatchMeta, as specified by [projection identity](frames.md#projection-identity). A mismatch is `lost.settings`
  before painting; a changed key or identity closes the open deposit. `classify` builds MappingKey from the same
  fields it classifies as mapping, including plot sizes. One batch never uses two projections. Acceptance rejects
  and counts another key as `rejected.stale_key` or an old epoch as `rejected.epoch`.
- **Conversion.** Volts, sample time and trigger refinement come from BatchMeta, never live Settings. An already
  painted deposit from an older hardware generation with its original mapping key and current epoch remains valid
  for the picture if that key still matches.
- **Standard.** Keep it as the equality oracle through F2/F3 and M12a acceptance. F4 (M12b) deletes it and its
  selectors. It is not a second target composition path. See [F0-F7](migration.md#f0-f7-frame-pipeline).
- **Route per batch, from the batch's own tag.** The acquisition thread sends every `Batch` to the pipe and knows
  nothing about drawing policy. `AcqWant` carries the `route` tag from the change stage, and the thread copies it into
  the `BatchMeta` of each batch it arms (README.md,
  [Acquisition intent](README.md#acquisition-intent)). The pipe routes each batch by its tag and deposits it
  with the identity-checked mapping it took from `DepositView` at first claim:
  - on the deposit path the whole batch contributes to one `Deposit`;
  - off it, the workers decode the frames and build the records (`to_record`), and pass them to the UI as one
    `RecordBatch`. The UI adds them to the record store (mask counts, zones, software average, Bus trigger, Single,
    segments) and draws the step's records as the pseudo-batch.
  - Decoding and `to_record` use the batch's calibration and trigger snapshot from `BatchMeta`, never `Settings`.
    `to_record` takes the screen window (`window`, `t_first`) from the mapping fixed at first claim, like the deposit.
    They stay off the first A72 in both routes. Today the acquisition thread decodes on the second A72.
  - A route change is an `acq` change: it changes what the UI needs from every frame, so it raises `gen`. A batch of
    an older `gen` keeps its route tag: an already painted deposit can be shown if its original key and epoch match;
    an unclaimed batch cannot use the newer view. Never turn it into a RecordBatch because the current route changed.
    Records drop RecordBatches of an older `gen` (records.md). So batches acquired before mask, Single or segments
    were switched on are never counted or taken as the Single frame.

## Weights

The frame specification defines the [mode weights and F6 grade transition](frames.md#invariants-and-units) and
the [difference-plane capacity bound](frames.md#difference-plane-capacity). Every source goes through the same
kernel per mode, so a picture's brightness does not depend on the route a record took.
Z input uses `draw_ex`, 24 x z per point; it stays outside the deposited phosphor path.

Sparse records are interpolated to at least 2 points per column, then drawn as spans. Dots keep compression off: a
min/max pair has no sample density. `Replace` and live deposits use the same
[source-density rule](frames.md#invariants-and-units).

## Window and grade

- **Min persistence:** follow the frame specification's
  [window age, eviction and publication contract](frames.md#minimum-persistence-and-gpu-hand-off) and
  [combined capacity bound](frames.md#difference-plane-capacity).
- **Persistence above Min:** exponential decay with tau = the persistence, in u32 16.16 fixed point. W decays the
  same way. The decay stops when no frames come. Infinite: no decay, halve on saturation with `.max(1)`.
- **Roll, scan, STOP, Single done, history and segment replay:** a window of 1, through `Replace`.
- **Colour grade:** count / W through the heat LUT. The GPU gets W where it got `vmax`.
- **Output:** the accumulator exposes `Picture` (`ph`, `phz`, W, grade, the arena) read-only. `compose` and the GPU
  upload read it by `&` once per step; nothing else reads the maps.

## Invalidation

The accumulator reacts to the change point's `ChangeSet` only:

| Class | Accumulator action |
|---|---|
| `mapping` | drop window and reject in-flight deposits of the old key as `rejected.stale_key`; `Replace(last[0])` under the new key as the preview; the first batch of the new key ends it |
| `phosphor` | clear maps and W (persistence setting changed, CLEAR) |
| epoch raised (CLEAR, RUN after STOP, SINGLE, an autoset or Bode stage) | drop deposits of an epoch older than the current one at merge, counted as `rejected.epoch`. Without it, deposits in flight from before CLEAR come back after the reset, and deposits from before a STOP after RUN (today `paused` in `hw.rs` drops them). Epoch raises are rare events, never knob spins, so nothing visible is lost |
| `acq` | nothing: batches convert with their own `BatchMeta` |

STOP alone issues no `Replace`. It puts the accumulator into its `Frozen` state: the maps and W stay as they are, and
deposits that arrive are counted as `rejected.frozen`. So the stopped picture is the last live picture, with its
persistence, not a single record. Nothing is redrawn on every step.

`Replace { Stopped }` replaces it in two cases only:
- a mapping change while stopped (the picture must move): `last[0]` drawn once under the new key;
- the picture was drawn from pairs, and the STOP final raw read (`MeasRead`) arrives: the raw record replaces the
  envelope so the stopped picture shows what was measured. It is drawn with the mode's kernel and the weight of one
  live frame, so it has the brightness of a live picture of the same mode within 3 %.

RUN, SINGLE and CLEAR leave `Frozen`.

## Records and measurement reads

- Records never draw on the fast path. The decoded newest `Frame` is for measurement.
- A `MeasRead` (raw newest, deep window, STOP final) is never part of a live picture. It is drawn only alone, through
  `Replace`: the preview after a mapping change, and the STOP final raw read after a pairs picture.
- Off the fast path the records of one step, built from `Batch` frames, form one pseudo-batch with W = their weight
  sum, drawn with the same mode kernel. Records from a `Frame` or `MeasRead` are never in it.
- The route is off the fast path when the picture needs it (XY, roll and scan) or when every frame must become a
  record (software mask and zones, software average, Bus trigger, Single, segments). Zoom stays on the fast path:
  a second plane set for `phz` uses the zoom projection, with the same lines weight or its own dots density weight
  ([the weight model](drawing.md#one-weight-model-per-display-mode)).

## Rules

- Only the accumulator writes `ph`, `phz`, W and the window.
- A picture holds whole batches of one mapping key and epoch, or exactly one `Replace` record.
- Every source uses the mode's [normalization contract](frames.md#invariants-and-units).
- Nothing draws from `Settings` state that a batch carries itself.
- Workers deposit exclusive column bands into one set; no share-map sum remains. The accumulator folds up to
  three deposits in one pass per channel ([Data path cost](README.md#data-path-cost)). No lock guards pixel data.

## Counters

The canonical [frame counters](frames.md#counters) and
[conservation rule](frames.md#back-pressure-and-conservation) supersede old drop accounting in F0/F3.
Existing `stale` and `partial` counters describe legacy routes; target deposits always contain whole batches.

## Tests

- Per mode: pictures of raw, codes, pairs, deep pairs and records within 3 % of each other at equal content
  (`pair_raw_mix_report`, `depth_mix_report` as asserting tests).
- No deposit drawn under two mapping keys. No partial deposit merged.
- Queue a batch, advance DepositView for coupling, bandwidth or inversion, then claim it: no painting, no preview
  replacement, and every record counted as `lost.settings`. Change the view during a multi-slice batch: all slices
  keep the claimed projection. A mapping-only change can project an unclaimed batch under the unchanged pair;
  an already painted older-gen deposit is accepted only with its original key and current epoch.
- Block the UI deposit slot and coalesce several batches: history receives exactly one newest-record tap per batch,
  in order, including when the picture rejects their deposit. Fill the tap FIFO: the pool waits without replacing
  entries, the UI never waits, and draining returns all tap storage and DMA buffers. Use completion events, not sleeps.
- With synthetic picture periods, feed the read plan's maximum batch rate and burst using one- and two-record
  batches. The [tap capacity](frames.md#tap-capacity) permits every batch's history delivery without source waits;
  assert FIFO, total storage and per-step drain bounds, including arrivals during a drain and old/new-plan overlap.
- A `Replace` picture has the brightness of a live picture of the same mode within 3 %. Roll, scan, Single done,
  preview and replay each produce a `Replace` and nothing else.
- STOP alone produces no `Replace`: the stopped picture equals the last live picture, maps and W unchanged, over
  100 steps with deposits arriving; a STOP after a pairs picture with a final raw read gives one `Replace { Stopped }`
  within 3 % of the live brightness.
- Deposits fed before CLEAR never reach the picture after it; deposits fed before STOP never reach it after RUN.
- A mapping change gives a preview at the next step and never a blank or old-position picture.
- Dots density normalization: raw at 86 samples per column versus replay at 3.7, main versus zoom, and mixed
  densities sum their own W; equal sample distributions grade equally, including through `Replace`.
- Window bounds: exercise the [window contract](frames.md#minimum-persistence-and-gpu-hand-off), including a
  blocked UI slot coalescing old and new batches, exact age boundaries, conservation and expiry on a later step
  without new input; frozen pictures stay frozen.
- Capacity: flat dots at the [sample-hit bound](frames.md#difference-plane-capacity), before and after F6,
  cannot wrap. Test the last fitting whole batch and the first exceeding it, a batch near the bound on its own,
  and the combined three-deposit window.
- Persistence: a single hit stays visible for at least tau; nothing remains after decay to below one count.
- perf-guard: kernels per mode and the window rebuild, best-of timing.

## Module layout (proposal)

| Module | Contents | Today |
|---|---|---|
| `picture/acc.rs` | accumulator: window, merge, preview, invalidation | `Ui::step` parts, `drawpipe::merge` |
| `picture/grade.rs` | W, LUT, persistence decay | `grade_refs`, `picture_ref`, `Phosphor::decay` |
| `picture/deposit/pipe.rs` | `Batch` intake, tile pool, `Deposit` slot, free buffer return | `drawpipe.rs` |
| `picture/deposit/kernel.rs` | span and dots kernels per source | `fastdraw.rs`, `compress::deposit_pairs`, `draw_frame` |
| `wave::Phosphor` | map storage only | as now, without `draw_ex` lines |
