# Display stutter (2026-11-14)

Symptom on the unit (build 20261114-083030, dho804 eba0da5; `acq.pipeline = batch`, `perf.place = on`,
`acq.prearm = on`, `graphics = gpu`): the trace does not update continuously, short freezes, while fps (29.8), step
(15 ms) and the waveform rate look fine. Setups: A) CH2 ripple, AC, BW 20M, 20 mV/div, Normal, 5-50 us/div;
B) CH2 DC 200 mV/div offset -5 V, trigger rising 5.10 V, Normal, bursts every ~1.5 ms (found at 200 ns/div).

## Plan

1. Instrument pacing (`ui/src/pacing.rs`): per picture the new frames, gaps between pictures with new frames,
   frame-loop pass intervals and step times, per batch the wait, interval, fill, read and drawing time; percentiles
   over the last 300 in `:SYSTem:STATe?` (` pacing ...`), passes over 60 ms logged to /tmp/oos-ui.log.
2. Measure on the unit: setup B at 200 ns to 50 us/div, Normal and Auto.
3. Root cause with numbers, fix, measure again.

## Root cause

Not throughput, pacing. The frame loop ran evenly (pass p99 33.6-36 ms, step p99 under 30 ms, no slow pass logged), so
the GPU and the UI thread are not the cause. Two things in the batch pipeline were:

1. Batch cadence. A batch waited a fixed 25 ms for the ring to fill after its first frame, then read it. Fill plus
   the rest of the batch (arm or the wait of a batch armed ahead, first trigger, read) gave a batch every 33-34 ms at
   200 ns/div and every 40-55 ms at 1-50 us/div, free running against the 33.3 ms screen frame. Pictures without a
   new batch (shown again unchanged: the freeze) and pictures with two alternated: up to 14 % empty pictures, gaps
   between pictures with new frames p90 50-65 ms, p99 70 ms. Auto sweep gave the same numbers as Normal at 1 us/div:
   not the trigger, the pipeline.
2. Split delivery. The newest frame of a batch went to the UI queue at once, the other frames a picture later once the
   deposit workers had drawn them (11-16 ms). At 1 us/div one picture showed the newest frame alone, the next one the
   other ~70 (new frames per picture p10 = 1): the trace alternated between a single sweep and the full picture.

## Fix (branch perf/stutter)

- `drawpipe::Pipe::feed_with`: the newest frame goes to the queue from the batch's `deliver`, run by the UI right
  after the merge that brings the batch's other frames. One picture gets the whole batch.
- `pacing::Fill`: the ring-fill time of each batch is planned so that the reads come one screen frame apart, each the
  workers' drawing time (`ready`, measured) plus 3 ms before a screen step, from the measured rest of a batch and its
  spread. A batch that starts early fills longer to keep its slot. Where the rest leaves no room in a frame the fill
  is the least (4 ms), without a slot. Reading every second slot evenly was tried and dropped: the longer fill read
  more frames, the read grew, and the reads fell 20-30 ms behind (gaps to 108 ms).
- hw thread helper: it starts a frame only if its usual draw time ends within the fill (a large frame after the
  fill's end delayed the read by several ms).

Files: `ui/src/pacing.rs` (new), `ui/src/drawpipe.rs` (deliver), `ui/src/board/dho/hw.rs` (fill plan, deliver,
helper bound, `push_frames`), one line each in `ui/src/ui.rs`, `ui/src/main.rs`, `ui/src/cmd.rs`.

## Numbers (unit, setup B signal, Normal, 300 pictures each)

| time base | before: empty, gap p90/p99, new p10 | after: empty, gap p90/p99, new p10 |
|---|---|---|
| 200 ns/div | 0 %, 36/40 ms, 69 | 0 %, 36/39 ms, 65 |
| 1 us/div | 0 %, 51/57 ms, 1 | 0 %, 35/43 ms, 49 |
| 5 us/div | 13 %, 64/70 ms, 0 | 9-16 %, 49-65/102-105 ms, 0-4 |
| 10 us/div | 12-14 %, 64/71 ms, 0 | 2 %, 41/67 ms, 10 |
| 50 us/div | 5 %, 48/72 ms, 1 | 1 %, 37/73 ms, 10 |

Before = build 20261114-104544 (instrumentation only), after = 20261114-114958. Frames per batch fell with the
shorter fill (1 us/div 73 to 58) while the batches per second rose: acquired frames per second about the same.

## Round 2 (rebased on dho804 1a0e4a2)

- 5 us/div, live compression k = 15 (no prearm): `pacing::FrameCap` reads at most the frames whose read leaves room
  for the time before the fill, its spread, a 6 ms fill and the margin (`CAP_FILL_MS`); a full ring ends the fill
  early; with prearm the cap only moves by more than a quarter. 5 us/div: 9-16 % -> 0 % empty pictures, gap p99
  102-105 -> 41 ms; acquired about the same (26 frames every 39 ms against 27 every 44 ms). 2, 10, 20, 50 us/div and
  200 ns, 1 us/div: 0 % empty, gap p99 39-41 ms.
- Setup A (CH2 AC, BW 20M, 20 mV/div, trigger AC 30 mV, Normal) ran at 5-13 fps, step 120-190 ms: `Ui::fast_ok`
  left out AC coupling and the 20 MHz limit (upstream rule from before the hardware source set them in the AFE; its
  records are not conditioned in software), so every frame took the record path, 48 records a step. Now on the fast
  path; a step also stops taking records after 15 ms (`RECORDS_BUDGET`). Then the merge took 18-34 ms: the UI waited on
  a mailbox lock while an A53 worker added its maps under it. Workers now hand their maps on in a list (`Mail::full`),
  and adds and clears touch only the channels that are on. Setup A after: 29.6-30 fps, step 11-13 ms, 0 % empty, gap
  p99 44-52 ms at 5, 10 and 50 us/div; Auto and level 0 V the same. About a quarter of the frames is still dropped
  there (the workers need 25-32 ms per batch: on this noisy signal the smoothed edge search finds no edge, edges
  `None` 35 k of 37 k, and every frame pays the wide search); the pictures are even all the same.
- Settling after a change (`:TIMebase:SCALe`, `:CHANnel2:SCALe`, single steps and knob-like runs 120 ms apart; setups
  A and B): the hardware takes the settings 25-50 ms after the change, steady pictures 5-150 ms after it, at most 1
  empty picture (2-3 with the old fixed fill). A reconfigure does not stall the display. Logged per change in
  /tmp/oos-ui.log (`pacing: settings change`) and in the status line (`settle`).
- Auto without a trigger: one frame per ~170 ms (the stock auto timeout, `scu::auto_trigger`: t2 at least 100 ms), so
  about 5 pictures a second while no edge matches, e.g. while the level is being found. Stock behaviour; a shorter OOS
  timeout would make that smoother. Not changed: needs a decision.
- Probe comp (CH1, 200 us/div, 100 k, dots Auto): no signal on CH1 (Vpp 0.36 V, no frequency): not measured.

## Status

- Done: instrumentation, delivery with the batch, fill planning, helper bound, frame cap, AC/BW 20M on the fast path,
  record budget, lock-free merge. Host tests, arm64 check, perf-guard pass.
- Open: the dropped frames on noisy signals (cheaper edge refinement when the smoothed search keeps failing); the auto
  timeout decision; the probe comp check with the probe on CH1.
