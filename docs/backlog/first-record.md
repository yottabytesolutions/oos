# First record of a batch

The FPGA can retain frame 0 from the settings that preceded an arm. Converting that waveform with new
V/div, offset or probe settings can put a stray trace on the picture. The 16-byte header has a batch-local
slot number and a timestamp with evidence of a one-record lag. It carries no settings generation or
per-record trigger position. A periodic signal can cross the trigger level near `pre` in an older record.

The default for an ordinary running batch permits slot 0 after a complete ordinary read under the same
applied acquisition generation and epoch. The source forgets that proof at each RUN, Clear, STOP,
calibration, reconfiguration, disarm, cancelled fill, read error or retained-ring loss. Extra FPGA
writes between batches also invalidate the proof for the next batch. If slot 0 is at most one arm old,
both candidate arms used the same settings, so the waveform is valid for the current picture. The
host simulator verifies the selection rule. The hardware age bound remains an assumption.

The decoded record path checks the early slots at any depth. The pipe checks them up to 100,000 points,
or at any depth when only two slots are available. Both paths already use those limits for slot 1.
In a two-slot batch a rejected slot 1 suppresses the whole batch, leaving the picture and newest
waveform on the previous batch. With more slots, a rejected slot 1 can leave a hole in the physical
slots sent to the worker. A recovered slot 0 on the record path goes through mask, history and counting
like every other accepted record. Held STOP, SINGLE, hardware average, segments, roll, scan and the
existing singleton selection rules are separate. `acq.frame0 = drop` restores the previous rule for A/B
runs. `acq.frame0 = keep` bypasses proof and both early-slot checks as before.

The offline corpus had 4,042 distinct payloads across 360 saved batches. Eligible edges were in the
window for 271 of 274 frame-zero observations and 2,474 of 2,513 later observations. The three frame-zero
misses followed visible settings changes. The dumps did not record every arm, so they cannot establish
how far back a distinct payload can originate.

To test the age bound on the unit, log every arm and readout in one continuous run and record a
trigger-synchronous monotonic or uniquely coded marker in the payload on a second channel. Correlate the
marker for slot zero with the immediately preceding arm across consecutive batches, pauses, and a change
that makes old and new samples visually distinguishable. Repeat with multiple channels, time bases
and depths. Compare `drop` with the default and check the first pictures after V/div, offset or probe
changes for ghost traces, wrong amplitudes or offsets, and horizontal trigger misalignment. Check that
the newest waveform still comes from the same slot and that the recovered early slots are accounted for.
This is pending hardware verification.

## Writes between ordinary batches

The trigger status clear is part of every normal readout. It writes 0x4034 twice after reading the
sticky flags. A pure poll or the XADC sample at 0x1438 leaves the proof intact. The extra writes
below are separate from ordinary playback. At 30 pictures/s and B batches/s, the normal RUN rates
are as follows.

| Operation | FPGA writes | Cadence when enabled |
|---|---|---|
| Raw replay of the newest compressed record | Pass-through at 0x1170/0x1174, 0x1130/0x1134 and 0x1154 to 0x1164, plus the playback window, stop/reset and one-slot range (0x1010, 0x1000, 0x1100/0x1108, 0x11f0, 0x4004/0x4008/0x400c, 0x4010/0x4014/0x4018, 0x4068/0x406c and 0x4080) | One due replay per batch at most, capped at 5/s or 1/6 per picture on average. Deep measurement raw windows follow their own throttle. |
| LA read and restore | LA pass-through at 0x1280, 0x1290/0x1294, 0x1298/0x129c and 0x12b0 to 0x12bc, window/length at 0x1100/0x1108/0x12c0, header gate at 0x1010, stop/play at 0x1000/0x4004/0x400c/0x4080, and stream select and restore at 0x4008 | Each LA batch, or B/s and B/30 per picture, until the three-failure cutoff. LA capture already uses singleton batches. |
| Frequency counter | Start or reconfigure at 0x2200 to 0x2214, stop at 0x2210, and latch at 0x2218 before reading 0x221c to 0x2240 | Polls every pass when configured. Steady latches occur at most every 0.35, 0.6 or 1.1 s, depending on digits. That is up to 2.86, 1.67 or 0.91/s (0.095, 0.056 or 0.030 per picture). Not-due passes perform no counter I/O. |
| Hardware mask or zone counters | Latch pulse at 0x1190 | Once per batch for each active reading. |
| DVM reading | Latch pulse at 0x2248 | Up to 2/s, or 1/15 per picture. Start and stop already disarm. |

In steady multi-record RUN with no extra writes, all batches can check slot 0 after the first proof,
subject to the edge and newest-record checks. A raw replay invalidates the next batch, and a due
counter or DVM latch invalidates the batch that follows it. Over N batches, subtract one per distinct
invalidated successor within the observation window. A failed raw or LA read also disqualifies its
own batch. With raw replay on every batch, or a hardware mask/zone latch on every batch, steady
reuse is 0%. This also applies to deep raw measurements requested on every eligible batch. Throttled
deep reads run at most once a second, with a longer interval for windows over 2 MiB. LA has no
multi-record batches eligible for reuse. The singleton can still be published under its existing
rule. These rates are derived from code.

## One decision for a completed batch

The [host comparison](../../ui/src/board/dho/hw/acq/tests/record_selection.rs) runs the same DMA bytes and
armed settings through both dispatch requests. It observes the record queue, worker intake, composed picture,
newest waveform and loss counters. Empty handoff suppression and average readiness are implemented. A common trigger probe across routes is a separate proposal.
For raw substitution the fixture injects a separately decoded waveform at the readout boundary. The host
fixtures establish selection and accounting. Hardware freshness and comparator equivalence still need unit
evidence.

### Host outcomes and remaining divergences

Slots are zero-based. Ranges are half-open. A retained raw replacement uses the sequence of its DMA slot.

| Host case | Decoded record path | Deposit request | Which outcome serves the user |
|---|---|---|---|
| Two normal records, frame 1 off edge, drop or steady | No output. One frame 0 and one frame 1 loss. | No submission. Same losses and no picture. | Both records already have a loss reason. The same no-submission rule applies when a short buffer leaves two full slots or when frame 1 is deep. |
| Ready hardware average, two records, off edge, drop | Publishes the newest average. One average loss. | Same outcome via record fallback. | A composite waveform can smear or move the crossing even when its input triggers were valid. Four records still produce the newest average and three average losses. |
| Deep interior frame 1, off edge, drop | Keeps `2..3`. One frame 0 and one frame 1 loss. | Keeps `1..3`. Only frame 0 is lost because the depth exceeds 100,000 points. | Unknown, needs the unit. At depths on both sides of the cutoff, capture a trigger-synchronous signal with identifiable epochs and compare slot stamps, the local edge and known-fresh sample windows. A bounded probe can remove the CPU reason for a depth cutoff. |
| Frame 1 crosses only before ADC core correction, drop | Keeps `2..3` with correction enabled. Lacks a crossing on the resulting waveform. | Keeps `1..3` with correction enabled. Finds the crossing on the DMA codes before correction. | Unknown, needs the unit. Sweep a reference edge across the trigger level with cores on and off. Compare the external reference and the FPGA trigger marker against the same lane's raw and corrected crossings, including a known-fresh and known-stale slot. |
| Two records, DMA frame 1 off edge, separate raw newest on edge | Rejects the slot before substitution. | Keeps slot 1 and taps its raw replacement. | Unknown, needs the unit. Read both representations of the same slot before any overlapping arm. Verify their trigger-relative sample windows and slot identity against a signal with a per-trigger marker. |
| Two records, DMA frame 1 on edge, separate raw newest off edge | Taps the off-edge raw replacement of slot 1. | Rejects slot 1 and makes no submission. | Unknown, needs the unit. In addition to the paired reads above, measure whether the separation is caused by raw replay alignment, the filtered trigger, or the slot being overwritten. Use the waveform carrying the proven slot and time alignment for both paths. |

Both paths accept frame 1 when a high-frequency signal puts a crossing in every window. The test uses a
periodic signal above 20 MHz. A crossing within 32 samples verifies local alignment for eligible records.
To test freshness on the unit, repeat the capture across arms with an identifiable envelope or epoch marker
independent of the crossing, or verify a capture stamp with that marker. The local edge check supplements the previous-arm settings proof. Zero or one complete slot in a short buffer uses the same selection
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

These are the legacy drop and keep controls. Apply the first applicable row. `n` is the number of complete slots. An eligible raw replacement can be the
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

The matcher takes the selected DMA slots in physical order, including the DMA slot underlying a separate
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
