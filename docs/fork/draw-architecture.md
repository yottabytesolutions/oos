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

Batch mode also uses route D:
- When the pipe is not accepting, every frame of a batch is decoded into the queue.
- When no read-out buffer is free (`take_buffer` None), the other frames are dropped and only the newest is queued.

Both give batch-mode pictures of a different composition. `acq.pipeline` defaults to `frames` (`drawpipe::mode_from`):
route D is upstream's path and every non-unit user's.

Kernels in `draw_frame` (`drawpipe.rs:38`), weight 24:
- `fastdraw::deposit` / `deposit_codes`: one min/max span per screen column.
- `deposit_dots`: every sample its pixel.
- `compress::deposit_pairs`: one span per column, built from the pairs whose centre lies in it.
- `sparse::draw`: interpolation, then `draw_ex`.
- `to_record` + `draw_ex`: envelope, roll, scan and deep raw windows.

**Two weight models.** A deposit puts one span per column per frame (24). `draw_ex` in lines joins consecutive record
points: 3600 points over 970 columns is about 3.7 segments per column, so about 3.7 x 24 per pixel per frame. Every
route E/F picture, sparse frames and deep pair frames weigh about 3.7x a deposited frame. Measured
(`ui::tests::depth_mix_report`, compress, a766aca): one deep frame 3.73x a live pair frame in summed counts, 5.0x
at the brightest pixel.

Dots weights:
- `deposit_dots`: one hit per sample, about 86 per column at 100 k / 200 us/div. On a flat trace that is about 30
  hits per pixel per frame (86 samples on 2-3 pixels), x 24, so about 720 counts per frame.
- `draw_ex` dots on a record: one hit per record point, about 3.7 per column, so about 1/20 of a deposited dots
  frame.
- Pairs with dots: one span per pair.

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
`stamp`, but `hw_stamp` changes only for Single (`ui.rs:1762`), Bode (`ui/bode.rs` 418, 584) and autoset
(`ui/autoset.rs` 180, 324, 362). `settings_changed` sets `dirty_map` only in Run. On a reconfiguration the
acquisition thread does not clear `Shared::queue` (`hw.rs` ~1510). It clears it only on RUN after a pause and for roll
partials. A fill that `changed()` ends early on a new `want` is still read and queued under the old configuration:
another stale source. So frames acquired under
the old settings can be drawn with the new mapping, by the workers (same stamp, same `gen`) and by routes C/D.
Verified on the host for a depth change (`depth_mix_report`): 2 live + 1 deep frame in one step gave exactly
2 x live + deep counts. Nothing filters them. On the unit the window is small (the first deep batch comes at least
configure + fill + about 68 ms read after the change), but a late UI step can merge both. With persistence > 0 the
mix lasts for the decay time.
Measured settle on the unit (stutter.md 5): configured 25-50 ms after the change, steady 5-150 ms, at most 1 empty
picture.

## 2. The day's bugs on that flow

| Bug | Where | Property broken |
|---|---|---|
| Flicker 500 us/div (flicker.md 1) | route C `draw_ex`, 3.6x a deposit | one weight per frame |
| Dots + compression flicker (flicker.md 2) | raw newest (RAW_PERIOD) via `deposit_dots` among pair frames | one weight; measurement read drawn. Avoided, not restored: no compression with dots |
| Deep raw window 1 Hz pulse (flicker.md 3) | raw window via dense deposit among `to_record` pairs, x1.44 | one weight; measurement read drawn |
| Trigger lock-in pumping (trigger-lockin.md) | `pic_frames` varies per picture | brightness tied to frames per picture |
| Newest split delivery (stutter.md 2) | route C a picture before A | whole batches |
| Partial batches, helper and worker shares (fix/batch-newest 2ac0ca1, 988312b) | A/B shares merged at different steps | whole batches |
| AC / BW 20M at 5-13 fps (stutter.md 4) | `fast_ok` excluded them: route E for every frame | drawing tied to measurement options |
| `RECORDS_BUDGET` demo bug (8dd7eb0) | wall-clock bound in the record loop | picture content tied to UI timing |
| Fill timing, sparse events (stutter.md 1, open) | batch cadence vs 33.3 ms steps | picture depends on batch arrival |
| Live (<= 1 M) and deep (> 1 M) frames in one picture after a depth change (host-verified, not seen on the unit) | `deposit_pairs` 1x vs `to_record` 3.7x; `Key` without mdepth; queue not cleared | one weight; no stale frames |

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

A batch is one ring read under one settings generation. The pipe gives it a `BatchUnit`: its frame count, its
generation and the list of its shares.
- Workers and the helper draw shares into maps they own, as today.
- A finished share goes into its batch's unit, not into a mailbox.
- The share that brings the drawn and dropped count to the batch's frames publishes the unit, all shares, to the UI
  inbox.
- The UI adds the shares at merge, as now, on the A72, each only over the rows it touched (per-share row bounds,
  kept by the worker). No map add on the A53 and no lock on the hot path. Adding into
  one `BatchMap` would be about 120 map adds a second on the little cores, serialised on the batch lock.
- The UI only ever sees whole batches. This is 988312b's gating per batch instead of per mailbox, so its hold-chain
  residual cannot occur.
- The newest frame is a frame of the batch like the others (route C goes away). The record the UI gets for it is for
  measurement only.

This replaces the per-worker `Acc` lists, `Tally`, `parts`, `waited`, `MERGE_WAIT_MAX`, `Deliver` and the helper's
early `help_done`. Two costs:
- **Memory:** a share's maps are 4 x 970 x 448 x 2 bytes = 3.5 MB, used channels only. As many in flight as today's
  per-worker maps.
- **Latency:** a batch shows when its last share is in, about the time the Fill plan already targets.

### 3.3 One weight model per display mode

Every frame contributes a known weight per screen column (w = 24 in lines), whatever its source:

- **Lines:** each source becomes per-column spans (`fastdraw::spans`, joined to the previous column), one span per
  column per frame. Sources: raw f32, read-out words, pairs, deep `to_record` pairs, and records of routes E/F
  (zoom, replay, Z, average). `draw_ex` lines rasterises through the same span builder.
  - What changes is the shape inside the band, not the brightness of a picture of one route. `draw_ex`'s overlapping
    segments make a graded core with dimmer edges. A span per column is uniform. Zoom, Single, segments,
    Peak/Avg/HiRes, roll/scan and the stopped view look different; mixed pictures stop jumping.
  - Z input keeps its current drawing (`draw_ex`, 24 x z per point; team-lead's decision). A per-column z weight would
    be inexact, and Z is a route of its own that never mixes with deposited frames. Documented as the one exception.
  - Open design point, sparse records (fewer points than columns): the interpolation is the content. They are
    interpolated to at least 2 points per column (`sparse.rs`), then the same spans, so the segment joining survives
    as the interpolation, not as a second kernel.
- **Dots:** normalising to w per column does not survive u16. At 86-100 samples per column the share per sample is
  0.24-0.28 counts and truncates to 0. So dots keep one count per sample in u16 maps, and the frame's weight
  W_frame is its samples per column (86 for a raw 100 k frame at 200 us/div, about 3.7 for a 3600-point record).
  Brightness = count / W then equals across sources, with no float maps and no extra memory.
  - W_frame is per map: the zoom map has its own samples per zoom column, so `phz` has its own W, as it has its own
    grade today.
  - No pixel rounds to 0. A noisy column spreading 86 samples over 40+ rows gets about 2 counts per pixel, but every
    count is an integer hit. The division by W happens only in the grade, where any count > 0 gets at least the LUT
    floor.
  - Frames of one generation have the same samples per column, so they weigh the same in a picture.
  - Raw and record frames in one dots picture weigh by their own W, correctly normalised. Such a mix happens only
    during a change, because the newest frame and the deep window no longer draw (3.4).
  - Headroom: a flat dots trace puts about 30 counts per pixel per frame. Fine at Min; long persistence halves, as
    below.
  - A pair has no sample density, so dots keep compression off (`compress::allowed`, kept).
- **Zoom:** a second span set per frame for `phz` with the zoom mapping. The same w, so the same grade.

### 3.4 Records and measurement never draw

- Records (`wave::Record`) feed the measurements, history, masks, protocols, XY and SCPI.
- The raw newest frame (RAW_PERIOD) and the deep raw window are measurement reads. They reach `deepmeas` and the
  measurements, never the picture.
- On the fast path (`fast_ok`) the UI takes only the newest record of a step for measurement: every frame is in the
  picture through the pipe. Off the fast path every record still goes through the record loop: it feeds mask counts,
  the zone pass, the software average, the Bus trigger decision, Single, segments and history. There the records of
  one step form one pseudo-batch (W = their weight sum) into the picture.
- `fast_ok` keeps only conditions on what the picture needs (zoom, XY, roll/scan). Measurement options never move a
  frame to another route.
- Roll and scan frames are the whole sweep so far: each replaces the picture (a window of 1, outside 3.6's window).

**STOP.** STOP freezes the accumulated maps and W as they are. Nothing is redrawn while stopped. Single done is the
same. A view change while stopped (time base, offset, zoom) replaces the maps with `last[0]` drawn once at W = w.
History and segment replay do the same for the record they show. Today's 24 x 6 redraw on every step goes.

With persistence above Min, the frozen accumulated picture is in pixels of the old mapping and cannot be re-mapped.
On the first view change while stopped it is dropped and replaced by `last[0]` drawn once. OOS today loses it the
same way (the stopped redraw clears the maps). Against the stock firmware it may be a change: not checked whether
stock rescales a stopped persistence picture. It goes in the user notes as a known behaviour.

**Persistence changes.** A change between Min and a decay setting, or between decay settings, clears the maps and W
and starts from `last[0]` drawn once (the preview of 3.5).

### 3.5 Settings changes: preview, mapping key, hardware generation

A knob spins at 8-10 detents a second; a new configuration settles in 70-130 ms. Blanking until the new configuration
shows would blank for the whole spin. So:

- **Frames carry their own conversion.** A batch is decoded with the settings it was acquired under (`Pipe::feed_*`
  takes the batch's `Settings`; `draw_frame_codes` takes its `code_map`). Its samples are volts at times relative
  to its trigger point. So a frame drawn under a newer display mapping is placed correctly: an old V/div, offset or
  time base frame lands where its signal belongs, only its clip range or window width differs.
- **The batch carries its acquisition snapshot** for the trigger refinement: trigger source, level, slope,
  coupling, noise reject and the source's V/div (for the hysteresis). A frame kept across a level change is refined
  against the level it was acquired at. It never reads the live `Model`, which today's `draw_frame` does through the
  `View`.
- **The mapping key** is what turns volts and times into pixels: time base, delay, V/div, offset, channel set, dots,
  zoom, size. It is fixed per batch at its first claim (the batch keeps the `View` it was first drawn under, so all
  its shares share one mapping). A published batch of another mapping key is dropped by the UI (pixels in the wrong
  place); its frames are not redrawn.
- **The hardware generation** (`gen_acq`, raised when `configure` runs: depth, k, coupling, bandwidth, trigger,
  acquisition mode) does not invalidate the picture. Live and deep frames after a depth change may share a picture;
  with one weight model (3.3, step 3) that is correct, so `depth_mix_report` becomes step 3's test, not step 1's. A level, slope or coupling change keeps the old batches valid as
  pictures. It matters to the records: measurements and Single take only the current generation. The queue is
  cleared on reconfigure, and a fill that `changed()` ended under the old configuration is dropped.
- **Preview.** On a mapping change the UI draws `last[0]` once under the new mapping as the picture (W = w, window of
  1), at the next step. The first batch drawn under the new mapping replaces it. During a spin the picture is a
  preview or a real batch at every step, never blank and never at old pixel positions under new labels.

Input to photon: the preview at the next step (<= 33 ms). The first batch under the new mapping: the batches in
flight are dropped, the next one is drawn under the new view (fill + read + draw, <= about 70 ms). A batch of the new
hardware configuration: configure (25-50 ms measured) + fill + read + draw, about 70-130 ms.

### 3.6 Grading

Three options. Numbers for a frame weight w = 24, 30 pictures/s.

**(A) Per picture, exact weight.** Clear per picture, picture = the newest whole batch. Brightness = count / W, W = sum
of the frame weights in the maps.
- Brightness of the core is exact whatever the frame count.
- The visible noise band depends on the frames n in the picture. A pixel hit with probability p per frame shows with
  probability 1-(1-p)^n. At p = 0.1: n = 1: 10 %, n = 6: 47 %, n = 13: 75 %, n = 27: 94 %.
- Batches of 13 and 27 alternating (sparse events) or of 3 change the band visibly.
- No ghost: a picture holds one batch.

**(B) Exponential decay over wall-clock time, constant tau, normalised by the decayed W.**
- N_eff = rate x tau. At 180 frames/s and tau = 33 ms, N_eff = 6. A missing batch changes the picture by its share.
- The previous picture remains at e^-1 = 37 % after one screen frame and 14 % after two: a moving trace smears over
  2-3 pictures. A ghost at a setting called "minimum".
- Cost: one decay pass over all maps per step, about 1-2 ms on the A72 plus `phz`, and the precision problem below.

**(C) A capped frame-count window, exact weight.** The picture is the newest whole batches until they hold at least
N frames (N = frames per screen frame, smoothed), capped at 3 batches and at an age of 2 screen frames (67 ms).
Brightness = count / W. Rebuilt each step from the window's batches (1-3 map adds, 1-2 ms on the A72, no subtraction,
no saturation: a batch is at most 27 x 24 = 648 per pixel).
- Band visibility varies within [N, N + one batch), not from 3 to 27.
- A batch shown in two consecutive pictures is a one-batch ghost at full weight, at most 67 ms old.
- A rate drop cannot leave seconds of ghost: the age cap empties the window to the newest batch.
- Live triggered batches only; roll, scan, STOP and replays use a window of 1.

The 2ac0ca1 shots were a band-width effect: a picture of one frame has a narrower union of spans. count / W
normalises brightness but cannot widen the band. So A and C need whole batches, and C also a floor on the frames in a
picture; B needs neither. The user chooses between A and C (5-line comparison and pictures below); B is not
offered at Min because of its ghost.

**Persistence > Min: B with tau = the persistence, in u32 fixed point.** Today's decay, `(x * k) >> 16` in u16,
truncates: it removes about 1 per step from small counts, so a single hit (24) is gone in 0.8 s at "10 s". Options:
- deferred decay (apply when the accumulated factor drops below about 0.94): still truncates, about 1 count per
  application, so a single hit is still gone well before tau at 10 s;
- stochastic rounding: unbiased, but faint pixels sparkle from picture to picture;
- u32 counts in 16.16 fixed point: exact to 2^-16 per step. Memory 2x: 4 x 970 x 448 x 4 bytes = 7 MB for `ph`, 7 MB
  more with zoom. The decay pass costs about the same on the A72.

Chosen: u32 fixed point for the persistence maps only (Min keeps u16 batch maps and the window). A single hit (24)
stays above one count for tau x ln 24 = 3.2 tau, so it is visible for at least tau. A pixel below one count goes to 0,
so no residual ghost. W decays in the same fixed point. Near saturation counts and W halve together. The decay stops
when no frames come (STOP, Single done, replay). Infinite: no decay; on saturation halve with `.max(1)`, which keeps
rare hits.

**Colour grade:** count / W per pixel through the heat LUT, merged channels summed. Zoom: `phz` with its own W.

### 3.7 GPU upload and CPU budget per step

- **GPU:** the LUT maps count / vmax today. With vmax = W it computes count / W exactly, so neither the protocol nor
  the shader changes: the UI passes W where it passed `vmax`. The UI copies the changed rows of the picture into the
  arena (today: copy 0.6 ms; helper upload 2.3 ms and draw 7 ms on its own core).
- **Screen step** (33.3 ms on the first A72; today at setup B 200 ns: step 6 ms, render 2.9 ms): merge and window
  rebuild <= 2 ms, decay pass (persistence only) <= 2 ms, grade <= 0.5 ms, render and arena copy <= 6 ms. Target step
  p99 <= 15 ms.
- **A53:** per frame today 0.5-2.7 ms of worker time. At 1100 frames/s x 1.9 ms that is 2.1 cores of 4. Target <= 70 %
  per little core in the worst mode. No map adds on the A53 (3.2).
- **Memory:** the window holds at most 3 batches of shares (3 x up to 4 shares x 3.5 MB worst case, used channels
  only), plus 7-14 MB of u32 maps with persistence. Bound: RSS growth <= 60 MB against today.

### 3.8 Pacing with sparse events

"No hiccups" includes setup B at its present rail (1650 events/s at 5.1 V): batches came every 34-40 ms against
33.3 ms screen steps, 2-5 % empty pictures (stutter.md). C keeps such a picture steady (the window holds), but the
trace still updates late. The Fill plan should end the fill so a batch is ready every screen step when events are
rare: the fill is planned from the measured rest (arm, first trigger, read) and shortened when rest + fill exceeds a
frame, down to `FILL_MIN`. Scheduled as step 8.

## 4. Acceptance criteria (unitlab)

Setups:
- probe comp CH1 (100 mV/div, 0.15 V, DC) at 200 ns, 5 us, 200 us (100 k) and 1 ms (10 M, deep);
- setup B at 200 ns and 5 us, at 5.05 V and at 5.1 V (sparse events);
- setup A at 10 us;
- 2 and 4 channels at 5 us (worst case).

Each in lines and dots; persistence Min, and 500 ms and 10 s at probe comp 200 us.

| Criterion | Target | How |
|---|---|---|
| fps | >= 29.5 | `:SYSTem:STATe?` fps |
| gap p99 | <= 45 ms where events >= 2 per screen frame | pacing line |
| empty pictures | <= 1 % (same condition), also at 5.1 V after step 8 | pacing line |
| picture steadiness | 20 screenshots: trace pixel count std <= 2 %, mean brightness std <= 2 % | `shotstats.py trace` |
| input to photon per knob | preview <= 1 step; first batch under the new mapping <= 70 ms; new configuration <= configured + 100 ms | pacing settle fields, new `preview` and `age` fields |
| continuous spin | 2 s spin of offset, trigger level and time base: no picture older than 150 ms; never an empty trace area for more than one step; no frozen trace | pacing `age` (newest frame in the picture) p100; screenshots every step |
| RUN/STOP/RUN, Single | brightness of the stopped picture within 3 % of the live one | screenshots |
| history replay | equal to the live picture of the same record (3 %) | screenshots |
| rare-pixel retention | a pixel hit once stays visible >= tau at 500 ms and 10 s | host test; probe comp single-shot screenshots |
| stale or partial | `stale` drawn 0, partial merges 0 | counters |
| A53 load | <= 70 % per little core, 4 channels worst case | `/proc/stat` (`oos-baseline.py`) |
| UI step p99 | <= 15 ms, 4 channels worst case | status step |
| RSS | growth <= 60 MB against dho804 32041d0 | `/proc/<pid>/status` VmRSS |
| dropped frames | not more than today | counters |

## 5. Implementation plan

Each step: one branch, host tests that fail before and pass after, gates (release and debug, arm64, perf-guard,
`gpu/test-host.sh`), then the unit check named. Steps 3 and 6 ship behind a temporary `draw.model = old|new` in
system.conf for A/B on the unit; step 9 removes the flag and the old code after acceptance.

| # | Step | Days | Host tests (fail before) | Unit check | Deletes |
|---|---|---|---|---|---|
| 0 | Counters: `stale`, `partial`, `preview`, picture `age`, frames and W per picture | 0.5 | counters count in pipe and UI tests | status line shows them | none |
| 1 | Mapping key split (3.5): batch keeps its first `View`, UI drops published batches of another mapping; preview of `last[0]` on a mapping change; `gen_acq` for records; queue cleared on reconfigure, early-ended fill dropped | 1.5 | a mapping change gives a preview at the next step and never a blank trace; no batch drawn under two mappings; records of an old generation not measured | knob spin, settle over 20 changes | `hw_stamp` use for the view, blank-on-change |
| 2 | Measurement reads never draw (RAW_PERIOD raw newest, deep window); on `fast_ok` the UI takes only the newest record; off it, all records as before | 0.5 | a raw measurement read leaves the picture unchanged; mask counts, zones, average, Bus trigger, Single, segments unchanged off the fast path | probe comp 200 us dots/lines | `compress::hold` draw branch, `deepmeas` draw cases, `RECORDS_BUDGET` |
| 3 | One lines kernel: spans for raw, codes, pairs, records (`draw_ex` lines; Z keeps its drawing; sparse per 3.3); behind `draw.model` | 4 | `pair_raw_mix_report` and `depth_mix_report` x1.000; record-path pictures alone within 3 % | screenshot pairs old/new per mode for the user's eye | `draw_ex` segment joining for lines (after step 9), deep-raw branch in `draw_frame` |
| 4 | Dots: one count per sample, W_frame = samples per column | 2.5 | dots brightness within 3 % across raw/record frames; no dot lost at 100 samples per column over 40 rows | dots set | none |
| 5 | `BatchUnit`: shares published together, row bounds; newest frame drawn by the pipe; route C gone; frames mode (default) makes each step's queued frames one batch | 2.5 | every merged picture holds whole batches; the newest frame in the same merge; frames and batch mode give the same picture | stress set, `partial` 0 | `Acc` lists, `Tally`, `Deliver`, `MERGE_WAIT_MAX`, the early `help_done` (the loop-end one stays), the `fast_ok` frame branch in `Ui::step` |
| 6 | Grading and records together: window at Min (C or A per the user), u32 decay above, STOP freeze, persistence change; routes E/F as pseudo-batches with W; zoom and colour grade on W; GPU gets W as vmax. Behind `draw.model` | 4 | brightness within 3 % for 1..27 frames per picture; band pixel count within 2 % for 13/27 alternating; window age <= 67 ms; a drop from 27 to 1 frame per step leaves no batch older than the cap in the picture; single hit visible >= tau at 500 ms and 10 s; stopped picture constant over 100 steps; replay equals live; zoom brightness equals main; `gpu/test-host.sh` 0 differ | full acceptance set, screenshot pairs per mode | `picture_ref`, `per_frame*`, `pic_frames`, `vmax` follow, 24 x 6 redraw (after step 9) |
| 7 | (cut: GPU needs no change, vmax = W, folded into 6) | 0 | | | |
| 8 | Sparse-event pacing: fill shortened when rest + fill exceeds a frame | 1.5 | Fill plan with rest 24 ms yields a batch every <= 33.3 ms in the pacing tests | setup B at 5.1 V: empty <= 1 % | none |
| 9 | Acceptance, then remove `draw.model` and the old paths | 1 | full suites | full acceptance set | the old kernels and grade |

Order: 0, 1, 2 (step 2 next to step 1), 3, 4, 5, 6, 8, 9. About 18 days of host work, plus unit rounds after 1, 3, 5,
6 and 9 (about 3 days). Risk: high for 3 and 6 (look of every mode: A/B by flag, screenshot pairs, the user's eye);
medium for 1 and 5 (concurrency, knob behaviour); low for the others.

Existing fixes:

| Fix | Fate |
|---|---|
| flicker-weight (newest via `draw_frame`) | absorbed by 5, code removed |
| dots-flicker (`compress::allowed` false with dots On) | kept |
| f536413 (deep raw window) | draw part removed by 2; the near-only trigger search for the window in `to_record` kept: the measurement window (i0..i1) depends on the trigger point |
| deliver after merge (`Deliver`) | absorbed by 5 |
| fix/batch-newest 988312b (`Tally`) | interim only: merge it if the stress round needs it before step 5, which replaces it. Its hold-chain residual is not fixed |
| `picture_ref` (trigger lock-in) | removed by 6/9 |
| `RECORDS_BUDGET` (hw only since 8dd7eb0) | removed by 2 |
| AC/BW 20M on the fast path | kept; `fast_ok` shrinks further |
| Fill plan, FrameCap, helper bound | kept; extended by 8 |
| near-only trigger gate, AC level | kept |
| live compression, RAW_PERIOD read | kept for measurement, not drawn |

Decisions taken (team-lead): frames mode stays the default (the unit keeps `acq.pipeline = batch`); Z input keeps
its current drawing. Open for the user: A or C at minimum persistence.

Not in scope: XY and LA (canvas), the auto timeout.
