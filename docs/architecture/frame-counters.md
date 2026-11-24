# Frame pipeline counters

`:SYSTem:STATe?` appends these versioned groups. The groups appear in the order
`stage.v1`, `present.v2`, `work.v1`, `route.v1`, `source.v1` and `fallback.v1`. The
[state-query reference](../spec/state-query.md) covers the other fields.

Stage, work and source observations reset on an acquisition generation or epoch change.
Route, presentation and fallback tallies accumulate for the process. Missing quantile observations
print as zero. Zero can also be a measured value.

## stage.v1

Each interval below is a p50/p90/p99 triple in microseconds. The histogram uses 1 ms
bins and reports their lower edges. The final bin contains 255 ms and above.
Every interval has a second field with the `rejected.` prefix. Rejected batches
contribute only intervals with both endpoints available. For those fields, the
outcome timestamp is the rejection time.

| Fields | Unit | Observation |
|---|---|---|
| `read_submit`, `rejected.read_submit` | µs | Batch read completion to intake submission. |
| `submit_dequeue`, `rejected.submit_dequeue` | µs | Intake submission to the coordinator receiving the batch message. |
| `dequeue_deposited`, `rejected.dequeue_deposited` | µs | Coordinator receipt to the timestamp after painting and deposit accounting. |
| `deposited_published`, `rejected.deposited_published` | µs | Painted batch to deposit publication in the hand-off slot. |
| `published_claimed`, `rejected.published_claimed` | µs | Slot publication to claim for first use at composition. Early staging into the UI owner queue leaves this claim timestamp unset. |
| `claimed_composed`, `rejected.claimed_composed` | µs | First-use claim to the CPU composition outcome. |
| `trigger_composed`, `rejected.trigger_composed` | µs | Host observation that starts ring fill, after the first record wait, to the CPU composition outcome. |
| `stage.truncated` | Replaced batch clocks | Each deposit initially holds four clocks. Further inserts keep the first three and replace the fourth with the newest. |

See [batch clocks](../../ui/src/drawpipe/clock.rs), [UI owner composition](../../ui/src/drawpipe/stage/owner/compose.rs)
and [histograms](../../ui/src/drawpipe/metrics.rs).

## present.v2

Presentation timings use p50/p90/p99 in microseconds. They describe DRM panel flips that
completed. Helper-only intervals require helper timestamps. The history holds the last 300
completed flips and a separate 300-observation history for trigger-to-flip.

| Field | Unit | Observation |
|---|---|---|
| `flip.shown` | Pictures | Queued panel requests with a matching completed flip. |
| `flip.late` | Pictures | Completed flips with a DRM vblank sequence after the target sequence, or a timestamp after the target window for steps without a vblank. |
| `flip.timeout` | Requests | Queued requests that reach the 50 ms wait limit without completion. |
| `flip.modeset` | Requests | Panel requests that complete via a modeset. |
| `flip.failed` | Requests | Requests returning failure. A busy request remains staged for retry. |
| `flip.hz` | Pictures/s | Number of flip intervals divided by elapsed time from the oldest to the latest retained completion. |
| `flip.target_refreshes` | Refreshes | Current seal-to-flip target, or zero before a device clock publishes it. Each picture keeps its own target through completion. |
| `helper.skipped` | Picture steps | A later picture-clock period superseded a step deferred while the graphics helper was busy. |
| `composed_submitted` | µs | End of the UI step to graphics helper submission. |
| `submitted_done` | µs | Helper submission to render completion. Pixel readback has a separate reply. |
| `done_requested` | µs | Helper completion to the successful queued DRM request. |
| `requested_flip` | µs | Queued request timestamp to the DRM completion timestamp. |
| `seal_flip` | µs | Actual start of the UI picture step to DRM completion. |
| `done_flip` | µs | Ready timestamp to DRM completion, using helper completion or CPU panel staging. |
| `trigger_flip` | µs | Sampled first-record host observation to DRM completion. The picture carries up to 20 accepted batch timestamps from composition. |

`seal_flip` starts at the actual UI step, including when the helper defers the step. The
picture target is fixed at seal, relative to the step vblank. A deferred step keeps its
original vblank. CPU and helper output both wait for the preceding vblank before requesting a
next-refresh flip. If that event is lost, the queue can submit half a refresh before the predicted target time.
An immediate modeset waits for the target itself or the end of its estimated refresh window.

With divisor three, pictures of one view are evenly spaced. The target is two or three
refreshes. The clock uses the most recent 32 completed renders of the same view, about a
second on the DHO900. It starts at two refreshes and moves to three when two or more of
those renders exceed two refreshes less 0.5 ms of queueing margin. One slow render in the
window is treated as an outlier. It returns to two only after all 32 fit with 1 ms of
additional headroom. The asymmetry keeps a view on the boundary at an even three instead of
alternating: measured on the unit, 4-channel views that alternated missed 3 to 7 % of their
targets. Changes are at least three seconds apart.

View changes reset the sample window and retain the current target while it fills. The panel
continues to seal every three refreshes. A target transition shifts the phase by one refresh
for a single interval. Renders beyond the three-refresh deadline still count as misses.
The timer setting and other divisors use the fixed target derived from the pipeline budget.

Timer steps without a step vblank target the first refresh after the midpoint preceding the
nominal target time. A completion beyond that one-refresh window is late. Helper completion
time contributes to `done_flip` and `done_requested`. The former `present.v1` late count
measured readiness to flip and cannot be compared with `present.v2` target misses.
See [presentation](../../ui/src/pacing/presentation.rs),
[frame clock](../../ui/src/pacing/frame_clock.rs) and [panel events](../../ui/src/system/device/flip.rs).

## work.v1

Each field is p50/p90 in nanoseconds per deposited record over up to 64 batches.
Each batch contributes its measured durations divided by the number of records it deposited.

| Field | Unit | Observation |
|---|---|---|
| `refine` | ns/record | Timed trigger refinement summed across placement jobs. |
| `envelope` | ns/record | Placement worker time less timed refinement. Includes adapter and placement work. |
| `tiles` | ns/record | Band painting time summed across jobs, including serial decoded-dot painting when used. |
| `pool` | ns/record | Sum of wall times across placement and tile dispatches, including the serial decoded-dot pass. |

See [placement accounting](../../ui/src/drawpipe/stage.rs), [tile accounting](../../ui/src/drawpipe/stage/tiles.rs)
and [work quantiles](../../ui/src/drawpipe/metrics.rs).

## route.v1

The deposit worker credits one final plan label per placed record during tile accounting.
A zoom plan can supply both maps for that one credit. Record-path counts come from UI drawing.
These counts measure routing activity before final picture accounting.

| Field | Unit | Observation |
|---|---|---|
| `dense` | Records | Dense code placement. |
| `peak` | Records | Peak placement. |
| `volts` | Records | Decoded-volts placement. |
| `pairs` | Records | Min/max-pair placement. |
| `dots` | Records | Point placement. |
| `upsampled` | Records | Sparse interpolation placement. |
| `record` | Records | Record-path placement, including queued fast placement and eligible hardware record draws. |
| `boxcar` | Records | HiRes window placement. |
| `screen` | Records | Deep raw screen-window placement. |
| `zoom` | Records | Zoom placement with main and detail plans. |
| `wide_search` | Calls | Trigger refinement entered the wide fallback in `trignear`. This is an invocation count. |

See [route selection](../../ui/src/drawpipe/route.rs), [deposit accounting](../../ui/src/drawpipe/stage/tiles.rs),
[UI drawing](../../ui/src/ui/step_draw.rs) and [trigger search](../../ui/src/trignear.rs).

## source.v1

Quantiles are p50/p90 over up to 64 completed ordinary batch reads. The wait
quantiles use only sampled reads that had accumulated waits.

| Field | Unit | Observation |
|---|---|---|
| `wait_admission` | Wait calls, then p50/p90 in ms | Source or record-flow wait calls. The quantiles use their summed duration carried into the next observed read. |
| `arm` | ms | Arm writes, or measured arm service carried forward for a prearmed batch. |
| `fill` | ms | Start of ring fill after the first record wait to the end of fill polling. |
| `read` | ms | Execution of the C2H read step. |
| `readout` | ms | Contiguous read-out register writes before the C2H read. |
| `records_per_s` | Records/s | Records in the completed batch divided by elapsed time since the previous completed read. |
| `duty` | Fraction | Fill time divided by elapsed time between consecutive read completions. |
| `budget_ms` | Whole ms | Configured or derived work budget, truncated to whole milliseconds in the response. |
| `source.serial` | Reads | Completed ordinary reads using a serial grant. |
| `first_trigger` | ms | Arm completion to the observed end of the first record wait. |
| `grant` | Records | Source's record-count allowance for the read. |
| `bytes` | Bytes | Requested C2H transfer size for the measured read step. |
| `captured_discarded` | Records | Records reported by fill polling when the source cancels before reading. |
| `uncorrected` | Records | Decode or matching accounts for records left without requested ADC core correction. |
| `work_per_record_ns` | ns/record | Most recently reported application-model work estimate used in a grant. |
| `fixed_cost_ns` | ns/batch | Most recently reported application-model fixed service cost used in a grant. |
| `parallelism` | Worker lanes | The reported grant's record allowance capped at the worker count. |

See [source metrics](../../ui/src/drawpipe/metrics/source.rs),
[grant model](../../ui/src/drawpipe/admission/model.rs), [source timing](../../ui/src/board/dho/hw/batch_timing.rs)
and [HAL batch phases](../../hal/src/dho/seq/execute.rs).


## fallback.v1

Each field counts records that entered a particular volts fallback. A record may
increment several fields. Each field increments at most once per record per draw,
including when several channels, column jobs, bands or zoom maps enter the same
branch. Worker totals count records drawn into a deposit, before picture acceptance.
If preflight repeats placement, only the placement used for painting contributes.
A redraw through the direct record path has its own count. A retained decoded
waveform counts when drawing uses it.

| Fields | Observation |
|---|---|
| `no_adapter`, `no_words`, `invalid_layout`, `invalid_geometry` | Missing code adapter, adapter declined the record, invalid code layout or invalid ordinary projection. |
| `short_interpolation`, `interpolation_trigger_frame` | Codes were too short for screen interpolation, or the code interpolator ran with a decoded frame for trigger or record work. |
| `decoded_lines`, `decoded_pairs`, `scalar_pair_words` | Volts line spans, pair spans from decoded samples, or pair spans from short or unaligned words using volts projection. |
| `envelope_row_table`, `scalar_row_table`, `pair_row_table`, `dot_row_table`, `boxcar_row_table` | The row table refused a projection, causing that kernel to use volts. |
| `decoded_dots`, `dots_redecode` | Dots used decoded samples, or the worker decoded them again at paint time. |
| `boxcar_trigger_lane`, `boxcar_decoded`, `boxcar_rounding`, `boxcar_voltage_sum` | Decoded HiRes trigger lane, resampling from decoded samples, volts projection near a half-row tie, or the original per-word voltage sum for a rounding-sensitive window. |
| `decoded_sparse`, `decoded_screen`, `split_decoded_screen`, `direct_record` | Sparse projection, screen-grid resampling, column-sharded screen resampling, or the direct record renderer using volts. |
| `trigger_from_codes`, `trigger_from_volts` | Software refinement via calibrated samples read from codes or from decoded slices. |
| `zoom_record_unsupported`, `zoom_reduced_geometry`, `zoom_decoded`, `zoom_header_trigger`, `zoom_header_trigger_miss` | Ordinary code zoom refused the record, reduced code zoom refused the projection, zoom used a decoded frame, headered code zoom refined on calibrated samples, or headered code zoom could not establish an alignment. |
| `zoom_shared_extrema`, `zoom_lane_extrema`, `zoom_linear`, `zoom_sinc` | Code zoom converted extrema in the interleaved or per-lane reducer, or converted samples for linear or Lanczos interpolation. |
| `measurement_decode`, `zoom_measurement_decode` | A measurement fold needed a decoded frame during code placement or code zoom. |

Calibration used to construct a lookup or fixed-point projection is setup work.
The counters cover the draw path, including its trigger and fold work. Source
header logging, source publication of the newest waveform and callers that
supply an already constructed waveform record are outside this observation.

See the [draw route audit and fixture replay](draw-routes.md).
