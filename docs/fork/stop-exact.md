# STOP and the retained record

Register behavior behind alternate-slot preservation and replay is **assumed** on hardware. A
unit report from 2026-11-13 covers the [10 M point stopped-window
test](../protocols/acquisition-start.md#reading-a-stored-frame-again-stop).

Normal hardware batch acquisition holds the source on STOP. The final read can provide a raw frame for
live-compression pairs or a raw window for deep pairs when deep measurements need samples. See
[STOP and RUN](../architecture/frames.md#stop-and-run).

## Before

STOP never reached the acquisition thread. The UI stopped taking frames, while the thread kept arming and reading
with the last settings, so the FPGA memory behind the frozen record was overwritten at once. With raw deep
measurements the values after STOP belonged to the last raw record (up to 1 s older), not to the pair record on
screen. Upstream behaves the same: the hardware thread has no STOP state outside roll and scan (`Shared::halted`,
Single scan only).

## Now

- The UI requests `Shared::hold` in STOP once a record is displayed or the screen has been cleared.
  A source started in STOP can still show its first record.
- The normal batch path disarms any tracked successor on entering hold. On a pass without a stopped view,
  acquisition can reach configuration before its batch hold check. Calibration, generator updates, the hardware
  counter, XADC and LA probe checks continue in STOP. RUN releases the hold before publishing the new epoch and
  preserves the picture across the transition.
- A serial capture already armed at STOP can finish. Its first-record wait is capped at two seconds from the first
  poll that sees STOP, within the UI's three-second wait. A trigger timeout disarms via the acquisition counter,
  and the worker answers without the 200 ms retry delay. In overlapping Normal-trigger acquisition, a first-record
  wait with no signal ends at the next control poll (up to 10 ms), followed by the disarm poll (up to 10 ms).
  A completed overlapping record-queue batch follows normal publication and retains all its eligible records.
- `acq.stop_slot = zero` is the default in `system.conf`. Each live batch arms at frame zero. After an
  untriggered serial STOP, the host copy of the last record stays on screen. The arm can overwrite the FPGA
  copy before a trigger, so firmware ring reread eligibility is cleared.
- `acq.stop_slot = alternate` is an experiment that needs an operator at the unit. An eligible serial capture
  starts at a second frame index when the previous record is at zero, so it can preserve the previous slot for
  raw replay on timeout. The HAL bounds the index by the stock segmented-ring capacity. The
  [two-slot sequence](../protocols/acquisition-start.md#next-frame) still needs live hardware verification,
  including slot selection with LA enabled.
  `:SYSTem:STATe?` reports the selected startup value.
- An eligible STOP raises `final_req` when a serial capture can still arrive, the displayed record needs a raw
  reread, or Auto trigger is showing LA or a deep record. An overlapping Normal-trigger STOP on an already-raw
  deep record or LA view can settle in the first step. With a final request pending, the UI keeps taking data
  until `final_ack` reaches the request and the decoded-record queue is empty, or until `STOP_FINAL_WAIT` (3 s).
  An eligible completed serial capture or final raw reread can enter that queue and supply display,
  measurements and saved data.
- SCPI `*OPC?` tracks armed SINGLE through its completion receipt. STOP, RUN and the hold after SINGLE have no
  acquisition fence. The three-second final-record timeout ends the UI wait.
- Arming ahead (`acq.prearm = on`, the unit's set-up) is skipped for batches that may be replayed
  (`prearm_keeps_replay`: a live-compression batch, or a compressed deep record with deep measurements on), so the ring
  of the last batch stays for the replay. Prearm's gain is at short time bases; at deep records the read takes most of
  the batch anyway.
- No replay when that newest frame was already read raw (rate-limited deep read, periodic raw read of the live
  compression), when the next batch was armed ahead anyway, or when anything else ran on the FPGA since the read (the
  read is only remembered for one pass). Then the record shown is the newest one already queued, measured as it is.
- An eligible displayed raw or LA record requests a stopped view, even when the raw record is complete. The thread
  reads the stored record for the requested view (`stop_plan`) while the replay slot is valid. Zoomed out on a record
  above `LIVE_READ` (625 k points, for example 10 M at 1 ms/div), that read is the whole record as min/max pairs.
  Those pairs are drawn (`stop_pic`), while the raw record of the STOP (`stop_raw`, the final frame or a raw record
  already shown) stays `last[0]`, the record measured and read out (`:WAV:MODE RAW`). Far zoomed in, the thread reads
  every sample of the view. That window becomes the record shown and measured, and the raw record of the STOP comes
  back with the next view read as pairs. A view adds nothing to the statistics (`stop_reads_the_record_again_without_counting_it`); a
  time base or delay change clears them, as every setting does. RUN, Clear and the history let the raw record go.
  In a logic-analyzer view the reread carries the digital samples saved with the same capture, aligned to its trigger.
  The pair picture is drawn by the legacy stopped redraw; moving it to an accumulator input is left to M2.3e.

## Host tests

- `ui/src/ui.rs` `stop_shows_the_final_raw_frame_of_a_deep_record`: STOP on a pair record holds the thread and asks
  for the final frame; the batch's last pair frame and the raw final frame are taken, the raw one is shown and
  measured (Mean of its samples), status STOP; later frames stay unseen; RUN releases the hold; without an answer
  the STOP ends after 3 s. The shallow record in this test settles without a final request. Without taking records
  while waiting, the test fails.
- `ui/src/ui.rs` `stop_measures_the_final_raw_frame_while_the_views_are_drawn_from_pairs`: 10 M at 1 ms/div, the
  final raw frame, then the view read as pairs: the pairs are drawn, Mean, RMS, Std dev, AC RMS and Max stay those of
  the raw frame, the count stays; zoomed in and out (pairs): same values, statistics stay cleared; far zoomed in (raw
  window): that window is measured; out again: the raw frame of the STOP again; RUN lets it go. Fails without the
  change: "the view read as pairs: Vavg NaN instead of 0.5".
- `ui/src/ui.rs` `clear_in_stop_keeps_the_hardware_held`: started in STOP, not held until the first record; held
  after a Clear in STOP; RUN releases it.
- `ui/src/ui.rs` `run_after_stop_takes_no_frame_from_before`: a frame queued in STOP (the batch in progress) is not
  the first record of the next RUN; the first frame after the RUN is.
- `ui/src/board/dho/hw/tests/replay.rs` `final_replay_at_stop`: replay requires a compressed deep batch whose newest
  frame holds pairs, with deep measurements enabled and no successor armed. `prearm_keeps_replay` skips prearm
  for this batch. Prearm stays available for shallow records and for captures with deep measurements off.
- `ui/src/board/dho/hw/stop/tests.rs` scripts trigger completion and timeouts with an injected clock. It verifies
  slot preservation, counted disarm, calibration and hardware-control interruption. The UI STOP tests cover the
  immediate settlement of overlapping Normal-trigger STOP and draining a shallow serial capture.

## On-unit verification

1. Baseline any depth: RUN, STOP, RUN, SINGLE, STOP at 1 us/div and 1 ms/div. Picture freezes at STOP, resumes at
   RUN; SINGLE still takes the first trigger after the press. CPU of oos-ui in STOP drops (`top -d 1`): nothing is
   read any more.
2. Settings in STOP (V/div, time base, delay): as before (the record is redrawn, no new acquisition).
3. 10 M at 1 ms/div, probe comp on CH1, Vavg and Vrms on. STOP: the values belong to the record
   shown: `:WAV:MODE RAW;:WAV:DATA?` over the screen, mean computed on the host, equal to Vavg. Repeat 10 times at
   different moments. Check the final-frame answer and the measurement separately. Round 3 (unit-dho804.md): equal at
   2.3 s after STOP (4 of 4), still NaN or an older value at 0.8 s. A sub-second result is an unmet target.
   Then the views (A3): 10 M at 1 ms/div, generator 1 kHz square or sine on CH1, Vavg, Vrms, Std dev and AC RMS on
   the list, Measure statistics on, Screen gate. RUN a few seconds, STOP, note the values and the count. Wait 2 s:
   the values and the count stay (no "----"), the picture stays the whole record. Time base to 500 us and 200 us/div
   and back to 1 ms/div: the values stay valid at every view, the statistics are cleared by the change and the count
   stays 0 while nothing else changes. 10 us/div (a raw window): values of that window; back to 1 ms/div: the values
   of the STOP again. RUN: the values update again.
4. Normal trigger without a signal, STOP. Verify immediate settlement for overlapping raw or LA views and settlement
   within 3 s for serial capture, with the last host record still displayed. With an operator
   present, repeat with `acq.stop_slot = alternate` and check zoom, reread and save of the preceding record.
   Repeat with a trigger arriving during STOP and verify all three use the completed capture.
5. `acq.prearm = on` (the unit's set-up): step 3 again, same result; wfm/s at 1 us/div unchanged against prearm without
   the deep measurements (prearm stays on there); no `disarm` errors.
6. Auto set-up, self-cal, generator changes while stopped: work as before.
