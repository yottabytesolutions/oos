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
Side units run before a stopped-view read or its 20 ms unchanged-view poll.
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
[Held trace](../../ui/src/board/dho/hw/acq/tests/stopped_pass.rs) ·
[Calibration and held startup traces](../../ui/src/board/dho/hw/acq/tests/calibration_hold.rs) ·
[Side service](../../ui/src/board/dho/hw/acq/tests/side_service.rs)

