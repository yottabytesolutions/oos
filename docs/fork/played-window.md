# Played windows for live compression

`acq.play_window = on` enables cropped playback of live-compressed Normal batches up to 1 M
stored points per channel. The switch defaults to `on`. A singleton grant and a multi-slot arm
that fills fewer slots both use the admitted crop. Applied capture reduction and its full-rate
reference use whole stored records with playback compression off.

Cropped batches use the pipeline budget for full-record capture time, trigger holdoff, re-arm,
windowed read and placement. The source estimates capture time from the configured memory depth
and sample rate, and learns fixed setup time and DMA time per played record from completed reads.
Placement uses the same worker or record-route estimate as work admission. When one record
exceeds the estimate budget, the source grants one serial capture.

The planner takes the hull of the screen and hardware trigger and pads it on both sides
by `k * (NEAR_COLS * l + NEAR_PAD + l)` stored points, plus the Lanczos kernel's support
at the same step. Here `k` is the effective play factor and `l` is the trigger smoothing
width at the current main plot width published by the UI. Starts and output capacity are
aligned to groups of 8k stored points where the record allows it. This retains the whole-record phase of
2k-sample min/max pairs and the floor-rounded tail at the end of the record. HAL validates
the crop before admission. Rejected crops use whole-record play and count in `window.rejected`.

Cropped play uses the exact stored trigger index minus the window start, divided by k.
Whole-record play keeps the legacy floor of the compressed trigger index. The switch can
therefore move a no-crossing trace by up to (k − 1)/k of a played point, or change which
of two nearby edges is selected. The fractional anchor can change a crossing, so it stays
limited to cropped play to preserve the switch-off pictures. Both decoded plays use the
same screen-record adapter. Both DMA plays use the same batch envelope adapter.

With the window on, both routes refine only near the hardware anchor and keep that anchor
if the near search finds no edge. The decoded route can use the padding before the screen
to arm its comparator for a crossing inside its near window. The margin covers this refinement, so `window.short`
stays zero unless the placed screen extends beyond the available whole record. With the
window off, the existing search can still fall back to a wide search. In captures from
build `20261124-075955` at 625 MSa/s, 100,000 points and 10 µs/div on one channel, about
6 to 10% of records in this view sit at the anchor or a near edge with the window on,
where whole-record play can place them at a far smoothed edge. Whether that far jump is
appropriate for whole-record play remains open for a session with a known signal.

The newest raw replay still reads the whole stored record for readers and STOP. The batch
geometry in `:SYSTem:STATe?` uses `acq.play.lo`, `acq.play.n` and `acq.play.k`.

## Initial host read model

The [fixture excerpts](../../ui/tests/fixtures/played-window/README.md) are played with the
HAL model of register 0x1170. Each output pair is the minimum and maximum of 2k stored
points in order of occurrence, per channel. TX sends only the requested whole groups.
Sizes below include the 16-byte frame header.

The host comparison feeds both plays through the Batch DMA and decoded `fast_plan`
entry points. Across 388 raw records in the five views below, the played crop and whole
record differ in zero envelope columns, zero batch draw columns and zero decoded draw
columns. A no-crossing regression at 10 µs/div pins the 8 ns difference between the
crop's k = 15 fractional anchor and the whole-record floor.

| View | lo | n | k | Bytes/record before → after | N used for estimate | ms/record, scan from window / record start |
|---|---:|---:|---:|---:|---:|---:|
| 1 ch, 1 µs/div | 360 | 13,344 | 3 | 9,392 → 8,912 | 30 | 0.121 / 0.123 |
| 2 ch, 1 µs/div | 0 | 7,032 | 1 | 28,144 → 28,144 | 30 | 0.126 / 0.126 |
| 1 ch, 10 µs/div | 16,560 | 66,840 | 15 | 13,344 → 8,928 | 30 | 0.511 / 0.632 |
| 4 ch, 10 µs/div | 952 | 33,264 | 7 | 40,144 → 38,032 | 30 | 0.995 / 1.022 |
| 1 ch, 10 µs/div, 1 M | 433,008 | 133,920 | 31 | 64,528 → 8,656 | 4 | 1.153 / 4.314 |

Estimates use 0.7 ms / N plus 7.3 ns per stored interleaved word scanned. The two hypotheses
scan n or lo + n points per channel. At N = 2, the 100 k-point 10 µs/div crop takes
0.838 / 0.959 ms per record by the same model. These are estimates for the played read
alone. The 1 M-point whole-record comparison at N = 4 is 7.475 ms per record.
The current 1024-column planner uses lo = 434,000 and n = 131,936 for the 1 M fixture,
giving 8,528 bytes per played record. The other rows use the same geometry as the initial model.

## First use on the unit

Verified on a DHO924S on 2026-11-24 and on by default since then: register ladder word-exact against the raw
record, functional round and 31-view matrix with zero short and zero rejected windows, 1 M depth at 10 µs/div
46 to 253 records per second. The procedure below is kept for other units and models. Use the owner's test
release and device lease under the repository's scope rules. Use a CH1 probe-compensation
signal and a rising Normal edge trigger on CH1 at approximately 0.164 V. Use DC coupling,
50 mV/div, about -0.136 V offset, 10 µs/div, zero delay, one channel and full capture rate.

1. For the exact slot-count ladder, use `oos-fpgad` with oos-ui stopped. Dry-plan each
   row with `oos-fpgad plan` and the arguments below. Execute the same arguments with the
   approved `oos-fpgad run --execute` diagnostic. Use common arguments
   `--mask 1 --play4 --header --normal --src 1 --slope rise --tdiv 1e-5 --scale 0.05 --offset -0.136 --level 0.164 --afe`.
   Set a distinct `--out` path for each capture. The host dry plans use the synthetic
   calibration generated by the register matrix. On the unit, use its installed calibration.

   | Order | Arguments after the common settings | Expected bytes/frame |
   |---|---|---:|
   | 1 | `--depth 100000 --frames 2 --compress 1 --n 66840 --at -33440` | 133,696 |
   | 2 | `--depth 100000 --frames 2 --compress 15 --n 66840 --at -33440 --raw-too` | 8,928 |
   | 3 | `--depth 100000 --frames 30 --compress 15 --n 66840 --at -33440 --raw-too` | 8,928 |
   | 4 | `--depth 1000000 --frames 4 --compress 31 --n 133920 --at -66992 --raw-too` | 8,656 |

2. Check 60 seconds of each step before advancing. Verify DONE, header sequence and rising
   stamps, actual count N, and N × frame bytes. The CLI `--raw-too` reads the newest slot
   at k = 1 through the same crop. Strip the header and compare that slot word for word with
   the model. Compare read time against the estimate with a 10% tolerance. Stop at the
   first unexpected state under the scope rules.
3. For the live UI run, set `acq.capture = full`, `acq.compress = on`, `acq.frame_header = on`,
   and `acq.play_window = on` in system.conf before starting oos-ui. Restore the same view
   with 100 k, then 1 M memory, and verify `:SYSTem:STATe?`. UI admission chooses the
   actual batch count, and k = 1 stays on the whole-record path. Check that the periodic
   raw reader and STOP have the full depth, and that STOP pan and zoom and a subsequent
   RUN behave as before. Repeat the live comparison with the switch off.

The exact-count CLI ladder verifies geometry and same-slot pairs. Its raw helper retains the
crop, so a word-level comparison against a same-slot *whole-record* DMA capture still
requires a diagnostic that exports that DMA buffer. The production whole-record replay is
covered by the no-device acquisition test.
