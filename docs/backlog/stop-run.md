# STOP and RUN ordering

A first STOP/RUN attempt, which made STOP hold and RUN start clean in every entry path, was withdrawn. The host evidence below uses the restored batch-only
acquisition loop, the HAL simulator and the real deposit worker. The current hardware behavior of these
paths still needs unit verification.

## Configuration while held (resolved on host)

With an applied configuration, held edits leave it in place. With no view or side work, a held
pass waits 5 ms. The first held pass without an applied configuration, including after calibration
or at source startup, applies the current request once. DVM, counter and levels service require an
applied configuration. RUN or SINGLE applies the latest effective request before arming when it
differs from what was applied. Reverting an edit while held requires no configuration on RUN.

Calibration invalidates the retained ring batch and pending stopped-view replay, including during a
completed SINGLE hold. Each subsequent calibration requires a fresh configuration. Unapplied edits
preserve a valid ring batch, which is reread using its captured settings and identity. Each requested
identity clears the stream buffers once while held. Held roll and scan keep the acknowledgement from
entering STOP. Held segments retain the 5 ms poll.
The remaining view and acknowledgement gaps are listed below.
[Host traces](../../ui/src/board/dho/hw/acq/tests/stopped_pass.rs) ·
[Calibration and held startup traces](../../ui/src/board/dho/hw/acq/tests/calibration_hold.rs) ·
[Configuration and STOP handling](../../ui/src/board/dho/hw/acq/control.rs)

## RUN publication order (resolved on host)

RUN, SINGLE and accepted settings edits stage the pipe view and tap identity, source settings and
identity, segment command, stopped-view clearing and queue filtering under Shared. The source
hold is released and its events are notified after those writes. Equal settings can reuse the
applied setup on a new epoch. DMA admission checks the pair. An old DMA already in progress
may finish, and its records settle as settings loss. The existing handoff locks reject stale
worker taps and deposits. Rejection by the settings
observer leaves the previous publication and hold in force.

RUN keeps the last picture frozen until the first eligible contribution. The software hold and
final-record acknowledgement remain in use. The ordering has host coverage and awaits unit verification.
[Host traces](../../ui/src/board/dho/hw/acq/tests/run_order.rs) ·
[Acceptance](../../ui/src/ui/change/acquisition.rs) ·
[Picture invalidation](../../ui/src/ui/change/picture.rs) ·
[Preview](../../ui/src/ui/step_draw.rs)

## Stopped view and raw capture

In STOP, `last[0]` follows the view. A raw reread replaces that record. A pairs reread of a raw
capture can draw through `stop_pic`. One slot retains the accepted capture for full-record
measurements and RAW readout across view changes.

- Zooming out or panning in STOP keeps the previous window until the reread arrives. Projecting
  from the retained capture would fill the view immediately where it has samples.
- With history or segments active, the first record shown in STOP can become the retained capture.

A STOP coinciding with an identity change still uses the captured identity for a valid ring batch.
Every armed capture can write its full programmed range. A retained slot outside it stays valid.
The source drops every in-range ring reference and any pending replay regardless of the disarm
status. STOP keeps the last published frame when the ring reference is invalid.

The stop pulse can clear DONE after a record completes. The programmed range bounds possible
writes even when the status after the pulse reports no completed record.

A cancelled first-trigger wait disarms even before hold. The acquisition thread spends about
10 ms once per cancel (settings, identity or route change, force, or stopped view). An
ordinary RUN batch or record is not affected.
[Host trace](../../ui/src/board/dho/hw/acq/tests/stopped_raw.rs) ·
[Ring validity traces](../../ui/src/board/dho/hw/acq/tests/ring_validity.rs) ·
[Source replay](../../ui/src/board/dho/hw/acq/control.rs) ·
[UI assignment](../../ui/src/ui/capture_memory.rs#L291-L331)

### Disarm diagnostics

The post-pulse poll checks STOPPED in `0x4034` immediately after the write settle and then
at 50 µs intervals for up to 500 µs from the pulse. The pre-pulse poll still waits up to
10 ms. After the pulse the source samples DONE in `0x4080` before clearing play info.
The stop sequence has 17 writes and at least three reads, down from at least six reads.
The removed reads were two pre-pulse signature samples and the post-pulse ring position.

In one earlier unit run, the stop pulse cleared DONE on a running capture that still reported
the first slot. `no_completed_record` therefore describes post-pulse status and the drop rule
uses the programmed range for every outcome. See the [state schema](../spec/state-query.md#disarm-counters).

### `0x4034` status bits

| Bits | Evidence |
|---|---|
| 13 | `STOPPED`, the bit polled by the live stop sequence. |
| 4 | `WAIT_TRIG`, waiting for a trigger. |
| 6, 7 | Sticky trigger flags. The documented interpretation is TPU fired and FSM accepted a trigger, respectively. They can remain set after the trigger and do not identify a completed record. |
| 5, 8, 9 | Together with bit 13 these make up `0x2320`, a mask used by stock segmented code. Bits of the mask were observed during an ongoing segmented recording. The simulator sets bit 5 when frames begin to arrive. The individual meanings of bits 5, 8 and 9 have no confirmed decode here. |

The observed `0x20c0` sets bits 13, 7 and 6. It shows STOPPED alongside sticky trigger flags.
The post-pulse disarm classification takes DONE from `0x4080` bit 30.
[HAL status](../../hal/src/dho/scu.rs) · [Segmented recording](../../hal/src/dho/seg.rs) ·
[Trigger status observations](../fork/trigger-lockin.md#trigger-status-from-the-hardware-fpga-use)

## Side service while held

The held loop dispatches calibration and generator requests, starts and stops the frequency counter,
and refreshes the FPGA XADC. With an applied configuration it services DVM and level slots before
processing any stopped view. DVM uses the configured mask, channel scale and gain, and levels reads
the configured ADC layout. A valid batch allows one replay for a changed view and a 20 ms idle poll
for an unchanged view. If side service disarms and invalidates the retained slot, the pass skips
replay and takes the 5 ms held poll. After calibration, service resumes once configuration is restored.

The mask/zone fallback status follows the applied configuration while held. Automatic ADC core trim
can write offsets before qualifying the input. It stays deferred during STOP and after a SINGLE
completes. A new RUN or SINGLE can service a due trim before arming if it reuses the configuration.
Applying a different configuration starts a fresh 2 s settle timer for an eligible trim, so its
first capture proceeds before trim. The port tests verify service order, replay invalidation,
held status and trim deferral with virtual time.
[Host trace](../../ui/src/board/dho/hw/acq/tests/side_service.rs) ·
[Held pass](../../ui/src/board/dho/hw/acq/control.rs#L60-L100) ·
[Counter and XADC](../../ui/src/board/dho/hw/acq/service.rs#L35-L46) ·
[Counter gate](../../ui/src/board/dho/hw/units.rs#L76-L107) ·
[DVM](../../ui/src/board/dho/hw/acq/units.rs#L69-L98)

## Waits and acknowledgements

| Path | Current bound and result |
|---|---|
| Disarm on STOP or batch cancellation | One 10 ms STOPPED poll in 500 µs steps, then the stop pulse. The source polls STOPPED for up to 500 µs after the pulse and samples DONE before clearing play info. An unknown disarm outcome while held sets the source status once for that disarm. The host test masks bit 13 and sees one stop pulse, the status diagnostic and the usual software acknowledgement with no final frame. [Disarm plan](../../hal/src/dho/seq/arm.rs) · [HAL poll](../../hal/src/dho/seq/wait.rs) · [Cancellation path](../../ui/src/board/dho/hw/acq/read.rs) |
| STOP final record | Once a serial first-trigger poll observes hold, it allows up to 2 s for the trigger, checked on the HAL cancellation cadence. The normal first-trigger timeout can expire earlier. The simulated long-record case cancels at 2 s with no DMA. The ordinary UI final-record drain ends when the source request is acknowledged **and** its record queue is empty, or when the UI's 3 s deadline passes. Subsequent C2H reads have no acquisition-loop deadline. OOS loads XDMA with a 1 s timeout per transfer wait by default. [First wait](../../ui/src/board/dho/hw/stop.rs#L51-L68) · [Trigger timeout](../../hal/src/dho/seq/wait.rs#L14-L28) · [Drain acknowledgement](../../ui/src/wave/hw_queue.rs#L129-L141) · [UI deadline](../../ui/src/ui.rs#L3036-L3039) · [DMA reads](../../hal/src/dho/bus.rs#L79-L108) · [Driver parameter](../../kernel/modules/xdma/xdma/cdev_sgdma.c#L40-L46) |
| Tap capacity and handoff | Capacity waits have no elapsed deadline. Hold wakes the tap wait and retires its plan, leaving already queued taps for the consumer. A funded cycle wait is paced to its next start slot. DMA and deposit admission parks wait up to 100 ms per recheck, with no overall drain timeout. UI consumption has a per-step record budget and no synchronous STOP tap-drain barrier. The real tap condition-variable path lies outside the virtual clock. [Tap capacity](../../ui/src/picture/deposit/tap/funding.rs#L105-L137) · [Hold notification](../../ui/src/picture/deposit/tap/funding.rs#L219-L228) · [Funded cycle](../../ui/src/picture/deposit/tap/funding.rs#L313-L341) · [Admission park](../../ui/src/board/dho/hw/dma_source.rs#L300-L310) · [UI intake](../../ui/src/ui/step_draw.rs#L79-L87) |
| Stopped-view reply | The source marks the software hold request acknowledged before reading the requested view. An unchanged view polls every 20 ms. The reply itself depends on C2H completion, subject to the per-request driver timeout above. [Stopped view](../../ui/src/board/dho/hw/acq/control.rs#L177-L211) |
| Failed configuration during hold | Each failed attempt reports status and waits 500 ms before retrying. The view and configuration-dependent side units wait for a successful configuration. A newer `final_req` can leave `final_ack` stale during retries. The UI clears its final-record drain at the normal 3 s deadline even if the ack is still stale. `*OPC?` observes operation tokens, and completed SINGLE records finish their token when consumed. [Retry trace](../../ui/src/board/dho/hw/acq/tests/calibration_hold.rs) · [Operation tokens](../../ui/src/ui/scpi_operations.rs) |
| RUN and armed SINGLE | RUN has no separate hardware acknowledgement or aggregate timeout. A first-trigger attempt waits up to `max(2000 ms, 2 × record_ms + 500 ms)`; failure disarms and backs off for 200 ms before another attempt while running. In the short host setup, each first-trigger attempt lasts about 2 s. Neither RUN nor armed SINGLE publishes a record without a trigger. An armed SINGLE waiting for a trigger is expected to remain pending. [First-trigger plan](../../hal/src/dho/seq/wait.rs#L14-L25) · [Retry](../../ui/src/board/dho/hw/acq/batch.rs) |
| Completed SINGLE and software acknowledgement | The source allows up to 3 s after publication for a slow UI step to consume a deep record and request hold. On expiry it reports the missed takeover once, keeps the record queued and waits unarmed on the 5 ms held poll. The UI owns hold and can still consume the frame and request it. SCPI completes an ordinary SINGLE token on consumption of its complete frame, before proposing STOP. The UI's STOP drain starts at its own final-record request. [Handoff bound](../../ui/src/board/dho/hw/acq/control.rs) · [SINGLE consumption](../../ui/src/ui.rs#L2959-L2964) |

The missed SINGLE takeover and unconfirmed STOP diagnostics appear as `acq.status` in
`:SYSTem:STATe?` until the next accepted RUN or SINGLE. The software STOP
acknowledgement still covers the source request and queue drain. Hardware STOP completion remains
unconfirmed if the source reports that diagnostic. The reply protocol for remaining waits is open.
`*OPC?` still observes Autoset, SIM and armed SINGLE tokens, so ordinary STOP can still be settling
when the query replies. The armed SINGLE trigger wait remains open-ended and cancellable.
[Virtual-time traces](../../ui/src/board/dho/hw/acq/tests/bounds.rs) ·
[Existing tap wake test](../../ui/src/drawpipe/identity_tests.rs#L15-L49) ·
[Operation tokens](../../ui/src/ui/scpi_operations.rs#L89-L97) ·
[SCPI barrier](../../ui/src/scpi.rs#L810-L818)

## C2H DMA timeout

Both OOS loader paths set `c2h_timeout=1` by default, including after kexec. Roll and Scan request
only complete FIFO chunks. Capture playback uses stored data and splits larger reads into requests
of at most 4 MiB.

The synchronous XDMA timeout path aborts the queued transfer and stops the engine when its status
is accessible. The local driver returns `ETIMEDOUT` even if earlier chains in the syscall completed.
Userspace discards the failed read and takes the FPGA snapshot before the stop sequence. The sequence
clears play info and pulses the SPU and TX resets. The worker discards retained replay state and services
requests while polling for STOPPED. It repeats disarm every 200 ms and reconfigures when STOPPED arrives,
status reads fail, or the one-second recovery deadline expires. A stalled engine, late completion or unread
FIFO data could still affect the next read if the hardware does not honor the reset. Check the driver and
FPGA behavior on a DHO924S before relying on automatic recovery.

[XDMA return path](../../kernel/modules/xdma/xdma/libxdma.c) ·
[FPGA stop sequence](../../hal/src/dho/scu.rs) ·
[Worker recovery](../../ui/src/board/dho/hw/acq/dma_fault.rs) ·
[XDMA startup and observer](../system-conf-keys.md#xdma-startup)

## Review of the withdrawn attempt

- Preserve the picture on RUN. The attempt cleared `ph`, `phz` and the deposit window on each hardware
  run edge (`ui/src/ui/change/picture.rs:11` in the attempt). Keep calibration requests, counter and
  other side readings, and generator updates serviced during STOP. The attempt's hold shortcut returned
  ahead of those calls (`ui/src/board/dho/hw/acq/control.rs:23` in the attempt).
- Bound disarm retries, waits for final record delivery or tap drain, and waits for the source to acknowledge
  RUN or the hold after SINGLE. The attempted STOP timer covered non-SINGLE drain. An absent hardware
  acknowledgement could leave RUN or completed SINGLE pending indefinitely (`ui/src/ui/scpi_operations.rs:41`
  in the attempt). Preserve the existing distinction between a SINGLE still waiting for its trigger and
  an acknowledgement that is missing after the capture completed.
- Decide how SCPI should report a source timeout or disarm failure. The attempt reused `-410` and suppressed
  the query reply. The restored interpreter uses `-410` for a cancelled pending operation.
  [Cancellation path](../../ui/src/scpi.rs#L776-L791).
