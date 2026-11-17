OOS runs on a computer with simulated signals: as a desktop window on Windows, or scripted on macOS and Linux.

## On Windows

With Rust installed, build and open the desktop window from PowerShell:

```powershell
.\build-windows.ps1 -Run
```

The app is `ui\target\release\oos-ui.exe`. Mouse clicks and drags act as touch; Space toggles Run/Stop.
Plain mouse wheel pans horizontally, and Ctrl+wheel zooms the time scale around the pointer.
F12 saves a PNG in `Documents\OOS`. Use `--offline` to start
with the analog channels off for stored waveforms. Keyboard controls are listed in [ui/src/desktop.rs](https://github.com/yottabytesolutions/oos/blob/main/ui/src/desktop.rs).

For development, run the checkout and automatically restart after saving source changes:

```powershell
.\run-windows.ps1 -Watch -Offline -WebPort 18080 -ScpiPort 5555 -HislipPort 14880
```

Rust compiles changed code automatically. The window includes front-panel buttons and knob controls.
Use **Reference > Load file** or **Ctrl+O** to open CSV, OOS BIN, or WFM files; the web remote at `http://localhost:18080`
also accepts file selection and drag-and-drop. Loaded sources appear as **R1–R10** cards.
**SIM** (start menu, Analysis) starts a simulated reference; **Run/Stop**, **Single**, and trigger settings control its acquisitions.
**Live** restores the live session. Measurements follow each captured window and recompute when a stopped
reference is panned. **Measure** opens categorized choices with Source, Settings, and Selected; large readout
lists have page controls. The Windows app announces its actual SCPI and HiSLIP listeners over mDNS.

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

## On macOS and Linux

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

To try the web remote with the same demo signals, run this and open `http://localhost:8080/`:

```sh
cd ui && cargo test web_demo -- --ignored --nocapture
```

It runs for 5 minutes.
