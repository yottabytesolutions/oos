OOS runs on a computer with simulated signals, with its screen in a browser or scripted without a display.

## In a browser

With Rust installed, run this from the checkout on macOS or Linux. On Windows, run it in WSL 2 or a Linux virtual
machine and open the browser on Windows:

```sh
cargo run --release -p oos-ui -- --serve
```

Open the HTTP address it prints. The same line lists the SCPI, HiSLIP and VXI-11 ports for scripts.
The server listens on loopback ports. Stop it with Ctrl+C in the terminal. Add `--offline` to start
with the analog channels off for stored waveforms.

The native Windows window is maintained separately in
[oos-desktop](https://github.com/yottabytesolutions/oos-desktop).

The browser UI accepts file selection and drag-and-drop for CSV, OOS BIN, or WFM files.
Loaded sources appear as **R1–R10** cards.
**SIM** (start menu, Analysis) starts a simulated reference; **Run/Stop**, **Single**, and trigger settings control its acquisitions.
**Live** restores the live session. Measurements follow each captured window and recompute when a stopped
reference is panned. **Measure** opens categorized choices with Source, Settings, and Selected; large readout
lists have page controls.

**SIM > Advanced** adds SSC profile, deviation and modulation frequency, duty-cycle distortion, and
correlated random jitter with its corner frequency. PRBS-13 is available alongside the other patterns.
With SSC enabled, the stored record covers a complete modulation cycle at eight samples per bit.
Long records generate in the background; cancel or return to Live to keep the previous session.
Configurations exceeding the generation budget report an error without replacing the current waveform.
The HSS down-spread profile removes its DC term; centered spread uses plus/minus the configured deviation.
Choose **Auto Eye/Jitter** in the analysis menu/settings to analyze the selected live or reference
source. Automatic setup recovers an NRZ clock/rate, chooses voltage thresholds from the signal and opens the eye
with its TIE trend, histogram, spectrum and bathtub. The source/rate header keeps manual rate and explicit-clock
settings accessible. Flat, insufficient or sparsely sampled data shows its limit; model estimates are labeled.
The Eye analyzes a full stored reference once per changed record or setting, including while the trace is zoomed;
large files show Computing without blocking navigation. Live captures accumulate; stopped panning does not add
observations. Ordinary trace measurements use the selected Screen or Full record gate.

The same advanced settings are available through OOS SCPI. Values use seconds, hertz and fractional
deviation (0.005 means 0.5%). Configure them together, then start generation and poll `:SIM:BUSY?`:

```text
:SIM:PRESET "PCIe 3.0"
:SIM:SSC:MODE CENTER
:SIM:SSC:DEVIATION 0.005
:SIM:SSC:FREQUENCY 33000
:SIM:JITTER:DCD 4e-12
:SIM:JITTER:CORRELATED 2e-12
:SIM:JITTER:CORNER 2e6
:SIM:PATTERN PRBS13
:SIM:PLAY ON
:SIM:BUSY?
:SYSTEM:ERROR?
```

## Scripted without a display

OOS builds on macOS and Linux and can run scripted without hardware, with its simulated demo signals. It writes a
screenshot in PPM format next to the script:

```sh
cat > /tmp/demo.txt <<'EOF'
frames 30
shot demo.ppm
EOF
cd ui && cargo run -- --script /tmp/demo.txt
```

The commands are listed at the top of [ui/src/cmd.rs](https://github.com/yottabytesolutions/oos/blob/main/ui/src/cmd.rs). Add `--model DHO804` to act as another model.
