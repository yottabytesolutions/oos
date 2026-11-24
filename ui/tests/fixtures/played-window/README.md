# Played-window raw fixture excerpts

Recorded on build `20261123-231319` on 2026-11-24 in the J0 window capture round. The CH1 signal
was the 1 kHz probe-compensation square wave. FPGA compression was off and the 16-byte frame
header was enabled. The payload is little-endian interleaved 16-bit ADC words. Each retained
record contains every stored point. Frame indices and timestamp words are original.

| View | Source batch | Retained frame indices | Stored points/channel | DMA bytes |
|---|---|---|---:|---:|
| 1 ch, 1 µs/div | 007 | 1, 2 | 14,064 | 56,288 |
| 2 ch, 1 µs/div | 007 | 1, 2 | 7,032 | 56,288 |
| 1 ch, 10 µs/div | 007 | 1, 2 | 100,000 | 400,032 |
| 4 ch, 10 µs/div | 007 | 1, 2 | 35,160 | 562,592 |
| 1 ch, 10 µs/div, 1 M | 007 | 0 | 1,000,000 | 2,000,016 |

The `oos-batch.v2` sidecars retain the captured view and geometry. Counts and submitted ranges
describe the excerpt. The wide-search count recorded by J0 is for the original batch. The
played-window comparison uses the samples and captured view directly.

## Cropped trigger refinement

`cropped/` contains three compressed DMA batches from build `20261124-075955` on
2026-11-24. The view used one channel at 625 MSa/s, 100,000 stored points and 10 µs/div.
Each batch retains its original header, payload and submitted range, with a crop starting
at 16,560 stored points, spanning 66,840 points, and using a compression factor of 15.
The sidecars omit the separate newest-record replay and dump timing. The route counters
describe the original capture. The regression replays the submitted records through both
placement routes.

| Source batch | Submitted records | DMA bytes |
|---|---:|---:|
| 024 | 30 | 276,768 |
| 025 | 35 | 321,408 |
| 030 | 19 | 169,632 |

The batches and sidecars total 777,988 bytes.
