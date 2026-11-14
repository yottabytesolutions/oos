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
(draw-architecture.md 1). In the target every write goes through the accumulator's methods, driven by two inputs:
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
- **Mapping key.** It is fixed per batch at its first claim, so all shares of a batch use one mapping. It covers time
  base, delay, V/div, offset, channel set, dots, zoom, size, and per channel invert, coupling and bandwidth. A unit of
  another key is dropped at merge.
- **Frames convert with their own `BatchMeta`.** Volts, times and the trigger refinement come from the batch, not
  from `Settings` (acquisition.md). So a batch of an older hardware generation under the same mapping key is a valid
  picture.
- **Frames mode** (no worker pipe: the host, `acq.pipeline = frames`, a full buffer pool) is the same unit drawn by
  the UI thread itself. One composition, not two.

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
  (frames per screen frame, smoothed), at most 3 batches and at most 2 screen frames (67 ms) old. Each step rebuilds
  `ph` from the window's share maps. Brightness = count / W.
- **Persistence above Min:** exponential decay with tau = the persistence, in u32 16.16 fixed point. W decays the
  same way. The decay stops when no frames come. Infinite: no decay, halve on saturation with `.max(1)`.
- **Roll, scan, STOP, Single done, history and segment replay:** a window of 1.
- **Colour grade:** count / W through the heat LUT. The GPU gets W where it got `vmax`.

## Invalidation

The accumulator reacts to the change point's `ChangeSet` only:

| Class | Accumulator action |
|---|---|
| `mapping` | drop window and in-flight units of the old key; draw `last[0]` once under the new key as the preview (window of 1, W = w); the first batch of the new key replaces it |
| `phosphor` | clear maps and W (persistence setting changed, CLEAR) |
| `acq` | nothing: batches convert with their own `BatchMeta` |

STOP freezes the maps and W. A view change while stopped is a `mapping` change and replaces the frozen picture with
`last[0]` drawn once. Nothing is redrawn on every step.

## Records and measurement reads

- Records never draw on the fast path. The decoded newest `Frame` is for measurement. `MeasRead`s (raw newest, deep
  window) cannot reach a kernel (acquisition.md, records.md).
- Off the fast path (software mask and zones, software average, Bus trigger, Single, segments, zoom on records), the
  UI makes a record of every frame of each `Batch`. The records of one step form one pseudo-batch with W = their
  weight sum, drawn with the same span kernel. Records from a `Frame` or `MeasRead` are never in it.
- `fast_ok` depends only on what the picture needs (zoom, XY, roll and scan). Measurement options never move a frame
  to another route.

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
