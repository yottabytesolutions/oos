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

## Status

- Done: instrumentation, delivery with the batch, fill planning, helper bound. Host tests, arm64 check, perf-guard pass.
- Open, 5 us/div: the rest of a batch is 30-50 ms there (live compression k = 15: no prearm, an arm per batch of
  12-20 ms, a read of 21-29 ms for about 25 frames at about 0.8 ms each) and it varies, so a batch does not fit a
  screen frame; reads run as fast as they go and about one picture in ten has nothing new. Ways on: a frame cap per
  batch from the measured read time per frame (a full ring ends the fill early), prearm with live compression
  (`compress.rs`), a faster read-out. Not started: `compress.rs` is in work on fix/dots-flicker.
- Not measured: the probe comp on CH1 (Auto against Normal already separated trigger from pipeline); setup A's own
  signal.
