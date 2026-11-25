Which DHO800/DHO900 models OOS supports, what it does, and which parts have run on real hardware.

## Supported models

OOS reads the model from `/rigol/data/vendor.bin` at start and adapts channels, logic analyser, generator and EXT
trigger to it. The stock software has to be on the card for that; without it, set `model = ...` in `/oos/limits.conf`.
Details and limits: [docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md).

| Model | Rated bandwidth | Channels | Logic analyser | Generator | EXT trigger | Runs OOS |
|---|---|---|---|---|---|---|
| DHO802 / DHO812 | 70 / 100 MHz | 2 | - | - | yes | not yet |
| DHO804 / DHO814 / DHO824 | 70 / 100 / 250 MHz | 4 | - | - | - | a DHO804 runs it (user report) |
| DHO914 / DHO924 | 125 / 250 MHz | 4 | yes | - | - | not yet |
| DHO914-EDU / DHO924-EDU | 125 / 250 MHz | 4 | yes | - | - | not yet |
| DHO914S / DHO924S | 125 / 250 MHz | 4 | yes | yes | - | a DHO924S runs it (developer's unit) |

Each row is one hardware variant: its models differ only in the licensed bandwidth ([docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md#21-hardware-variants)
2.1), so a model whose row runs OOS counts as verified, and the USB install takes it without being told
([docs/install-usb.md](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md#4-models) 4). The other models are prepared from the known model table and have
never run OOS. The feature columns follow the model line-up; whether the EDU variants have the LA port is not
confirmed. DHO1000,
HDO1000/2000/4000 and MHO models are different boards and are not supported; [docs/porting.md](https://github.com/yottabytesolutions/oos/blob/main/docs/porting.md) says
what would carry over and how an owner can describe one.

Limits on all models:

| | 1 channel | 2 channels | 3-4 channels |
|---|---|---|---|
| Sample rate per channel | 1.25 GSa/s | 625 MSa/s | 312.5 MSa/s |
| Memory depth per channel | 125 M | 62.5 M | 31.25 M |
| Memory depth with the LA on | 31.25 M | 12.5 M | 10 M |

OOS does not limit the bandwidth to the rated value: "Full" opens the front-end filter on every model. The real
bandwidth depends on the hardware, and you can measure it with the [bandwidth test](https://github.com/yottabytesolutions/oos/blob/main/docs/bandwidth-test.md).

For how the hardware works, see the [hardware overview](https://yottabytesolutions.github.io/oos/hardware/overview.html).

## Features

### Acquisition

- Normal, peak detect, average and high-resolution modes (high resolution in software, it needs more samples than screen
  points); Auto or fixed memory depth up to 125 M points
- Time base from 1 ns/div; roll mode from 200 ms/div, scan mode at slow time bases
- Zoom window, pinch and drag on the touch screen
- Auto set-up, then a bar with four views of the signal: a few periods, many periods, the rising or the falling edge
- Channel bandwidth Auto (follows the sample rate, the default), Full, 20 MHz or a limit in 5 MHz steps

### Trigger

- Hardware trigger: edge, pulse, slope, video, pattern, duration, timeout, runt, window, delay, setup/hold, Nth edge
- Protocol trigger (bus events found by the decoders)
- Zone trigger: up to two boxes or polygons a waveform must or must not cross
- Noise reject (wider comparator hysteresis); a slow, noisy edge is drawn on the edge you set, not on the other one

### Display

- Dots or vectors, colour grade, waveform histogram, dotted graticule
- Waveform windows: the plot split into two windows side by side, two stacked, or four, each channel, math channel and
  the LA in a window of your choice, all on one time base
- Full screen: one tap at the top right of the plot hides the top bar and the cards, one more brings them back
- Text size Normal, Large or Larger for the bars, cards, measurement rows, menus and dialogs; backlight brightness
- HDMI output as a mirror, as a separate waveform workspace, or laid out again at the monitor's size
- the picture drawn on the GPU (Mali-T860, Panfrost; System > Graphics, CPU as before is the other choice): a third
  less load on the frame loop, a monitor scaled without CPU work ([gpu/README.md](https://github.com/yottabytesolutions/oos/blob/main/gpu/README.md)); the web picture as
  JPEG from the video encoder (VPU)

### Measurement and maths

- 35 automatic measurements plus delay and phase between channels, 14 at once, with statistics
- Time, voltage, tracking and XY cursors; DVM; frequency counter in the FPGA (reciprocal, up to 6 digits; frequency, period, totalize)
- Second pane under the plot, as high as its content: spectrum with a peak list, results table with statistics, cursors
  and peaks, decoder events, or the trends of two measurements
- Measurements labelled on the trace: level lines and arrows with their values
- Up to four views to switch between (Display > Views): each keeps its windows, second pane, measurements, cursors
  and zoom; a swipe from the edge of the plot or the numbers in the top bar switch
- Data logger: measurements as CSV at an interval from 1 s to 1 h, to the stick or the card
- 8 math channels, also on each other, with free formulas (`sqrt(CH1^2+CH2^2)`): add, subtract, multiply, divide, integrate, differentiate, abs, sqrt, log, ln, exp, Ax+B,
  low/high/band-pass and band-stop filters, FFT
- Reference waveforms with scale, offset, colour, name and CSV export and import, mask test

### Protocol decoders (26)

- Four decoders at once, with an event table you can export and that jumps to an event; DCC lists one command per row
  ("Loco 5  Fwd 50/128")
- Serial: UART, UART 9-bit, I2C, SPI, Quad-SPI, I3C, 1-Wire, DS2482, SWD, JTAG
- Automotive and aerospace: CAN, CAN-FD, LIN, FlexRay, SENT, ARINC 429, MIL-STD-1553
- Audio, lighting, industrial: I2S, S/PDIF, DMX512, Modbus RTU, Modbus ASCII
- Other: USB low/full speed, USB PD, parallel
- Model railways: NMRA DCC (contributed by Arjen Halma), with a view of the layout in Display > Second pane > Bus view:
  locomotives, turnouts, signals, and the track status, green while live and red after an emergency stop
- New decoders come as protocol modules: one file, one registration ([docs/protocol-modules.md](https://github.com/yottabytesolutions/oos/blob/main/docs/protocol-modules.md))
- New measurements come as measurement modules the same way ([docs/measurement-modules.md](https://github.com/yottabytesolutions/oos/blob/main/docs/measurement-modules.md))

### Analysers

- Spectrum analyser: resolution from the whole record, span that follows the signal in the pane, fundamental and harmonics
  found as numbered peaks, RBW, trace modes, waterfall, THD, SNR, SFDR, limit line from a trace
- Eye diagram with voltage and UI axes, density colours, TIE trend, histogram and spectrum, bathtub curve. Jitter split into TJ, RJ and DJ (dual-Dirac model) and measured DCD, DDJ and PJ, pooled over records
- Power analysis: quality, harmonics, switching, ripple
- Bode plot with the built-in generator or an external one on the LAN or the USB-A port (APPL or BSWV command sets, a
  UTG900 preset, generic SCPI), or from one record of a generator that sweeps on its own and marks the sweep start on a
  sync output
- Identify: names a signal and sets up a trigger and decoder for it
- Bandwidth test: rise time, overshoot and magnitude response from a fast edge

### Logic analyser

Models with an LA port, PLA2216 probe.

- D0..D15, hardware trigger on the digital lines, also in roll and scan mode. Thresholds are set as under stock, but on the unit tested so far neither system changes the level the lines switch at

### Generator

S models.

- Sine, square, ramp, pulse, noise, DC, arbitrary wave (also captured from a channel), AM, FM, PM

### Remote control

| Interface | Port | Purpose |
|---|---|---|
| SCPI over TCP | 5555 | [SCPI manual](https://github.com/yottabytesolutions/oos/blob/main/docs/scpi.md): DHO800/DHO900 subset and OOS extensions; RAW reads retained native CH memory, and references retain their own native timing |
| VXI-11 | 111, 9009 | on the scope, where port 111 is free: `TCPIP::<ip>::INSTR` |
| HiSLIP | 4880 | on a computer: the payload is the same SCPI interpreter; `TCPIP::<ip>::hislip0::INSTR` |
| Web remote | 8080 | live screen, keys, knobs, touch in the browser; saves screenshots and CSV to your computer |
| LXI identification | 8080 | `/lxi/identification` |
| LAN announcement | 5353 | mDNS of the SCPI socket, VXI-11 and HiSLIP; the HTTP record's path is `/lxi/identification` |
| MCP | 8080 | `POST /mcp`: tools `scpi` and `screenshot` for AI assistants |
| SSH | 22 | root shell with the scope's own password or a trusted key, off by default ([docs/network.md](https://github.com/yottabytesolutions/oos/blob/main/docs/network.md) 7) |
| mDNS, DNS-SD | 5353 (UDP) | the scope by name, `http://dho924s-xxxxxx.local:8080`, and its services for browsing ([docs/network.md](https://github.com/yottabytesolutions/oos/blob/main/docs/network.md)) |

None of them slows the scope down: they are read without waiting, and the live picture of the web remote is made on
the slow cores with spare time only (on a DHO924S the screen stays at 30 frames a second with two web viewers). All of
them run over Ethernet and over WLAN with a USB stick in the front port (RTL8188EU; open, WPA and WPA2 personal
networks, see [docs/wlan.md](https://github.com/yottabytesolutions/oos/blob/main/docs/wlan.md)).

### System

- Own cold boot of all hardware: the stock system never has to run first
- Self-calibration into own files (`/oos/cal`); factory files are never changed
- System > Overclock: the sample clock in steps from 1.25 to 2 GHz, experimental, from the next power-on
- Saves setups, CSV, binary waveforms and PNG, also to a USB stick; System info page with model, temperatures and limits
- System > Date and time: the clock from an NTP server (off by default), time zones with daylight saving time, or set
  by hand; once the clock is set, file names carry the local date and time
- System > Network > 10 Mbit retry: an Ethernet link that comes up at 10 Mbit/s gets one more autonegotiation
- On-screen keyboard for names, formulas and WLAN passwords, with arrow keys to correct inside a long entry
- The start-up screen shows the version and the build
- Signed update packages with trial start and automatic rollback

## Status

| Area | State |
|---|---|
| Boot, cold boot, acquisition, trigger, roll and scan, touch, front panel, generator (sine, built-in Bode), self-calibration | run on a DHO924S |
| Signed updates from the web page and the shell, kexec trial start, trial and confirmation, install over ADB | run on a DHO924S |
| An update with another device tree, tried by kexec first (kexec-tools, from 0.1.7) | run on a DHO924S: 0.1.6 to 0.1.7 to 0.1.8 through the web page, with cold starts |
| System > Graphics = GPU, JPEG live picture from the video encoder | run on a DHO924S, on the panel and with a 1920x1080 monitor in Mirror, Waveform and Layout |
| Frequency counter in the FPGA, backlight, AC/DC by a long press | run on a DHO924S (the counter with the built-in generator, 1.5 Hz to 9.9 MHz) |
| Logic analyser with a PLA2216: lines, pulse widths, edge, pulse, pattern, duration and protocol trigger, UART, SPI and I2C on the lines, roll, scan, whole deep records, CSV | run on a DHO924S, three lines (D0 to D2) driven; thresholds have no effect here, under stock neither |
| Generator modulation (AM, FM, PM), deep memory, limits, web, MCP, VXI-11 | run on a DHO924S |
| HDMI output | run on a DHO924S |
| Automatic rollback of a bad update | run on a DHO924S on 2026-11-24 after a failed bridge trial with the release candidate |
| USB install without opening the scope | run on a DHO924S with a modified stock system: dry run, first install, run again |
| Generator noise, system info, bandwidth test | tests on the host only |
| Bode with a free-running sweep | tests on the host only (synthetic sweeps); the scan path for sweeps above about 0.9 s is not yet tried on a unit |
| Bode with a generator on USB (USBTMC), UNI-T UTG900 preset | tests on the host only (a simulated USBTMC device); no generator on USB tried yet |
| USB storage stick, 10:1 probe | run on a DHO924S |
| WLAN | driver, firmware and scan run on a DHO924S with an RTL8188EU stick; joining a network tested on simulated radios only |
| Overclocking (System > Overclock) | run on a DHO924S: 1.5 GHz with more noise, 2 GHz with wrong samples; the default stays 1.25 GHz |
| Spectrum, eye and jitter, results and trend panes, measurement labels, Auto | run on a DHO924S with the generator and an RF source |
| Protocol decoders | tested with simulated signals; only some with real signals |
| DHO804 | runs OOS, by user report over several releases ([docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md#21-hardware-variants) 2.1); the rows above were run on the DHO924S |
| All other models | never run; DHO814, DHO824 and DHO914S have the same hardware as a model that has, and count as verified ([docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md#21-hardware-variants) 2.1) |
| Hardware averaging, 800 ps/div | not implemented |

The technical documents in [docs/](https://github.com/yottabytesolutions/oos/blob/main/docs/README.md) say for each topic whether it is verified on the unit.

Known limits:

- Generator DC level: the output's middle jumps by about 85 mV where the generator switches its range at 2 Vpp.
  Whether the stock system shows the same is not known yet.
- High resolution only gains where the record has more samples than the screen has points.
- Logic analyser: D3 to D15 were not driven with a signal yet.
- WLAN: joining a network has not been tried on the unit.
