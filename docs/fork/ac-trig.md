# AC trigger coupling on the dense path (2026-11-14)

Setup A of docs/fork/stutter.md: CH2 ripple, AC, BW 20M, 20 mV/div, trigger AC 30 mV, Normal, 5-50 us/div. After the
stutter fixes it ran at 30 fps with even pictures, but about a quarter of the acquired frames were dropped: the deposit
workers needed 25-32 ms per batch, against about 6 ms for setup B (DC trigger).

## Cause

Two costs that only a filtered trigger path (AC or LF-reject coupling) pays, per frame:

1. The wide trigger search. The near search (`trignear`) could not decide on the noisy ripple, so every frame also
   smoothed and scanned up to half a screen on each side of the hardware trigger point (about 31 k samples each side at
   5 us/div). Edge counters of a 5 us/div run (within col+16 of the hardware point / other direction / far / none):
   15 / 85 / 1564 / 35279 of 36943 frames. For 95 % of the frames the wide search found nothing, and the frame stayed
   at the hardware point anyway. For 4 % it moved the picture to a far crossing, on average 4121 samples (0.66 div)
   and up to 30783 samples from the hardware point.
2. The full decode. `draw_frame_codes` refused AC and LF-reject coupling because the software level needs the frame
   mean, so every frame was decoded to f32 in full before drawing. DC frames decode only the near window.

The far edges are not corrections. The software level is the set level plus the frame mean, which only approximates
the high-pass the TPU applies before its comparators (trigger.md 2.5). The hardware decided on the filtered signal,
at the hardware trigger point within the decimation. The near window (16 screen columns plus 64 samples) covers the
decimation error the refinement exists for (trigger.md 8). An edge farther away is a crossing the hardware did not
trigger on.

## Plan

1. Per-frame cost counters (`ui/src/framecost.rs`): decode, level (frame mean), near search, wide search (with its
   decode on the codes path), whole frame. Sums in us since the start in `:SYSTem:STATe?` (` cost frames N wide N us
   decode/level/near/wide/frame ...`); a measurement takes the difference of two readings. Decode and frame count the
   deposit workers' frames; level, near and wide count every dense frame, also the UI's newest frame of a batch.
2. Filtered path near-only (`trignear::edge_near`): the edge nearest the hardware point within the near window, None
   without one, never the wide search. Then the hardware point stands, as it already did when no edge was found.
3. Filtered path on the codes path: the level from the same strided samples (stride max(len / 4096, 1): 4096 to 8191
   samples, all of them below 4096 points; the finite ones averaged), read from the words
   (`board::sw_trig_level_of`), then the near window as for DC.
4. Unit: setup A before (counters only) and after, 5/10/50 us/div; setup B unchanged.

DC and HF-reject coupling keep the near-then-wide search unchanged. In setup B the wide search also returned None for
51 k of 807 k frames and a far edge for 2139. That is the decimated case it was built for; not part of this change.

## Result on the unit

Builds 20261114-130005 (counters only) and 20261114-130527 (this change), signed, confirmed. 12 s windows after 12 s
settle. Cost in us per worker frame: decode / level / near / wide / whole frame.

| Setup A | Before: acq/used/drop per s | Before: cost | After: acq/used/drop per s | After: cost |
|---|---|---|---|---|
| 5 us/div | 1013/645/371 | 384/280/165/4203/6660 | 1111/1111/0 | 37/179/156/0/1938 |
| 10 us/div | 687/529/157 | 570/310/188/4993/7942 | 750/750/0 | 34/259/178/0/2243 |
| 50 us/div | 523/477/47 | 585/314/184/5775/8900 | 562/562/0 | 47/273/235/0/2729 |

- The wide search was most of the frame (63-65 %), the full decode 6-7 %. Both are gone; a frame costs 0.29-0.31 of
  before. No frames dropped, and more acquired (the workers no longer hold the ring).
- Pictures: 0 % empty before and after (one 1 % reading at 50 us/div); gap p99 46.7-50.4 ms before, 40.5-42.8 after.
- Frames moved within the near window at 5 us/div: 1740 of 13161, mean 103 samples (about 2 screen columns), against
  1458 frames moved 4121 samples on average before.
- The level (strided frame mean) is now the largest part after the deposit (180-270 us); fewer samples would cut it.
- Setup B unchanged: 200 ns/div 2255 acquired, 0 dropped, gap p99 35.9 before and 34.1 after; 5 us/div 761 and 760
  acquired, 0 dropped, wide 482 and 494 us per frame (DC keeps the wide search).

## Status

- Done: counters, filtered path near-only and on the codes path, host tests, unit before/after.
- Open: a cheaper level for the filtered path; whether DC needs the wide search at undecimated rates.
