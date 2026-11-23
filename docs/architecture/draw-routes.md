# Draw routes and volts conversions

This is a source audit of the pipeline after batch became its only form, and of its module split. The record forms below
are the forms the unit code can route. Frequency and reachability in a particular hardware setup require capture
evidence. A single record can use codes for its picture and volts for trigger refinement, diagnostics, statistics
or the separately delivered newest waveform.

The selector chooses a placement adapter from the frozen projection and the record's own metadata. Tile
accounting credits the final plan's label once per placed record, including a single `zoom` credit for its
two maps. The UI also credits `record` in `place_records` and `Composition::draw_record`.
`route.v1` measures placement activity before final picture accounting. Redraws of a deposit tap retain
its original credit. `wide_search` counts calls into the trigger fallback.
[Route selector](../../ui/src/drawpipe/route.rs),
[tile accounting](../../ui/src/drawpipe/stage/tiles.rs),
[UI accounting](../../ui/src/ui/step_draw.rs),
[route.v1 counters](frame-counters.md#routev1).

## Conversion points

- Whole-record decode starts with the worker decoder from `BatchMatch::dispatch`. `placement::input`
  and `placement_frame` call it when placement needs decoded input. `decode_matched` fills `Frame::ch`
  through `FrameMatch::decode` or the HAL's `decode_volts_into`. Workers can reuse an already decoded
  newest frame when it represents the correct picture input. A separate raw successor read can supply
  the newest waveform while the picture still uses the batch's compressed pairs.
  [Batch matching](../../ui/src/board/dho/matching.rs),
  [placement input](../../ui/src/drawpipe/feed/placement.rs),
  [worker stage](../../ui/src/drawpipe/stage.rs),
  [board decode](../../ui/src/board/dho/hw/decode.rs),
  [HAL conversion](../../hal/src/dho/samples.rs).
- The `Samples` implementation for `Codes` calls `Codes::volts` from `at` and `calibrated` from
  `extremes`. `Codes::extremes` finds the code extrema before converting them. `corrected.rs` applies
  the eight-phase ADC correction, then uses the corrected f64 calibration or the board's f32 conversion.
  `RowTable::build` evaluates that calibrated projection with floating point to construct a cached
  integer lookup. Warm lookup and integer reduction use the table directly. A declined table can still
  leave an integer reduction followed by voltage projection at the extrema.
  [Sample access](../../ui/src/fastdraw/samples.rs),
  [ADC correction and calibration](../../ui/src/fastdraw/corrected.rs),
  [code-to-row lookup](../../ui/src/fastdraw/code_rows.rs).
- `batch_trigger` and `refine_batch` can consume calibrated trigger-lane samples through `Samples::at`.
  Fixed or native hardware trigger positions bypass software refinement. `probe_codes` converts the
  trigger lane for optional header logging. `readout::fold_codes` can decode a frame for measurements
  that need it. `BatchMatch::submit` decodes the newest DMA waveform for record consumers even when the
  worker paints from codes.
  [Trigger refinement](../../ui/src/picture/deposit/standard.rs),
  [header probe](../../ui/src/board/dho/hw/decode.rs),
  [worker measurements](../../ui/src/drawpipe/stage/readout.rs),
  [batch submission](../../ui/src/board/dho/matching.rs).

## The ten labels

| `route.v1` | Unit record forms and path to the pixels | Work needed to keep drawing in codes |
|---|---|---|
| `dense` | Dense, non-envelope Lines with valid code geometry and layout, normally Normal mode, at most 0.5 columns per input sample. `finish_placement` calls `batch_plan_timed`, then `kernel::place_at` and the integer `Scratch::place`. Long records can use `Columns::codes` before the same reduction. `envelope::spans_into` can convert the reduced extrema when the row table declines the projection. | Keep the code-to-row rounding and predecessor join. Extending integer coverage to declined row tables requires the same clipping and calibrated row rounding for those projections. |
| `peak` | Dense, non-envelope Peak Lines selected after the dots and sparse guards. Code-backed input uses the same integer envelope as `dense`, at one time step per word. Without words, `peak` still labels the decoded `SamplesRef::Volts` branch of `kernel::place_at`. Reduced Peak that exposes min/max pairs goes to `pairs` or `zoom` instead. | Preserve one-word timing for plain Peak and pair-centre timing for reduced Peak. Provide a usable code view and row mapping for any decoded Peak placement. |
| `volts` | Ordinary dense Lines left after the mode, envelope, depth and dots guards, with no usable words. The worker gets decoded input if the code adapter or layout was unavailable, or discards the code view when geometry is invalid. `batch_plan_timed` sends the resulting channel slices through `kernel::place_at` to `fastdraw::spans_into`. | Preserve a code view with its captured calibration and geometry through placement. A source that provides only decoded values, such as synthetic or stored data, needs to retain its volts input. |
| `pairs` | Compressed live min/max pairs and reduced Peak pairs, outside HiRes. Non-deep valid code input uses `Scratch::pairs` and integer min/max with pair-centre geometry. A short or unaligned group can take the scalar pair path, whose `spans_with_edges` works in volts. Without words it takes that same path over decoded slices. Deep min/max pairs select `screen_plan` or the split screen grid, after decode. | Extend the code reducer to any unsupported payload and reproduce pair-centre alignment, the nearest-end predecessor join for Lines, a partial last pair, and the weighted legacy screen segments for deep pairs. A single hull per column would change deep counts. |
| `dots` | Dense point drawing and reduced-capture sparse points remapped by `Route::for_dma`. Code-backed tiles use `kernel::add_placed_band` → `deposit_code_dots`. If the table declines the projection, the fallback converts each sample through `Samples::at`. With no retained word view, `Painter::paint_decoded_dots` reuses the newest frame or calls the decoder again into scratch, then draws volts. | Keep a code view through tile painting, including partial payloads where supported. Preserve per-sample column/row rounding, clipped dots and wide-count hit bounds. |
| `upsampled` | Sparse records with more than 0.5 columns per sample, or HiRes with fewer than two samples per screen point. `placement_frame` still decodes an eligible code-backed non-Dots, non-envelope record for trigger and record work. `screen::place` uses fixed-point `polyphase::place` for its drawing samples when the code view has at least 16 points or is reduced capture. Otherwise it uses decoded `sparse::project` or `record::resample_channels`, including Dots and envelope inputs selected by the HiRes guard. | Extend the code interpolation adapter to the remaining eligible record forms and preserve the exact screen grid, kernel support, edges and Dots weights. Avoiding the whole auxiliary decode also requires code-backed screen-fit trigger refinement. |
| `record` | The selector names roll/scan frames `Record`, with a decoded screen raster if passed through the worker. On the unit, roll and scan use the UI stream/record path. The UI counter also covers queued decoded fast placement and eligible hardware record drawing, including modes or views outside `Ui::fast_ok` and fallback records when a batch cannot use the pool. `Ui::draw_queued_prefix` uses `standard::fast_plan`. `deep_draw::record` uses `Phosphor::draw_ex` or `draw_frame_reusing` with an already decoded frame. | The label covers multiple consumers. A hardware-only replacement needs captured code storage and timing through publication, along with stream continuity, record resampling, trigger/segment semantics and view-specific rendering. Software masks or triggers, bus, XY, Z, math and measurements may still need decoded samples. |
| `boxcar` | HiRes at two or more samples per screen point, backed by the unit's plain capture. For Lines with words, `placement_frame` converts the trigger lane when software refinement needs it. `Scratch::boxcar` uses integer sums and fixed-point rows for drawing. `Filtered::row_at` can call `Filtered::at` near a rounding tie, which can replay calibrated f32 samples in the original sum order. A declined row table projects the filtered extrema through the volts sampler. With no word view the Lines branch resamples decoded volts. HiRes Dots use the decoded screen raster. | Preserve the legacy overlapping, clipped windows, the divisor for each window, gaps and half-row decisions. An exact code-domain equivalent to the rounded-voltage sum is needed at boundary ties, plus a point raster on the same screen grid for Dots. |
| `screen` | Deep raw window from a record above `compress::MAX_DEPTH`, with no pair envelope and outside HiRes. `placement_frame` drops words for the screen raster and uses the decoder or the supplied same-DMA frame. `screen::place` resamples the decoded windows. Long records can use `Columns::screen` with `Task` screen shards. The resulting record reaches the weighted segment raster. | Use code-backed trigger refinement and window min/max with the same gaps and overlap, and retain the legacy screen-point positions, per-segment weights and row rounding. |
| `zoom` | A frozen worker zoom view branches to `stage::zoom::place` before the ordinary route selector. The reduced code path `zoom_reduced_plans` places both maps from codes using envelopes, dots or fixed-point detail interpolation. Trigger refinement can still use calibrated samples. For ordinary Normal raw codes, `zoom_code_plans` may place detail Lines with a code filter. Its overview and other detail cases use `captured_codes`. `EnvelopeGrid::append` and the four-lane `shared::append` convert extrema, and `GridCache::resample` converts interpolation support or points to volts. Headered Normal codes use legacy screen-fit trigger search on the coded samples. If code planning declines, `zoom_plans` uses the decoded frame for both maps, including non-Normal or pair forms where enabled, and unsupported reduced geometry such as shifted raw Dots. | Preserve the shared trigger and the two windows, captured-window alignment, per-channel deskew and origin rounding. Keep extrema/interpolation and both weighted raster variants in codes, extending support for the decoded cases while matching per-map Dots and Lines counts. |

The adapter calls above are in [worker placement](../../ui/src/drawpipe/stage.rs),
[split placement](../../ui/src/drawpipe/stage/preparation_columns.rs),
[standard placement](../../ui/src/picture/deposit/standard.rs),
[the integer envelope](../../ui/src/picture/deposit/envelope/placement.rs),
[the HiRes boxcar](../../ui/src/picture/deposit/envelope/adapt/boxcar.rs),
[screen placement](../../ui/src/picture/deposit/standard/screen.rs),
[worker zoom](../../ui/src/drawpipe/stage/zoom.rs),
[ordinary code zoom](../../ui/src/picture/deposit/standard/zoom/codes.rs),
[shared zoom extrema](../../ui/src/picture/deposit/standard/zoom/codes/shared.rs),
[reduced code zoom](../../ui/src/picture/deposit/standard/zoom/reduced.rs) and
[decoded zoom](../../ui/src/picture/deposit/standard/zoom.rs).
See [UI admission](../../ui/src/ui.rs) and [record drawing](../../ui/src/ui/deep_draw.rs) for the UI route.

## Callers retained after batch-only acquisition

- `stage::owner::Owner` owns deposit composition. `Pipe::accept` and `accept_into` are test entry points into that
  owner. The former non-owner composer was already absent at the base.
- `fastdraw::par_map` is called by `step_draw::place_records`. `Ui::draw_queued_bands` and
  `Phosphor::fold_rows` call `par_each`, and `fold_rows` calls `fold_t`. These draw or fold the fallback records.
- Acquisition `publish` calls `Pipe::set_view` and `set_identity` when disabling worker placement.
  `DmaSource::take` calls `Pipe::take_buffer` to reclaim the DMA destination. `Dump::capture_live` calls
  `fixture_view_generation` to record the generation in the dump. `BatchScript::feed_step` calls
  `feed_at` and waits in `host::finish_batch` for the scripted buffer to return.
- `draw_frame_reusing` is called by `deep_draw::record` for eligible decoded frames.
  `deposit_dots` serves that path, while `deposit` and `deposit_codes` serve the host hot-path allocation probe.
- `fastdraw::edge_near` serves unit `frame1::missing_edge` and header probing. `edge_smooth` serves
  `picture::deposit::kernel::place` for decoded input. `dma_poller_cpu` is called by `helper_cores`.

## Module split

The split of `drawpipe.rs` and `fastdraw.rs` by responsibility moves code only. Every function keeps its
pre-split body token for token, and `tools/check-rust-split.py` compares the bodies against a base commit.
A first version of the split rewrote seven functions. On the unit its live matrix fell on three groups,
repeatable over three runs: 1 ch 5 ns to 0.952, 1 ch 50 ns to 0.940 and 4 ch 200 ns to 0.921 of the
reference. The SD mode of those runs is not recorded. The bodies were restored verbatim, and the loss stayed.
The stage benchmark on one cold core showed no difference between the builds before and after the split
(1 ch 5 ns: 273.3, 274.0 and 271.7 µs per record). Installed one by one, every commit of the chain lost 1 to
3 % on the interpolated route, including commits that do not touch drawing. That is code layout of the
production binary, not a defect in a commit. Function and loop alignment flags (`-align-all-functions=6
-align-loops=32`) moved the loss between groups and were not adopted. Tuning the build for the
[A53 cores](../development.md#keeping-build-artifacts-small) closed it.

## Floating-point line inventory

A line counts once if it contains `f32` or `f64`, including type suffixes and intrinsic names. This includes
tests and comments. Geometry, calibration and voltage sample processing can all contribute a line. The scope
is the two root source files and their submodules. A dash marks a file introduced by the split. Unlisted files have zero
matches in both versions.

| File under `ui/src/` | Before | After |
|---|---:|---:|
| `drawpipe.rs` | 33 | 0 |
| `drawpipe/admission/model.rs` | 1 | 1 |
| `drawpipe/batch.rs` | – | 4 |
| `drawpipe/feed.rs` | 3 | 3 |
| `drawpipe/frame.rs` | 3 | 4 |
| `drawpipe/metrics/source.rs` | 10 | 10 |
| `drawpipe/pipe.rs` | – | 1 |
| `drawpipe/reuse.rs` | 3 | 3 |
| `drawpipe/reuse/tests.rs` | 2 | 2 |
| `drawpipe/route.rs` | 6 | 6 |
| `drawpipe/route/tests.rs` | 2 | 2 |
| `drawpipe/route/zoom.rs` | 8 | 8 |
| `drawpipe/stage.rs` | 7 | 7 |
| `drawpipe/stage/fixture_bench.rs` | 6 | 6 |
| `drawpipe/stage/fixture_bench/report.rs` | 1 | 1 |
| `drawpipe/stage/preparation_columns.rs` | 11 | 11 |
| `drawpipe/stage/preparation_columns_tests.rs` | 7 | 7 |
| `drawpipe/stage/tests.rs` | 6 | 6 |
| `drawpipe/stage/zoom.rs` | 8 | 8 |
| `drawpipe/stage/zoom/tests.rs` | 18 | 18 |
| `drawpipe/stage/zoom/tests/pixels.rs` | 13 | 13 |
| `drawpipe/stage/zoom/tests/tiles_bench.rs` | 2 | 2 |
| `drawpipe/tests.rs` | – | 8 |
| `drawpipe/tests/deposit.rs` | 4 | 5 |
| `drawpipe/tests/perf.rs` | – | 9 |
| `drawpipe/tests/routing.rs` | – | 8 |
| `fastdraw.rs` | 112 | 0 |
| `fastdraw/code_rows.rs` | 1 | 1 |
| `fastdraw/code_rows/tests.rs` | 5 | 5 |
| `fastdraw/corrected.rs` | 7 | 7 |
| `fastdraw/corrected/tests.rs` | 4 | 4 |
| `fastdraw/difference.rs` | 3 | 3 |
| `fastdraw/difference/fold_tests.rs` | – | 2 |
| `fastdraw/difference/zoom.rs` | 22 | 22 |
| `fastdraw/difference/zoom/polyphase.rs` | 21 | 21 |
| `fastdraw/difference/zoom/polyphase/tests.rs` | 11 | 11 |
| `fastdraw/mapping.rs` | 5 | 10 |
| `fastdraw/samples.rs` | – | 43 |
| `fastdraw/trigger.rs` | – | 8 |
| `fastdraw/trigger/tests.rs` | – | 7 |
| `fastdraw/validation.rs` | 35 | 35 |
| `fastdraw/whole.rs` | – | 12 |
| `fastdraw/whole/dots.rs` | 15 | 12 |
| `fastdraw/whole/tests.rs` | – | 27 |
| **Total** | **395** | **383** |
