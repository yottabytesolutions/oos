# Screen drawing architecture (design, 2026-11-14)

Goal (user decision): a fast, responsive UI without hiccups or flicker, done properly before the PR. This note is the
design and the implementation plan. compress reviews it, then an independent reviewer.

Why a redesign: the day's display fixes each restored one of two invariants by hand. First, every frame weighs the
same in a picture. Second, a picture holds whole batches. A third property sits under both: brightness is graded per
picture by frame count. The structure guarantees none of them, so every new route or representation can break them
again.

## 1. Current data flow (dho804 32041d0)

Acquisition thread (`board/dho/hw.rs`; second A72 with `perf.place`):

- It reads a batch from the FPGA ring and decodes the newest frame into `Shared::queue` for the UI.
- It hands the read-out buffer with the other frames to the deposit pipe (`drawpipe::Pipe::feed_with`).
- During the next ring fill it draws single frames of the open batch itself (`Pipe::help`).
- With live compression (k > 1, records up to 1 M) frames are min/max pairs. Every `compress::RAW_PERIOD` (200 ms)
  the newest frame is read again raw.
- Deep records (> 1 M) are always pairs. Every `deepmeas::PERIOD` (1 s) a raw screen window replaces the newest frame.

Routes into the phosphor (`wave::Phosphor`: u16 hit counts per channel; `ph` main, `phz` zoom):

| Route | Thread | Frames | Kernel |
|---|---|---|---|
| A. workers | 4 deposit workers (A53) | batch frames but the newest | `drawpipe::draw_frame` / `draw_frame_codes` |
| B. helper | acquisition thread, during the fill | single frames of the open batch | as A |
| C. UI newest | UI (`Ui::step`, `ui.rs` ~1938) | the newest, as a `Record` | `draw_frame` when `fast_ok` and not a deep raw frame, else `draw_ex` on the record |
| D. frames mode | UI (`Ui::fast_draw`) | every queued frame (no pipe) | `draw_frame` |
| E. record path | UI | every frame when not `fast_ok`: `Source::acquire` -> `to_record` (3600 points) | `draw_ex` into `ph`, with zoom also `phz` |
| F. stopped | UI, every step | the stored record (STOP, Single done, history replay) | `draw_ex` at weight 24 x 6 |

Route E covers:
- zoom, Z input (weight 24 x z per point), XY, Single, segments and segment replay;
- the software mask and zones;
- Peak, Average and HiRes (the software average blends into `last[0]`; `hw_avg` frames skip the blend);
- roll, scan, and the non-hardware sources.

XY and LA draw on the canvas directly, without phosphor or grade.

Kernels in `draw_frame` (`drawpipe.rs:38`), weight 24:
- `fastdraw::deposit` / `deposit_codes`: one min/max span per screen column.
- `deposit_dots`: every sample its pixel.
- `compress::deposit_pairs`: one span per column, built from the pairs whose centre lies in it.
- `sparse::draw`: interpolation, then `draw_ex`.
- `to_record` + `draw_ex`: envelope, roll, scan and deep raw windows.

**Two weight models.** A deposit puts one span per column per frame (24). `draw_ex` in lines joins consecutive record
points: about 3.6 segments per column at 3600 points, so about 3.6 x 24 per pixel per frame. Every route E/F
picture, sparse frames and deep pair frames weigh about 3.6x a deposited frame. With dots, a raw frame adds its
sample count per column, about 100 at 200 us/div k = 15. A pair frame adds one span.

**Grading** (`Ui::grade_refs`, `ui.rs:2545`; `picture_ref`, `ui.rs:7903`; `trace_lut`):
- Minimum persistence: the maps are cleared when a step brings frames. `pic_frames` = frames of the step. The
  reference is the smoothed per-frame maximum times `pic_frames`. That assumes equal weights and that `pic_frames`
  counts what is in the maps.
- Persistence > 0: the maps decay by exp(-dt/persist) per step. `vmax` follows the brightest pixel (up at once,
  down by 1/8 per step).
- Brightness = floor + sqrt(count / reference). The zoom map has its own `vmaxz`. The GPU (`graphics = gpu`)
  draws the uploaded maps with the same LUT.

**Settings changes.** `Pipe::set_view` bumps `gen` when `Key` changes. `Key` holds channels, time base, delay, trigger,
acquisition mode, dots and size, but not mdepth. `Ui::settings_changed` clears the maps (`dirty_map`). Frames carry
`stamp`, but `hw_stamp` only changes when Single is armed (`ui.rs:1762`). On a reconfiguration the acquisition thread
does not clear `Shared::queue` (`hw.rs` ~1510). From the code, not verified on the unit: frames acquired under the
old settings can still be drawn with the new mapping after the clear, by the workers (same stamp) and by route C.
Measured settle on the unit (stutter.md 5): configured 25-50 ms after the change, steady 5-150 ms, at most 1 empty
picture.

## 2. The day's bugs on that flow

| Bug | Where | Property broken |
|---|---|---|
| Flicker 500 us/div (flicker.md 1) | route C `draw_ex`, 3.6x a deposit | one weight per frame |
| Dots + compression flicker (flicker.md 2) | raw newest (RAW_PERIOD) via `deposit_dots` among pair frames | one weight; measurement read drawn |
| Deep raw window 1 Hz pulse (flicker.md 3) | raw window via dense deposit among `to_record` pairs, x1.44 | one weight; measurement read drawn |
| Trigger lock-in pumping (trigger-lockin.md) | `pic_frames` varies per picture | brightness tied to frames per picture |
| Newest split delivery (stutter.md 2) | route C a picture before A | whole batches |
| Partial batches, helper and worker shares (fix/batch-newest 2ac0ca1, 988312b) | A/B shares merged at different steps | whole batches |
| AC / BW 20M at 5-13 fps (stutter.md 4) | `fast_ok` excluded them: route E for every frame | drawing tied to measurement options |
| `RECORDS_BUDGET` demo bug (8dd7eb0) | wall-clock bound in the record loop | picture content tied to UI timing |
| Fill timing, sparse events (stutter.md 1, open) | batch cadence vs 33.3 ms steps | picture depends on batch arrival |
| Unverified: live (<= 1 M) and deep (> 1 M) frames in one picture after Auto depth crosses 1 M | `deposit_pairs` 1x vs `to_record` 3.6x; `Key` without mdepth | one weight; no stale frames |

The lines residue between pair and raw frames (137 edge-column pixels, x1.016-1.022) is position, not weight:
trigger refinement on pairs is good to about one pair, and a pair straddling a column boundary lands in the column
of its centre.

## 3. Design

### 3.1 Threads and ownership

- **UI thread (first A72)** is the only owner of the picture: the window of batch maps, `ph`, `phz`, the weight
  sums, the grade, the arena copy for the GPU. Nothing else writes them.
- **Deposit workers (A53 x 4)** and **the helper** draw shares of one batch into maps they own. They never touch the
  picture.
- **Acquisition thread (second A72)** owns the hardware, the batch buffers and the record queue. It never draws for
  the screen.
- **GPU helper process**: reads the arena the UI thread filled.

### 3.2 The batch is the atomic unit

A batch is one ring read under one settings generation. The pipe gives it a `BatchMap`: maps per channel, its frame
count, its weight sum W_b, and its generation.
- Workers and the helper draw shares and add them into the `BatchMap` when done (under its lock, once per share).
- The share that brings the drawn and dropped count to the batch's frames publishes the `BatchMap` to the UI inbox.
- The UI only ever sees whole batches.
- The newest frame is a frame of the batch like the others (route C goes away). The record the UI gets for it is for
  measurement only.

This replaces the per-worker `Acc` lists, `Tally`, `parts`, `waited`, `MERGE_WAIT_MAX`, `Deliver` and the helper's
early `help_done`. Two costs:
- **Memory:** a `BatchMap` is 4 x 970 x 448 x 2 bytes = 3.5 MB. Two or three in flight, as today's per-worker maps.
- **Latency:** a batch shows when its last share is in, about the time the Fill plan already targets.

### 3.3 One weight model per display mode

Every frame contributes weight w (24) per screen column it covers, whatever its source:

- **Lines:** each source becomes per-column spans (`fastdraw::spans`, joined to the previous column), one span per
  column per frame. Sources: raw f32, read-out words, pairs, deep `to_record` pairs, and records of routes E/F
  (zoom, replay, Z, average). `draw_ex` lines rasterises through the same span builder. Z input multiplies w per
  column by the column's z. Sparse records (< 2 points per column) interpolate first, then the same spans.
- **Dots:** per column, the frame's samples are counted per row, then normalised to w per column. A pair has no
  sample density, so dots keep compression off (`compress::allowed`, kept).
- **Zoom:** a second span set per frame for `phz` with the zoom mapping. The same w, so the same grade.

### 3.4 Records and measurement never draw

- Records (`wave::Record`) feed the measurements, history, masks, protocols, XY and SCPI.
- The raw newest frame (RAW_PERIOD) and the deep raw window are measurement reads. They reach `deepmeas` and the
  measurements, never the picture.
- STOP, Single done, history and segment replay draw their stored frame once, as a batch of one frame with W = w,
  not 6w every step.
- `fast_ok` keeps only conditions on what the picture needs (zoom, XY, roll/scan). Measurement options never
  move a frame to another route.

### 3.5 Settings generation, no stale frames

- The acquisition thread numbers each configuration (`gen_acq`, raised whenever `want` differs and `configure`
  runs). Every frame and batch carries it.
- The UI's view key is the full acquisition key: today's `Key` plus mdepth, the compression factor, the source and
  the slow mode.
- A batch or record of another generation is dropped and counted (`stale`), in the pipe and in `Source::acquire`.
- The picture is cleared when the first batch of the new generation is merged, not when the knob turns. Until then
  the last correct picture stays.

Latency from a settings change to the first correct picture: configure (25-50 ms measured) + the first fill and read
(at most one fill, <= 33 ms) + drawing (Fill plan: ready before the next step) + at most one screen step. About 70-130
ms. Today it is about the same, but stale frames can appear before it.

### 3.6 Grading

Three options. Numbers for a frame weight w = 24, 30 pictures/s.

**(A) Per picture, exact weight.** Clear per picture, picture = the batches merged in this step. Brightness = count / W
with W = sum of the weights of the frames in the maps.
- Brightness of the core is exact whatever the frame count.
- But the visible noise band depends on the frames n in the picture. A pixel hit with probability p per frame shows
  with probability 1-(1-p)^n. At p = 0.1: n = 1: 10 %, n = 6: 47 %, n = 13: 75 %, n = 27: 94 %.
- Batches of 13 and 27 frames alternating (sparse events, stutter.md) or a 1-frame picture (the 2ac0ca1 shots)
  change the band visibly.
- A step without a batch shows the previous picture again: unchanged, no dip.

**(B) Exponential decay over wall-clock time, constant tau, normalised by the decayed W.**
- N_eff = rate x tau. At 180 frames/s and tau = 33 ms, N_eff = 6.
- A missing batch changes the picture only by its share of N_eff.
- But the previous picture remains at e^-1 = 37 % after one screen frame and 14 % after two: a moving trace (offset
  knob, drift) smears over 2-3 pictures. That is a ghost at a setting called "minimum".
- u16 counts: a pixel of count 24 decays below 1 after tau x ln 24 = 3.2 tau. At tau = 33 ms that is 105 ms, so below
  about 10 frames/s the trace vanishes between frames. Avoiding that takes either float maps (2x memory and bandwidth)
  or holding the decay while no frames come.
- Cost: one pass over all maps per step, 4 x 970 x 448 u16 = 1.7 M operations, about 1-2 ms on the A72 with NEON,
  plus `phz`.

**(C) A frame-count window, exact weight.** The picture is the newest whole batches until they hold at least N frames.
N = the frames that arrived per screen frame, smoothed. Brightness = count / W.
- The band visibility then varies only within [N, N + one batch), not from 1 to 27.
- A step without a batch keeps the same window: the same picture.
- An older batch stays in the picture only while the newest ones hold fewer than N frames: at most one batch,
  about 33 ms, and only when batches are small.
- No decay pass, no float maps.
- Cost: keep a running sum of the window (add the new batch maps, subtract the batches leaving it; integer, exact).
  1-2 map adds per step, 1-2 ms on the A72. The subtraction is exact only without saturation. Batch maps stay below
  u16 (27 frames x 24 = 648 per pixel), so the window sum is rebuilt from its batches whenever a pixel saturates.

**Choice:**
- **Minimum persistence: C.** It keeps replacement semantics (no exponential ghost), makes the brightness exact, and
  bounds band jitter by the batch size. A cannot hold band jitter at sparse events or partial pictures. B puts a
  ghost on a setting meant to have none.
- **Persistence > minimum: B with tau = the persistence (100 ms .. 10 s), normalised by the decayed W.** There the
  ghost is the feature.
  - The decay stops when no frames come (STOP, Single done, replay), so the last picture does not fade.
  - The 100 ms setting meets the u16 floor below about 3 frames/s. There the decay is applied only on steps that
    bring frames.
- **Infinite:** no decay. On saturation counts and W halve together (`Phosphor::halve`).
- **Colour grade:** count / W per pixel through the heat LUT. Same normalisation, merged channels summed.

compress's view (answers, 2026-11-14): time decay plus one weight model everywhere, batch-atomic merging then
unnecessary for brightness. I take B above minimum. At minimum, C needs whole batches, so 3.2 stays.

### 3.7 GPU upload and CPU budget per step

- **GPU:** the UI copies the changed rows of the picture maps into the arena (`gpu.rs`, today: copy 0.6 ms; helper
  upload 2.3 ms, draw 7 ms, on its own core time). New: one reference per channel, 1/W, in the frame message instead
  of `vmax`. The LUT is unchanged in form. No protocol change beyond that field.
- **Screen step budget** (33.3 ms on the first A72; measured today at setup B 200 ns: step 6 ms, render 2.9 ms):
  - merge into the window: <= 2 ms
  - grade: <= 0.5 ms
  - render + arena copy: <= 6 ms
  - target step p99 <= 15 ms, leaving room for menus and measurements.
- **A53:** per frame today 0.5-2.7 ms of worker time (setup A/B, probe comp). At 1100 frames/s x 1.9 ms that is 2.1
  cores of 4. Target <= 70 % average per little core in the worst measured mode. The FrameCap keeps batches to what
  the workers draw in time.

## 4. Acceptance criteria (unitlab)

Setups:
- probe comp CH1 (100 mV/div, 0.15 V, DC) at 200 ns, 5 us, 200 us (100 k) and 1 ms (10 M, deep);
- setup B at 200 ns and 5 us, trigger 5.05 V (the rail now; stutter.md sparse section);
- setup A at 10 us;
- 2 channels at 5 us.

Each in lines and dots. Persistence Min, plus 500 ms at probe comp 200 us.

| Criterion | Target | How |
|---|---|---|
| fps | >= 29.5 | `:SYSTem:STATe?` fps |
| gap p99 | <= 45 ms where trigger events >= 2 per screen frame | pacing line |
| empty pictures | <= 1 % (same condition) | pacing line |
| picture steadiness | over 20 screenshots: trace pixel count std <= 2 %, mean brightness std <= 2 % | `tools/oos-stress/shotstats.py trace` |
| stale frames drawn | 0 | new `stale` counter; host test |
| settings change to first correct picture | <= configured + 100 ms, no stale picture | pacing settle fields (`first`, `steady`) over 20 time-base and V/div changes |
| forced or partial merges | 0 | pipe counters |
| A53 load | <= 70 % average per little core | `/proc/stat` (`oos-baseline.py`) |
| UI step p99 | <= 15 ms | status step, pacing pass |
| dropped frames | not more than today in the same set-up | counters |

## 5. Implementation plan

Each step: one branch, host tests that fail before and pass after, gates (release and debug, arm64, perf-guard,
`gpu/test-host.sh`), then the unit check named. Days are host work.

| # | Step | Days | Host tests (fail before) | Unit check | Deletes |
|---|---|---|---|---|---|
| 0 | Counters: `stale`, frames and W per picture, partial merges | 0.5 | counters present and counting in pipe tests | status line shows them | none |
| 1 | Settings generation on frames and batches; full key incl. mdepth/k; drop stale; clear on first new batch | 1 | old-generation frames never drawn (pipe and `Source::acquire`); live then deep after depth crossing 1 M | settle over 20 changes, `stale` drawn 0 | `hw_stamp` use for the view |
| 2 | One lines kernel: spans for raw, codes, pairs, records (`draw_ex` lines) | 3 | `pair_raw_mix_report` x1.000 for raw/pairs/deep/record mixes; record-path picture equals deposit picture | probe comp 200 us and 1 ms screenshots | `draw_ex` segment joining (lines), deep-raw branch in `draw_frame` |
| 3 | Dots normalised per column | 1.5 | dots brightness within 3 % across raw/record frames and sample counts | dots set | none |
| 4 | `BatchMap`: shares added per batch, published whole; newest frame drawn by the pipe; route C, D gone | 2.5 | 2ac0ca1/988312b tests rewritten: every merged map holds exactly its batch's frames; the newest frame in the same merge | stress set, `partial` 0 | `Acc` lists, `Tally`, `Deliver`, `MERGE_WAIT_MAX`, `fast_draw`, frames mode, the `fast_ok` frame branch in `Ui::step` |
| 5 | Measurement reads never draw (RAW_PERIOD raw newest, deep window); `RECORDS_BUDGET` gone | 0.5 | a raw measurement read leaves the picture unchanged; record loop takes only the newest | probe comp 200 us dots/lines | `compress::hold` draw branch, `deepmeas` draw cases, `RECORDS_BUDGET` |
| 6 | Grading C at Min, B above, freeze when stopped; zoom and colour grade on the same W | 2.5 | brightness within 3 % for 1..27 frames per picture; band pixel count within 2 % for batches of 13/27 alternating; stopped picture constant over 100 steps | full acceptance set | `picture_ref`, `per_frame*`, `pic_frames`, `vmax` follow |
| 7 | Routes E/F through the same kernel and W (zoom, Z, average, replay at weight w once) | 1.5 | zoom picture brightness equals main; replay picture equals the live picture of the same frame | zoom, replay, Single | 24 x 6 redraw |
| 8 | GPU: 1/W per channel in the frame message | 0.5 | `gpu/test-host.sh` 0 differ | GPU on/off screenshots equal | `vmax` in the protocol |

About 13 days of host work, plus a unit round after steps 1, 2, 4, 6 and 8 (about 2 days). Risk:
- **High:** steps 2 and 6 change the look of every mode. Every record-path picture gets about 1/3.6 of its counts,
  and the grade changes. They need screenshots before and after in each mode, and the user's eye.
- **Medium:** step 4 (concurrency).
- **Low:** the others.

Existing fixes:

| Fix | Fate |
|---|---|
| flicker-weight (newest via `draw_frame`) | absorbed by 4, code removed |
| dots-flicker (`compress::allowed` false with dots On) | kept |
| deep raw window via the record path (f536413's draw part) | removed by 5; the window does not draw |
| deliver after merge (`Deliver`) | absorbed by 4 |
| fix/batch-newest 988312b (`Tally`) | interim only: merge it if the stress round needs it before step 4, which replaces it. The hold-chain residual is not fixed |
| `picture_ref` (trigger lock-in) | removed by 6 |
| `RECORDS_BUDGET` (hw only since 8dd7eb0) | removed by 5 |
| AC/BW 20M on the fast path | kept; `fast_ok` shrinks further |
| Fill plan, FrameCap, helper bound | kept; the sparse-event planner stays a separate open item |
| near-only trigger gate, AC level | kept |
| live compression, RAW_PERIOD read | kept for measurement, not drawn |

Not in scope: XY and LA (canvas), the sparse-event planner, the auto timeout.
