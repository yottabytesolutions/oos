# Performance and reliability

This fork puts reliability, engineering and performance ahead of adding features. The work covers update recovery,
checks for changes to the acquisition and drawing paths, and how much captured data reaches the screen.

## Measurement conditions

The pinned-trigger rerun compared upstream OOS 0.1.15 and this fork's 0.2.1 on a DHO924S. Both builds used
High Speed (HS) card mode. The table shows the median of three SCPI runs per build, using shown waveforms/s
from the public state counters.

The first published table did not pin the trigger. Its 1-channel, 200 ns/div row was wrong. The rerun puts the
two builds at effectively the same rate in that view.

## Pictures, waveforms and response

| View | Shown waveforms/s, upstream 0.1.15 | Shown waveforms/s, this fork 0.2.1 |
|---|---:|---:|
| 1 ch, 5 ns/div | 6,372 | 14,788 |
| 1 ch, 50 ns/div | 5,518 | 10,569 |
| 1 ch, 200 ns/div | 22,632 | 22,781 |
| 1 ch, 1 µs/div | 5,405 | 8,685 |
| 1 ch, 10 µs/div | 612 | 1,577 |
| 1 ch, 100 µs/div | 333 | 448 |
| 1 ch, 1 ms/div | 41 | 58 |
| 2 ch, 1 µs/div | 4,952 | 7,078 |
| 4 ch, 5 ns/div | 1,419 | 5,113 |
| 4 ch, 200 ns/div | 1,249 | 2,769 |
| 4 ch, 1 µs/div | 3,862 | 6,326 |
| 4 ch, 10 µs/div | 362 | 868 |
| 4 ch, 1 µs/div, peak | 499 | 4,773 |
| 4 ch, 1 µs/div, high resolution | 499 | 2,065 |
| 4 ch, 1 µs/div, average | 488 | 4,027 |
| 4 ch, 1 µs/div, zoom | 411 | 1,125 |
| 4 ch, 1 µs/div, measurements | 3,483 | 6,344 |

Upstream reported about 30 pictures/s in most views, 10.4 in peak and high resolution, 10.2 in average and 8.6
in zoom. This fork reported 32.5 pictures/s in every view.

The host round trip for `*OPC?` was 26 to 30 ms on upstream and 62 to 84 ms in its slow views. This fork took
1.3 to 1.8 ms. The acquisition counters can count different events between builds, so the table uses the shown
waveform rate.

## Update and storage evidence

- The updater verifies signed packages. The hardware watchdog stays armed through the kexec handoff. In
  handoff testing on the unit on 2026-11-15, the watchdog had a 21.5 s hardware period, reset the unit and
  the previous build returned. The staged handoff for the release candidate moved 44.6 MB after staging the
  image on the card. The whole-package handoff carries about 89 MB for this image.
  [Update design and watchdog measurements](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md#purgatory-and-the-watchdog-budget).
- On 2026-11-24 a bridge update from upstream 0.1.15 entered trial on the test unit. Repeatedly stopping
  the UI forced a failed trial. The updater reported `oos-ui crashed 3 times`, restored the previous image
  automatically, and the next start ran upstream 0.1.15. The settings and calibration copy were unchanged.
  [Trial and rollback design](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md#trial-start-by-kexec).
- In three power pulls during an update on the same unit, the system came back and the data area remained intact.
  The bridge installs that reached confirmation took 99 to 116 s from upload, including upstream's handoff.
- Startup checks the `/oos` filesystem on a scratch copy and verifies repairs before writing them back. The area
  keeps an ext3-compatible metadata journal, and the UI can show a repair notice. The journaled area mounted on
  both builds during the switch. Repair, conversion and replay were tested on synthetic host images only.
  [Data area checks and journal](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md#data-area-after-an-unclean-shutdown).

## Engineering checks

The full `cargo xtask check` runs the toolchain check, formatting, source and architecture rules, link and text
checks, the protected-storage check, host and ARM Clippy, the acquisition plan matrix, decoder builds, debug and
release Rust tests, performance guards, coverage checks, script syntax and Python tests. The workspace evidence
from 2026-11-18 records 2,738 passing debug tests and 2,737 passing release tests.
[Check inventory](https://github.com/yottabytesolutions/oos/blob/main/docs/development.md#executable-inventory-and-completion-evidence),
[recorded counts](https://github.com/yottabytesolutions/oos/blob/main/docs/tasks/0095-faster-pipeline.md#status).

## Outside this comparison

The bench run did not compare startup time, CPU load, memory use or measurement accuracy. Update testing did not
measure how long upstream's own packages take to install or put those updates through forced trial failures or
power loss.
