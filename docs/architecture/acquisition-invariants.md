# Acquisition invariants

These host tests describe the current acquisition and worker behaviour. They use the HAL simulator and event-driven worker completion.

## Record conservation

Holds in `auto_paints_each_admitted_record_once_under_backpressure_and_across_stop_run` for AUTO batches of
1, 2, 4 and 6 with frame zero kept, a successor waiting to submit after DMA, and STOP during or between batches.
STOP during an overlapping fill can complete the read and charge the whole batch to `lost.settings` before
submission, while STOP between these unarmed batches adds no read.
[Host test](../../ui/src/board/dho/hw/acq/tests/picture.rs) · [Rejected handoff](../../ui/src/board/dho/hw/acq/dispatch.rs#L81)

## One deposit per record

Holds in the same AUTO test, which gives each record its own row and checks that row's hit count and the number of
accepted records at each composition. After STOP and RUN the first picture of the new epoch contains only the newly
submitted records, and a second composition contributes zero.
[Host test](../../ui/src/board/dho/hw/acq/tests/picture.rs)

## Raw newest record

The universal raw guarantee fails in `live_compression_taps_use_pairs_until_another_raw_replay_is_due`, which
finds the last eligible record as raw samples on successful due replays and as DMA pairs between them.
A failed replay can also leave DMA pairs and deep replay can supply a raw window, while the records route
updates its latest frame from the last queued record, two records with frame-zero selection enabled and frame
one rejected by the edge check supply no new waveform, and a tap from an obsolete acquisition identity is stale.
[Tap test](../../ui/src/board/dho/hw/acq/tests/raw.rs) · [Selection tests](../../ui/src/board/dho/hw/acq/tests/deposit.rs)
· [Raw read](../../ui/src/board/dho/hw/acq/read.rs#L172) · [Tap dequeue](../../ui/src/picture/deposit/tap.rs#L273)
· [Frame-one selection](../../ui/src/board/dho/frame1.rs#L112) · [Record publication](../../ui/src/board/dho/publication.rs#L102)

## Selection losses

The deposit selection tests show that frame zero and off-edge frame one each increment their named counter when
excluded, with zero change to either drop aggregate on deposit dispatch. An unreliable initial ordinary singleton
publishes no waveform and is charged to settings.
[Selection tests](../../ui/src/board/dho/hw/acq/tests/deposit.rs) · [Dispatch](../../ui/src/board/dho/hw/batch.rs)

## Short buffers

Holds in `short_buffers_submit_only_complete_records_and_count_missing_slots_as_errors`, where two or three of
four records are available after the fixture truncates the completed DMA buffer. The missing slots go to
`lost.error` and selection uses the available count, so any tap carries the last available record that survives
selection.
[Host test](../../ui/src/board/dho/hw/acq/tests/deposit.rs) · [Read accounting](../../ui/src/board/dho/publication.rs#L373)

## STOP and RUN boundaries

The source defers settings edits while held with an applied configuration. A held pass without an applied
configuration, such as after calibration or at source startup, applies the current request once before
polling the acquisition side units. The mask/zone fallback status stays with the applied configuration
while held. DVM uses its channel mask, scale and gain, and levels reads the configured ADC layout.
Side units run before a stopped-view read or its 20 ms unchanged-view poll. If a side unit disarms
and the slot rule drops the retained batch, the pass skips the replay and takes the 5 ms held poll.
Counter service waits for an applied configuration. Generator dispatch and XADC service remain available.
Automatic ADC core trim waits for RUN or SINGLE because it writes offsets. RUN or SINGLE applies the
latest request before arming if the effective settings still differ.

Each calibration invalidates the retained ring batch and pending replay, including during a completed
SINGLE hold. When restoring configuration fails, the loop retries after 500 ms and can leave a newer
hold request unacknowledged. The UI's final-record drain still expires after 3 s.

Unapplied edits preserve a valid batch and its capture identity. An eligible stopped view selects a
window in that batch and reads it using the captured channel layout, memory depth, sample rate and
acquisition mode. An already armed successor makes the ring reference unsafe even when STOP coincides
with an identity change. The source keeps the last published frame for that case.

Both STOP rereads use the newest retained slot. Every batch arms at frame zero and can write its entire
programmed range, including while the source waits for read admission or polls before disarm. A slot outside
that range stays valid. The source drops every in-range slot and clears pending replay regardless
of the disarm status. STOP keeps the published capture when the retained slot is invalid.

The stop pulse can clear DONE even after a record completes. The fill count cannot bound writes
before STOPPED.

A cancelled first-trigger wait disarms before the next hardware operation, including before hold.
The acquisition thread spends about 10 ms on disarm once per cancel (settings, identity or route
change, force, or stopped view). Ordinary RUN batches add no observation work. A stopped view
published before hold polls every 20 ms until held or withdrawn. The host simulator exercises the
slot rules, including a stop pulse that clears DONE after a record completes.
[Held trace](../../ui/src/board/dho/hw/acq/tests/stopped_pass.rs) ·
[Ring validity traces](../../ui/src/board/dho/hw/acq/tests/ring_validity.rs) ·
[Slot and wait traces](../../ui/src/board/dho/hw/acq/tests/stopped_slots.rs) ·
[Calibration and held startup traces](../../ui/src/board/dho/hw/acq/tests/calibration_hold.rs) ·
[Side service](../../ui/src/board/dho/hw/acq/tests/side_service.rs)

Shared's `hold` is the source's sole run/hold authority. The UI prepares the worker view, including
its keys and zoom projection, before taking Shared. The view and tap state, settings, identity, route,
segment command, queue filtering and hold are committed under Shared, taking the pipe locks inside it.
The source and tap-command waiters are notified after staging the transaction. The source applies
changed settings before arming and checks the generation and epoch before DMA admission. Late DMA
or worker output from a superseded pair is rejected. A rejected change leaves the previous
publication and hold in force.
[RUN traces](../../ui/src/board/dho/hw/acq/tests/run_order.rs)

A stopped-view request can yield a raw window tagged with the original capture identity
while the complete record remains the source's latest frame. The UI can install that window as its latest
record. A replay completing after RUN or a change of the requested generation or epoch is discarded.
[Stopped view trace](../../ui/src/board/dho/hw/acq/tests/stopped_raw.rs) ·
[UI consumer](../../ui/src/ui/capture_memory.rs#L314-L331)

The software hold acknowledgement can advance without a hardware STOPPED acknowledgement or final record.
Clear during an in-flight serial STOP read discards the stale record and acknowledges the request at completion.
Held roll and scan keep their initial STOP acknowledgement. Held segments poll every 5 ms without arming.
The virtual-time tests cover the 10 ms disarm poll, serial STOP cancellation after 2 s without a trigger,
and RUN and SINGLE retry after a missing trigger.
[Wait traces](../../ui/src/board/dho/hw/acq/tests/bounds.rs)
