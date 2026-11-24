# Known issues

Observed on a DHO924S on 2026-11-23 and 2026-11-24.

| Condition | What you see | Workaround | Status |
|---|---|---|---|
| Trigger refinement on decimated whole records when the smoothed signal stays inside the trigger hysteresis band | With a bursty signal in a one-channel view at 625 MSa/s, about 9% of records appear as stray traces tens of microseconds from the hardware trigger point. A clean 100 kHz square wave produced zero wide searches in ten views. | Keep the played window enabled (the default). Setting `acq.play_window = off` brings the issue back. | The played-window path is unaffected for records up to 1 M points. Whole-record refinement remains affected. |
| Experimental capture reduction with `acq.capture = auto` | At 10 µs/div, the FPGA read falls from 17.4 ms to about 3 ms, but the picture rate falls to about 25 per second. The full-rate reference record takes a picture slot five times per second. | Leave `acq.capture = full` (the default). | Experimental. |
| Explicit memory depth of 1 M points or more | 27–28 pictures per second, compared with 32.5 in other views. | Use a shallower depth when the measurement allows it. | Measured throughput limit. |
| Four channels active or measurements enabled | To keep an even cadence, pictures can appear one refresh later, 30.8 ms after the acquisition step instead of 20.5 ms. In the measurement view, 2.2% of pictures still missed their target. | Reduce active channels or disable measurements when possible. | Measured scheduling behavior. |
| A DMA read stalls | Acquisition pauses for 10 s, then recovers. | Wait for acquisition to recover. | Observed only with an invalid playback plan. |
| A kexec update payload exceeds 90 MiB | The updater refuses the kexec payload. See the [measured margin at 89.1 MB](update.md#purgatory-and-the-watchdog-budget) for the test conditions. | Use a signed package whose staged payload fits the limit. | Size limit in effect. |
| A rolled-back or foreign image is in `recovery` | Start-up marks the update state untracked when no trial is open, so the next signed install is not refused as older. A build this fork tracked itself keeps the downgrade guard. | Run `oos-update status` to see the reason. Before leaving for another system, run `oos-update leave`. | By design. |
| Power is lost while an update starts its new image | The update is not installed and the running image is kept. The updater cannot tell a power cut from a card that failed at SDR104, so it records a failed SDR104 trial and the next install selects HS. | From a shell: set `sd.mode = hs` in `/oos/system.conf` and run `oos-update sd-mode --execute`, then set `sd.mode = sdr104` and run it again. The second change takes effect at the next start, on trial. | Conservative by design. A menu setting is planned. |
| An ordinary restart or power-off | The data area is not unmounted first. The next start replays its journal. | None needed. | A clean unmount is in work. |
| Logic analyser acquisition | The logic analyser reads data for one record per batch. | None known. | Current acquisition limit. |
| The offline filesystem check cannot recover the data area | The data area stays unmounted. | Follow the Data area notice and retain the startup log. | Startup writes repairs only after a clean read-only check. |
| A cut during first format on an older build leaves nonzero data without any ext superblock or OOS first-format intent | The data area stays unmounted. | Follow the Data area notice and retain the startup log. | An unmarked partial old format cannot be distinguished safely from foreign data. First-format intent recovery has been exercised on synthetic host images. |
| Power loss during a kexec restart-marker or diagnostic write, or during a wpa_supplicant configuration save | The restart marker, diagnostics or Wi-Fi settings may be incomplete after boot. | Review the update status or re-enter Wi-Fi settings as needed. | The full durable replacement sequence remains open on these paths. |

Trigger refinement was checked with the probe compensation output, a bursty logic signal and a 100 kHz,
3.3 V square wave. Sine and slow-edge signals remain untested.
