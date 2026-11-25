# UI system.conf keys

This inventory covers [SysConf](../ui/src/sysconf.rs), its [startup switches](../ui/src/sysconf/switches.rs)
and the parsers they call. Run `python3 docs/check-system-conf-keys.py` from the repository root to check
that those parsers' keys have table rows.

Defaults are built-in values before the image, user settings or environment overrides. The user file takes
precedence over the [shared image defaults](../rootfs/initramfs/etc/oos/system.conf). The startup switches
take the first matching line in that order, even if its value is empty. `acq.frame0`, `trig.serial_hw`,
`zone.batch` and `mask.map.*` use only the user file. Ordinary settings use the last valid assignment and
can also be changed through the UI where supported. See [network settings](network.md) for keys read by
image scripts and [ADR 0012](adr/0012-hardware-verification.md) for experimental switch removal.

Startup skips the retired `acq.play` and `acq.play.inferred` keys with one notice for the pair.
It also skips `acq.pipeline` and `acq.stop_slot` with one notice each. Batch starts every capture at ring
slot zero, so the programmed range is `[0, count)`. Unrecognized `key = value` lines produce no UI
configuration error and are preserved when the UI saves the file.

The updater reads `sd.mode = hs|sdr104` from the device-local file. It defaults to `hs` and selects the device tree
for the next image. After a persistently slow start, choose `hs` and follow the restaging procedure in
[Device boot card mode](boot.md#device-boot-card-mode). The startup warning uses the running device tree and the card's recorded history.

## XDMA startup

`xdma.params` accepts space-separated driver arguments. Use each key at most once. An invalid line
falls back to polling with a C2H timeout of 1 second. The image requests `poll_mode=0` on normal
boots. Kexec forces `poll_mode=1` and keeps valid timeout overrides.

- `poll_mode` accepts `0` for interrupt completion or `1` for polling. If neither completion
  argument is present, OOS uses `1`.
- `interrupt_mode` accepts `0` for auto, `1` for MSI, `2` for legacy INTx or `3` for MSI-X. If
  omitted, the driver uses its own default.
- `c2h_timeout` and `h2c_timeout` accept decimal integers `1` through `10` seconds. OOS defaults
  C2H to `1` and leaves H2C at the driver default of `10` unless explicitly overridden.

The timeouts apply to driver transfer waits. Acquisition can wait longer for a trigger before
starting DMA. C2H play reads use requests of at most 4 MiB, or about 13.51 ms each at the measured
3.22 ms/MB. On C2H timeout the driver returns an error, including after a partially completed request.
The acquisition worker discards a failed live batch. System status reports `acq_dma_timeouts` for
all acquisition reads that time out and `acq_batch_dma_failures` for failed live batch DMA reads.
Each field appears after its first event.

The acquisition log contains the playback writes, bytes requested and bytes returned by successful
read calls, and the FPGA state before reset (0x4034, DONE at 0x4080, and slot bounds at 0x4010 and
0x4014). The same diagnostic goes to ramoops pmsg, which survives warm reboot. Every timeout counts.
Detailed logs are limited to one per five seconds. Bytes transferred inside the failing syscall are
inaccessible to userspace. The XDMA timeout message reports any bytes from completed descriptor
chains within that call.

After a timeout the worker disarms, services requests between 20 ms polls and retries disarm every
200 ms. It reconfigures from the current request after STOPPED, a link error, or a one-second deadline.
The state report and log distinguish an unreadable FPGA status from a deadline without STOPPED.

On a unit running the observer build (`private-picture-counts`), run these commands in the unit shell
to request a stall on the next supervised UI start.

```sh
touch /tmp/oos-observe-dma-stall-once && killall -TERM oos-ui
```

The `rootfs/initramfs/bin/oos-ui-loop` supervisor removes the RAM marker and passes
`OOS_OBSERVE_DMA_STALL_ONCE=1` for the next start. The first ordinary batch DMA asks for eight extra
bytes. Inspect the result with `grep -F 'c2h DMA timeout' /tmp/oos-ui.log`. A release-configuration build
ignores the variable. The one-second configuration, injection and restart still need verification on hardware.

## Acquisition and rendering switches

| Key | Values | Default | Effect | Unit evidence |
|---|---|---|---|---|
| `acq.prearm` | `on`, `off` | `off` (image `on`) | Allow eligible batches to arm ahead. Presence of `OOS_PREARM` enables it. | Prior DHO804 checks in [transfer efficiency](fork/unit-dho804.md#transfer-efficiency-spec-10-verification-on-hardware). |
| `acq.compress` | `on`, `off` | `off` (image `on`) | Allow live play-side min/max compression. `OOS_COMPRESS` overrides the file. | Pair layout and representative reads were checked on the unit. See [compression evidence](fork/compress.md#evidence). |
| `acq.interp` | `exact`, `fast` | `exact` | Select sparse interpolation. `OOS_INTERP` overrides the file. | Fast-mode acceptance on the unit is unconfirmed here. |
| `acq.frame0` | `keep`, `drop` or the default rule | Steady batches check frame zero after a complete read with the same settings. | `drop` restores the former exclusion. `keep` bypasses the first-frame exclusion and the early trigger-window checks. | See [first-record selection](backlog/first-record.md). The steady policy awaits unit verification. |
| `acq.capture` | `full`, `decimate:n`, `peak:n` with n = 2, 4, 8, 16, 32 or 64 | `full` | Request reduced storage for eligible full-rate Normal captures. Unsupported plans fall back to full. | Reduced capture is unverified on the unit. See [capture reduction](fork/capture-reduction.md). |
| `acq.capture.inferred` | `on`, `off` | `off` | Permit inferred reduction profiles, including every Peak request. | Unverified on the unit. |
| `acq.play_window` | `on`, `off` | `on` | Play the picture window of eligible live-compressed records up to 1 M points. `off` plays whole records. | DHO924S, 2026-11-24: register ladder word-exact, functional round and 31-view matrix with zero short and zero rejected windows. See [played windows](fork/played-window.md). |
| `acq.dump` | `on`, `off` | `off` | Enable the bounded acquisition fixture dump. `OOS_BATCH_DUMP` overrides it (only `1` and `on` enable it). | No dated unit evidence located for this switch. |
| `acq.dump.count` | Nonnegative `usize` | `4` | Set the count per capture or view change, clamped to 1 through 32. `OOS_BATCH_DUMP_COUNT` overrides it. The default destination is `/tmp/oos-batches`, overridden by `OOS_BATCH_DUMP_DIR`. | No dated unit evidence located. See the [fixture reference](architecture/fixture-benchmark.md). |
| `acq.frame_header` | `on`, `off` | `off` | Request and log frame headers. `OOS_FRAME_HEADER=off` requests headers with selective logging. Any other set value requests all-frame logging. | Header observations were verified on DHO924S on 2026-11-16. See [frames 0 and 1](protocols/acquisition-start.md#frames-0-and-1-of-a-batch-issue-5). |
| `acq.write_gap` | `0`, `10` (µs) | `0` | Group ordinary batch writes with one 10 µs settle per sequence, or delay 10 µs after each write. `OOS_ACQ_WRITE_GAP` overrides it. | No dated unit comparison located for the two policies. |
| `acq.sched` | `fifo`, `other` | `fifo` | Request acquisition on the second A72 with Linux `SCHED_FIFO` priority 1. Use normal scheduling if the request fails or `other` is selected. `OOS_ACQ_SCHED` overrides it. | No dated unit comparison located for the two policies. |
| `acq.work_budget_ms` | Integer 4 through 20 (ms) | 36% of the picture period | Set the source's measured-service work grant. `OOS_ACQ_WORK_BUDGET_MS` overrides it. | No dated unit limit sweep located. |
| `gpu.cpu` | `ui`, `hw` | `ui` | Inherit UI affinity. With `hw`, request the distinct acquisition core when available (the second A72 on RK3399). `OOS_GPU_CPU` overrides it. | No dated unit comparison located. |
| `pacing.clock` | `flip`, `timer` | `flip` | Use vblank-paced picture steps with timer fallback, or the 30 Hz timer. `OOS_PACING_CLOCK` overrides it. | No dated panel verification located for the current helper-deferred pacing. |
| `pacing.flip_divisor` | `2`, `3`, `4` | `3` | Set the number of refresh intervals per flip-paced picture. `OOS_PACING_FLIP_DIVISOR` overrides it. | No dated unit comparison located for all three divisors. |
| `perf.place` | `on`, `off` | `off` (image `on`) | Enable fixed thread and DMA interrupt placement. `OOS_PLACE` overrides it. | No dated unit acceptance located for the current policy. See [placement](architecture/system.md). |
| `perf.cpu_floor` | `on`, `off` | `on` | Request a reversible 1,416,000 kHz policy4 minimum during hardware RUN. | See the [RUN clock-floor evidence](architecture/system.md#run-clock-floors). |
| `perf.gpu_floor` | `on`, `off` | `on` | Request a reversible 600,000,000 Hz GPU minimum during hardware RUN. | See the [RUN clock-floor evidence](architecture/system.md#run-clock-floors). |
| `cal.core_offset` | `tight` enables it. Other values disable it. | Disabled | Match ADC core offsets on first configuration of each ADC mode. | See the dated [DHO804 core-offset rounds](fork/core-offset.md). |
| `trig.serial_hw` | `on`, `off` | `off` | Use the FPGA UART receiver for serial triggering. | See [UART evidence](fork/hwuart.md). Unit acceptance of the switch remains open. |
| `dvm.hw` | `on`, `off` | `off` | Use the FPGA DVM block. | See the [DHO804 comparison](fork/unit-dho804.md#round-5-on-dho804-da8e2a0-build-20261113-193501) on 2026-11-13. |
| `mask.hw` | `on`, `off` | `off` | Use the FPGA comparator for mask testing. | Comparator mapping was explored on the unit. See [mask evidence](fork/hwmask.md). |
| `zone.hw` | `on`, `off` | `off` | Use the FPGA comparator for zone qualification. | See [zone evidence](fork/hwzone.md). |
| `zone.batch` | Unsigned 32-bit integer clamped to 1 through 1000 | `1` | Limit frames per batch while the hardware zone trigger runs. | See [zone rate observations](fork/hwzone.md#rate). |
| `mask.map.domain` | `readout`, `pregain` | `readout` | Select read-out or pre-gain comparator codes. | [Mapping sweep](fork/hwmask.md#sweep-of-the-mapping-unit-round-2). |
| `mask.map.cpd` | Finite number, minimum 1 | `7500` | Set comparator codes per division. | Mapping sweep. |
| `mask.map.mid` | Finite number | `32768` | Set the comparator code at screen centre. | Mapping sweep. |
| `mask.map.shift` | Finite number clamped to 0 through 16 and truncated to an integer | `8` | Shift comparator codes into eight bits. | Mapping sweep. |
| `mask.map.offs` | Finite number | `0` | Add an offset to each eight-bit comparator code. | Mapping sweep. |
| `mask.map.invert` | `1`, `on`, `true` enable it. Other values disable it. | `0` | Reverse the comparator code axis. | Mapping sweep. |
| `mask.map.cols` | `record`, `clocks`, `stock`, count, or `raw:count:samples_per_strobe` | `record` | Choose column strobe layout. An explicit raw count takes a positive sample interval. | [Strobe sweep](fork/hwmask.md#strobe-sweep-round-8). |
| `mask.map.range` | Two finite column indices as `a-b`, converted to unsigned 16-bit integers and sorted | `0-999` | Restrict comparator testing to selected columns. | Mapping sweep. |

## Configured tap funding

These are host-checked declarations for [F2 evaluation](architecture/frames.md#tap-capacity).
Unit memory shares and acceptance remain unverified. Each numeric declaration is required in the profile.
Byte and entry counts must be positive integers representable as `usize`.

| Key | Values | Default | Effect |
|---|---|---|---|
| `f2.stage` | `configured-evaluation` enables the staged profile | Unconfigured | Require tap funding before staged acquisition. |
| `f2.envelope_bytes` | Bytes | Unset | Total declared memory envelope. |
| `f2.tap_bytes` | Bytes | Unset | Tap, retained old plans and control storage partition. |
| `f2.tap_entries` | Entries | Unset | Tap capacity declaration. Fewer than eight cannot fund a profile. |
| `f2.min_cycle_ns` | Positive nanoseconds, at most 1,000,000,000 | Unset | Minimum batch cycle for the evaluation profile. |
| `f2.window_graphics_bytes` | Bytes | Unset | Window and graphics reservation. |
| `f2.dma_bytes` | Bytes | Unset | DMA reservation. |
| `f2.shared_bytes` | Bytes | Unset | Shared newest and STOP ownership reservation. |
| `f2.copies_analysis_bytes` | Bytes | Unset | Decoded copies, analysis and scratch reservation. |
| `f2.runtime_bytes` | Bytes | Unset | Runtime reservation. |

The tap partition and the five owner reservations must fit in the envelope.

## Operating settings

These are software controls. Unit verification is unrecorded in this inventory unless linked below.
The ordinary boolean settings accept `on`, `true`, `1` and `off`, `false`, `0`.

| Key | Values | Default | Effect |
|---|---|---|---|
| `ntp` | Boolean | `off` (image `on`) | Enable time synchronisation. |
| `ntp.server` | Hostname, address, or `dhcp` | `dhcp` | Select the server or take it from a lease. |
| `tz` | POSIX time zone or `dhcp` | `UTC0` (image `dhcp`) | Select local time or take the zone from a lease. |
| `tz.fallback` | POSIX time zone | `UTC0` (image CET/CEST) | Select the zone used when DHCP supplies no usable zone. |
| `eth.retry_10m` | Boolean | `on` | Enable the Ethernet 10 Mbit retry. |
| `remote.web` | Boolean | `on` | Enable the web service. |
| `remote.scpi` | Boolean | `on` | Enable the raw SCPI socket. |
| `remote.hislip` | Boolean | `off` | Enable HiSLIP. |
| `remote.vxi11` | Boolean | `on` | Enable VXI-11. |
| `remote.ssh` | Boolean | `off` | Enable SSH. |
| `ssh.password` | Boolean | `on` | Allow password login when SSH is enabled. |
| `remote.mdns` | Boolean | `on` | Enable multicast DNS service discovery. |
| `net.name` | Valid host label | Empty, device-derived name | Set the DHCP and mDNS name. The parser lowercases the value. |
| `net.allow` | Network allowlist syntax | Image allowlist when the user line is absent | Restrict network access. See the [allowlist reference](network.md). |
| `net.ipv6` | `on`, `off` | `off` (image `on`) | Enable IPv6 services. `OOS_IPV6` overrides the file. |
| `graphics` | `gpu`, `cpu` | `gpu` | Choose GPU or CPU rendering. |
| `backlight` | Integer percentage 10 through 100 | `100` | Set display backlight. |
| `features.off` | Space-separated feature IDs | Empty | Hide selected UI features. |
| `decoders.off` | Space-separated decoder IDs | Empty | Hide selected protocol decoders. |
| `mem.soft_mb` | Integer 1 through 4096 MiB | `768`, capped at hard limit | Set the memory watchdog warning threshold. |
| `mem.hard_mb` | Integer 1 through 4096 MiB | `1024` | Set the memory watchdog hold threshold. |

## Removed temporary paths

The obsolete `acq.pipeline` setting is ignored and logged once at startup. Batch is the sole pipeline.

`meas.deep_raw` no longer selects a path. The UI derives raw deep reads from measurement and view
requirements. `trig.hw_status` and `autoset.levels = unit` were removed on 2026-11-17 (#124).
`acq.wait` and `acq.held` are [state-query](spec/state-query.md) lock timing probes.
