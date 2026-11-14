# Display > Dots: Off, On, Auto (2026-11-14)

The dots setting has three values. Auto is the default.

| Setting | YT trace | XY |
|---|---|---|
| Off | lines between the points | lines between the points |
| On | a dot per point | dots |
| Auto | lines; the real samples marked over them once they lie 4 px or more apart | dots |

One resolver, `model::trace_style(mode, xy, x_per_sample)`, decides. Every place that drew by the old flag now asks
`Model::dots_at(x_per_sample)` ("do the deposits draw dots"): `drawpipe::draw_frame` and `draw_frame_codes`,
`compress::deposit_pairs`, `Phosphor::draw_ex`, and `compress::allowed`. `draw_xy` asks `trace_style` with `xy` set.
The drawing cache key holds the setting itself.

## Sample marks (Auto)

At short time bases a frame has fewer samples than the screen has columns, and the trace is the sinc-interpolated
line (`sparse.rs`, `board::to_record`). Auto marks the samples the scope really took once they lie at least
`MARK_SPACING_PX` (4) pixels apart. Each mark is a 2 x 2 square centred on the sample, in the channel colour
lightened towards white.

The marks come from the newest record's frame (`Record::raw`) and are drawn on the layer over the traces
(`Ui::draw_sample_marks`, ui/marks.rs). They are not in the phosphor maps. So every frame of a picture keeps the same
deposit and weight, and `picture_ref` grades the lines alone. With persistence the line keeps its history and the
marks show the newest frame. There are no marks for min/max pair frames, roll, scan or the Z input channel.

## Compression

`compress::allowed` holds with Off and Auto and not with On (flicker.md, second section). Auto and the live
compression never meet. Compression starts at 8 samples per column (`K_MIN` x `POINTS_PER_COLUMN`), while Auto
marks start at 4 pixels per sample. The test `hw::tests::auto_dots_never_meet_the_live_compression` checks every
time base, depth and 1, 2 and 4 channels.

## Settings and SCPI

The saved set-up key is `dots = Off|On|Auto`. A set-up saved before this has a bool there, and either value loads
as Auto. The old `true` was the old default, not a choice the user made. SCPI is `:DISPlay:TYPE VECTors|DOTS|AUTO`. `:DISPlay:TYPE?` answers `VECT`, `DOTS` or `AUTO`, the
stock spelling for the first two. Other values give -224.

## Unit check (screenshots)

Use CH1 on the probe-comp output, and CH1 and CH2 for XY. Take a screenshot (`/screen.png`) for each of Off, On and
Auto at:

- 2 ns/div: Auto shows the interpolated line with marks about 35 to 40 px apart at 1.25 GSa/s.
- 20 ns/div: about 4 px per sample at 1.25 GSa/s. That is the border, so Auto shows marks or plain lines
  depending on the plot width. Note which one.
- 200 us/div: Auto shows plain lines and `compress k` is greater than 1. On shows `compress k 1`.
- XY: On and Auto show dots, Off shows lines.

At each point note fps and `:SYSTem:STATe?`. Check by eye that the trace brightness stays steady with Auto.
