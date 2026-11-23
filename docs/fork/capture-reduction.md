# X2 capture-side reduction

The documented stock-driver write rules below are **decoded from stock**. Computed capture tuples,
inferred Peak word layout and unlabeled register effects are **assumed**.

The 156.25 MHz FPGA read-out scans stored words before play-side compression. X2 measures whether a smaller capture
shortens that scan. A 14,064-word record corresponds to about 90 µs of scan at that clock. These are prior sweep
measurements. X2 has not been run on hardware.

## Register evidence

The repository's stock-driver reconstruction gives the following rules. The evidence column names the
source for each claim. **Assumed** items require explicit opt-in when used by the experiment.

| Register | Value or role | Repository source | Evidence |
|---|---|---|---|
| `0x10d0`, `0x10d8`, `0x10d4` | Above 62.5 MSa/s, `0x8000` OR `max_rate/rate`, then `1`, `0`. At or below 62.5 MSa/s, `0`, `62.5 M/rate`, `0`. | `docs/protocols/horizontal-config.md:181–197`, `docs/fork/specs/04-peak-decimation.md:9–18` | **Decoded from stock**. The CH1 Average 16 fixture includes `0x8002, 1, 0` but is a hand-checked OOS sequence (`hal/testdata/avg-writes.txt:14–16`, `hal/src/dho/avg.rs:127–135`). |
| `0x1004` bit 1 | Set in Peak below the maximum rate, cleared in Normal and at the maximum rate. | `docs/protocols/horizontal-config.md:251–262`, `docs/fork/specs/04-peak-decimation.md:13–18` | **Decoded from stock**. A 2.4 µs pulse at 50 kSa/s was visible in OOS Peak on a unit on 2026-11-07. |
| `0x1004` bit 2 | Set when `rate < max_rate/8`. An averaging filter interpretation is **assumed**. OOS unit measurements at 31.25 MSa/s found full-amplitude aliasing. | `docs/protocols/horizontal-config.md:193–197,262`, `docs/fork/specs/02-hires-fir.md:24–29` | **Decoded from stock**. The separate OOS observations are undated here. |
| `0x1004`, `0x1120` family | HiRes clears the same mode bits as Normal. Stock shifts the play-side fine delay by `−T/2`; attributing that correction to a capture filter is **assumed**. The HiRes bits UI choice does not select a capture register. | `docs/fork/specs/02-hires-fir.md:9–29`, `docs/protocols/horizontal-config.md:350–351` | **Decoded from stock**. |
| `0x10dc` | Stock writes `1` when anti-aliasing is switched off. The effect of the register is **assumed**. OOS leaves it alone. | `docs/fork/specs/04-peak-decimation.md:18,29–32,69–73`, `docs/protocols/horizontal-config.md:193–197,263` | **Decoded from stock**. |
| `0x10e4`, `0x4050`, `0x4054` | Stored points per channel and pre/post point counts. At the centre, pre and post split the depth. | `docs/protocols/horizontal-config.md:59–90,163–179,332–335` | **Decoded from stock**. The earlier unit account reports 2.5 M, 6.25 M and 25 k points for one channel, and 6,250 points/channel for four channels. See lines 110–111. |
| `0x10e0` | Segment size in bursts, calculated from `stored_points × ratio + 384`. Ten stored words per burst is **assumed** from the arithmetic. | `docs/protocols/horizontal-config.md:114–142`, `docs/hardware/fpga.md:228–235` | **Decoded from stock** for the computation. The word-per-burst interpretation is **assumed**. |
| `0x1100`, `0x1108`, `0x11f0`, `0x1170` | Raw play uses the stored-word window and pass-through. Play-side compression emits min/max from that window. | `docs/protocols/horizontal-config.md:269–271,388–389`, `docs/protocols/acquisition-start.md:386–397`, `hal/testdata/second-frame-writes.txt` | **Decoded from stock** for the mode and window writes. The compression experiments describe play-side pairs. Capture-side byte layout remains **assumed**. |

Normal capture keeps one sample per decimation interval. The raw read has 16-bit words, interleaved by the hardware
ratio of 1, 2 or 4 for one, two or three/four channels. The raw Peak payload is believed to contain min/max pairs
(`docs/fork/deep-meas.md:38`). Consecutive pair boundaries, order and the relation of each pair to the programmed
divider are **assumed** for X2, and the Peak window beyond factor 8 is unknown (`docs/hardware/fpga.md:468`).

Stock uses the fixed depth list and 1-2-5 time bases, choosing the lesser of full rate and depth divided by the
ten-division screen time, then applying its exact decimal-to-divider remaps. Auto instead uses one screen at maximum
rate until it hits the depth limit (125 M, 62.5 M or 31.25 M points per channel). For example the documented stock
model at 20 ms/div in Auto gives divider 2 for one, two and four channels at those respective depths. With fixed 1 M
depth at 200 µs/div, one channel uses divider 4 after the 500 to 312.5 MSa/s remap. CH1 at 100 ms/div in Auto Roll
uses 62.5 M points at 62.5 MSa/s with the slow path (`0x10d0 = 0`, `0x10d8 = 1`). These are stock-driver-derived
examples (`docs/protocols/horizontal-config.md:30–112`, `docs/protocols/roll-mode.md:58–64,255–263`), with no recorded
raw stock register trace for those reduced tuples in the checked repository paths.

## Capture plans

`acq.capture` in `system.conf` defaults to `full`. `decimate:n` and `peak:n` request a divider `n` of 2, 4, 8, 16,
32 or 64, relative to the full rate of the channel set. X2 uses CH1, CH1+CH2 or CH1–CH4, the nominal ADC clock,
Normal acquisition, Auto depth, a centred edge trigger and the ordinary triggered analog path. The base plan must
be at full ADC rate. Requests outside this set use `full`.

With `acq.capture.inferred = off`, choose the smallest stock fixed depth from 1 k, 10 k and 100 k that is at least
`ceil(full_points / n)` and less than the full record. The selected profile is checked against the stock remap model.
The table gives the documented rule-derived examples for 1 k. Multiply the time/div by 10 or 100 for the other
depths to keep the same divider. These profiles use Normal mode. Their exact captures have no stock trace here.

| Channels | Divider | Stock time/div at 1 k | Requested → stored rate (MSa/s) | `0x10d0` |
|---|---:|---:|---:|---:|
| CH1 | 2 | 100 ns | 1000 → 625 | `0x8002` |
| CH1 | 4 | 200 ns | 500 → 312.5 | `0x8004` |
| CH1 | 8 | 500 ns | 200 → 156.25 | `0x8008` |
| CH1+CH2 | 2 | 200 ns | 500 → 312.5 | `0x8002` |
| CH1+CH2 | 4 | 500 ns | 200 → 156.25 | `0x8004` |
| CH1–CH4 | 2 | 500 ns | 200 → 156.25 | `0x8002` |

With explicit `acq.capture.inferred = on`, use `floor(full_points / n / 8) × 8` stored points per channel. This
depth and the 78.125 MSa/s profiles (one channel at 16, two at 8 and four at 4) are **assumed**. The opt-in is also
required for every Peak request. Peak uses one stored word per programmed point and lane. The display treats
consecutive words as minimum then maximum, at one pair per two stored words. Pool placement sends the pairs to
the column envelope, using the hardware trigger position on that pair grid. The raw word order, pair rate
and window still need verification from the captures.
Factors beyond 16, 8 and 4 respectively would need the slow path. Its integer `62.5 M/rate` divider cannot represent
those exact rates, so selecting them leaves capture at `full` even with opt-in. A depth below eight points also
falls back to `full`.

Reduced capture reads the whole stored record with play-side compression disabled. Per-frame bytes are
`2 × stored_points × ratio` plus 16 if the header is enabled. The read stride is the frame size rounded up to eight
bytes and is equal to frame size for X2. The dump sidecar carries `capture`, `capture_inferred` and
`capture_base_depth` alongside `rate`, `stored_points`, `frames`, `frame_bytes`, and `frame_stride`.

## Operator procedure

1. With the owner present and the scope released for X2, save `/oos/system.conf` and the UI setup. Use the
   owner-approved signed build. Start with `acq.capture = full`, `acq.capture.inferred = off`, `acq.compress = off`,
   `acq.prearm = off`, `acq.frame_header = on`, `acq.dump = on`
   and `acq.dump.count = 4`. Restart `oos-ui` through the approved workflow after each switch change. Keep the
   generator settings, clocks and capture route fixed between A/B runs.
2. Set `:ACQuire:TYPE NORM`, `:ACQuire:MDEPth AUTO` and `:TRIGger:SWEep NORM`. Set CH1 only, delay zero, DC coupling
   and bandwidth limit off. Use a rising CH1 edge trigger at the midpoint of an external square wave. Disable LA, mask, zone,
   segmented recording and roll. Use a 1 MHz, 1 Vpp square wave for scan timing. For the Peak check, use a repeatable
   signal with a reference edge on CH1 and a pulse narrower than the decimated sample interval. Vary the pulse's
   delay from the reference edge so it moves through multiple sampling positions without changing the channel mask.
3. For a stock-rule smoke test use CH1 at `:TIMebase:MAIN:SCALe 1e-7`. With `:RUN`, record the `full` baseline,
   then `decimate:2` with inferred off. Use the same comparison at 200 ns/div for CH1+CH2 and 500 ns/div for
   CH1–CH4. The reduced sidecar should show 1,000 stored points per channel for these smoke tests.
4. For the timing sweep, set `:TIMebase:MAIN:SCALe 1e-6`. For each channel set, start with `full`. Then request
   `decimate:2`, iterating the exact factors above in ascending order with inferred on. Once decimation delivers
   correctly aligned reads, repeat with `peak:n` and inferred on. Restart the UI between switch changes. Run each
   configuration again with `full` if needed to check repeatability. Stop at the first unexpected result.
5. For each run, use `:SYSTem:STATe?` to check `acq.capture.requested`, the frame-confirmed `acq.capture` and
   `acq.capture.stored_points`. Poll once per second over ten seconds after the first valid frame. Save the
   complete status lines, the UI log, and the four `fixture.txt`/`dma.bin` pairs from `/tmp/oos-batches` before
   changing the setup. `source.v1 bytes`, `read`, `readout`, `grant`, `fill`, `first_trigger`, `records_per_s`, and
   `duty` report rolling p50/p90 (times are in ms). For fixed-count timing also repeat with `:SINGle` and check for
   a one-frame sidecar. Use RUN for the repeating throughput measurements.
6. Check `dma.bin` length equals `frames × frame_stride`; inspect headers and all lane boundaries. Compare bytes
   and scan time at matched record counts, plot `read` against stored bytes, and check the prior 3.22 ms/MB slope.
   For Normal, compare the retained sample positions with full-rate words allowing phase alignment. For Peak, try
   both alternating min/max orders and compare the narrow pulse and its phase sweep against full-rate windows.
   Report captures where the hypothesis fails and keep the original dumps for offline analysis.

Abort on the first missing or shifted frame, inconsistent byte count, or stalled `acquired` count with a known
triggering signal. A wedged capture may leave `acq.capture=pending`, `source.v1 bytes`/`read` at zero or frozen at
their last successful values, and no new `records_per_s` observations. `oos-ui hw: acquire:` may show a poll timeout
for `acquisition done`, or `c2h read of … bytes` with `0x4034` stuck busy (play state 7, e.g. `0x1187` or `0x11c7`
in `docs/protocols/record-play.md:190–211`). A blocked DMA may produce no next log line. Preserve the last status and
logs, stop the script, and tell the owner. Recovery is an owner-performed power cycle with `acq.capture = full` and
inferred off. Restore the saved configuration after the experiment.
