# Picture

Target architecture of the path from acquired frames to the phosphor maps the grade and the GPU read. Part of
[architecture.md](../architecture.md). Derivations, measurements and the step plan are in
[fork/draw-architecture.md](../fork/draw-architecture.md). This page states the structure and the rules.

## Owners

| Owner | Thread | Owns | Never |
|---|---|---|---|
| Accumulator | UI (first A72) | the batch window, `ph`, `phz`, the weight sums W, the persistence maps, the grade, the arena copy | draws a frame itself, except the preview and the record pseudo-batch |
| Deposit workers | 4 x A53 | the share maps of the batch they draw | touch the picture or another share |
| Helper | acq (second A72) | the share it draws during the fill | as the workers |
| GPU helper | own process | nothing; reads the arena | |

The accumulator is the only writer of `ph` and `phz`. Today eight `ph.clear()` sites and six routes write them
(draw-architecture.md 1). In the target `ph`, `phz`, W and the window are private fields of the accumulator, and
readers get only `&Picture`, so the compiler enforces the single writer. Every write goes through the accumulator's
methods, driven by two inputs:
- the `ChangeSet` of the change point ([architecture.md](../architecture.md));
- whole batches from the pipe.

## The batch path

```
acq: Batch (buffer + BatchMeta) -> pipe -> shares claimed by workers / helper -> share maps
                                       -> BatchUnit complete -> UI inbox
UI step: ChangeSet -> accumulator: drop units of another mapping key, add whole units to the window,
         rebuild ph/phz from the window (Min) or decay and add (persistence), W, grade, arena
```

- **`BatchUnit`.** The pipe creates one per `Batch`: frame count, `BatchMeta`, mapping key, list of shares. A share
  is a map set plus its row bounds. The share that brings drawn plus dropped to the frame count publishes the unit.
  The UI sees only whole batches.
- **Mapping key.** It is fixed per batch at its first claim, so all shares of a batch use one mapping. It has one
  source: `classify` builds `MappingKey` from exactly the fields it classifies `mapping`, in the same destructure, so a
  field cannot be in the class and missing from the key (architecture.md, [Change detection](../architecture.md#change-detection)).
  Today these are time base, delay, V/div, offset, channel set, dots, zoom, and per channel invert, coupling and
  bandwidth, plus the main and zoom plot sizes from `UiAcq` (today the `(w, h)` check in `drawpipe::merge`). A unit of another key is dropped at merge.
- **Frames convert with their own `BatchMeta`.** Volts, times and the trigger refinement come from the batch, not
  from `Settings` (acquisition.md). So a batch of an older hardware generation under the same mapping key is a valid
  picture.
- **Frames mode** (no worker pipe: the host, `acq.pipeline = frames`, a full buffer pool) is the same unit drawn by
  the UI thread itself. The UI thread groups one step's queued frames of one mapping key into one unit; a single
  delivered frame in a step is a unit of one. One composition, not two (adr/0013). One unit per step matters for cost:
  48 frames as 48 units would mean 48 x 2 full-map passes per step (about 2.5 GB/s); perf-guard measures the 48-frame
  case.
- **Route per batch, decided by the pipe from `PipeView`.** The acquisition thread sends every `Batch` to the pipe
  and knows nothing about drawing policy. The change stage publishes `PipeView { gen, epoch, mapping_key, route }`, a
  latest-value cell next to `AcqWant`, derived from the same `ChangeSet` (architecture.md, [State and
  ownership](../architecture.md#state-and-ownership)). The pipe routes each batch by its own `gen` and `epoch`
  against `PipeView`:
  - on the fast path the batch is deposited (shares, `BatchUnit`);
  - off it, the workers decode the frames and build the records (`to_record`), and pass them to the UI as one
    `RecordBatch`. The UI adds them to the record store (mask counts, zones, software average, Bus trigger, Single,
    segments) and draws the step's records as the pseudo-batch.
  - Decoding and `to_record` use the batch's calibration and trigger snapshot from `BatchMeta`, never `Settings`.
    `to_record` takes the screen window (`window`, `t_first`) from the batch's mapping key, fixed at first claim like
    the deposit mapping.
    They stay off the first A72 in both routes. Today the acquisition thread decodes on the second A72.
  - A route change is an `acq` change: it changes what the UI needs from every frame, so it raises `gen`. A batch of
    an older `gen` is routed as it was fed: shown if its mapping key matches, never turned into records. Records drop
    batches of an older `gen` (records.md). So batches acquired before mask, Single or segments were switched on are
    never counted or taken as the Single frame.

## Weights

One weight model per display mode. Every source goes through the same kernel per mode, so a picture's brightness does
not depend on the route a frame took.

| Mode | Kernel | Weight per frame |
|---|---|---|
| Lines | per-column spans (`fastdraw::spans`), one span per column per frame, for raw, codes, pairs and records | w = 24 per column |
| Dots | one count per sample | W_frame = samples per column (per map: `phz` has its own) |
| Z input | `draw_ex`, 24 x z per point | the one exception; never mixed with deposited frames |

Sparse records are interpolated to at least 2 points per column, then drawn as spans. Dots keep compression off: a
min/max pair has no sample density.

## Window and grade

- **Min persistence:** a capped frame-count window. It holds the newest whole batches until they reach N frames
  (frames per screen frame, smoothed), at most 3 batches and at most 2 screen frames (67 ms) old. N and the age are
  the bounds that shape the picture. The 3-batch cap (the user's option C) bounds how many units the window
  rebuilds; with one unit per step in frames mode it never binds before the age cap. Each step rebuilds
  `ph` from the window's share maps. Brightness = count / W.
- **Persistence above Min:** exponential decay with tau = the persistence, in u32 16.16 fixed point. W decays the
  same way. The decay stops when no frames come. Infinite: no decay, halve on saturation with `.max(1)`.
- **Roll, scan, STOP, Single done, history and segment replay:** a window of 1.
- **Colour grade:** count / W through the heat LUT. The GPU gets W where it got `vmax`.
- **Output:** the accumulator exposes `Picture` (`ph`, `phz`, W, grade, the arena) read-only. `compose` and the GPU
  upload read it by `&` once per step; nothing else reads the maps.

## Invalidation

The accumulator reacts to the change point's `ChangeSet` only:

| Class | Accumulator action |
|---|---|
| `mapping` | drop window and in-flight units of the old key; draw `last[0]` once under the new key as the preview (window of 1, W = w); the first batch of the new key replaces it |
| `phosphor` | clear maps and W (persistence setting changed, CLEAR) |
| epoch raised (CLEAR, RUN after STOP, SINGLE, an autoset or Bode stage) | drop units of an epoch older than `PipeView`'s at merge, counted as stale. Without it, units in flight from before CLEAR come back after the reset, and units from before a STOP after RUN (today `paused` in `hw.rs` drops them). Epoch raises are rare events, never knob spins, so nothing visible is lost |
| `acq` | nothing: batches convert with their own `BatchMeta` |

STOP freezes the maps and W. A view change while stopped is a `mapping` change and replaces the frozen picture with
`last[0]` drawn once. Nothing is redrawn on every step.

## Records and measurement reads

- Records never draw on the fast path. The decoded newest `Frame` is for measurement.
- A `MeasRead` (raw newest, deep window, STOP final) is never part of a live picture. It is drawn only alone, as
  `last[0]`: the preview after a mapping change and the stopped picture (window of 1).
- Off the fast path the records of one step, built from `Batch` frames, form one pseudo-batch with W = their weight
  sum, drawn with the same span kernel. Records from a `Frame` or `MeasRead` are never in it.
- The route is off the fast path when the picture needs it (XY, roll and scan) or when every frame must become a
  record (software mask and zones, software average, Bus trigger, Single, segments). Zoom stays on the fast path: a
  second span set per frame for `phz` with the zoom mapping and the same w (`fork/draw-architecture.md` 3.3).

## Rules

- Only the accumulator writes `ph`, `phz`, W and the window.
- A picture holds whole batches of one mapping key.
- Every frame of a mode weighs the same per column, whatever its source.
- Nothing draws from `Settings` state that a batch carries itself.
- No map adds on the A53. No lock on the deposit hot path beyond the claim and the publish.

## Counters

On the state line (draw plan step 0): stale, partial, picture deposit and record counts, weight, picture age. In the
target:
- stale = frames of a unit dropped for another mapping key, counted at merge;
- partial = pictures that hold part of a batch, which must stay 0.

## Tests

- Per mode: pictures of raw, codes, pairs, deep pairs and records within 3 % of each other at equal content
  (`pair_raw_mix_report`, `depth_mix_report` as asserting tests).
- No unit drawn under two mapping keys. No partial unit merged.
- Units fed before CLEAR never reach the picture after it; units fed before STOP never reach it after RUN.
- A mapping change gives a preview at the next step and never a blank or old-position picture.
- Window bounds: N, the 3-batch cap, the 67 ms age cap.
- Persistence: a single hit stays visible for at least tau; nothing remains after decay to below one count.
- perf-guard: kernels per mode and the window rebuild, best-of timing.

## Module layout (proposal)

| Module | Contents | Today |
|---|---|---|
| `picture/acc.rs` | accumulator: window, merge, preview, invalidation | `Ui::step` parts, `drawpipe::merge` |
| `picture/grade.rs` | W, LUT, persistence decay | `grade_refs`, `picture_ref`, `Phosphor::decay` |
| `picture/deposit/pipe.rs` | `Batch` intake, claims, `BatchUnit`, free buffer list | `drawpipe.rs` |
| `picture/deposit/kernel.rs` | span and dots kernels per source | `fastdraw.rs`, `compress::deposit_pairs`, `draw_frame` |
| `wave::Phosphor` | map storage only | as now, without `draw_ex` lines |
