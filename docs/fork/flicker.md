# Trace flicker at minimum persistence (2026-11-13)

Report: at CH1 100 mV/div, 500 us/div, Auto sweep, 100 k points, the 1 kHz probe-comp trace flickers; less or
not at other time bases. Build d9800d9, `acq.pipeline = batch`, `graphics = gpu`. `:SYSTem:STATe?`: 30 fps,
about 80 frames acquired per second, so 2 to 3 frames per picture, some pictures none.

## Cause

A picture at minimum persistence holds the frames that arrived since the last screen refresh. On the hardware
source they reach the phosphor in two ways (`Ui::step`):

- every frame but the newest: the fast path, `drawpipe::draw_frame` -> `fastdraw::deposit`, one min/max span per
  screen column, weight 24 per pixel per frame;
- the newest: `Source::acquire` makes it a 3600-point record, drawn with `Phosphor::draw_ex`. A flat trace has
  about 3.6 record points per column, so it adds about 3.6 x 24 per pixel.

The grade of the picture (`picture_ref`, the earlier flicker fix) assumes frames of equal weight: the brightest
pixel per frame times the frames in the picture. With one record frame and k deposited frames the brightest pixel is
about 86 + 24 k, so the brightness per frame is 86 (k = 0), 55 (k = 1), 45 (k = 2). The per-frame reference
follows a higher value at once and a lower one slowly, so after a picture without deposited frames the next ones are
graded about twice too dark. The mix changes with the batch sizes, hence the flicker. At short time bases a picture
holds many frames and the one record frame matters little; at 500 us/div with 2 to 3 frames it dominates. The GPU
draws the maps it is given (the same LUT from `vmax`), so CPU and GPU show the same.

## Fix (branch fix/flicker-weight)

When the fast path applies and the record has its frame, the newest frame is drawn with `draw_frame` too. Every
frame of a picture then has the same weight, and `picture_ref` grades them alike. Other cases (zoom, Z input,
averaging, sparse records without a frame) keep `draw_ex`; there no frame goes through the fast path.

Host test `hardware_pictures_look_the_same_whatever_the_frames_per_picture`: a hardware source fed 1 to 5 equal
frames per picture; every picture's brightness within 3 of 255 of the first. Without the fix: off by 255.

## Verification on the unit

Build 20261113-165356 (dho804 6223967): the user confirmed by eye that the flicker is gone at the reported settings
(2026-11-13).

At the reported settings: screenshots in a row (`:DISPlay:DATA?`, only the changed ones count), the number of trace
pixels and their mean red must stay within a few percent; by eye the trace is steady.

# Flicker with dots and live compression (2026-11-14)

Report: CH1 100 mV/div, 200 us/div, 100 k points, Auto, the 1 kHz probe-comp square, dots on, `compress k 15`.
Build 20261114-083030 (dho804 eba0da5). With dots off most of the flicker goes.

## Cause

The same mechanism as above, with another pair of deposits. With live compression the frames of a batch are min/max
pairs, drawn by `compress::deposit_pairs`. With dots, that is one span per column at weight 24 per pixel. Every
`compress::RAW_PERIOD` (200 ms) the newest frame is read again raw. It reaches the picture through `draw_frame` ->
`fastdraw::deposit_dots`: about 100 samples per column on a band of 2 to 3 pixels, so many times the counts per
pixel. `picture_ref` grades as if all frames were equal, so a picture holding the raw frame resets the grade and
the pair pictures after it come out dark. That repeats at about 5 Hz.

Host measurement (`ui::tests::pair_raw_mix_report`, the user's set-up, k = 15). Brightness of the whole trace
against a picture of pair frames only:

| | picture with the raw frame | pair pictures after it |
|---|---|---|
| dots | x0.81, 1844 pixels off by more than 3 of 255 | x0.57, every pixel off |
| lines | x1.02, 137 pixels off (edge columns) | unchanged |

With lines a raw frame is a span per column like a pair, so the grade holds. Its edges can land one column away
from the pairs' (the trigger refinement on pairs is good to about a pair).

## Fix (branch fix/dots-flicker)

`compress::allowed` is false with dots. A pair cannot carry the dot density of its samples. With dots the frames
stay raw and are all drawn by `deposit_dots`. Since then Display > Dots has Off / On / Auto, with Auto the default and drawing lines where frames
compress (display.md). Only On keeps frames raw. The UI publishes the wanted settings on every step (`Source::acquire`), and the acquisition thread
re-plans when they differ. Toggling dots therefore switches the compression without a restart: `compress k` in
`:SYSTem:STATe?` goes to 1 with dots and back with lines.

Tests: `compress::tests::opt_in_and_the_model_conditions`, a "dots" row in
`hw::tests::modes_that_need_raw_frames_never_compress`, and
`ui::tests::pair_and_raw_frames_in_one_picture_keep_the_grade_with_lines`. The last one checks that with lines the
raw frame changes the trace brightness by less than 5 % and leaves the pictures after it unchanged. The same check
fails with dots.

# Deep records with `meas.deep_raw` (2026-11-14)

Records above `compress::MAX_DEPTH` are read through the FPGA compression whatever the display, as envelope frames.
`draw_frame` draws those through the record path (`board::to_record`, min/max pairs per record point, then
`draw_ex`). Once per `deepmeas::PERIOD` (1 s) the raw screen window that `deepmeas` reads replaces the newest frame.
It was a plain dense frame, so it took the dense deposit (`deposit` or `deposit_dots`). One picture a second mixed
the two styles.

Host measurement (`pair_raw_mix_report` with `deep`: 10 M at 1 ms/div, k = 16, 1.2 M-sample window, 3 frames per
picture), against pictures of pair frames only:

| | picture with the raw window | pictures after it |
|---|---|---|
| lines | x1.44, 4333 pixels off | unchanged |
| dots | x1.28, 4453 pixels off | x0.54 |

## Fix

`draw_frame` sends a deep raw frame (`deepmeas::deep_raw_frame`) through the record path of its pair frames.
`to_record` makes min/max pairs per record point from the raw samples as it does from the pairs, so the picture is
the same. On the host it matches with lines and with dots: 0 pixels off, x1.000. The test is
`ui::tests::deep_raw_window_draws_like_its_pair_frames`, and it fails without the fix. A deep raw record shown at
STOP or in Single now takes the same path as the running pictures.
