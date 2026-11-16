# OOS system: boot, kernel, crates (upstream description)

Upstream's architecture description, kept for boot, kernel, `hal`, `dsp`, `fpgad` and the data path of one
acquisition. Its `oos-ui` part (threads, sources) describes the code before the overhaul; the target is in
`README.md`.

How the pieces of OOS fit together: from the SD card over the kernel to the Rust crates and the GUI. The register-level
details are in `../protocols/`.

## Boot and system

```
SD card (stock layout, ../boot.md)
 ├─ boot       stock Android kernel; magic "KRNL" -> stock boots, "OOSK" -> the loader falls back to recovery
 ├─ recovery   OOS image (kernel 6.12 + DTB + initramfs), built by boot/build-dev-image.sh
 ├─ backup     copy of the stock boot image (fallback), rollback slot, /oos data partition (../update.md)
 └─ rigol      stock data incl. /rigol/data/cal_*.hex; OOS mounts it read only (oos-mount-rigol)

initramfs /etc/rcS:  oos-oneshot (KRNL back, next power-on = stock) + start count -> oos-board early (FPGA BOOT command) ->
                     network in the background -> SSH (when on: password or key), telnetd -> oos-board drivers (touch edt-ft5x06, beeper pwm-gpio) ->
                     oos-ui (start-up screen) -> in the background oos-mount-rigol (read only) +
                     oos-board bring-up (oos-fpgad cold-boot --execute --power-on) (order and status: ../boot.md)
FPGA:                cold boot by OOS (default); the warm hand-over from the stock system still works
```

Kernel: a 6.12 GPL kernel rebuilt in Docker (`kernel/`; sources and licences in `kernel/README.md`), with the DHO900 DTS (fan hog, pwm-gpio beeper,
AFE/PLL GPIO line names). PCIe host and XDMA are modules, loaded only when the FPGA is up.

### Restart after a panic or a hang

A 10 s panic restart enabled by rcS after confirmed stock selection (`panic=0` before that), the hardware watchdog and the crash archive (`../boot.md`, "Restart after a panic or a hang"). Facts from
the unit (dho804, 2026-11-15, verified on the unit):
- **The watchdog's hardware maximum is 21.5 s.** The driver logs `dw_wdt: No valid TOPs array specified` and uses the
  fixed TOPs: 2^31 cycles of `pclk_wdt`. The kernel pings in between to reach the 30 s that `oos-watchdog` asks for;
  a kexec start has at most 21.5 s minus the time since the last ping until the new kernel registers the driver.
- **`/proc/uptime` is no boot identity.** After a watchdog reset it keeps counting from power-on instead of starting
  at 0. Code that has to tell a new boot from the same one uses `/proc/sys/kernel/random/boot_id`, not uptime.

## Rust crates

```
            ┌─────────────── oos-ui (ui/) ────────────────┐      ┌── oos-fpgad (fpgad/) ──┐
            │ GUI, SCPI :5555, web :8080, control FIFO    │      │ CLI: plan/run/status/  │
            │ sources: Demo | Replay | Live | Hw          │      │ rd/wr/cold-boot/temp/  │
            └──────┬───────────────────────────┬──────────┘      │ beep/guard             │
                   │                           │                 └───────────┬────────────┘
         ┌─────────▼─────────┐       ┌─────────▼──────────────────────────────▼─────────┐
         │ oos-dsp (dsp/)    │       │ oos-hal (hal/)                                   │
         │ pure signal       │       │ FPGA register sequences, AFE/ADC/PLL buses,      │
         │ processing        │       │ cold boot, panel MCU frames, board management    │
         └───────────────────┘       └───────────────────────┬──────────────────────────┘
                                                             │ Bus trait / GPIO chardev / sysfs
                                     /dev/xdma0_bypass (BAR1), /dev/xdma0_c2h_0 (stream), /dev/gpiochipN,
                                     /dev/ttyS0 (panel MCU), /sys/class/thermal, /sys/class/pwm
```

Rule for all crates: no vendor code is in the tree. Only dependency: `libc`.

### oos-hal: hardware layer

Pure functions build lists of `(offset, value)` writes and update software shadows of write-only registers, so the
host tests check them against fixtures from the Python models (`tools/*model.py`) and the device logs. Applying a list
is a separate step through `dho::bus::Bus` (`Bar1` on the unit, `Recorder` in tests).

The crate root holds what does not depend on the board: `model`, `update`, `wlan`, `bootstatus`, `vendorkeys`. Everything
of the DHO800/DHO900 board is in `oos_hal::dho` (`hal/src/dho/`); a port to another board family adds a module next to
it ([porting.md](../porting.md)).

| Module (in `dho` unless noted) | Role |
|---|---|
| `bus`, `gpio` | BAR1 access (mmap of /dev/xdma0_bypass), GPIO character device by DT line name |
| `scu`, `spu`, `hz`, `acq`, `tpu`, `wpu` | FPGA blocks: run/stop/arm/read-out, sample rate, time base, acquisition configuration, trigger unit (12 types), read-path gain |
| `afe`, `adc` | RT1642 front ends (bit-banged SPI, efuse trim, V/div, offset, bandwidth), ADC modes and calibration |
| `seq` | settings -> complete step list (ADC mode, AFE, trigger, acquire, arm, read-out) with a persistent shadow state; execution and channel de-interleaving |
| `boot` | cold boot steps 1-21 (FPGA boot command, PLL, AFE, ADC, SPU/SCU, checks, calibrations) against a `Hw` bundle, plus a unit model for dry runs |
| `panel` | front-panel MCU frames: LEDs, power commands |
| `board` | XADC (FPGA temperature, VCCINT), thermal zones, over-temperature guard, beeper |
| `afg` | built-in generator (`../protocols/afg.md`) |
| `clock` | ADC clock domain (`../models.md`) |
| `model` (root) | model identity, capabilities and limits (`../models.md`) |
| `update` (root) | signed updates with rollback (`../update.md`) |
| `wlan` (root) | control socket of wpa_supplicant, its answers, key derivation (`../wlan.md`) |

### oos-dsp: signal processing

Platform independent, no hardware. Software trigger for all 12 types, 35 measurements, math and FFT, UART/I2C/SPI
decoders, mask test, signal generators for tests and the demo source. Waveforms are `&[f32]` volts with a fixed `dt`.

### oos-ui: the scope screen

Superseded by `README.md` on the `overhaul` branch. This section describes the code before the overhaul and
is kept for the migration.

One process, software renderer on DRM dumb buffers (`drm`, `gfx`, `font`), layout in `ui`, state in `model`.

Protocol decoders that come as modules (`ui/src/protocols`, [protocol-modules.md](../protocol-modules.md)) bring their
menu rows, trigger rules, labels, demo traffic and an optional view of their own through one trait, `Protocol`.

Everything the UI asks of the hardware goes through the trait `board::Board` (`ui/src/board/mod.rs`): what the
acquisition thread is to apply, the generator, the front-end steps, the front panel, the board readings. The UI speaks
in the model's terms and gets frames in volts back (`board::Frame`, `board::to_record`). `board::dho` implements it
for the DHO800/DHO900 and is the only part of the UI that uses `oos_hal::dho`; two `confine` rules
in `xtask/data/architecture.txt` keep it that way. The board is chosen when OOS is built (`board::Active`).

Threads:

| Thread | Job |
|---|---|
| main | input events -> model, acquisition step of the active source, analysis (dsp), render, page flip; reads the web, SCPI and VXI-11 connections without waiting (`net::read_now`): nothing in the loop may block on the network |
| input | front-panel UART (keys, knobs, LED frames) and touch (evdev) |
| hw (`--hw`) | the board's acquisition thread (`Board::spawn`; on the DHO800/DHO900 `board/dho/hw.rs`: `hal::dho::seq` plan/execute in a loop); hands frames over through `board::HwShared` |
| live (`--fpga-live`) | experimental, DHO800/DHO900: repeats the verified read-out (no arming) |
| scpi, http | SCPI server on 5555, web remote on 8080 (polled by the main thread) |
| web-accept, web-png, web-send | the web remote's PNG of the screen (made only of a changed screen, at most 20 a second and half a core) and the large answers; SCHED_IDLE on the slow cores (A53), so the frame loop and the acquisition never wait for them |
| control FIFO | `/tmp/oos-ui.ctl` text commands (`cmd`), used by remote tests |
| ntp | SNTP exchange and clock step when Time from NTP is on (`sntp`, `ui/time.rs`; `../network.md`) |

Sources (`wave::Source`): `Demo` (dsp generators, triggers checked by the software trigger), `Replay` (a saved frame),
`Live` (read-out loop), `Hw` (real acquisitions of the board; on the DHO800/DHO900 the TPU trigger is set to the
selected type, the software trigger finds the point again in the data).

### oos-fpgad: bring-up and service tool

A thin CLI over `hal`: prints any plan without hardware (`plan`, dry runs everywhere), runs it only with `--execute`,
checks the FPGA version before writing, refuses 0x1008 and the excluded FPGA slot. Board commands (`temp`, `beep`,
`guard`). Meant to become the daemon that owns the FPGA once oos-ui and SCPI share one acquisition engine.

On `overhaul` that plan is not followed. While oos-ui runs, its acquisition thread is the only owner of the
acquisition hardware (`acquisition.md`), and SCPI reaches the same engine through oos-ui's remote adapters. `oos-fpgad`
stays the bring-up and service tool: cold boot before oos-ui starts, and service commands while oos-ui is stopped. It
must not touch the acquisition registers while oos-ui runs.

## Data path of one acquisition (`hal::dho::seq`)

```
Settings (model) ── plan ──> steps: ADC mode words (0x1034) -> AFE (GPIO SPI) -> read-path gain (WPU) ->
                             trigger (TPU) -> acquisition configuration (SPU/SCU) -> auto trigger -> arm ->
                             wait 0x4080 bit30 -> read-out (play window, TX size) -> c2h read
c2h bytes ── deinterleave ──> per-channel u16 codes ── volts (scale, offset, cal) ──> f32 -> dsp -> screen
```

## Tests

| Crate | Host tests | What they pin down |
|---|---|---|
| hal | `cargo test` in hal/ | write sequences against fixtures and device logs, cold boot against the unit model, panel CRCs, board logic |
| dsp | `cargo test` in dsp/ | every trigger, measurement, decoder and math function against signals with known answers |
| ui | `cargo test` in ui/ | menus, rendering (screenshots), every trigger type triggers on the demo, storage |
| fpgad | `cargo test` in fpgad/ | option parsing, status interpretation |

Builds for the unit: `--target aarch64-unknown-linux-musl` (static, rust-lld).

## Lessons from the first device tests

- Read-out after an own arm needs the play-side chain in pass-through (`scu::passthrough`), else the live compression
  starves the TX and the c2h read times out (`../protocols/record-play.md`).
- The read-path gain/offset (WPU) must be OOS's own: values left by the stock system shift the codes.
- 0x1210 counts the ADC data clock, which runs only after the ADC sync and eye scan: check it after step 12.
- UI state: settings survive oos-ui restarts within a boot (`/tmp/oos-ui.setup`); the settings of the scope itself
  (NTP, time zone, Ethernet) are in `/oos/system.conf` (`sysconf`, `../network.md`), apart from the setups; the power key
  talks to the panel MCU (`FA 05 02`, dialog, `FA 05 01/03`; `../protocols/board.md` 4).

## Platform tuning

Candidates from the kernel and driver study (2026-11-15, read-only: the fork, the stock DTB, stock dmesg, kallsyms and
rc files), then measured on the unit the same day (unit repo: `docs/logs/oos_tuning_20261115.md` and `_phase1.txt`).
A candidate becomes a decision only with its measurement. The architecture's budgets ([Performance
budgets](README.md#performance-budgets)) are provisional until the open rows are measured.

### Measured on the unit (2026-11-15)

- **XDMA on normal boots runs on MSI already:** IRQ 78 on cpu1, 31 interrupts/s at setup B and 8/s at 4 channels
  10 M; no poll threads. Poll mode runs only in kexec sessions.
- **PCIe:** MPS is already 256, MRRS 512, ASPM off.
- **Thread placement measured before task 0086:** `oos-ui` main and `web-accept` on cpu4, `acq` on cpu5, `deposit0` to `deposit3` and
  `measure` on cpu0 to cpu3.
- **Kernel:** timer slack is the 50 us default on every thread; `PREEMPT_VOLUNTARY`, HZ 250; THP off.
- **Governor A/B:** `performance` on the A72s held 1.8 GHz (baseline mostly 816 MHz), cut the A72 load by about a
  quarter (32 to 24 %) and the `acq` thread from 29 to 19 %, at +2 C. Picture rate, acquired/s, C2H read time and
  latency stayed within the baseline spread. Kept: schedutil; task 0086 adds reversible RUN minima
  ([RUN clock floors](#run-clock-floors)), not yet verified on the unit.
- **`pm_qos_resume_latency_us = 100`:** no measurable effect. Dropped (row 5).
- **Deep 4-channel reads:** at 4 channels, 1 ms/div, 10 M, a C2H read takes about 282 ms per acquisition (3.1
  acquired/s) in every state tested. There the DMA read time, not the CPU, is the limit (row 9).
- **INTx:** failed (row 2).

### Recovery after hardware experiments

After any interrupt-mode, PCIe or FPGA experiment the unit is recovered by a power cycle, not a warm reboot: the
FPGA and the PCIe endpoint keep their state across a warm reboot, so a warm reboot can start the next kernel against a
device in the experiment's state. The INTx revert of 2026-11-15 ended in a halted panic after a warm reboot; that
the FPGA or PCIe state caused it is assumed, not shown, because the power cut cleared the panic text from ramoops.
`panic=10` and the pstore archive ([Kernel panics](README.md#kernel-panics)) keep the next one visible, as long as
the boot reaches OOS's rcS before a power cut.

### Facts that correct earlier assumptions

- **The xdma poll threads are pinned.** `xdma_threads_create()` (`kernel/modules/xdma/xdma/xdma_thread.c` 299) binds
  one `cmpl_status_th` per channel with `kthread_bind` to the first online CPUs: cpu0 and cpu1, both A53s. The poll
  loop yields every 100 polls (`libxdma.c` 1295). In poll mode (kexec sessions) it burns an A53 and competes with the
  deposit workers.
- **Device IRQs.** GICv3 routes a wired SPI to cpu0 by default, so IRQ placement has to be explicit
  (`/proc/irq/<n>/smp_affinity_list` from rcS). Corrected by measurement: the xdma MSI already lands on cpu1.
- **The likely kexec MSI cause.** GIC-500 `GICR_CTLR.EnableLPIs` cannot be cleared once set. Linux reuses the previous
  kernel's LPI tables only when they come through the EFI `LINUX_EFI_MEMRESERVE` table, and OOS boots through U-Boot
  without EFI. So after kexec, MSIs (LPIs) never arrive. Inferred from the GIC and kernel behaviour; the symptom is
  verified (`../update.md`). INTx would avoid the ITS, but it does not work under this kernel (row 2), so kexec keeps
  poll mode.
- **Corrected study assumptions:** the study assumed poll mode on normal boots (it is MSI), MPS 128 (it is 256), and
  all device IRQs on cpu0 (the xdma MSI is on cpu1).

### Candidates, ranked by benefit per risk

| # | Candidate | Where | Measurement | Pass criterion |
|---|---|---|---|---|
| 1 | Timer slack 1 ns: `prctl(PR_SET_TIMERSLACK, 1)` early in `main`, inherited by later threads. Today each 10 us BAR-write gap lasts 60 to 70 us (50 us default slack), a configure of about 230 writes 15 ms instead of 2.3 ms | `oos-ui` (M3c) | configure time and the poll period, before and after | configure <= 4 ms; poll period within 10 % of nominal |
| 2 | XDMA by INTx on normal boots (`poll_mode=0 interrupt_mode=2`, as stock). **Failed 2026-11-15:** the legacy line came up as IRQ 76 on chip `dummy` at about 230,000 interrupts/s on cpu0, 10 C2H time-outs in 2 min, 3.8 fps; after the revert reboot the unit stayed dark with a kernel panic on screen; a power cycle recovered it (verified on the unit), and the cause is not known. The RK3399 PCIe legacy interrupt routing in this kernel gives no usable line; needs a fix first. Normal boots keep MSI | kernel command line | as measured | rejected as configured |
| 3 | XDMA by INTx after kexec instead of poll mode. **Not tested: blocked by the normal-boot failure of row 2**; kexec keeps `poll_mode=1` unless M5c finds a working alternative | `oos-fpga-up`, `oos-fpgad` (M5c) | a kexec start with acquisition for 10 min | no C2H time-out, no busy A53 |
| 4 | Big cluster clock. **Measured:** the `performance` governor lowered A72 load (32 to 24 %) and `acq` (29 to 19 %) at +2 C, but picture rate, acquired/s, read time and latency stayed within the spread; schedutil stays. Candidate: `uclamp_min` 1024 on main and the acquisition thread only (`sched_setattr`, `UCLAMP_TASK=y` is built) | `oos-ui` (P1) | `scaling_cur_freq`, step time p99, wake latency, thermal zone after 30 min | step p99 or wake latency improves; temperature below the 70 C passive trip |
| 5 | Shallow idle on the A72s: `pm_qos_resume_latency_us = 100` on cpu4 and cpu5. **Dropped 2026-11-15:** no measurable effect | none | as measured | dropped |
| 6 | PCIe MPS 256. **Done:** measured already 256 (MRRS 512, ASPM off); nothing to set | none | as measured | done |
| 7 | Console `loglevel=4` (today 7 on a legacy 8250 console written synchronously at 11.5 kB/s) | kernel command line (P1) | stall time of a warning burst | no printk stall in a step |
| 8 | Preemption: build `PREEMPT_DYNAMIC`, A/B `preempt=full`; HZ stays 250; no `isolcpus`, no `nohz_full`; `rcu_nocbs=4-5` optional | kernel config (P1) | wake latency p99 under DMA, DRM and MMC load | p99 improves, throughput within 2 % |
| 9 | Persistent DMA mapping in xdma: one streaming mapping, mmapped, `dma_sync_single_for_cpu` over the bytes received, instead of pinning and mapping 1024 pages per 4 MB read (0.5 to 2 ms per 4 MB, inferred). Measured: at 4 channels, 10 M, the C2H read is about 282 ms per acquisition (3.1 acquired/s); the DMA read time, not the CPU, limits that mode, so this task and the link rate are where it can improve | driver task (after M5c) | `read()` wall time minus bytes / link rate; ftrace on `dma_map_sg_attrs` | per-read overhead below 0.2 ms; acquired/s at 4 x 10 M up |
| 10 | GPU: `glFlush` plus a fence instead of `glFinish`; `glFinish` only on readback; Panfrost stays; GPU floor 400 MHz, 600 MHz optional | helper (M9b) | helper CPU and GPU time per composition; tearing under load | budgets met, no tearing |
| 11 | HDMI scaling on the VOP win0 scaler (`DRM_IOCTL_MODE_SETPLANE`) instead of on the CPU, when the GPU path is off | display (later) | CPU per HDMI frame | about 8 MB less CPU writes per 1080p frame |
| 12 | Allocator: pools on the hot paths (adr/0016 zero allocations); optionally `MADV_HUGEPAGE` on DMA and decode buffers with THP in `madvise` mode | `oos-ui` (data-path steps) | page faults per frame, TLB misses | 0 faults per frame after warm-up |

### No change (verified)

- DMC and DDR devfreq stay disabled, as in stock: DMC switching through ATF can disturb the VOPs.
- ASPM: off (measured); no common state between the root port (L1) and the endpoint (L0s) anyway.
- CMA 16 MB; raise to 64 MB only if the VPU or HDMI run out (`CmaFree`).
- IOMMU strict mode: VPU only, negligible. No debug option with runtime cost.

### Stock for comparison

Stock runs the `interactive` governor with no frequency floor, no pinning, no real-time priorities, xdma without
parameters (it ended on INTx), and thermal trips at 70 C and 85 C (passive), 115 C (critical), 120 C (shutdown). Taken
from stock: the thermal trips as the limit when raising clocks. Stock's INTx mode does not carry over (row 2).

### Unit checks

The read-only checks for each unit round (governor, idle states, `pm_qos`, thermal zones, xdma threads and
parameters, IRQ affinity, PCIe link, printk level, timer slack per thread) are in the study's check list, kept with
unitlab's scripts.

### RUN clock floors

Task 0086 implements plan P5 while keeping schedutil and the GPU governor.
`perf.cpu_floor = on` and `perf.gpu_floor = on` are independent, default-on
startup switches in system.conf. Set either to `off` and restart oos-ui to
disable that device's floor. Only `on` and `off` are accepted; unrecognised
values are logged and treated as `off`. They do not depend on `perf.place`, and saved
measurement setups do not change them.

Accepted hardware RUN requests policy4 `scaling_min_freq = 1416000` (kHz) and
the GPU devfreq `min_freq = 600000000` (Hz). Higher prior minima are preserved.
The system clock adapter discovers the `.gpu` devfreq node, reads each previous
minimum first, and writes only values listed in that node's available frequencies.
Missing nodes, unavailable frequencies and IO failures are reported without
changing the other device's policy. Successful applied values are logged once
per device per process as structured `clock_floor` fields.

Before every floor write, saved minima are atomically renamed into the tmpfs
receipt `/tmp/oos-clock-floors`. STOP, the memory watchdog's hold, worker teardown
and guard drop restore the saved minima, verify read-back, then remove the receipt.
The Linux panic hook tries the controller lock without waiting before invoking the
previous hook; SIGINT/SIGTERM use orderly exit. `oos-ui-loop` restores and logs
the receipt after every exit, including unclean deaths (SIGKILL, SIGSEGV and
allocator abort) and clean exits with pending recovery, even without a restart. Startup independently restores it
before reading new previous minima, including when switches are now off. Failed
or invalid recovery keeps the receipt and disables new floors for that process.
Failed STOP/thermal restores retry at most once per second; teardown still
tries immediately. Only the first restore failure and eventual success are
logged per device. A reboot discards tmpfs and resets the kernel's clock policy.

While RUN requests floors, the adapter samples `cpu-thermal` and `gpu-thermal`
by type name at most once a second. Each drop limit is the lowest passive trip
exported by the running DT through sysfs, minus 5000 millidegrees C. For a 70 C
passive trip this means dropping at 65 C and re-arming below 60 C (5000
millidegrees hysteresis); these margins are task 0086 policy. Each thermal drop
and re-arm logs the zone type and temperature, or the read error, once per
transition. Re-arm reports both zones. Both minima drop if either zone reaches
its limit or its temperature cannot be read. Missing named zones or passive trips
disable floors at construction. Linux still owns cooling, governors and maximum
frequencies. The exact live trips and GPU cooling-map bindings are not yet
verified for this change; task 0086 records the required unit capture.

Host fake-filesystem tests cover recovery, thermal inhibition and lifecycle
contracts. Actual read-backs, web/upload latency and a 10-minute RUN thermal
check remain pending in task 0086. The earlier measured 1.8 GHz governor
experiment is evidence for a candidate, not verification of these lower floors.
