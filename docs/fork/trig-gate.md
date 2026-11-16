# Trigger refinement: near-only at the full rate (2026-11-14)

Follows docs/fork/ac-trig.md. Two costs left in the dense path's trigger refinement after that change.

## Cause

1. DC wide search at undecimated rates. In setup B at 5 us/div (1.25 GSa/s), every frame ran the wide search (494 of
   1219 us per worker frame) and found nothing: the column average (51 samples) hides the burst edges at 5.1 V. At
   200 ns/div it found a far edge for 50 of 27 k frames. trigger.md section 7 measured the hardware point off the
   edge only with decimation (about 0 % at 5 us/div, 30 % at 50 us/div, 90 % at 500 us/div), at 18-340 samples, which
   the near window (16 columns plus 64 samples) covers. At the full rate the wide search returns either None (the
   frame stays at the hardware point anyway) or a crossing the hardware did not fire on.
2. The AC level: the frame mean from 4096 strided samples cost 180-270 us per frame on the A53, a cache miss each.

## Plan

1. `Frame::pre_exact`: the DHO decode sets it for a read at the full rate of the channel set
   (`rate * ratio == hz::CLOCK`), raw or with the live compression (the point is then exact to its pair). Every other
   frame (decimated reads, other boards, roll, scan, tests) keeps false.
2. `drawpipe::near_only`: the near window only (`trignear::edge_near`) for AC and LF reject (ac-trig.md) and for an
   edge trigger on an exact frame. The wide search stays for decimated reads and other trigger types (they fire at
   the end of a condition, possibly far from the edge). The first version also kept it for noise reject and a
   holdoff; the unit showed neither moves the point at the full rate (result below), so both conditions went.
   With compression the largest k at the full rate is 156 (a 1 M record); the near window there is 128 points
   against the 2 points of a pair (`compressed_full_rate_reads_search_near_decimated_ones_wide`).
3. `board::frame_mean`: tried with 1024 samples instead of 4096; no gain on the unit (result below), back to 4096.
4. Unit, when the scope is free (setups A and B: the USB 5 V rail on CH2, A the AC ripple view, B the DC overshoot
   view): setup B at 5 us/div and 200 ns/div before/after (cost counters), setup B at
   5 us/div with noise reject on and with a holdoff (edge counters: is the point on the edge at the full rate), setup
   A at 5/10/50 us/div for the level cost, and the 1 kHz probe comp on CH1 (DC edge) at 200 us/div 100 k Auto and at
   5 us/div: the gate on a clean square, decimated and at the full rate.

## Error of the mean

Written for the 1024 trial; the mean is back at 4096 (worst 0.009 div in the same host test). The reasoning on locked
periods holds for any stride.

Against the mean of all samples, at 1 V/div. Noise: sigma/sqrt(N), sigma/32 at 1024. A periodic signal not locked to
the stride: about A/N. Locked (period step/j): a fixed error up to A at any N, as with 4096. A period j*step/k
(j, k coprime) is sampled at j distinct phases; a sine's mean cancels over whole cycles of them, a partial cycle or
another waveform leaves an error.

The locked periods are at most one step, about one screen column. The edge search averages over a column, which
flattens such a signal, so the level does not move the picture. Measured on the host (100 k points, column 103 samples,
+-4 div sine, 256 frames; error p50/p99 in div, edge search on the result):

| Period | Fixed stride | Start per frame | Stratified random |
|---|---|---|---|
| 97 (= step) | 1.56/1.56, no edge | 2.77/4.00, no edge | 0.05/0.24, edge spread 0.17 col |
| 97.5 | 0.09/0.09, no edge | 0.14/0.19, spread 0.09 col | 0.06/0.21, spread 0.14 col |
| 98.04 (12.75 MHz) | 0.014/0.014, no edge | 0.017/0.023, no edge | 0.06/0.21, spread 0.13 col |
| 48.5 | 1.56/1.56, no edge | 2.77/4.00, no edge | 0.06/0.22, spread 0.07 col |
| 400.3 | 0.002/0.002 | 0.001/0.002 | 0.025/0.096 |

A start per frame (the first version) turns the locked error into a jump of up to A between frames. Stratified random
offsets turn it into noise of A/sqrt(2N), but for every period below the step (p99 0.21-0.24 div, above the 0.1 div
bound), and their p99 is above the fixed stride's at every unlocked period in the table. The fixed stride was kept
for its repeatability and these results: stable from frame to frame, and no higher p99 than the others at the
unlocked periods of the table. It is not shown to be the smallest error for every period or metric (p50 at 97.5
and 400.3 is lower for another method), and a period near lock can still give a large fixed error.

Host tests: `frame_mean_within_a_tenth_of_a_division_at_full_screen` (sines of +-4 div at five periods, a square,
ripple with noise, lengths 12.5 k to 1 M: worst 0.055 div, bound 0.1; locked periods at levels across +-4 div: an edge
found stays within a column of the hardware point). `smaller_mean_keeps_the_near_result_on_ripple` (setup A's signal,
ripple at 20 mV/div, noise, AC trigger at 30 mV): the same edge or none in all 72 cases (60 with an edge), moved by
at most 0.53 screen columns against the 4096 mean.

## Result on the unit

Builds 20261114-164623 (dho804 32a0497, before) and 20261114-164716 (this change on 32a0497, after), signed,
confirmed. 12 s windows after 12 s settle. Cost in us per worker frame: decode / level / near / wide / whole frame.
Edges: at the point / other direction / far / none.

| Run | Before: wide, cost, edges | After: wide, cost, edges |
|---|---|---|
| B 200 ns (1.25 GSa/s, raw) | 545, 25/2/33/17/597, 8710/0/5/541 | 0, 25/2/41/0/619, 8668/0/0/562 |
| B 5 us (1.25 GSa/s, live compression) | 4687, 44/2/51/702/1728 | 4684, 46/2/65/684/1767 |
| A 5/10/50 us, level | 181/261/269 | 260/178/180 |
| A 5/10/50 us, frame | 1940/2240/2669 | 2094/2188/2608 |
| probe comp 200 us 100 k (31.25 MSa/s) | 0, 59/2/48/0/907, all at the point | 0, 58/2/50/0/906, all at the point |
| probe comp 5 us (1.25 GSa/s, raw) | 0, 41/2/45/0/724, all at the point | 1, 38/2/49/0/724, all at the point |

- The gate works where it applies: at B 200 ns the wide search and the 5 far edges are gone. It saves little there
  (17 us a frame). B 5 us is a compressed read (min/max pairs), not exact by this definition, so its wide search
  (about 0.7 ms a frame) stays. A compressed read at the full ADC rate has its point within one pair; extending
  `pre_exact` to it is the change that would pay.
- The smaller AC mean gave no measurable gain: the level time switches between about 180 and 260 us in both builds,
  and the frame time is the same. The cost is not the number of samples; a likely cause is the first touch of the
  just-read frame (cache misses that the deposit pays otherwise) and the larger stride defeating the prefetcher. Not
  verified.
- Full rate with noise reject (wide search kept by the gate): probe comp 5 us 6658 at the point, 0 far; B 200 ns 8693
  at the point, 4 far (the before build without noise reject had 5). With a 10 us holdoff: probe comp 6669 at the
  point, B 200 ns 8988 at the point, 0 far. At the full rate neither moves the point off the edge; trigger.md 8.7 is
  not a fixed time. Both conditions can go.

Follow-up (compressed full-rate reads near-only, noise reject and holdoff dropped from the gate, mean back to 4096):
builds 20261114-172318 (d2762d8, before) and 20261114-172401 (97ca9a9, after), both on dho804 f536413, installed with
`--no-kexec` and a reboot.

| Run | Before: wide, cost | After: wide, cost |
|---|---|---|
| B 5 us (1.25 GSa/s, compressed) | 4683, 40/2/168/609/1653 | 0, 47/2/169/0/1068 |
| B 5 us, noise reject | 4702, 47/2/165/644/1661 | 0, 43/2/167/0/1062 |
| B 200 ns (raw) | 0, 25/1/32/0/585 | 0, 25/1/42/0/641 |
| A 10 us (level 1024 / 4096) | 0, 36/170/238/0/2185 | 0, 35/276/232/0/2264 |

- B 5 us: the wide search is gone, a worker frame costs 0.65 of before (drawn p99 17.5 to 13.0 ms). Edges: none found
  in either build (the column average hides the burst edges), so the picture stays at the hardware point as before.
- The level time moved between about 170 and 280 us in every round regardless of the sample count; the A frame time
  is within the spread of the rounds.


- Done: 1-4 on 3b34f1b. Rebased onto dho804 f536413. Follow-up: noise-reject and holdoff conditions dropped,
  `pre_exact` for compressed reads at the full rate, mean back to 4096.
- Done: follow-up unit check (above).
