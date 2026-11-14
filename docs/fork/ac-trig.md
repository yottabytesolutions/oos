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
3. Filtered path on the codes path: the level from the same 4096 strided samples, read from the words
   (`board::sw_trig_level_of`), then the near window as for DC.
4. Unit: setup A before (counters only) and after, 5/10/50 us/div; setup B unchanged.

DC and HF-reject coupling keep the near-then-wide search unchanged. In setup B the wide search also returned None for
51 k of 807 k frames and a far edge for 2139. That is the decimated case it was built for; not part of this change.

## Status

- Done: counters (1), commit "Count per-frame decode, level and trigger search time".
- Done: filtered path near-only and on the codes path (2, 3) with host tests. The filtered refinement equals
  `edge_smooth` limited to the near window and stays within it, from the whole frame and from the decoded window
  alike. A frame with only a far edge is drawn at the hardware point with AC and LF reject, and at the far edge with
  DC and HF reject. The DC refinement equals `trignear::edge_smooth_near`. The codes path equals the decode path for
  AC and LF reject. The perf guard includes an AC frame (host: 0.72 of decode + draw).
- Next: unit before/after (4) when the scope is free.
