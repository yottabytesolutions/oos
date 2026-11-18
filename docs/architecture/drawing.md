# Screen drawing architecture (design, 2026-11-14)

Goal (user decision): a fast, responsive UI without hiccups or flicker, done properly before the PR. This note is the
design and the implementation plan. compress reviews it, then an independent reviewer.

Why a redesign: the day's display fixes each restored one of two invariants by hand. First, every frame weighs the
same in a picture. Second, a picture holds whole batches. A third property sits under both: brightness is graded per
picture by frame count. The structure guarantees none of them, so every new route or representation can break them
again.

## Current data flow (dho804 32041d0)

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

Both give batch-mode pictures of a different composition. `acq.pipeline` defaults to `batch`
(`drawpipe::mode_from`), matching the generic image. `standard` or `frames` explicitly selects route D for comparison
until the M12 decision.

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
Measured settle on the unit (../fork/stutter.md 5): configured 25-50 ms after the change, steady 5-150 ms, at most 1 empty
picture.

## The day's bugs on that flow

| Bug | Where | Property broken |
|---|---|---|
| Flicker 500 us/div (../fork/flicker.md 1) | route C `draw_ex`, 3.6x a deposit | one weight per frame |
| Dots + compression flicker (../fork/flicker.md 2) | raw newest (RAW_PERIOD) via `deposit_dots` among pair frames | one weight; measurement read drawn. Avoided, not restored: no compression with dots |
| Deep raw window 1 Hz pulse (../fork/flicker.md 3) | raw window via dense deposit among `to_record` pairs, x1.44 | one weight; measurement read drawn |
| Trigger lock-in pumping ([trigger lock-in](../fork/trigger-lockin.md)) | `pic_frames` varies per picture | brightness tied to frames per picture |
| Newest split delivery (../fork/stutter.md 2) | route C a picture before A | whole batches |
| Partial batches, helper and worker shares (fix/batch-newest 2ac0ca1, 988312b) | A/B shares merged at different steps | whole batches |
| AC / BW 20M at 5-13 fps (../fork/stutter.md 4) | `fast_ok` excluded them: route E for every frame | drawing tied to measurement options |
| `RECORDS_BUDGET` demo bug (8dd7eb0) | wall-clock bound in the record loop | picture content tied to UI timing |
| Fill timing, sparse events (../fork/stutter.md 1, open) | batch cadence vs 33.3 ms steps | picture depends on batch arrival |
| Live (<= 1 M) and deep (> 1 M) frames in one picture after a depth change (host-verified, not seen on the unit) | `deposit_pairs` 1x vs `to_record` 3.7x; `Key` without mdepth; queue not cleared | one weight; no stale frames |

The lines residue between pair and raw frames (137 edge-column pixels, x1.016-1.022) is position, not weight:
trigger refinement on pairs is good to about one pair, and a pair straddling a column boundary lands in the column
of its centre.

## Design

### Threads and ownership

- **UI thread (first A72)** is the only owner of the picture: the window of batch maps, `ph`, `phz`, the weight
  sums, the grade, the arena copy for the GPU. Nothing else writes them.
- **Deposit workers (A53 x 4)** and **the helper** draw shares of one batch into maps they own. They never touch the
  picture.
- **Acquisition thread (second A72)** owns the hardware, the batch buffers and the record queue. It never draws for
  the screen.
- **GPU helper process**: reads the arena the UI thread filled.

### The batch is the atomic unit

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

### One weight model per display mode

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
    during a change, because [records and measurement never draw](#records-and-measurement-never-draw).
  - Headroom: a flat dots trace puts about 30 counts per pixel per frame. Fine at Min; long persistence halves, as
    below.
  - A pair has no sample density, so dots keep compression off (`compress::allowed`, kept).
- **Zoom:** a second span set per frame for `phz` with the zoom mapping. The same w, so the same grade.

### Records and measurement never draw

- Records (`wave::Record`) feed the measurements, history, masks, protocols, XY and SCPI.
- The raw newest frame (RAW_PERIOD) and the deep raw window are measurement reads. They reach `deepmeas` and the
  measurements, never the picture.
- On the fast path (`fast_ok`) the UI takes only the newest record of a step for measurement: every frame is in the
  picture through the pipe. Off the fast path every record still goes through the record loop: it feeds mask counts,
  the zone pass, the software average, the Bus trigger decision, Single, segments and history. There the records of
  one step form one pseudo-batch (W = their weight sum) into the picture.
- `fast_ok` keeps only conditions on what the picture needs (zoom, XY, roll/scan). Measurement options never move a
  frame to another route.
- Roll and scan frames are the whole sweep so far: each replaces the picture (a window of 1, outside the [grading window](#grading)).

**STOP.** STOP freezes the accumulated maps and W as they are. Nothing is redrawn while stopped. Single done is the
same. A view change while stopped (time base, offset, zoom) replaces the maps with `last[0]` drawn once at W = w.
History and segment replay do the same for the record they show. Today's 24 x 6 redraw on every step goes.

With persistence above Min, the frozen accumulated picture is in pixels of the old mapping and cannot be re-mapped.
On the first view change while stopped it is dropped and replaced by `last[0]` drawn once. OOS today loses it the
same way (the stopped redraw clears the maps). Against the stock firmware it may be a change: not checked whether
stock rescales a stopped persistence picture. It goes in the user notes as a known behaviour.

**Persistence changes.** A change between Min and a decay setting, or between decay settings, clears the maps and W
and starts from `last[0]` drawn once (the [settings-change preview](#settings-changes-preview-mapping-key-hardware-generation)).

### Settings changes: preview, mapping key, hardware generation

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
- **The mapping key** is what turns volts and times into pixels, plus what the hardware does to the waveform itself:
  time base, delay, V/div, offset, channel set, dots, zoom, size, and per channel invert, coupling and bandwidth.
  The hardware applies inversion, coupling and bandwidth (`Source::is_hw`), so a frame from before such a toggle
  shows the old waveform: those batches are dropped like a mapping change. It is fixed per batch at its first claim (the batch keeps the `View` it was first drawn under, so all
  its shares share one mapping). A published batch of another mapping key is dropped by the UI (pixels in the wrong
  place); its frames are not redrawn.
- **The hardware generation** (`gen_acq`, raised when `configure` runs: depth, k, coupling, bandwidth, trigger,
  acquisition mode) does not invalidate the picture, because each frame is refined with its own trigger snapshot
  (above). Live and deep frames after a depth change may share a picture;
  with [one weight model](#one-weight-model-per-display-mode) (step 3) that is correct, so `depth_mix_report` becomes step 3's test, not step 1's. A level, slope or coupling change keeps the old batches valid as
  pictures. It matters to the records: measurements and Single take only the current generation. The queue is
  cleared on reconfigure, and a fill that `changed()` ended under the old configuration is dropped.
- **Preview.** On a mapping change the UI draws `last[0]` once under the new mapping as the picture (W = w, window of
  1), at the next step, from its stored trigger point (not refined again). The first batch drawn under the new mapping replaces it. During a spin the picture is a
  preview or a real batch at every step, never blank and never at old pixel positions under new labels.

Input to photon: the preview at the next step (<= 33 ms). The first batch under the new mapping: the batches in
flight are dropped, the next one is drawn under the new view (fill + read + draw, <= about 70 ms). A batch of the new
hardware configuration: configure (25-50 ms measured) + fill + read + draw, about 70-130 ms.

### Grading

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
picture; B needs neither. Decision (user, 2026-11-14): C with the cap (3 batches, 2 screen frames). B is not
used at Min because of its ghost. Host render behind the decision (noisy 1 kHz square, 200 us/div, batches 27, 3, 13,
3, 27, 13, N = 14): lit pixels per picture A 12,891-24,319 (1.9x range), C 19,583-28,138 (1.4x).

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

### GPU upload and CPU budget per step

- **GPU:** the LUT maps count / vmax today. With vmax = W it computes count / W exactly, so neither the protocol nor
  the shader changes: the UI passes W where it passed `vmax`. The UI copies the changed rows of the picture into the
  arena (today: copy 0.6 ms; helper upload 2.3 ms and draw 7 ms on its own core). The arena holds u16 counts. With
  the u32 persistence maps the copy converts (>> 16, saturating), a per-pixel pass of about 1 ms, and W is passed in
  the same unit after the shift.
- **Screen step** (33.3 ms on the first A72; today at setup B 200 ns: step 6 ms, render 2.9 ms): merge and window
  rebuild <= 2 ms, decay pass (persistence only) <= 2 ms, grade <= 0.5 ms, render and arena copy <= 6 ms. Target step
  p99 <= 15 ms.
- **A53:** per frame today 0.5-2.7 ms of worker time. At 1100 frames/s x 1.9 ms that is 2.1 cores of 4. Target <= 70 %
  per little core in the worst mode. No map adds on the A53 ([whole batches](#the-batch-is-the-atomic-unit)).
- **Memory:** the window holds at most 3 batches of shares (3 x up to 4 shares x 3.5 MB worst case, used channels
  only), plus 7-14 MB of u32 maps with persistence. Bound: RSS growth <= 60 MB against today.

### Pacing with sparse events

"No hiccups" includes setup B at its present rail (1650 events/s at 5.1 V): batches came every 34-40 ms against
33.3 ms screen steps, 2-5 % empty pictures (../fork/stutter.md). C keeps such a picture steady (the window holds), but the
trace still updates late. The Fill plan should end the fill so a batch is ready every screen step when events are
rare: the fill is planned from the measured rest (arm, first trigger, read) and shortened when rest + fill exceeds a
frame, down to `FILL_MIN`. Scheduled as step 8.

### One change-detection point

A read-only review of upstream 0.1.12 (team-lead, verified counts) found three invalidation mechanisms side by side:
- the `Want` diff in the acquisition thread;
- `hw_stamp` bumps in ui.rs, bode.rs and autoset.rs;
- `settings_changed()` / `mark_changed()`, 39 calls in the fork, plus 16 direct `dirty_map = true`.

`Model` has about 178 pub fields (upstream count), about 103 direct writes and the string-keyed `store::set` for SCPI and HTTP. A
write that skips one mechanism leaves stale state, in the phosphor and outside it: measurement statistics (`mstat`),
history, segments, the decode cache. `Ui::step` relies on an implicit order with about 8 `ph.clear()` sites.

Design (adopted from that review):
- **(A) One point.** Once per step, before acquisition and drawing, the UI diffs the relevant `Model` sub-structs
  against last step's copy (`Model` derives `PartialEq` already) and derives one `Invalidate` set: `acq` (hardware
  generation), `mapping` ([mapping key](#settings-changes-preview-mapping-key-hardware-generation): preview, drop old batches), `phosphor` (reset of maps and W), `stats`
  (measurement statistics), `decode` (protocol decode cache), `history` (history and segments). Every consumer acts
  on that set only.
- **Classification by construction.** A `classify(old, new)` destructures `Model` without `..`. A new field
  therefore does not compile until it is placed in a class, or explicitly in "changes nothing".
- **(B)** The [accumulator](#threads-and-ownership) is the only writer of `ph`/`phz`, driven only by (A) and frame intake.
- Not part of this: the 130-field `Ui` and other long functions stay as they are unless a step touches them. Splitting
  `Shared` (commands vs state under one lock) only if want/rearm races show.

## Acceptance criteria (unitlab)

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

## Implementation plan

Each step: one branch, host tests that fail before and pass after, gates (release and debug, arm64, perf-guard,
`gpu/test-host.sh`), then the unit check named. Steps 3 and 6 ship behind a temporary `draw.model = old|new` in
system.conf for A/B on the unit; step 9 removes the flag and the old code after acceptance.

| # | Step | Days | Host tests (fail before) | Unit check | Deletes |
|---|---|---|---|---|---|
| 0 | Counters: `stale`, `partial`, `preview`, picture `age`, frames and W per picture | 0.5 | counters count in pipe and UI tests | status line shows them | none |
| 1a | [One change-detection point](#one-change-detection-point): once per step, diff the relevant `Model` sub-structs against the last step and derive one `Invalidate` set; its consumers act on it; the ad hoc `settings_changed`, `dirty_map = true` and `hw_stamp` writes go | 3 | every `Model` field classified (exhaustive destructuring: a new field does not compile until classified); per class, changing a representative field resets exactly its consumers; SCPI/HTTP writes (`store::set`) reach the same point | knob and SCPI changes: clears and resets as before (screenshots, measurement statistics) | `settings_changed`, `mark_changed`, the 16 `dirty_map = true`, the `hw_stamp` bumps in ui.rs, bode.rs, autoset.rs |
| 1b | Split the acquisition thread's closure (`hw.rs` `spawn`, lines 1263-2154 on 32041d0) into functions: set-up, normal batch loop, batch read, roll, scan. Pure refactor | 1.5 | the existing hw tests unchanged; no behaviour change (same counters and pictures in the hw tests) | smoke: probe comp and setup B as before | none (moved code) |
| 1c | [Mapping key split](#settings-changes-preview-mapping-key-hardware-generation): batch keeps its first `View`, UI drops published batches of another mapping; preview of `last[0]` on a mapping change; `gen_acq` for records; the batch's trigger snapshot for the refinement; invert, coupling and bandwidth in the mapping key; queue cleared on reconfigure, early-ended fill dropped; check that decode reads the batch's calibration, not global state (self-cal reload, core-offset overlay) | 1.5 | a mapping change gives a preview at the next step and never a blank trace; no batch drawn under two mappings; records of an old generation not measured | knob spin, settle over 20 changes | `hw_stamp` use for the view, blank-on-change |
| 2 | Measurement reads never draw (RAW_PERIOD raw newest, deep window); on `fast_ok` the UI takes only the newest record; off it, all records as before | 0.5 | a raw measurement read leaves the picture unchanged; mask counts, zones, average, Bus trigger, Single, segments unchanged off the fast path | probe comp 200 us dots/lines | `compress::hold` draw branch, `deepmeas` draw cases, `RECORDS_BUDGET` |
| 3 | One lines kernel: spans for raw, codes, pairs, records (`draw_ex` lines; Z keeps its drawing; sparse per [weight model](#one-weight-model-per-display-mode)); behind `draw.model` | 4 | `pair_raw_mix_report` and `depth_mix_report` x1.000; record-path pictures alone within 3 % | screenshot pairs old/new per mode for the user's eye | `draw_ex` segment joining for lines (after step 9), deep-raw branch in `draw_frame` |
| 4 | Dots: one count per sample, W_frame = samples per column | 2.5 | dots brightness within 3 % across raw/record frames; no dot lost at 100 samples per column over 40 rows | dots set | none |
| 5 | `BatchUnit`: shares published together, row bounds; newest frame drawn by the pipe; route C gone; explicit frames mode makes each step's queued frames one batch | 2.5 | every merged picture holds whole batches; the newest frame in the same merge; frames and batch mode give the same picture | stress set, `partial` 0 | `Acc` lists, `Tally`, `Deliver`, `MERGE_WAIT_MAX`, the early `help_done` (the loop-end one stays), the `fast_ok` frame branch in `Ui::step` |
| 6 | Grading and records together: window at Min (C with the cap), u32 decay above, STOP freeze, persistence change; routes E/F as pseudo-batches with W; zoom and colour grade on W; GPU gets W as vmax. Behind `draw.model` | 4 | brightness within 3 % for 1..27 frames per picture; band pixel count within 2 % for 13/27 alternating; window age <= 67 ms; a drop from 27 to 1 frame per step leaves no batch older than the cap in the picture; single hit visible >= tau at 500 ms and 10 s; stopped picture constant over 100 steps; replay equals live; zoom brightness equals main; `gpu/test-host.sh` 0 differ | full acceptance set, screenshot pairs per mode | `picture_ref`, `per_frame*`, `pic_frames`, `vmax` follow, 24 x 6 redraw (after step 9) |
| 7 | (cut: GPU needs no change, vmax = W, folded into 6) | 0 | | | |
| 8 | Sparse-event pacing: fill shortened when rest + fill exceeds a frame | 1.5 | Fill plan with rest 24 ms yields a batch every <= 33.3 ms in the pacing tests | setup B at 5.1 V: empty <= 1 % | none |
| 9 | Acceptance, then remove `draw.model` and the old paths | 1 | full suites | full acceptance set | the old kernels and grade |

Order: 0, 1a, 1b, 1c, 2 (step 2 next to step 1), 3, 4, 5, 6, 8, 9. Step 1 in the text below means 1c. About 22 days of host work, plus unit rounds after 1c, 3, 5,
6 and 9 (about 3 days). Risk: high for 3 and 6 (look of every mode: A/B by flag, screenshot pairs, the user's eye);
medium for 1a, 1c and 5 (every clear and reset moves to one point; concurrency; knob behaviour); low for the others (1b is
a pure refactor).

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

Current default: Batch on main and in generic images. The M12 comparison in the migration plan chooses the
surviving path; Standard remains until that evidence and selection. Z input keeps its current drawing.
The minimum-persistence cap remains part of the drawing design approved on 2026-11-14.

Not in scope: XY and LA (canvas), the auto timeout.

## Status

- Step 0 (branch draw/step-0): counters in the pacing line, after `views`:
  - per picture, frames through a deposit kernel and through the record path (p10/p50/p90);
  - the weight in deposit frames (record frames x 3.7);
  - the age of the newest pipe batch in the picture (p50/p90/p99/max; batch mode only, frames-mode pictures have no
    age);
  - `stale`: frames shown that were acquired before the last change of `Want` (the hardware settings, trigger level
    and slope included), via `pacing::WANTED`/`APPLIED`, `Frame::cfg` and `Batch::cfg`. Counted where they reach the
    picture: pipe maps at merge (dropped maps do not count), routes C/D and the record path at the draw. Roll and scan
    frames carry the generation too. Not covered: changes outside `Want` (`Shared::needs_raw`, `slow`, `mask_want`,
    `zone_want`). The share decides stale at draw time: a share drawn just before a `Want` change and merged after
    it counts as not stale, so the baseline undercounts slightly at the edge. Step 1c brings a second counter for frames shown under [another mapping key](#settings-changes-preview-mapping-key-hardware-generation), which is the
    design's meaning of stale; this one is the baseline;
  - `partial`: batches merged in more than one picture.
  `preview` comes with step 1, which adds the preview.

## M12a host draw-route evidence (2026-11-18, #157)

Standard remains the unit-comparison baseline. The owner confirmed Batch as the code and image default on
2026-11-18; the M12 path selection and M12b remain open. Host equality does not establish the target weight/freeze
contract or explain the historical DHO924S 18.2 fps result in ADR 0035. No production behavior changes in
this evidence step.

Reproduce the deterministic cases with `cargo test -p oos-ui --bin oos-ui draw_routes`. The tests live in
`ui/src/ui/draw_routes.rs` because they inspect the actual UI maps and route selection. They use captured, seeded
board fixtures, injected step time, a fixed intake count and a buffer-return completion barrier. They compare
all channel maps, zoom maps, total deposited counts and `pic_frames`. The current code has no explicit W yet;
these counts are evidence about its existing weights, not a claim that M7/M9/M10 have landed.

| Input / transition | Current result | Target-spec interpretation |
|---|---|---|
| Raw and codes, 1/2/4 channels; dots | Pixel/count equality | Route agreement; dots still use 24 per hit and brightest-pixel grading rather than sample-density W |
| One frame and a 48-frame step at 10 us/div | Pixel/count equality | Guards single-frame delivery and step grouping, without one full-map merge per queued Standard frame |
| Live pairs, 3 frames at 200 us/div, 1/2/4 channels | Different maps/weights: CH1 Standard 2,219,592 counts; Batch 1,743,384 (27.3 % excess in Standard) | Batch agrees with independent serial `draw_frame` deposits. Standard `fast_plan` turns older envelope frames into records, whose overlapping segments violate one span per column |
| Deep pairs | Pixel/count equality | Both still use the record fallback; equality does not normalize its weight to a raw span frame |
| Zoom | Main and zoom maps/counts equal | Both use the shared record path; target zoom stays on the span fast path |
| Deep raw measurement window after pairs | Equal resulting maps; the live picture changes and `pic_frames` becomes 1 | Both violate the measurement-only rule; the live picture should stay on pairs |
| Roll/scan snapshots | Equal maps/counts, including subsequent empty steps | Shared record path, rather than the target explicit `Replace { Sweep }` |
| STOP, SINGLE done, history and software segment replay | Equal resulting maps/counts over subsequent steps | Shared stopped redraw; target freezes maps/W or draws one explicit replacement |

The STOP negative control starts with three raw frames: CH1 has 1,535,112 live counts and 6,229,440 after STOP.
`step_draw::stopped` clears and redraws the last record at `24 * 6`; it does not preserve the live picture.
Replay tests select stored history and completed software segments. Hardware segment read-back and final STOP raw
reads still need unit acceptance. None of these characterization assertions endorses the shared spec deviations.

### Host cost and four-channel diagnosis

Run `OOS_PERF_TESTS=1 cargo test -p oos-ui --release --bin oos-ui draw_routes -- --ignored --nocapture --test-threads=1`
with other builds/tests idle. `tools/perf-guard.sh` includes these ignored probes. `DRAW_ROUTE` rows report best,
p50 and nearest-rank p99 elapsed build time plus min/max process-wide allocation calls over 101 warmed steps.
The existing soak counting allocator is shared with the unit probes; timing includes its instrumentation.
There are no machine-speed assertions in the deterministic tests or new absolute performance thresholds.

The source uses production `hw::settings`, `hz::prep` and `hw::batch_plan`, synthetic 100 kHz square-wave words and
captured decoders. Compression is off. Rows cover 1/2/4 channels at 200 ns, 10 us and 200 us/div, with one frame
and a 48-frame composition stress case, plus the plan's read count at 10 us/div. The 48-frame stress buffer may exceed one hardware
read; it is explicitly a step/aggregation test, not a claim about ring or DMA batch size. Two host deposit workers,
no acquisition helper, no DMA, ADC learning, GPU composition, present or real acquisition cadence are modeled.

Observed on an unpinned macOS arm64 host with Rust 1.99.1, release optimization and the counting System allocator.
Times below are best/p50/p99 **milliseconds per step**; allocation ranges count alloc/zeroed/realloc calls,
including freed transients and worker calls. Elapsed tails include shared-host scheduling as well as work.
All 42 emitted route rows passed pixel/count comparison. The table retains all 48-frame cases and the actual
10 us/div read counts; the same log also reports single-frame rows.

| Channels | Time/div | Frames/step | Standard ms | Batch ms | Standard allocations | Batch allocations |
|---|---|---:|---|---|---|---|
| 1 | 200 ns | 48 | 0.51/0.61/2.20 | 0.45/0.63/1.63 | 348-359 | 169-171 |
| 1 | 10 us | 48 | 7.15/7.53/9.10 | 9.58/10.11/13.86 | 769-773 | 631-633 |
| 1 | 10 us | 20 | 5.33/5.62/6.06 | 6.69/6.93/7.25 | 651-652 | 575-577 |
| 1 | 200 us | 48 | 8.88/9.34/19.45 | 11.57/11.95/12.72 | 3988-3994 | 3851-3853 |
| 2 | 200 ns | 48 | 1.85/1.94/2.14 | 2.35/2.44/2.53 | 568-572 | 468-470 |
| 2 | 10 us | 48 | 9.38/9.78/11.46 | 7.37/7.58/7.92 | 1344-1350 | 1058-1060 |
| 2 | 10 us | 14 | 7.22/7.52/11.47 | 6.65/6.83/7.08 | 1133-1136 | 1024-1026 |
| 2 | 200 us | 48 | 13.21/13.59/14.28 | 15.38/16.00/16.43 | 7790-7791 | 7546-7548 |
| 4 | 200 ns | 48 | 2.55/2.67/2.93 | 4.15/4.33/4.48 | 870-872 | 666-668 |
| 4 | 10 us | 48 | 8.40/8.83/9.23 | 7.77/8.03/8.29 | 2401-2402 | 1914-1916 |
| 4 | 10 us | 14 | 6.13/6.52/6.83 | 6.00/6.41/6.65 | 2050-2051 | 1880-1882 |
| 4 | 200 us | 48 | 27.34/36.30/93.65 | 23.72/24.57/34.85 | 15361-15365 | 14926-14928 |

`DRAW_ROUTE_PHASES` separates UI fast/merge/record work. `DRAW_ROUTE_COST` records worker decode/trigger/frame
counters. `DRAW_DEPOSIT_PROFILE` alternates identical decoded inputs between per-frame pixel deposits and
Standard's step difference-span accumulation, verifies equal maps, and reports best elapsed/p99 process CPU.
The latter includes Standard's band parallelism; it is not a serial-kernel-only ratio.

At four channels and 10 us/div the captured plan is 312.5 MSa/s, 35,160 samples/channel, k=1,
281,280 bytes/frame and 14 frames/read. `CODES_MAX_STRIDE = 2` sends stride-4 frames through full decode.
The measured worker hot path is `draw_range -> decode_into_probed -> draw_frame -> fastdraw::deposit_from`.
Across 4,747 worker frames, counters show 174,543 us decoding, 17,061 us near-trigger work, zero wide searches,
and 464,505 us total worker frame time (elapsed sums across workers). The remaining approximately 59 % includes
span building/pixel deposit and orchestration; it is not a sampled CPU attribution. Main Batch merge p50 is
0.208 ms versus 1.684 ms for Standard's queued-frame fast phase; newest-record p50 is 0.278/0.276 ms respectively.
These phase rings include warm-up; build rows above exclude it and also include decoding/intake before `Ui::step`.

The alternating deposit experiment on those same 48 decoded frames gives per-frame pixels / Standard step spans:
best elapsed **2.739/1.551 ms**, p99 process CPU **3.692/9.540 ms**. Standard's parallelism saves elapsed time
while costing more summed CPU. A candidate fix is to accumulate differences across a worker's batch, then fold once,
reusing Standard's `diff_span`/`fold_t` machinery instead of painting every frame; retain touched-row bounds for
merging. This crosses worker share/ownership and record/pair handling, so it is not a small local fix for this PR.
Simply enabling stride-4 direct codes is not justified: the existing guard documents repeated read-out scans and
only a marginal earlier benefit. No production fix is made.

On the actual 14-frame plan the host routes are close (p50 6.52/6.41 ms Standard/Batch). The host does not
reproduce or prove the cause of the historical 18.2 versus 26.1 fps unit observation. There are no acquired/displayed-fps measurements here. Host stage counters
and the paired kernel experiment identify costs to test on A53/A72; external sampling was unavailable under the
host sandbox. Keep ADR 0035's deletion hold until a matched unit profile establishes that cause.

Unit measurements still required before selecting/deleting a route: reproduce the 100 kHz square setup and record
actual rate/depth/decimation/compression/read size, worker placement and helper use for both routes; collect
step/worker/merge/records/compose/present p50/p99, acquired/used/dropped/partial frames, input latency and RSS;
repeat 1/2/4 channels at 10 us/div and neighboring timebases, then the specified load/soak. Check brightness and
freeze/replacement behavior for the matrix above, including hardware segment replay and STOP final reads.
These measurements are not yet verified on hardware. M12a full-gate/prerequisite acceptance and the owner's
route decision remain with the coordinator; M12b and its deletions are not authorized by host results alone.
