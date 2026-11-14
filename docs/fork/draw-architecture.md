# Screen drawing: where frames enter the picture, and a target design (2026-11-14)

Design note, no code. Question: is the drawing architecture the deeper cause of the day's chain of display fixes?
Answer: yes. Most of the fixes kept two invariants that the structure does not hold by itself. First, every frame
weighs the same in a picture. Second, a picture holds whole batches. A third property sits behind both: brightness
is graded per picture by frame count.

## 1. Current data flow

Acquisition (`board/dho/hw.rs`, batch mode): a batch is read from the FPGA ring. Its newest frame is decoded and
queued for the UI (`Shared::queue`). The read-out buffer goes to the deposit pipe (`drawpipe::Pipe::feed_with`) with
the other frames. With live compression (k > 1) the frames are min/max pairs. At most once per `compress::RAW_PERIOD`
(200 ms) the newest frame is read again raw. Deep records (> 1 M) are always pairs. Once per `deepmeas::PERIOD`
(1 s) a raw screen window replaces the newest frame.

Routes into the phosphor (`wave::Phosphor`, u16 hit counts per channel, `ph`, and `phz` for zoom):

| Route | Thread | Frames | Kernel, weight |
|---|---|---|---|
| A. workers | deposit workers (A53) | batch frames but the newest | `drawpipe::draw_frame` or `draw_frame_codes` |
| B. helper | acquisition thread during the ring fill | single frames of the open batch | same as A |
| C. UI newest | UI (`Ui::step`) | the batch's newest, as a `Record` | `draw_frame` on `rec.raw.frame` when `fast_ok`, else `draw_ex` on the 3600-point record |
| D. UI queue (frames mode, no pipe) | UI | every frame but the newest | `Ui::fast_draw` -> `draw_frame` |
| E. record path (not `fast_ok`) | UI | every frame, via `Source::acquire` -> `to_record` | `draw_ex` into `ph`, and `phz` with zoom |
| F. stopped | UI, every step | the stored record (STOP, Single done, history replay) | `draw_ex` at 24 x 6 |

Inside `draw_frame` (`drawpipe.rs:38`) a frame takes one of these kernels, weight 24 each:

- dense raw, lines: `fastdraw::deposit` (one min/max span per column), or `deposit_codes` from the read-out words;
- dense raw, dots: `fastdraw::deposit_dots` (every sample its pixel: the count follows the samples per column);
- live pairs: `compress::deposit_pairs` (a span per column, with dots too);
- sparse (< 2 samples per column): `sparse::draw` (interpolation);
- envelope, roll, scan and deep raw windows: `board::to_record` (3600 points, pairs) -> `Phosphor::draw_ex`.

Route E covers zoom, Z input (weight 24 x z per point), XY, Single, segments (and segment replay), the software
mask and zones, Peak/Average/HiRes (the software average blends into `last[0]`, then one record per frame; `hw_avg`
frames skip the blend), roll and scan, and the non-hardware sources. XY and LA draw straight on the canvas, without
phosphor or grade. The GPU (`graphics = gpu`) draws the same maps with the same LUT.

Two weight models exist. The deposits put one span per column per frame (24). `draw_ex` in lines joins consecutive
record points: at 3600 points about 3.6 segments per column, so about 3.6 x 24 per pixel per frame. Records are drawn
by `draw_ex`, so routes C (record branch), E, F, sparse frames and the deep record path weigh about 3.6x a deposited
frame. With dots and pairs `draw_ex` draws one span per pair (24).

Grading (`ui.rs:2545 grade_refs`, `picture_ref` at `ui.rs:7903`):

- Persistence > 0: the maps decay by exp(-dt/persist) per step, and `vmax` follows the brightest pixel.
- Minimum persistence: the maps are cleared when a step brings frames (`clear_pending`). `pic_frames` = frames of the
  step. The reference is the per-frame maximum times `pic_frames`. This assumes that every frame of the picture has
  the same weight and that `pic_frames` counts what is in the maps.
- Brightness = floor + sqrt(count / reference) (`trace_lut`).

So the brightness of a pixel in a picture depends on (a) how many frames came in that step, (b) which route and
kernel each one took, (c) whether the picture holds all of a batch, and (d) how the per-frame maximum was smoothed
over earlier pictures.

## 2. The day's bugs on that flow

| Bug | Route or step | Property it broke |
|---|---|---|
| Flicker at 500 us/div (flicker.md 1) | C drew `draw_ex` at 3600 points, about 3.6x a deposit | equal weight per frame |
| Dots + compression flicker (flicker.md 2) | the raw newest (RAW_PERIOD) via `deposit_dots` among `deposit_pairs` frames | equal weight; measurement read drawn |
| Deep raw window, 1 Hz pulse (flicker.md 3) | the raw window via dense deposit among `to_record` pairs, x1.44 | equal weight; measurement read drawn |
| Trigger lock-in pumping (trigger-lockin.md) | `pic_frames` changes from picture to picture | brightness tied to frames per picture |
| Newest split delivery (stutter.md 2) | C a picture before A | whole batches per picture |
| Partial batches (2ac0ca1) | A and B shares merged at different steps | whole batches per picture |
| 2ac0ca1 review: delivery race, ready_ms base | merge snapshot vs. live completion | whole batches; pacing measured on the wrong event |
| AC / BW 20M at 5-13 fps (stutter.md 4) | `fast_ok` excluded them: every frame through C's record path | drawing tied to measurement options |
| `RECORDS_BUDGET` demo bug (8dd7eb0) | wall-clock bound in the loop that makes records | picture content tied to UI timing |
| Fill timing, sparse events (stutter.md 1, open item) | batch cadence vs. 33.3 ms steps | a picture needs a new batch to keep its brightness |
| Not seen yet, unverified: live (<= 1 M) and deep (> 1 M) frames in one picture after Auto depth crosses 1 M | `deposit_pairs` 1x vs. `to_record` -> `draw_ex` about 3.6x; `drawpipe::Key` has no mdepth; whether the UI queue drops frames of an older stamp is unchecked | equal weight per frame |

The lines residue between pair and raw frames (137 edge-column pixels, x1.016-1.022 in `pair_raw_mix_report`) is
position, not weight. The trigger refinement on pairs is good to about a pair, and `deposit_pairs` puts a pair that
straddles a column boundary wholly into the column of its centre.

Each fix was local: draw the newest through `draw_frame`, forbid compression with dots, send the deep window through
`to_record`, deliver after the merge, tally the shares. Each added a condition to a route. The invariants hold only as
long as every new route or representation keeps them by hand.

## 3. Target design

1. **The batch is the atomic unit.** The pipe builds one map set per batch. Workers and the helper draw into shares
   of it. The last share to finish adds them up and publishes the batch map. The UI merges whole batch maps only.
   This replaces the per-worker `Acc` lists, `Tally`, `parts`, `waited`, `MERGE_WAIT_MAX` and the `Deliver` closure.
2. **The UI thread draws no live frame.** The newest frame is drawn by the pipe like the others. The UI queue
   carries records for measurement, history, masks and protocols only. That removes route C and its special cases.
   STOP, Single, segments and history draw a stored frame through the same kernels (one batch of one).
3. **One representation per display mode, one kernel.**
   - Lines: every source (raw f32, read-out words, pairs, deep `to_record` pairs, records of route E/F) becomes
     per-column spans (`fastdraw::spans`), deposited at weight w per column per frame. That ends the 3.6x of
     `draw_ex`; `draw_ex`'s segment joining stays for sparse records only, normalised to w per column.
   - Dots: per-column sample histograms normalised to w per column. Then a raw frame, a pair frame and a deep frame
     weigh the same per column. Only the distribution inside the column differs.
4. **Measurement reads never draw.** The raw newest (RAW_PERIOD) and the deep raw window feed `deepmeas` and the
   measurements only. The picture is made of the frames the display mode chose.
5. **Brightness by time, not by frames per picture.** Keep a decayed weight W(t) = sum of w_i·exp(-(t-t_i)/tau)
   next to the maps (decayed the same way). Grade count / W against a fixed reference. tau is the persistence; at
   minimum, about one screen frame. One frame more or less in a picture then changes the brightness by about
   1/(rate·tau) instead of 1/frames.

Where this is wrong or naive:

- **Dots on pairs.** A pair cannot carry the dots density. Normalising the weight only stops the brightness jump; the
  shape inside the column still differs, so dots stay raw (`compress::allowed`).
- **Minimum persistence means replacement.** With tau of one screen frame, a ghost of the previous picture stays
  (e^-1, about 37 %). Users expect replacement. The normalisation (count / W) removes the frame-count dependence of
  the brightness; the decay is what makes partial batches, helper shares and empty pictures harmless, because older
  frames still contribute. compress's view: time decay plus one weight model is the target and makes batch-atomic
  merging unnecessary for brightness. Mine: the decay changes the look of minimum persistence (ghost, band width), so
  it is a user decision. Clearing per picture with count / W keeps today's look and still needs whole batches.
- **Decay must stop with the frames.** STOP, Single done, history and segment replay must freeze the decay (or
  the last picture fades). The stopped redraw (24 x 6) needs the same normalisation.
- **Decay cost.** All maps every step: 4 x 970 x 448 u16, about 1.7 M operations, about 1-2 ms with NEON on the A72,
  plus `phz`. Steady state at 800 frames/s, weight 24 and tau 33 ms is about 640 counts per pixel: fits in u16.
- **Low frame rates.** At 1 frame/s, count / W shows the last frame at full brightness, which is right. A plain
  decay would fade it.
- **Latency and memory.** A whole-batch picture shows a batch only when its last share is in, up to one batch period
  later. A batch map is 4 x 970 x 448 x 2 bytes, 3.5 MB. Two or three in flight are 7-10 MB, larger than the A53's
  L2. Today's per-worker maps are the same size, so it is no worse.
- **The record path stays** for zoom (a second map per frame), Z input, averaging, XY, roll and scan. Item 2 covers
  live Normal acquisition, not every mode.
- **Pacing stays** (Fill, FrameCap). Atomic batches do not make batches arrive every 33.3 ms. With sparse events a
  picture without a new batch is an unchanged picture; with count / W at least it keeps its brightness.

## 4. Migration

| Step | Change | Days | Risk | Proof | Deleted afterwards |
|---|---|---|---|---|---|
| 1 | lines kernel from spans for raw, codes, pairs, records (`draw_ex` lines) | 3 | medium (every record-path picture gets about 1/3.6 of its counts, so grading and intensity change; edge columns move about one pair) | `pair_raw_mix_report` x1.000 for raw/pairs/deep mixes; a live and a deep frame in one picture equal; record-path and deposit pictures equal | `draw_frame`'s deep-raw record branch |
| 2 | dots normalised per column | 1.5 | medium (dots look) | dots brightness within 3 % across raw/pairs/deep frames | none (compression stays off with dots) |
| 3 | newest frame through the pipe, UI draws no live frame | 1 | low | `batch_picture...equals_the_frames_mode`, flicker tests unchanged | the `fast_ok` frame branch in `Ui::step` (`ui.rs` ~1931), `fast_draw`, route D |
| 4 | per-batch maps, published by the last share | 2 | medium (concurrency) | 2ac0ca1's tests; invariant: every merged map holds exactly its batch's frames | `Tally`, `Acc::parts/waited`, `MERGE_WAIT_MAX`, `Deliver`, the helper's early `help_done` |
| 5 | grade by count / W of the frames in the maps | 2 | high (look of every mode, colour grade, GPU LUT) | brightness within 3 % for 1-5 frames per picture and for raw/pair mixes; screenshots at each persistence | `picture_ref`, `per_frame*`, `pic_frames` |
| 6 | measurement reads off the picture; `fast_ok` only for UI conditions; `RECORDS_BUDGET` gone | 1 | low | AC/BW 20M rate test; deep-raw measurement tests | `deepmeas` draw special cases, `compress::hold` drawing branch |

First, cheap: a host test for live and deep frames in one picture after an Auto depth change across 1 M (the
unverified row in section 2). About 11 days on the host plus 2 on the unit: the stress set (probe comp at 200 us/div, setups A and B, deep
1 ms/div, dots on/off) before and after each step, with pixel std over 20 screenshots as the flicker measure.
Steps 1, 3 and 6 are worth doing on their own. Step 5 only after 1-4, because it relies on exact weights.
