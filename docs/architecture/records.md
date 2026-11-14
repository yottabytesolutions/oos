# Records and measurements

Target architecture of compression, deep memory, records and the measurement path. Part of
[README.md](README.md). Details and unit numbers are in [fork/compress.md](../fork/compress.md),
[fork/deep-meas.md](../fork/deep-meas.md) and [fork/stop-exact.md](../fork/stop-exact.md).

## Data kinds

The acquisition produces three kinds of data (acquisition.md). Their roles do not overlap:

| Kind | Content | Drawn | Becomes a record |
|---|---|---|---|
| `Batch` | every frame of one ring read: raw words, live-compressed pairs (k > 1) or deep pairs (> 1 M) | yes, the only drawn kind | off the fast path: every frame (the record loop); on it: none |
| `Frame` | the newest frame of a batch, decoded | no | yes, on the fast path |
| `MeasRead` | a raw re-read: the newest frame of a compressed batch (`RAW_PERIOD`), the deep screen window (`deepmeas::PERIOD`), the STOP final frame | no | yes, and it is the record shown |

The day's flicker came from measurement reads drawn among pair frames ([drawing bugs](drawing.md#the-days-bugs-on-that-flow)). In the target a
`MeasRead` is a type that no kernel takes. The off-path pseudo-batch is built from `Batch` frames only.

## Compression and deep reads

Policy and mechanism are split:
- **Policy (UI, pure):** `compress::allowed(&Settings)`, `raw_consumers_off`, `needs_raw` and `deepmeas::want` decide
  what the screen and the readers allow. The change point evaluates them into `AcqWant` (`compress`, `needs_raw`,
  `deep_raw`). Changing any of them raises `gen`.
- **Mechanism (acq):** k from rate, time base and depth (`compress::k_for`), the deep read plan (`batch_plan`,
  `deep_raw_settings` with the narrowed margins), the raw re-reads and their cadence. The thread publishes what its
  plan does for the current `gen` in `Readings`:
  - `k`;
  - `raw_planned { gen, planned }`: whether raw re-reads come for that generation (today `deep_ok`). Until the
    thread has published a plan for the current `gen`, the plan is unknown, and unknown means hold: pair records are
    not measured. The thread publishes the plan in `apply`, before any output of that `gen` (acquisition.md,
    `apply` step 2), so a pair record never arrives before its plan.

The UI never estimates the plan itself (the 2026-11-14 two-lane bug).

Dots keep compression off. So do every reader of the record on screen (decoder, math, FFT, analyzer views, AC/LF/HF
trigger coupling, mask, zones, protocol watch, XY), Single, segments, LA, zoom and Z. One table in `compress.rs`
lists them. `modes_that_need_raw_frames_never_compress` stays the test.

## Records

`wave::Record` stays the unit every reader consumes: volts on the screen window (`to_record`, 3600 points), `t0`,
`dt`, `pairs`, and `Raw` (the `Arc` of the frame and the sample range) for measurements on every sample. In addition
it carries its `BatchMeta` (`id`, `gen`, `epoch`, `mode`).

- **Owner:** the record store (today `Ui::last`, `history`, `seg_*`). It keeps `last[0]`, history and segments. Its
  `ChangeSet` classes are `history` (clear history and segments) and `phosphor` (CLEAR drops `last[0]`).
- **Built by the UI** from a `Frame`, `MeasRead` or batch frame. The window comes from the current view. The trigger
  point comes from the frame's own `BatchMeta` (`record::trigger_point`, near-only search for deep raw windows).
- **Readers:** measurements, cursors, math and FFT, decode, XY, the software mask and zones, the software average,
  the Bus trigger decision, Single, history, SCPI `:WAVeform`, autoset and Bode. Each reader states which records it
  accepts:

| Reader | Accepts |
|---|---|
| measurements and statistics | current `epoch`; after a reconfiguration the current `gen`; raw records, or pairs when no raw read is planned |
| Single | the first qualifying record of its `epoch`: current `gen`, built from a batch armed in that epoch (not one armed ahead before it) |
| autoset, Bode | their stage's `epoch` |
| software mask, zones, average, Bus trigger | current `gen`; a `RecordBatch` of an older `gen` is dropped whole (picture.md, route per batch) |
| decode | any; the cache is keyed by record `id` and decoder settings, so it needs no invalidation |
| history | per route: off the fast path every record of the `RecordBatch`; on it the newest `Frame` per batch. History density therefore follows the route, and each entry carries its `BatchMeta` so replay knows which it is |

## The measurement path

```
record (raw, current epoch) -> measure job (record id, epoch, stats gen) -> measure thread (A53, one scoped thread
per channel) -> result -> UI: accepted if epoch and stats gen are still current -> values, statistics
```

- **One hold rule.** Today two holds exist: `compress::hold` and `deepmeas::hold`. While `raw_planned` is true for the
  current `gen`, pair records are not measured, and the values stay those of the last raw record. When no raw read is
  planned, pair records are measured on their envelope (`meas_on_pairs`). Every value belongs to one record.
- **Which record.** The newest raw record of the step, not the newest record. A raw record that comes while the
  worker is busy waits (`meas_pending`); a newer raw one replaces it.
- **Jobs carry identity.** Today a job carries `meas_n`, a counter raised by clears and by local measurements. The
  target job carries the record `id`, its `epoch` and the statistics generation. A late result for an older epoch or
  from before a statistics reset is dropped by comparison, not by counter conventions.
- **Cheap path.** Deep raw windows (millions of samples) measure only the cheap measurements when only those are shown
  (`framestats::cheap_results`, NEON min/max).
- **Cost on the state line:** `deepmeas::cost` (reads, DMA, decode, record, measure) stays as instrumentation.

## Statistics

Two feeds go into the Measure statistics (`mstat`):
- **per frame:** the deposit workers fold the cheap measurements of every raw frame they draw (`framestats::fold_frame`).
  In the target the folds travel in the `BatchUnit` and reach `mstat` when the unit is merged, with the unit's
  `epoch`.
- **per record:** the measurement results.

Rule: every raw frame counts once. Per route:
- fast path: frames count through the worker folds. The record result of the newest `Frame` updates the values, not
  the counts. Pair frames are never folded, so with pairs only raw `MeasRead` records count.
- off the fast path: no folds. Every record of the `RecordBatch` that is measured counts once.
- `MeasRead`: always counts once. It is made only of pair batches (the `RAW_PERIOD` read with live compression, the
  deep window of deep pairs, the STOP final when the record shown is pairs), whose frames are never folded.

The `stats` class of `ChangeSet` resets `mstat` and raises the statistics generation, `stats_gen`, the third number
owned by `Generations` (README.md, [Generations](README.md#generations)). Today `settings_changed`
clears `mstat` on every settings change, including changes that alter no measurement. Which fields belong to `stats`
is decided in `classify`.

## STOP and Single

- STOP (`AcqCmd::Hold`): the thread finishes the batch, then reads the final frame raw when the record shown is of
  pairs (`Final(id)`). That `MeasRead` becomes `last[0]` and is measured. The wait is state across steps, never a block: the record
  store keeps `final_pending { id, since }`, each step checks `Readings::acked`, and after `STOP_FINAL_WAIT` it keeps
  the last record it has. The step never sleeps on it.
- Batches that a STOP may need to replay are not armed ahead (`prearm_keeps_replay`). Open lever: let the armed batch
  finish and replay its newest frame instead (deep-meas.md, open levers).
- Single: one batch per `epoch`. In Single, deep raw windows come with every batch.

## Rules

- Only `Batch` frames are drawn. `Frame` and `MeasRead` feed records only.
- Records carry `BatchMeta`. Readers filter on `epoch` and `gen`, never on wall-clock time.
- Policy in the UI, mechanism in the acquisition. The UI reads the plan from `Readings` and never re-derives it.
- One hold rule for pair records.

## Tests

- Kept: `modes_that_need_raw_frames_never_compress`, `deep_raw_window_for_one_and_two_lanes_at_10m`,
  `deep_record_gives_vavg_from_the_raw_window`, the stop-exact host tests.
- New:
  - a `MeasRead` never changes the picture, in every route that exists until step 9;
  - a measurement result of an older epoch is dropped;
  - every raw frame counts once in the statistics (fold plus record);
  - pairs are measured only when no raw read is planned.

## Open levers (cost of deep raw reads)

From deep-meas.md, measured at 10 M, 1 ms/div, CH1, Vavg:
- with deep raw reads on: -21 % acquired/s;
- prearm per batch with exact STOP: about +10 %;
- a 2 s `PERIOD`: about half the remaining read cost;
- decode of the window off the acquisition thread, handed over like `feed_codes`: about -5 %.

None are part of the migration. They are candidates once the structure is in place.
