# Mean-type measurements of deep records from raw samples

The raw deep measurement behavior was **verified on the unit (2026-11-15)** for the reported setups.
Generalizations about read-out register effects are **assumed**.

Raw deep measurements were verified on the unit 2026-11-15. The configuration switch and its off path were
removed on 2026-11-17 (#124); Normal acquisition requests raw reads when a shown measurement or the software
DVM needs samples.

## Problem

A record above 1 M points is read for the screen through the FPGA compression (`hw::batch_plan`: the whole record
to ~625 k min/max points) unless the screen is a small part of it. Since `fix/envelope-stats` the measurements of such
a record leave out what pairs cannot give: Mean, RMS, AC RMS, Std dev, Area and the cycle variants show NaN
(`framestats::needs_samples`).

## Design

- The UI publishes `Shared::deep_raw` (`deepmeas::want`): Normal acquisition, and a shown measurement or the
  software DVM needs samples. `every` in Single.
- After a compressed deep batch (`deep_raw_settings`: compression on, no window, record above 1 M, Normal, no LA) the
  acquisition thread reads the screen window of the newest frame once more, raw (`seq::time::read_raw_again_with_clock` with a window),
  before the next arm, as the live compression's raw newest frame does. Window: the screen and 4096 points each side,
  widened to keep 4096 points around the hardware trigger point, aligned to 8 points and clipped to the record; or the
  whole record if it fits. The selected window across all `ratio` acquired lanes must fit `deepmeas::MAX_BYTES` = 24 MiB
  (12,582,912 points of one lane). The quarter-screen margins of the first version are gone (see "Cost on the unit").
  The C2H path splits it into reads of at most 4 MiB.
- Rate: once per `deepmeas::PERIOD` = 1 s while running; every batch in Single.
- The raw read can replace the newest frame in the record queue or supply the newest waveform tap on the
  deposit route. Measurement and record consumers can use the raw window. The deposit can still use pairs
  from the batch. See the [newest-record contract](../architecture/frames.md#newest-record-in-a-picture).
- While raw deep reads come (`deepmeas::hold`: running, not Single, a pair record whose window fits the budget) the UI
  does not measure the pair records in between. Every value shown belongs to one raw record, at most about 1 s old,
  and the Measure statistics count each raw record once. Max, Min and Pk-pk then update once a second instead of per
  record.
- The raw record measured is the newest raw one of the UI step, not the newest record: at 2 ms/div and up (10 M) a
  batch is one frame, so the next pair frame is often queued behind the raw one. The UI drew the raw frame on the fast
  path (or replaced it as the record) and the value stayed at the last raw record's for good (unit round 5). A raw
  record that comes while the background worker is busy waits for it (`meas_pending`); Clear drops it with the record
  it belongs to.
- In Peak (the memory holds peak pairs), above the budget, or with the deep screen-window read (short time base,
  already raw): as before.

## What STOP shows

Since `fix/stop-exact` (docs/fork/stop-exact.md): STOP holds the acquisition thread, which reads the newest frame of
its last batch raw when the record shown is of pairs; that frame becomes the record on screen, so the measurements
belong to it. With `acq.prearm = on` such batches are not armed ahead, so the replay works there too.

The views after the STOP are read again from the stored record; zoomed out on a deep record that read is of pairs
again. It is drawn, while the raw frame of the STOP stays the record measured, so Mean, RMS, Std dev and AC RMS keep
their values and the statistics count nothing more (docs/fork/stop-exact.md, "Views after the STOP").

## Constants

| constant | where | value |
|---|---|---|
| `PERIOD` | `ui/src/deepmeas.rs` | 1 s |
| `MAX_BYTES` | `ui/src/deepmeas.rs` | 24 MiB |
| window rule | `hw::deep_raw_settings` | screen + 2 x 4096, trigger point +-4096, or the whole record if it fits |

## Evidence

| claim | level |
|---|---|
| a frame of the ring can be played again after a batch read | unit: LA read, segment stamp plays, and the raw replay of the live compression returns the same record (round 2, compress.md) |
| a raw window read of a deep record (screen_window) | unit, existing deep-memory path |
| replay of a deep frame with a window after a compressed read | unit at 10 M, 1 ch: STOP raw-window Vavg equals the host mean, 4 of 4 (round 3); live updates at 1 ms/div (round 5). Trigger alignment of raw against pairs, other depths, channel sets and delays not yet verified on hardware |
| cost at 10 M, 1 ch, 1 ms/div | measured: acquired/s 11.41 -> 9.0 (about -21 %), fps unchanged ("Cost on the unit") |

## Host tests

- `ui/src/deepmeas.rs`: opt-in parsing, `want` (Max alone needs nothing, Single sets `every`, Peak and channel off
  refuse, the software DVM asks), `hold` (only running, not Single, pair records of deep frames within the budget),
  budget and rate.
- `ui/src/board/dho/hw.rs` `deep_raw_window_for_the_measurements`: 10 M at 1 ms/div: window holds screen and trigger
  point, within 24 MiB, decodes to a raw frame with the right trigger point; none above the budget (25 M at 2 ms/div),
  for the short-time-base window read, 1 M, Peak. Two lanes at 10 M and 1 ms/div fit (section "Two lanes").
- `ui/src/ui.rs` `deep_raw_records_carry_the_measurements_between_pair_records`: nothing on pairs before the first
  raw record, the raw record's Mean, then three pair records keep it and are not counted in the statistics; off: pair
  records measured with Mean NaN. Mutation check: without the hold the test fails.
- `ui/src/ui.rs` `live_raw_deep_records_are_measured_while_the_worker_is_busy`: at 1, 2 and 5 ms/div, a new value
  each round, the worker busy when the raw record comes, pair records after it (at 2 and 5 ms/div one already queued
  behind it in the same step). Fails without `meas_pending` (Mean NaN) and without the step's raw record (NaN at
  2 ms/div).
- `ui/src/ui.rs` `clear_drops_the_record_waiting_for_the_worker`: a raw deep record waits for the busy worker, Clear,
  a shallow record is measured, a pair record after it keeps that record's Mean, not the one cleared away.

## On-unit verification

Probe on the 1 kHz compensation output, CH1, 1 V/div. The historical baseline used the off path, which was
removed on 2026-11-17.

1. 10 M depth, 1 ms/div (10 periods on screen), Auto trigger. Measure Vavg, Vrms, Std dev,
   Vmax. The off-path baseline: Vavg/Vrms/Std dev show NaN ("----"). Note wfm/s and `:SYSTem:STATe?` (acquired/s).
2. With raw deep reads, within about 1 s Vavg, Vrms and Std dev show values;
   Vavg about half the square's amplitude plus its low level, Vrms consistent with a 50 % square. Compare with the same
   measurements at 1 M depth (raw records, no compression): within 1 %.
3. Rate: acquired/s against step 1. The 10 % target was not met: 11.41 -> 9.0/s, about -21 % ("Cost on the unit").
   `top -H -d 1`: oos-ui CPU, the measure thread on the little cores. /tmp/oos-ui.log: no `raw read of the deep
   window` errors.
4. The raw record is the same acquisition: Single at 10 M, then the record shown (`:WAV:DATA?` RAW over the screen)
   against its Vavg: equal. A slow edge on screen must sit at the same place in the raw record as in the pair picture
   before it (no shift of the window: trigger point check).
5. Budget: 1 channel at 25 M and 2 ms/div (window above 24 MiB): mean-type stay NaN, Max/Min update per record, no
   raw reads in the log. Two lanes at 10 M and 1 ms/div fit and do read raw.
6. STOP while running at 10 M: see docs/fork/stop-exact.md step 3 (with the views after the STOP); then RUN again.
7. Peak mode, roll, zoom: no raw deep reads (log), as before.

Record the results here and in `docs/fork/unit-dho804.md`.

## Two lanes and the hold (2026-11-14)

Unit round on a1ef5ea: 1 ms/div, 10 M, `:MEASure:ITEM VAVG,CHANnel2` showed `----` (9.9E37) for good. CH2 alone is
acquired on two lanes (ratio 2, the trigger source CH1 with it). The screen window with quarter-screen margins was
9.4 M points x 2 lanes x 2 bytes = 37.5 MB, above `MAX_BYTES`, so `deep_raw_settings` planned no raw read. The UI's
own estimate (`fits`) counted only the shown channels (one) and held the measurement for a raw record that never
came. This predates 1a0e4a2, which only changed how the raw window is drawn.

Fix:

- `deep_raw_settings` narrows the margins, down to 4096 points, when the quarter-screen ones do not fit. Two lanes
  at 10 M and 1 ms/div now read about 6.3 M points.
- The acquisition publishes whether its plan reads the raw window (`Shared::deep_ok`). `deepmeas::hold` follows
  that and no longer estimates for itself.

Host tests: `hw::tests::deep_raw_window_for_one_and_two_lanes_at_10m` and
`hw::tests::deep_record_gives_vavg_from_the_raw_window` (end to end: pair frames, then the raw window, Vavg equal to
its level). Both fail without the narrow window. Mean-type values at 10 M were then still opt-in; since "Default on"
below the key absent means on.

The `measured` counter of `:SYSTem:STATe?` is wired. It counts the frames whose worker-side fold
(`framestats::fold_frame`, the cheap measurements, `acq.pipeline = batch` on the unit) gave at least one result.
Live-compression pair frames never count. Deep envelope frames count for Max, Min and Pk-pk, not for mean-type
measurements. The raw records the UI measures itself are not counted. So whether it reads 0 at 1 ms/div depends on the
measurements shown, not on the time base alone. It counts at time bases without compression and with frames to spare
for the workers, for example 1 us/div with Vavg on.

## Default on (2026-11-14)

Off, a user who adds Vavg sees `----` above 1 M points while 1 M works. So the default is now on. One raw read is
made at most once a second while running, and only while a shown measurement needs samples (`want`). Without a
mean-type measurement shown, nothing changes.

### Cost on the unit

1 ms/div, 10 M, CH1 only (probe comp), trigger on CH1, Auto, Vavg CH1 shown. Rates over 15 s. The last read's
times come from `:SYSTem:STATe?` `deep ...` (`deepmeas::cost`).

| Build | deep_raw | prearm | acquired/s | fps |
|---|---|---|---|---|
| 0b5662d | off | on | 11.41 | 29.9 |
| 0b5662d | off | off | 10.27 | 29.6 |
| 0b5662d | on | on | 9.0 | 30.0 |
| 145819 (before) | on | on | 4.45 | 22.4 (trigger on CH2, mask 0x3) |

One raw read, before -> after:

- DMA: 106 -> 52 ms, for a window of 18.8 -> 12.5 MB.
- Decode: 80 -> 51 ms, in the acquisition thread.
- Screen record: 197 -> 5.5 ms, `to_record` on the UI thread.
- Measurement: 1157 -> 58 ms, in the worker. The measure thread went from 109 % to 7 % of a core.

The changes behind these numbers:

- **Window:** the screen and 4096 points each side, not quarter-screen margins.
- **Screen record:** the pair spans of `to_record` use the NEON `min_max`.
- **Trigger search:** only near the hardware point, falling back to that point. Without that, a flat trigger source cost about 160 ms per window.
- **Measurement:** shown measurements that are all cheap are measured from the sums (`framestats::cheap_results`), not the full analysis.

The raw window reads at about 240 MB/s. Chunking is not the cost: it is the batch read path, 4 MiB per `read()`. A
10 M record through the FPGA compression takes 68 ms for its 1.25 MB, which is the compressor's rate.

The remaining -21 % acquired/s comes from two causes. About half is prearm, which is off for every batch while
deep reads are wanted (-10 %, measured). The other half is the acquisition thread blocked about 103 ms a second by
DMA and decode. The frame rate is unaffected. The 5 % goal was moved for this case (team-lead, 2026-11-14): it
applies only at deep records with a mean-type value shown, and the levers below change STOP or the update rate.

### Open levers

- **Prearm per batch (about +10 %, measured):** prearm only where no raw read follows. That conflicts with the exact
  STOP (`final_replay` needs the ring of the last batch). A variant keeps STOP exact: at STOP with a batch armed
  ahead, do not disarm; let it finish, read its pairs, then read its newest frame's raw window. Picture and
  measurement then come from that newer record, at one more record time (16 ms at 10 M plus the trigger wait,
  within `STOP_FINAL_WAIT`). Unit check needed.
- **2 s `PERIOD`:** halves the remaining read cost (about -10 % -> -5 %). The measurement then updates every 2 s.
- **Decode off the acquisition thread:** the worker decodes the window from its bytes, handed over like the
  pipeline's `feed_codes`. About -5 %.

Estimate: the first two together about -5 to -6 %, all three about -3 %.

## Risks

- Replay of a deep frame after a compressed read gave the right means at 10 M (round 3), but the trigger alignment of
  the raw window against the pairs is not yet verified on hardware (step 4 decides); a wrong window would put the
  measurement on the wrong samples.
- A 12 M-point raw frame is copied once into `Shared::frame` (48 MB) and run through `to_record` and the measurement
  threads: about once a second; the measurement of 12 M samples on the little cores may take longer than the period,
  then results come less often.
