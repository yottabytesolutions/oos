# Performance and reliability

This fork puts reliability, engineering and performance ahead of adding features. The work covers update recovery,
checks for changes to the acquisition and drawing paths, and how much captured data reaches the screen.

## Measurement conditions

The comparison used one unit, a DHO924S, on 2026-11-24. Both builds ran on the same
bench within one hour. A 100 kHz, 3.3 V square wave fed CH1 and CH2. CH3 was connected to a USB supply and CH4 to the
rear trigger output.

- [Upstream OOS](https://github.com/dekajoule/oos) ran release 0.1.15, build `20261121-211913`, installed from its
  GEL. The card ran in HS mode.
- This fork ran release candidate `20261124-200342` in release configuration, with the card in SDR104 mode.
- The same script used only SCPI. It read the public `:SYSTem:STATe?` fields that both builds report (`frames`,
  `acquired`, `used`, `dropped`, `step` and `render`). Each view settled for 6 s and was measured over 12 s.
  The table compares one run per build, with no estimate of run-to-run spread.

## Pictures, waveforms and response

| View | Pictures/s upstream | Pictures/s this fork | Shown waveforms/s upstream | Shown waveforms/s this fork | Ratio | Step ms upstream | Step ms this fork | `*OPC?` ms upstream | `*OPC?` ms this fork |
|---|---|---|---|---|---|---|---|---|---|
| 1ch 5ns | 30.0 | 32.5 | 6,015 | 17,769 | 3.0x | 4.1 | 2.6 | 26 | 2 |
| 1ch 50ns | 29.9 | 32.5 | 5,555 | 12,140 | 2.2x | 4.4 | 2.4 | 27 | 2 |
| 1ch 200ns | 29.9 | 32.5 | 5,375 | 6,758 | 1.3x | 3.2 | 2.4 | 27 | 2 |
| 1ch 1us | 29.7 | 32.4 | 5,250 | 7,062 | 1.3x | 3.3 | 1.9 | 28 | 2 |
| 1ch 10us | 29.9 | 32.5 | 408 | 906 | 2.2x | 4.8 | 1.9 | 28 | 2 |
| 1ch 100us | 29.8 | 32.5 | 206 | 362 | 1.8x | 3.8 | 2.0 | 29 | 2 |
| 1ch 1ms | 29.9 | 32.5 | 39 | 55 | 1.4x | 4.8 | 2.0 | 28 | 2 |
| 2ch 1us | 29.8 | 32.4 | 4,813 | 7,068 | 1.5x | 7.5 | 3.8 | 27 | 2 |
| 4ch 5ns | 30.0 | 32.5 | 1,438 | 5,242 | 3.6x | 17.3 | 6.6 | 27 | 2 |
| 4ch 200ns | 29.9 | 32.5 | 1,309 | 2,931 | 2.2x | 16.3 | 10.2 | 27 | 2 |
| 4ch 1us | 29.9 | 32.5 | 3,797 | 6,387 | 1.7x | 10.9 | 7.9 | 27 | 2 |
| 4ch 10us | 29.6 | 32.5 | 357 | 868 | 2.4x | 16.2 | 6.6 | 26 | 2 |
| 4ch 1us peak | 10.9 | 32.4 | 524 | 4,751 | 9.1x | 85.1 | 12.1 | 51 | 6 |
| 4ch 1us hires | 10.8 | 32.5 | 520 | 2,063 | 4.0x | 85.9 | 10.3 | 56 | 2 |
| 4ch 1us average | 10.7 | 32.5 | 513 | 4,023 | 7.8x | 87.0 | 9.5 | 59 | 2 |
| 4ch 1us zoom | 8.5 | 32.4 | 410 | 1,087 | 2.7x | 109.9 | 7.6 | 63 | 2 |
| 4ch 1us measure | 29.6 | 32.4 | 3,517 | 6,373 | 1.8x | 13.0 | 7.6 | 28 | 2 |
| 4ch 1us again | 29.9 | 32.5 | 3,389 | 6,393 | 1.9x | 11.7 | 7.1 | 28 | 2 |

- This fork reported 32.4 to 32.5 pictures/s in every view. Upstream reported 29.6 to 30.0 in normal acquisition
  and 8.5 to 10.9 in the four-channel peak, high-resolution, average and zoom views. Upstream's step took 85 to
  110 ms in those four views.
- The shown waveform rate for this fork was 1.3 to 9.1 times upstream's rate. At one channel and 5 ns per
  division the ratio was 3.0.
- Where upstream reads higher is its `acquired` counter. At one channel 5 ns it reports 36,907 acquired per second
  against our 17,755. It then drops 30,893 per second and shows 6,015. Ours shows what it acquires (0 dropped).
  The counters may not count the same event in both builds. The shown rate is the comparable figure.
- Median `*OPC?` response on upstream was 26 to 29 ms in normal acquisition and 51 to 63 ms in the four views
  above. This fork reported 2 ms except for 6 ms in four-channel peak. For `:SINGLE` followed by `*OPC?`, the
  readings were 63 ms on upstream, 7 ms on this fork in SDR104 and 16 ms on this fork in HS.

The last digit should be read as display precision for this run. Repeating the run with this fork in HS mode
changed the shown waveform rate by at most 2.0%. This comparison applies to the upstream release of 2026-11-21.

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
