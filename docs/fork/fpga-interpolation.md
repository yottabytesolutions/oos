# FPGA display play experiment

Register recipes labelled stock reconstruction are **decoded from stock**. Computed tuples and unlabeled
hardware effects are **assumed**. The experimental presets have host evidence.

Wave F measured 1,959 records/s with one channel at 5 ns/div. On one A53, resampling took 488 µs and tile
construction took 1,209 µs per record in that setup, versus 58 µs for tiles on the dense route. The
experiment asks whether the FPGA can supply a display-length trace from the same stored record. The experiment
has host evidence only. It does not change picture placement or software resampling.

## Register evidence

**Decoded from stock** identifies the reconstructed stock write values. The table also names the
supporting unit observations. Those observations have no dated raw trace in this reconstruction.
**Assumed** identifies computed tuples, interpreted effects and missing parts of the stock recipe.
The OOS column describes intended software writes. Stock writes refer to analog acquisition
with zoom and LA off.

| Register | Stock display path | OOS raw path |
|---|---|---|
| `0x1004` bit 2 | **Decoded from stock** stock reconstruction: the capture-side average/sinc select follows `rate < max_rate/8`. It is clear at the nominal full rates here. Its relation to the display interpolator is **assumed**. [Horizontal configuration, lines 255–265](../protocols/horizontal-config.md#3-acquire-modes-interpolation-fpga-vs-software) | Same rate rule in Normal mode. OOS forces the bit on in hardware Average. [Acquisition setup, lines 108–113](../../hal/src/dho/acq.rs) |
| `0x1100`, `0x1108` | **Decoded from stock** stock reconstruction: screen input length + `(0x48 \| ratio<<9) + k`, then first input point. The exact sub-sample start convention is **assumed**. [Horizontal configuration, lines 273–288](../protocols/horizontal-config.md#31-display-trace-what-the-live-path-transfers) | Whole record by default, or requested window. Writes points + `(0x48 \| ratio<<9)` and the start, per channel. [SCU read-out](../../hal/src/dho/scu.rs) |
| `0x1104`, `0x110c` | **Decoded from stock** zoom window length and start use the zoom parameter copy. [Horizontal configuration, lines 337–342](../protocols/horizontal-config.md#41-example-1-ch1-only-200-usdiv-auto---25-mpts-at-125-gsas) | No zoom window writes. [SCU read-out, lines 156–178](../../hal/src/dho/scu.rs) |
| `0x1118/0x111c`, `0x1130/0x1134/0x1138`, `0x1140/0x1144` | **Decoded from stock** stock reconstruction: main/zoom input length + 40, enable in bit 31 and factor in each of the low three bytes, with coefficient RAM loaded by row and phase. The auxiliary units use factor 200. Available main/zoom factor families are recorded. The exact selection, phase and coefficient values below one sample per column are unknown. [Cold boot, lines 282–291](../protocols/cold-boot.md#38-sinc-coefficient-ram-step-21) and [sinc analysis, lines 5–30](specs/05-sinc-play-chain.md#1-what-the-stock-firmware-does) | Writes `0x00010101` to main and zoom, skips input length and the other units and does not load coefficients. [SCU pass-through, lines 207–227](../../hal/src/dho/scu.rs) |
| `0x1154`, `0x1158/0x1160`, `0x115c/0x1164` | **Decoded from stock** stock reconstruction: bits 0/1 disable main/zoom, bits 8/9 enable ×2, phase words are 0, increments are `floor((2^32 - 1) × input/output)` or `0xffffffff` with linear off. A unit delivered a 10× linear trace at 50 kSa/s after stock hand-over. The two-channel 1 µs/div register tuple in the reconstruction is explicitly **assumed**. [Horizontal configuration, lines 264–267 and 360–412](../protocols/horizontal-config.md#42-example-2-ch1--ch2-1-usdiv-auto) and [unit trace, lines 67–78](../protocols/adc-spu-init.md#first-waveform-frame-read-under-oos) | Writes `0x1154=1`, then `3`, zero phase and `0xffffffff` increments. [SCU pass-through](../../hal/src/dho/scu.rs) |
| `0x1170/0x1174`, `0x11f0/0x11f4`, `0x11fc`, `0x1010` bit 31 | **Decoded from stock** stock reconstruction: k=1 is pass-through, bits 28–29 are preserved, and bit 31 enables min/max for k>1. TX sends `roundup8(1000 × points/column)` points per channel, `0x11fc=1000`, with header bit clear. The 625,000-point one-channel play at 200 µs/div was verified on the unit. Zoom has a separate TX length. [Horizontal configuration, lines 278–288](../protocols/horizontal-config.md#31-display-trace-what-the-live-path-transfers) and [live unit read, lines 254–259](../protocols/record-play.md#62-test-b-live-mode-frame) | Compression off for ordinary short sweeps. `0x11f0` uses the raw window and TX group rounding; header is switchable; normal reads do not program `0x11fc` or `0x11f4`. [SCU](../../hal/src/dho/scu.rs) and [read sequence](../../hal/src/dho/seq/readout.rs) |
| `0x4008` bit 25, `0x105c`, `0x14c8..0x1514` | **Decoded from stock** zoom enable and main/zoom gain and offset. Gain is Q10 with optional ADC ×2. With 1/2/4 channels the gain lanes are `AAAA/ABAB/ABCD`. Values depend on vertical calibration. [Play registers, lines 140–153](../protocols/record-play.md#3-state-the-play-path-depends-on) and [AFE reconstruction and unit verification, lines 306–320](../protocols/afe-settings.md#41-digital-gain-and-offset) | Type-4 raw play has zoom off. The acquisition setup already programs the main and zoom gain/offset copies when it programs the AFE. [Sequence](../../hal/src/dho/seq.rs) and [gain setup](../../hal/src/dho/wpu.rs) |
| `0x1120/0x1124`, `0x1128/0x112c`, `0x1290/0x1294`, `0x131c`, `0x10a0` | **Decoded from stock** register packing: coarse/fine main and zoom use an enable header followed by lane and 16-bit value. **assumed** and incomplete: actual lane words and enable flags for these profiles, nominal 3/2 sample delay plus deskew, and what the filter word `16` changes. Wave-early has nominal word `0x02580044` with LA off and bits 15:12 preserved. LA has separate delays. [Cold boot, lines 299–307](../protocols/cold-boot.md#39-registers-of-the-first-configuration) and [horizontal configuration, lines 344–351](../protocols/horizontal-config.md#41-example-1-ch1-only-200-usdiv-auto---25-mpts-at-125-gsas) | No analog coarse/fine or filter writes. OOS sets wave-early for LA and restores `0x02580044` when leaving LA. Existing settings can reflect the prior firmware or cold-boot state. [LA setup](../../hal/src/dho/la.rs) and [configuration](../../hal/src/dho/seq.rs) |

The reconstruction does not give an exact stock short-time-base recipe. The fitted register differences below
apply only to the documented linear branch at one or more samples per column. The stock-derived crop, phase
increment and output count are **assumed** for each setup. The experiment leaves existing gain, wave-early,
coarse/fine delay and filter state in place. It does not enable sinc or assume the coefficient RAM is populated.

## Nominal-clock comparison

Each cell lists stock Auto stored points, OOS Auto raw TX points, and experimental TX points. Stock Auto depths
are derived from the documented model at 1.25 GHz, with a per-channel rate of 1.25 G, 625 M or 312.5 MSa/s.
OOS adds a capture margin in Auto. The table models a centred trigger, `acq.capture = full`, and header-free reads.
The supplied 5 ns/div unit result used 256 raw points. The table gives the values computed for this nominal Auto setup.
The dash means the experiment uses raw. Exact short-time-base stock display lengths below one sample per column
are unknown. [Stock model, lines 59–110](../protocols/horizontal-config.md#12-algorithm) and
[OOS Auto depth](../../ui/src/board/dho/hw/plan.rs).

| Time/div | One channel | Two channels | Four channels |
|---|---:|---:|---:|
| 1 ns | 14 / 80 / — | 8 / 72 / — | 4 / 72 / — |
| 2 ns | 25 / 96 / — | 14 / 80 / — | 8 / 72 / — |
| 5 ns | 64 / 128 / — | 32 / 96 / — | 16 / 80 / — |
| 10 ns | 125 / 192 / — | 64 / 128 / — | 32 / 96 / — |
| 20 ns | 250 / 320 / — | 125 / 192 / — | 64 / 128 / — |
| 50 ns | 625 / 704 / — | 314 / 384 / — | 158 / 224 / — |
| 100 ns | 1,250 / 1,408 / 2,000 | 625 / 704 / — | 314 / 384 / — |
| 200 ns | 2,500 / 2,816 / 3,000 | 1,250 / 1,408 / 2,000 | 625 / 704 / — |
| 500 ns | 6,250 / 7,032 / 7,000 | 3,125 / 3,520 / 4,000 | 1,564 / 1,760 / 2,000 |
| 1 µs | 12,500 / 14,064 / 13,000 | 6,250 / 7,032 / 7,000 | 3,125 / 3,520 / 4,000 |

The **assumed** main words for the supported 1-2-5 steps are as follows. In raw mode, start is zero, linear step is
`0xffffffff` and the lengths are the middle numbers above. Both paths use `0x1170=0x40000001`.

The experimental tuples write `0x1154=2`, `0x1158=0` and `0x11fc=1000`. Their use in these setups is
**assumed**.

| Time/div | Enabled channels | `0x1100` | `0x1108` | `0x115c` | `0x11f0` | Bytes/frame with header |
|---|---|---:|---:|---:|---:|---:|
| 100 ns | CH1 | `0x072b` | 79 | `0x9fffffff` | 2,000 | 4,016 |
| 200 ns | CH1 | `0x0c0d` | 158 | `0xd5555554` | 3,000 | 6,016 |
| 200 ns | CH1+CH2 | `0x092b` | 79 | `0x9fffffff` | 2,000 | 8,016 |
| 500 ns | CH1 | `0x1ab3` | 391 | `0xe4924923` | 7,000 | 14,016 |
| 500 ns | CH1+CH2 | `0x107e` | 197 | `0xc7ffffff` | 4,000 | 16,016 |
| 500 ns | CH1–CH4 | `0x0e64` | 98 | `0xc7ffffff` | 2,000 | 16,016 |
| 1 µs | CH1 | `0x331d` | 782 | `0xf6276275` | 13,000 | 26,016 |
| 1 µs | CH1+CH2 | `0x1cb3` | 391 | `0xe4924923` | 7,000 | 28,016 |
| 1 µs | CH1–CH4 | `0x147e` | 197 | `0xc7ffffff` | 4,000 | 32,016 |

`acq.play = raw` is the default. `acq.play = stock` with `acq.play.inferred = on` requests the linear setups
above. The plan requires Normal analog acquisition, Auto depth, one/two/four channels at nominal full rate,
a centred edge trigger, zero deskew and an uncompressed type-4 play. A profile outside those conditions,
with an integral samples-per-column ratio or above 1 µs/div uses raw. The computed input window and the last
interpolator neighbour must fit the stored record. The programmed TX length drives DMA sizing:
`2 × ratio × TX points`, plus 16 bytes with a header, rounded up to an 8-byte stride. The stock-play dump
sidecar adds `play`, `play_inferred`, `play_window` (points per channel and start), `play_linear_step` and
`play_frame_len`. The existing `window` field describes the raw request. Actual delivery of those frames
remains to be measured. [Frame sizing, lines 190–200](../hardware/fpga.md#frame-size-and-the-tx-length).

## Operator procedure

1. With the owner present and the unit released for the experiment, save the configuration and use the
   owner-approved signed build. Start with `acq.play = raw`, `acq.play.inferred = off`, `acq.capture = full`,
   `acq.compress = off`, `acq.prearm = off`, `acq.stop_slot = zero`,
   `acq.frame_header = on`, `acq.dump = on` and `acq.dump.count = 4`. Restart through the approved workflow
   after switch changes.
2. Use Normal acquisition, Auto memory, Normal edge trigger on CH1, horizontal delay and deskew zero, ADC
   cores Raw, zoom/LA/mask/zone/segments off. Start with CH1 at 1 µs/div and a stable 1 MHz sine wave. Also use a
   square wave for the edge alignment check. Run through CH1, CH1+CH2 and CH1–CH4. Apply known distinct amplitudes
   to identify lane order. Keep the trigger and generator settings fixed between runs.
3. Capture a raw baseline. Repeat with `acq.play = stock` first with inferred off to verify fallback, then with
   inferred on. Collect `:SYSTem:STATe?` including `acq.play.requested`, frame-confirmed `acq.play`,
   `acq.play.frame_len`, and `source.v1 bytes` (p50/p90 bytes per batch). Save the logs and the batch dumps
   before each switch change. Check `dma.bin` length against `frames × frame_stride`, examine header and
   lane boundaries, and correlate the byte distribution with the frame count.
4. For a same-record comparison, capture a one-frame dump with SINGLE. Keep the ring stopped and use the
   existing STOP view reread to fetch that slot as raw data; zoom out in STOP if needed to include the
   entire stored window. After the STOP view updates, use `:WAV:MODE RAW;:WAV:FORM WORD;:WAV:PRE?` and
   `:WAV:DATA?` for each channel. Use its preamble to recover the sample positions. Check that the ring
   was preserved from the recorded read. Discard the comparison if any new arm or lost replay could have
   changed the slot.
5. Strip header bytes and deinterleave the payload. Use the recorded window start and phase step to linearly
   interpolate the corresponding raw samples offline. Compare delay, polarity, amplitude, per-lane residual
   and edge position, then compare to the software Lanczos interpolation on the same time grid. Sweep 500 ns/div
   and the eligible 100–200 ns/div profiles only after all channel counts return consistent frames at 1 µs/div.
   At 5 ns/div confirm the raw fallback. Save throughput and `source.v1 read` and `readout` p50/p90. The
   existing software picture path can still dominate the measured time.

Abort at the first missing or misaligned frame, wrong byte count or frozen `acquired` count with a known
triggering signal. `source.v1` can show `bytes`, `read` and `records_per_s` frozen at their last values or
zero while the new configuration is pending. A blocked C2H read can emit no next log line. In authorized
read-only observations a persistent `0x4034` with bit 13 clear and low three bits equal to 7 (for example
`0x1187` or `0x11c7`) indicates a play stuck waiting for TX. State 7 is also a normal transient during play.
Preserve the logs and stop the experiment. Restore `acq.play = raw` and `acq.play.inferred = off` through the
approved workflow. If acquisition stays dead, the owner power-cycles the unit. [Status evidence, lines 160–203](../protocols/record-play.md#32-tx-length-model-and-the-stall).
