How to work with OOS once it runs.

## Controls

Front-panel keys and knobs work as labelled. The Menu button at the bottom left reaches every function.
A channel key held for more than half a second, or a finger held on a channel's card, switches that channel between
AC and DC coupling. A short press switches a channel that is off on and opens its menu. With several channels on, the
first press on one that is not selected only selects it, so the knobs act on it (its card has the coloured border);
the next press opens its menu, and a press with its menu open switches it off. A tap on a channel's card selects and
opens the menu the same way; a tap with the menu open closes it.

## Backlight

Display > Backlight sets the panel's brightness from 10 to 100 %, in steps that look even to the eye. It is
a setting of the scope (`/oos/system.conf`).

## Auto

Works on the channels that are on, at full bandwidth, and sets the memory depth back to Auto; each channel then
gets its own bandwidth back. With no signal it restores the set-up. Afterwards a bar at the top of the plot offers a few
periods, many periods, the rising edge or the falling edge of the trigger channel; its x or a tap elsewhere closes it.

## Windows and full screen

Display > Windows splits the plot into two windows side by side, two stacked, or four. The
same page puts each channel, math channel and the LA into a window; a new split spreads the channels that are on. All
windows share the time base, and a drag in a window moves a channel of that window. Zoom, XY and the analysers use one
window. The button with four corners at the top right of the plot gives the plot the whole screen; it shows inward
corners then and brings the top bar and the cards back. The box button under it arms one drag: the next drag on the
plot draws a box, and that box is the zoom window. A tap does not zoom.

## Views

Instead of one screen with everything on it, Display > Views (2, 3 or 4) gives screens to switch between.
Each view keeps what is shown and where: the windows, the second pane, the measurements and their bar, DVM and
counter, the cursors, the zoom, the histogram, the analyser page, XY and full screen. The channels, the time base,
the trigger and the acquisition are the same in all of them; what changes while a view is shown stays with it, and a
view not used yet starts with the plot alone. To switch: the numbers left of the top bar's buttons, a swipe from the
left or right edge of the plot inwards (a vertical drag there still moves a channel or the trigger level), the Quick
key set to Next view, or SCPI `:DISPlay:VIEW <n>` (`:DISPlay:VIEW:COUNt <1..4>`). The views are saved with a setup.

## Text size

Display > Text size makes the text larger: Normal is the original look, Large and Larger use fonts 2 and 4
pixels bigger in the top bar, the cards, the measurement and cursor rows, the menus, the keyboard and the dialogs. The rows
under the plot grow with them, so the plot gets a little shorter. Where a long text would not fit (a narrow channel card
with four channels on, a long button label in the start menu, a narrow table column) it stays at the next smaller size, and
the dense figure tables of the eye / jitter view keep the normal size. The setting is kept with the other preferences and
across restarts; Default set-up and Recall keep it.

## Bandwidth and noisy signals

A channel's bandwidth is Auto by default: the limit follows the sample rate, as on the
stock system. With Full, the noise of a slow signal can be larger than the trigger hysteresis and the hardware triggers
on the other edge too; the display still shows the edge you set, and Trigger > Noise reject widens the hysteresis.

## Keyboard

Names, formulas and WLAN passwords are typed on the on-screen keyboard. The arrows either side of Space
move the insertion point, a tap into the entry puts it there; typing and Delete work at that point.

## Generator

Tap the frequency in the generator window for a keypad with units; the limits apply by themselves. The - and
+ buttons step it. Changes inside one amplitude range take effect at once and keep the output running.

## Analysers

Menu > Analysers holds Spectrum, Eye / jitter, Power analysis, Bode and the bandwidth test. Eye/Jitter
opens as a full analysis page with optional side or lower panes and has source/rate settings and Close in its pane header. Other analyser views
use the button with three dots at the top right for Exit and Settings.

## Bode with an external generator

In Analysers > Bode pick the Generator preset (Rigol DG, Siglent SDG, UNI-T
UTG900 for the UTG932 and UTG962, or SCPI for one that takes `FREQ` and `VOLT`), then Connection: LAN with Gen address
(IP, port 5555 if none is given; the knob moves the last number) or USB for a generator in the USB-A port of the scope.
On USB the scope takes the first USBTMC instrument it finds (no PC, no driver), asks it for `*IDN?` and shows the
answer; without one the sweep stops with the reason (no USB device, a device that is not USBTMC, no permission). Rigol
DG and Siglent SDG generators are USBTMC instruments as well. The scope sets channel 1 of the generator to a sine of
Amplitude (Vpp) without offset, switches its output on and steps through the frequencies. Over SCPI,
`:BODE:GENerator "usb"` selects USB and `:BODE:GENerator "192.0.2.20:5025"` the LAN again.
- UNI-T UTG900 commands: `:CHANnel1:MODe CONTINUE`, `:CHANnel1:BASE:WAVe SINe`, `:CHANnel1:AMPLitude:UNIT VPP`,
  `:CHANnel1:BASE:AMPLitude`, `:CHANnel1:BASE:OFFSet 0`, `:CHANnel1:BASE:FREQuency`, `:CHANnel1:OUTPut ON`, from the
  UNI-T UTG900 series programming manual (REV 00, 2022-07-15; the English UTG900E manual has the same commands). Not
  yet tried with a real UTG900.

## Bode with a free-running sweep

For a generator that the scope does not control, but that sweeps on its own and
gives a sync (marker) pulse at the start of each sweep. Wire the generator to the input of the device under test and to
the input channel (CH1), the output of the device to the output channel (CH2), and the sync output to CH4. In Analysers >
Bode set Generator = Free-running sweep, then the same Start freq, Stop freq, Sweep time and Sweep (Lin or Log) as on the
generator, the Sync channel, its edge and level (the sync pulse must cross the level; set the channel's scale so the
pulse is on the screen). Input (ref) Off compares the output with Ref level in dBV (0 dBV = 1 V RMS) instead of an
input channel. Points is the number of frequencies read from the sweep. Start / stop sweep sets the time base and the
memory for one sweep, triggers once (SINGLE) on the sync edge and waits; start the sweep on the generator. The plot
shows the gain over frequency, the settings come back afterwards.
- Each frequency is read in a short window of the record, about 4 periods long and never wider than 25 % in frequency,
  by a fit of a sine that follows the sweep. Where the sweep passes a frequency in less than one period, that frequency
  is left out (the message says so). Over three decades, 20 Hz needs a sweep time of a few seconds; a linear sweep
  needs much more time for its low end than a logarithmic one.
- Phase comes from the fit of both channels and is shown only with an input channel, where both fits are clean.
- Sweeps up to about 0.9 s run as normal records (100 ms/div at most), the trigger one division from the left edge.
  Longer sweeps run in scan mode (Auto Roll off during the sweep, up to 200 s/div, 500 s and more): the trigger sits
  at the centre, the sweep in the right half, and the record holds min/max pairs. There the gain is half the
  peak-to-peak value in each window, and there is no phase. The start of the sweep is taken from the sync edge in the
  record itself.
- On a 2-channel model use CH1 for the output, CH2 for the sync and Input (ref) Off.
- `:BODE:RUN ON` starts it over SCPI, `:BODE:DATA?` returns the points (phase NaN where there is none).

## Panes

Display > Pane puts a second view under the waveform: Spectrum with a peak list, Results with statistics, cursors
and peaks, decoder Events, or Trend with two charts. Tap the pane's title for its settings, a trend chart to change its
measurement, the peak list's button to sort it. The x closes the pane.

## Measurements

A default set-up shows six in the strip under the plot. Measure > Dock moves that strip to the left, the
right, or above the trace. In Measure > Add item a tap adds a measurement of the source channel and a second tap removes
it. Measure > Labels on trace draws level lines and arrows with their values on the waveform.

## Overclocking

System > Overclock sets the sample clock from the next power-on. Above 1.25 GHz the noise rises, and 2 GHz
gives wrong samples: it is for tests only. See [docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md).

## Web remote

Open `http://<scope>:8080/`. Click on the picture to touch the screen, drag to swipe. It scales with the
window, so it also works on a phone. The buttons at the bottom save a screenshot or the last record to your computer;
records above 2 million points have to be saved on the scope or read over SCPI. The live picture is a PNG of about 90 KB; "Live picture
format" can switch it to the uncompressed 1.8 MB BMP, or to a JPEG of about 75 KB that the scope's video encoder
makes at under a third of the processor time of a PNG (text a little less sharp; screenshots stay PNG). The choice is kept in the
browser. The PNG is made off the
scope's frame loop, only of a changed screen, up to 20 a second (about 13 reach a browser over Ethernet); the page gets
each new picture as soon as it is made, and while the screen stands still it gets "not modified" instead. The scope's
own screen does not slow down for it. Pause stops the pictures, and a tab in the background stops them too. The Update link leads to the update page.

## Files

Setups are kept on OOS's own persistent area `/oos` and survive a power cycle. Screenshots and CSV files go to
a USB stick in the front port (FAT32, exFAT or ext4), into the folder `OOS`, and can be pulled whenever nothing is being
saved. Without a stick they go to `/tmp/oos`, which is in RAM and lost at power-off, unless you set another directory
with `--store-dir`. The web remote saves them to your computer too. `/oos` holds 16 MiB for calibration and settings.

## Self-calibration

System > Self-cal. Let the scope warm up for 30 minutes and disconnect all inputs first. It takes
about 5 minutes per channel and pass, with 4 passes. Results go to `/oos/cal` and are preferred over the factory files,
which are never changed.

## Updates

Take `oos-<version>.oosupd` from the [latest release](https://github.com/yottabytesolutions/oos/releases/latest), or
build a package yourself (`target/image/oos-dev.oosupd`), then install it on the running scope:

```sh
curl --max-time 300 --data-binary @oos-<version>.oosupd http://<scope>:8080/update
```

or open `http://<scope>:8080/update` in a browser. The scope restarts into the new build by kexec before anything is
written, and the new build installs itself from there; if its kernel hangs, a power-cycle starts the previous build
again. After the install, power-cycle: that start is the trial (coming from 0.1.0, which has no kexec start, too).
From 0.1.6 or older go to 0.1.7 first: 0.1.8 and later bring another device tree (the GPU and the video encoder on),
and only 0.1.7 and later start such an image by kexec with its own tree; 0.1.6 refuses it.
The new build runs on trial. If it crashes in a loop or never becomes stable, the previous image is written back. System > Update shows the state and
System > Roll back update returns to the previous image. Details: [docs/update.md](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md).

## Network

Tap the network box at the top right: it shows the addresses of Ethernet and WLAN and the address of the web
remote. For WLAN, put an RTL8188EU stick into the front USB port, tap Find WLAN networks, tap a network and enter the
password. It joins by itself from then on. Everything open on Ethernet is then open on that network too. Details:
[docs/wlan.md](https://github.com/yottabytesolutions/oos/blob/main/docs/wlan.md). The page also shows the speed of the Ethernet link; a link at 10 Mbit/s gets one more
autonegotiation (10 Mbit retry, on by default, [docs/network.md](https://github.com/yottabytesolutions/oos/blob/main/docs/network.md)).

## Date and time

The scope has no clock battery and starts in 2013. System > Date and time > Time from NTP (off by
default) sets the clock from `pool.ntp.org` or another server whenever a network link comes up, and every hour; Time
zone picks the zone with its daylight saving rules (or takes a POSIX `TZ` string), Set the clock takes it by hand. From
then on file names carry the local date and time (`shot-20261111-143005.png`) instead of a number. The settings are in
`/oos/system.conf`, see [docs/network.md](https://github.com/yottabytesolutions/oos/blob/main/docs/network.md).

## Waveform files

Storage saves CSV and a compact binary file `wave-NNN.bin` (`wave-<date>-<time>.bin` once the clock
is set), little endian: `OOSW`, u32 version (1), u32 channels, u64
samples per channel, f64 dt, f64 t0, then per channel its number (u32) and the samples (f32, volts). Both hold the record
on the screen.

## Experimental limits

`/oos/limits.conf` overrides limits for your own tests. Every applied value is shown at start
and in the System info. See the [disclaimer](Disclaimer) and [docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md).
