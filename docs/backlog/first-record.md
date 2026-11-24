# First record of a batch

A withdrawn typed first-record change tried keeping frame 0 in batches of two to four records
when an analog edge crossed the trigger level within 32 samples of `pre`. It checked frame 1 first
so the selected records would form a contiguous range.

The FPGA can retain frame 0 from the settings that preceded the arm. After a V/div, offset, or
probe change, converting those samples with the new settings can draw the wrong amplitude or offset.
For an eligible periodic waveform with a period below 64 samples, even an old record can have a
crossing in the search window. Keeping frame 0 automatically would require a per-record capture
stamp or settings generation that proves the record belongs to the arm. That provenance has yet
to be verified on hardware.

A later rewrite, also withdrawn, tried to restore the exclusion rule by sharing selection across routes.
It changed the held path. A two-record batch with frame 1 off trigger published nothing on STOP,
where the existing dispatch publishes the newest available record. Short DMA buffers also need
coverage because the completed record count can exceed the number of whole records in the buffer.
Two-record average batches need their own cases because average progress determines which record
can be published. The host matrix used the selection code of the drawpipe and fastdraw module split as its baseline.

Before another refactor, write dispatch-level tests against that selection code that pin record selection
and publication on every path. Include worker deposits, decoded records, held STOP, SINGLE and
average dispatch, with both first-frame switch values. Cover two-record batches with frame 1 on
and off trigger, short DMA buffers and two-record average batches. Assert the record indices and
publication outcomes at dispatch so a helper-level match cannot hide a change in what is drawn.

## One decision for a completed batch

The [host comparison](../../ui/src/board/dho/hw/acq/tests/record_selection.rs) runs the same DMA bytes and
armed settings through both dispatch requests. It observes the record queue, worker intake, composed picture,
newest waveform and loss counters. Empty handoff suppression and average readiness are implemented. The shared
selection policy below is a proposal.
For raw substitution the fixture injects a separately decoded waveform at the readout boundary. The host
fixtures establish selection and accounting. Hardware freshness and comparator equivalence still need unit
evidence.

### Host outcomes and remaining divergences

Slots are zero-based. Ranges are half-open. A retained raw replacement uses the sequence of its DMA slot.

| Host case | Decoded record path | Deposit request | Which outcome serves the user |
|---|---|---|---|
| Two normal records, frame 1 off edge, drop | No output. One frame 0 and one frame 1 loss. | No submission. Same losses and no picture. | Both records already have a loss reason. The same no-submission rule applies when a short buffer leaves two full slots or when frame 1 is deep. |
| Ready hardware average, two records, off edge, drop | Publishes the newest average. One average loss. | Same outcome via record fallback. | A composite waveform can smear or move the crossing even when its input triggers were valid. Four records still produce the newest average and three average losses. |
| Deep interior frame 1, off edge, drop | Keeps `2..3`. One frame 0 and one frame 1 loss. | Keeps `1..3`. Only frame 0 is lost because the depth exceeds 100,000 points. | Unknown, needs the unit. At depths on both sides of the cutoff, capture a trigger-synchronous signal with identifiable epochs and compare slot stamps, the local edge and known-fresh sample windows. A bounded probe can remove the CPU reason for a depth cutoff. |
| Frame 1 crosses only before ADC core correction, drop | Keeps `2..3` with correction enabled. Lacks a crossing on the resulting waveform. | Keeps `1..3` with correction enabled. Finds the crossing on the DMA codes before correction. | Unknown, needs the unit. Sweep a reference edge across the trigger level with cores on and off. Compare the external reference and the FPGA trigger marker against the same lane's raw and corrected crossings, including a known-fresh and known-stale slot. |
| Two records, DMA frame 1 off edge, separate raw newest on edge | Rejects the slot before substitution. | Keeps slot 1 and taps its raw replacement. | Unknown, needs the unit. Read both representations of the same slot before any overlapping arm. Verify their trigger-relative sample windows and slot identity against a signal with a per-trigger marker. |
| Two records, DMA frame 1 on edge, separate raw newest off edge | Taps the off-edge raw replacement of slot 1. | Rejects slot 1 and makes no submission. | Unknown, needs the unit. In addition to the paired reads above, measure whether the separation is caused by raw replay alignment, the filtered trigger, or the slot being overwritten. Use the waveform carrying the proven slot and time alignment for both paths. |

Both paths accept frame 1 when a high-frequency signal puts a crossing in every window. The test uses a
periodic signal above 20 MHz. A crossing within 32 samples verifies local alignment for eligible records.
To test freshness on the unit, repeat the capture across arms with an identifiable envelope or epoch marker
independent of the crossing, or verify a capture stamp with that marker. Keep the explicit frame-0 policy
until such provenance is established. Zero or one complete slot in a short buffer uses the same selection
as any other batch with that many available slots. Count the incomplete slots once as DMA errors.

### Inputs and result

Decide at the source after validating DMA geometry, taking the average progress snapshot and attaching any
raw reread, before matcher learning, the tap and publication. A pre-read admission reservation can already exist.

- Use the completed physical count and the number of whole DMA slots, clipped to the completed count.
  Track average progress separately as ordinary capture, average pending or average ready. The average
  counter uses the number of triggers consumed by the hardware.
- Use the armed trigger kind, slope, source, coupling, level and channel calibration, plus the frame-0 policy
  and the existing first-singleton stale predicate. A separate raw read can represent the last DMA slot
  when the read is complete and still owns that slot. Use its own capture geometry.
- Frame 1 has one verdict: `OnEdge`, `OffEdge` or `Unchecked`. Only eligible analog edge triggers with DC or
  HF-reject coupling, an active trigger lane, a valid `pre`, at least 64 samples and a non-pair waveform
  are checked. Keep the slope, 0.2-division hysteresis and ±32 sample search, with up to 4,096 earlier
  samples for arming. Leave the verdict unchecked when selection has no use for it.
- The proposed common probe uses the armed calibration and rounded f32 volts on both inputs. On bytes,
  skip the header, read one interleaved lane and convert only the bounded window with the same conversion
  as an uncorrected decoded frame. On decoded volts, borrow that window from the same representation and
  its own window-relative `pre`. The host byte check currently uses f32 affine arithmetic. The decoded path
  uses decoded and possibly core-corrected volts. Pin thresholds and rounding before replacing either.
  Start with an uncorrected input independent of learning. If unit evidence calls for correction, supply a
  read-only correction snapshot to both representations and keep learning after selection.
- For two slots, the proposed probe checks the selected newest representation, including an eligible raw
  reread. Confirm its identity and alignment on the unit before enabling this rule. For three or more slots
  the check is on physical slot 1. The raw newest then represents a different slot.

The proposed result is plain data.

```rust
struct BatchSelection {
    slots: std::ops::Range<usize>,
    newest: Option<NewestSource>,
    sequence_slots: usize,
    losses: SelectionLosses,
}

enum NewestSource {
    Dma { slot: usize },
    Raw { slot: usize },
}

struct SelectionLosses {
    missing: u32,
    average: u32,
    frame0: u32,
    frame1: u32,
    first_settings: u32,
}
```

`SelectionLosses` has disjoint counts for missing slots, average suppression, frame 0, frame 1 and
first-singleton settings loss. It carries counts only. Commit them at one boundary; consumers handle only
subsequent delivery failures. Empty selections carry no newest. A raw replacement contributes one physical
slot and keeps that slot's sequence and batch identity. A short transfer disqualifies the raw replacement
because the last complete DMA slot can be older than the slot reread raw.

### Outcome table

Apply the first applicable row. `n` is the number of complete slots. An eligible raw replacement can be the
newest in each nonempty normal row. Hardware-average delivery stays decoded and uses its average result.

| Input | Surviving slots | Newest | Selection losses |
|---|---|---|---|
| Average pending | Empty | None | All completed triggers to average suppression, as today |
| No complete slot, with ordinary or ready average read | Empty | None | Missing slots to error |
| Average ready, at least one complete slot | `n-1..n` | DMA average at `n-1` | `n-1` to average suppression |
| Ordinary singleton, first capture marked unreliable | Empty | None | One settings loss |
| Ordinary singleton, eligible to publish | `0..1` | Slot 0 | None |
| Ordinary multi-slot, keep | `0..n` | Slot `n-1` | None |
| Ordinary multi-slot, drop, frame 1 on edge or unchecked | `1..n` | Slot `n-1` | One frame 0 |
| Ordinary two-slot, drop, frame 1 off edge | Empty | None | One frame 0 and one frame 1 |
| Ordinary three or more slots, drop, frame 1 off edge | `2..n` | Slot `n-1` | One frame 0 and one frame 1 |

For ordinary and ready-average reads, add one error per missing slot in every row. At dispatch consume
sequences for the complete slots, including slots excluded by selection. Average progress suppression
and the existing obsolete-before-dispatch exit consume zero sequences. Keep the current SINGLE
first-capture exemption and completion rule. Preserve the held STOP dispatch as a separate lifecycle
decision. The existing held path may publish the newest available slot even when the running selection
is empty. Keep its replay slot, generation, final-request acknowledgement and overwrite protections
with the source.

The matcher takes the selected DMA range in physical order, including the DMA slot underlying a separate
raw newest as it does today. Selection probing is read-only with respect to matcher learning. Keep the
worker's existing distinction between painting from DMA and tapping a raw reread. A tap or fallback uses
the same slot and sequence. Refund an unused reservation once on empty selection, cancellation or rejected
handoff. A retained tap keeps its backing charge until release. Count each physical record once through
selection and final delivery, including pre-worker rejection and held publication. A tap for a deposited
slot earns no second record credit. Use the disjoint loss buckets for conservation and keep the established
record-queue and deposit telemetry meanings.

### Remaining user-visible changes

- Subject to the deep-slot comparison, the deposit path will reject off-edge interior frame 1 above 100,000 points and count one frame 1 loss.
- Subject to the comparator comparison, a crossing present only before ADC core correction will keep frame 1 on the decoded path as well as the deposit path.
- Subject to the reread comparison, both paths will keep slot 1 when its validated raw reread is on edge and reject it when that reread is off edge.

### Follow-up jobs

1. Done on host. An empty selection finishes at the source with zero worker submissions, a full tap refund and
   the same sequence consumption and loss totals. Output-slot release still wakes the worker to publish a retained deposit.
2. Done on host. Publish by average progress, independent of frame-1 alignment. The pending and four-record
   controls still pass. Reconfiguration clears the accumulator, and the identity check charges obsolete results to settings.
3. Test identical verdicts at threshold ties, window bounds, alternate trigger lanes, steps and pair layouts.
   Extract one bounded probe with byte and decoded adapters, preserving route behavior through explicit inputs.
4. Test one decision and both consumers against the complete host matrix, held STOP, SINGLE, matcher exclusions
   and refund/cancellation paths. Replace the repeated selection with that result, keeping the existing probe
   choices as explicit compatibility inputs until each policy difference is settled.
5. After the unit measurements, change the deep cutoff, correction input and raw-source verdict in separate
   test-first changes. Keep per-slot sequence and conservation assertions and compare the CPU goldens at each step.
