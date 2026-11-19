# Frame pipeline

Owner-approved target, 2026-11-18. Part of #196. This design covers the path from an FPGA read to the picture.
It supersedes the worker-share kernel, publication and window details in [drawing](drawing.md) and
[picture](picture.md). [Migration](migration.md#f0-f7-frame-pipeline) defines F0-F7. Nothing in this design is
implemented or measured on hardware unless explicitly marked as existing evidence.

The source review used `origin/main` a8eb80da and `origin/perf/184-batch-merge` 22fcf38a (#186).
Source locations below belong to those snapshots and will move. The design builds on [records](records.md),
[the architecture](README.md), ADR 0031, ADR 0034, ADR 0035 and ADR 0036. It keeps `Batch`, `BatchMeta`,
`MappingKey`, `Replace`, `Picture` and accumulator. It changes three contracts:

- Workers write column differences into one plane set. They no longer own full maps.
- The UI receives one `Deposit` per picture period, containing whole batches.
- Acquisition waits for a buffer. It never reads records just to discard them under load.

## Existing evidence

Standard has the right kernel on the UI thread. It reduces records to column spans, writes two difference entries
per column and folds once per picture (`ui/src/ui.rs`, `ui/src/fastdraw.rs`). It uses decoded f32 copies and a
wall-clock cut. Batch has the right ownership: it works off the UI thread from read-out words and uses every
record (`ui/src/drawpipe.rs`). Its workers paint pixels into separate full maps, which must then be summed.

The target puts Standard's kernel into Batch's pipeline. The UI folds once per picture. There are no worker maps
to sum. Fold cost depends on pixels and enabled channels, not records or worker count.

Current routing, decoded copies and picture writers are inventoried in the
[architecture overview](README.md#frame-loop-and-data-flow-today) and
[drawing audit](drawing.md#current-data-flow-dho804-32041d0). The key mismatches this target removes are:

- `fastdraw::whole::deposit` paints every pixel in each span; workers clear and publish separate maps per batch.
- Standard uses difference bands, but scoped threads and a wall-clock intake budget remain on the UI path.
- Route selectors and duplicate sample/mapping types give the newest record a different path from its siblings.
- Uncounted settings/queue losses and read-and-discard overload obscure capture rate.

## Vocabulary and types

A **record** is one triggered acquisition of all channels as stored by the FPGA. A **batch** is one ring read:
N records in one buffer under one `BatchMeta`. A **picture** is the phosphor at a screen refresh.
Use record, batch and picture for the new data types. Existing `Frame` names remain during migration.

These types carry the pipeline. Ownership moves between stages; immutable batches can have several readers.
These Rust sketches show the target data and contracts. Owned fields remain private, with
constructors and read-only accessors where needed, as required by [ownership](README.md#enforced-by-visibility).

```rust
/// One ring read, immutable after read. Last Arc drop returns the buffer to the pool.
pub struct Batch {
    meta: BatchMeta,
    buf: Lease<DmaBuf>,
    records: Range<u32>,       // excludes the first record and an edge-less second record
    record_bytes: u32,
}

/// Convert and place without live settings.
pub struct BatchMeta {
    id: BatchId,
    identity: Identity,    // gen, epoch, stats_gen from oos_core::change
    dt: f64,               // seconds per sample
    pre: u32,              // hardware trigger sample index
    layout: WordLayout,    // stride, word per channel, affine words-to-volts
    form: SampleForm,      // Raw | Pairs { k }
    trigger: TrigSnapshot, // source, level, slope, coupling, hysteresis in volts
    read_at: Instant,
}

/// Borrow one channel of one record. No copy on the picture path.
pub enum SamplesRef<'a> {
    Codes(Codes<'a>),          // raw words; extremes on words, with NEON
    Pairs(Pairs<'a>),          // min/max pairs: compression, deep, sweeps
    Volts(&'a [f32]),          // stored records, average output, references, demo
}

/// Enabled values indexed by channel; None means off or absent.
pub type PerChannel<T> = [Option<T>; NCH];

/// Volts/seconds to rows/columns for main or zoom. Small and Copy; no Model.
pub struct Projection {
    key: MappingKey,
    cols: u16,
    rows: u16,
    t_first: f64,             // column 0 relative to trigger, seconds
    t_per_col: f64,
    ch: PerChannel<Vertical>, // rows per volt, row of 0 V; None means off
    style: Style,             // Lines | Dots
    interp: Interp,
}

/// One placed record. Spans use 4 bytes per column per channel.
pub struct Placed {
    trigger_at: f64,                  // refined position, samples
    outcome: TrigOutcome,              // AtPoint | WrongSlope | Moved(samples) | NotFound
    spans: PerChannel<Spans>,          // first/last row; None for dots or off
    stats: PerChannel<SampleStats>,    // max, min, sum, sum of squares in window
}

/// +q at first row, -q below last row; (rows + 1) x cols, wrapping u16.
/// Linear: several batches add by plain accumulation.
pub struct DiffPlane { cols: u16, rows: u16, d: Vec<u16>, touched: Range<u16> }

/// Separate main and optional zoom values under the same mapping key.
pub struct PerMap<T> { main: T, zoom: Option<T> }

/// Whole batches of one mapping key and identity, handed from pool to UI.
pub struct Deposit {
    key: MappingKey,
    identity: Identity,
    records: u32,          // record accounting and minimum-window target
    weight: PerMap<Weight>, // mode-specific W per map, independent of record count
    planes: PerMap<PerChannel<DiffPlane>>,
    stats: PerMap<PerChannel<SampleStats>>, // aggregated per-record measurements
    trig: TrigStats,       // includes the triggered-record count
    batches: BatchSpan,    // first/last BatchId, oldest and newest read_at
}

/// Private to accumulator. Readers get &Picture.
pub struct Picture {
    main: Counts, zoom: Option<Counts>, // u16 at Min; u32 16.16 with decay
    weight: PerMap<Weight>,
    dirty: Range<u16>,         // covering hull of rows changed since last publish
}
```

`BatchMeta` also carries the route tag copied from `AcqWant` at arm, as required by
[acquisition](acquisition.md#acqwant-and-generations). The sketch omits that existing routing detail.
`Waveform` is the one decoded form for readers: `BatchMeta`, record index, and owned samples in volts or min/max
pairs. It cannot hold an `Arc<Batch>`, pinned batch range or DMA lease. It replaces `board::Frame` in the queue and
`wave::Raw`. Borrowed batch access uses `RecordRef`, which yields `SamplesRef` without copying.
`wave::Record` remains the screen-window record for measurements, decoders, history and SCPI ([records](records.md)).
Long-lived readers keep their own copies under the [retaining-owner budgets](README.md#retaining-owners).
For every deposit-route batch, the pool sends one newest-record `Waveform` through a separate bounded FIFO to the
records stage, in batch order, reusing storage after warm-up. This tap is
independent of deposit coalescing and picture rejection: history receives the newest record of every batch, while
other readers apply their own identity and newest-value policies from [records](records.md#records). RecordBatch
already contains these records and needs no duplicate tap. No deposit in the open set, slot or window pins a DMA
buffer. Once the in-flight batch readers finish, their buffers return even if every deposit remains retained.

### Projection identity

The target `DepositView` publishes projections and their acquisition `(gen, epoch)` together in one snapshot,
derived from the same resolved `ChangeSet` as `AcqIntent`. At first claim, compare that pair with `BatchMeta`.
Only a matching pair may supply projections for painting; otherwise count the batch's records as `lost.settings`
before painting any slice. Still deliver its newest-record tap, whose readers apply their own acceptance rules.
Never assign a newer view's key to an older acquisition. Copy the matching projections once and keep them for the
whole batch, even if the view changes during placement. The deposit retains that key and the batch's identity.

A mapping-only change keeps `(gen, epoch)`, so an unclaimed batch may use the new projection with its own conversion
metadata. A batch already claimed keeps its previous key and is rejected as `stale_key` if that key is no longer
current. An already painted deposit from an older `gen` can still be shown when its original key and epoch match;
generation alone is not a picture acceptance filter. This does not permit an unclaimed batch from an older `gen`
to borrow the current view. `stats_gen` is checked separately for statistics and does not gate projection selection.

### Invariants and units

- Every read record belongs to exactly one `Deposit` or one named loss bucket. The record route accounts separately
  as specified under [conservation](#back-pressure-and-conservation).
- A `Deposit` contains whole batches of one `MappingKey` and one `(gen, epoch)`. A key or identity change closes it.
- `Deposit::records` equals the number of records deposited into its planes. It drives accounting and the
  minimum-window target N. `Deposit::weight` carries the separate normalization sums for main and zoom maps.
- After F6, lines contribute W = 1 per record, normalised per column: each column's counts are divided by the
  record count, with no factor for the number of columns. Dots contribute W = samples per column, derived from
  the record's sample spacing and each projection. Sum these weights across records, even when source densities differ.
  Main and zoom have their own W. Vertical clipping never reduces W. Grade divides sample-hit counts by W;
  `replace` computes the same mode-specific weight from its own source and projections.
- One thread writes a `DiffPlane` band at a time. Ownership moves by value. No lock guards pixel data.
- Only the accumulator writes `Picture`. Compose and GPU hand-off read `&Picture`.
- Picture conversion reads `BatchMeta`; placement reads `Projection`. Neither reads `Settings` or `Model`.
- Time is f64 seconds; sample positions are f64 indices; volts are f32. Rows and columns are u16, row 0 at the top.
- Move the fixed factor 24 into grade together with the W grade in F6 (M10). Until then, multiply both the
  deposited counts and their mode-specific W by 24. Removing this factor does not remove dots' density weight.

### Difference-plane capacity

Let q be the integer increment per span or sample hit: 24 before F6, 1 afterwards. For each enabled main or zoom
plane, let h(r) bound the number of hits from record r at any one pixel. Lines have h(r) = 1: a record contributes
at most one span per column, writing +q at its first row and -q below its last, so every pixel folds to at most q.
For dots, conservatively bound h(r) by the maximum samples mapped to one column, including column-edge rounding;
a flat trace can put them all on one row. For batch b, define H(b) = sum of h(r) over its records. H excludes q
and is separate from the fractional samples-per-column normalization W.

Require `q * sum_b H(b) <= 65_535` for the batches in each deposit. The bound applies to the nonnegative folded
pixel value, not to individual raw difference entries. Difference updates and prefix sums use wrapping u16
arithmetic; they reconstruct exact counts only while every folded pixel remains below 65,536.
For lines the record limits are floor(65,535 / 24) = 2730 before F6 and 65,535 afterwards.
For dots with h(r) = 86, they are floor(65,535 / (24 * 86)) = 31 before F6 and
floor(65,535 / 86) = 762 afterwards; 1000 such records cannot coalesce.

Check capacity before writing any slice of the next batch. If it would exceed the bound, close the current deposit
and wait for hand-off space. Bound read-plan batch sizes so one whole batch fits an empty set for every enabled
projection. A single record must fit too; a projection that cannot satisfy this requires wider planes before it
can be supported. Never split, thin or discard a batch to satisfy capacity. Apply the same bound to the combined
minimum-persistence window from F2 onward, including before F6: sum H over all batches in all retained deposits,
and evict oldest whole deposits until it fits before folding into the u16 Picture. Three individually fitting
deposits need not fit together.

## Stages

Each stage takes borrowed inputs and produces owned outputs. No stage reads global state.

```rust
// Acquisition thread.
fn read(hw: &mut Hardware, plan: &ReadPlan, buf: Lease<DmaBuf>) -> Result<Batch, AcqError>;

// Pool, parallel over records. Pure; sample form comes through SamplesRef.
fn place(record: RecordRef<'_>, proj: &Projection, want_stats: [bool; NCH], out: &mut Placed);

// Pool, parallel over tiles. One channel's column band is owned for this call.
fn paint_band(band: &mut DiffBand<'_>, placed: &[Placed], records: RecordsRef<'_>, proj: &Projection);

// UI thread. Fold differences, blend and mark dirty rows in one pass.
impl Accumulator {
    pub fn accept(&mut self, d: Deposit, now: Instant) -> Accepted; // rejection is counted
    pub fn replace(&mut self, w: &Waveform, reason: Reason, proj: &Projections);
    pub fn apply(&mut self, change: &ChangeSet);
    pub fn picture(&self) -> &Picture;
}

// UI thread, dirty rows only.
fn publish(picture: &Picture, arena: &mut Arena);
```

`DiffBand<'a>` is an exclusive mutable view of a `DiffPlane` column range, with a local touched-row range.
The pool splits the backing storage into disjoint bands before dispatch; workers cannot access other bands.
The completed bands' touched-row ranges are combined before hand-off.
`RecordRef` is a batch plus index, or a `Waveform`. It yields `SamplesRef` per channel.
`place` exists today as `fast_plan`; `paint_band` as `fast_deposit_t`. They currently take `&Model` and `&Frame`.
`fold_t` in `fastdraw.rs` supplies the fold. Dispatch once per channel per record on `SamplesRef`.
Inside each branch, spans use the generic trait, so inner loops remain monomorphic:

```rust
pub trait Samples {
    fn len(&self) -> usize;
    fn extremes(&self, a: usize, b: usize) -> (f32, f32);
    fn at(&self, i: usize) -> f32;
}
```

The target implementations are explicit:

| `SamplesRef` branch | Type implementing `Samples` | Values exposed |
|---|---|---|
| `Codes` | `Codes<'a>` (existing) | calibrated raw words |
| `Pairs` | `Pairs<'a>` (F1 adapter) | source min/max envelope |
| `Volts` | `[f32]` (existing) | decoded samples |
| Software HiRes on `Codes` or `Volts` | `Boxcar<S>` with `S: Samples + ?Sized` (F1 adapter borrowing `S`) | boxcar-filtered samples |

Select `Boxcar<Codes<'a>>` or `Boxcar<[f32]>` in the corresponding enum branch before entering the generic
kernel. The adapter's `len`, `at` and `extremes` describe the filtered sequence, using the existing HiRes window
and edge rules; `extremes` reduces filtered samples, not raw extremes. It borrows source and reused scratch,
with no decoded-channel copy or allocation after warm-up. Hardware HiRes needs no adapter. Min/max pairs
cannot recover a software boxcar sequence; apply software HiRes before pair compression.
F1 characterisation compares both adapters against the existing decoded HiRes result, including window edges.

### Threads

| Thread | Core | Owns | Runs |
|---|---|---|---|
| Acquisition | second A72 | hardware, buffer pool, `ReadPlan` | `read`; no decoding |
| Deposit pool: coordinator and three workers | four A53 | open `Deposit`, `Placed` arena | `place`, `paint_band`, newest-record decode |
| UI | first A72 | `Accumulator`, `Picture`, record store | `accept`, `replace`, `publish`, step |
| Measure, analysis, netio, GPU helper | as today | as today | unchanged |

The pool uses synchronous code on plain threads. Tokio stays in netio. Compute has no awaits: waits are channel
receive and DMA completion.

Per batch:

1. The coordinator receives `Arc<Batch>` on a bounded channel. It reserves owned tap storage, copies and queues the
   newest record, then selects projections using the [identity check](#projection-identity) before painting.
   A changed key or identity closes the open deposit first. Check the next whole batch against the
   [capacity bound](#difference-plane-capacity) before writing its first slice.
2. Place slices of at most 64 records. Workers claim indices with an atomic cursor and write their own `Placed`
   slot. The reused arena is 64 x 4 channels x 970 columns x 4 B, about 1 MiB.
3. Deposit each slice by tile: channels times column bands, with bands = ceil(4 / channels).
   Each band buffer moves to a worker and back. No two threads see the same band.
4. After the last slice, add the batch counts, per-map weights and oldest/newest read timestamps. If the UI slot is
   empty, move the deposit into it and open a recycled set. If full, keep accumulating within the per-plane hit
   bounds. At a bound, wait for hand-off space; the finite DMA pool propagates this pressure to acquisition.
   Never discard or split a batch to make space. Coalescing does not replace or merge newest-record taps.

Between batches, the coordinator waits on batch arrival, UI-slot availability and recycled-set returns together.
Taking the slot wakes the coordinator; returning a recycled set also wakes it. At each whole-batch boundary,
before claiming another batch or sleeping, recheck slot availability and hand off any nonempty open deposit when
space is available. A slot becoming free during painting is handled after that whole batch completes. Never
publish an empty deposit. Recheck readiness when registering the wait so a notification cannot be lost between
checking and sleeping. Publication must not require another batch or a capacity stall. If no recycled set is
available after hand-off, wait for its return before starting another batch; keep the five-set bound.

Batches coalesce when the UI is slow. The UI takes at most one deposit per step; coalescing never thins records.
The pool maps jobs over record indices and owned tiles, with owned or `Arc` arguments.
It replaces `fastdraw::together`, `par_map`, `par_each`, worker mailboxes and #186's `Reduction`, without `unsafe`.
The acquisition helper is removed in the first cut. Stateless placement allows later measured helper work without
private maps; it is not part of F0-F7.

Per UI step: apply the `ChangeSet`; mapping drops the window and requests preview, phosphor changes clear it.
Snapshot the queued newest-record tap count and process at most B taps, one at a time, independently of whether a
picture deposit is available or accepted. B comes from the read plan's [tap capacity](#tap-capacity).
Return each tap's storage before taking the next; never wait for a tap or drain beyond the snapshot count.
Take the slot; `accept` checks key, epoch, frozen state and minimum-persistence age and counts rejection reasons.
Blend once per channel over touched rows. Publish dirty rows. W replaces `vmax`; there is no maximum scan.

### Variants compose

| Variant | Representation or owner | Pipeline branch |
|---|---|---|
| 1-4 channels | `Projection::ch`; enabled planes only; channels x bands tiles | none |
| Normal | `SamplesRef::Codes` | none |
| Peak | `Codes` at maximum rate, or `Pairs` if words are min/max; column extremes either way | none |
| Live compression, deep memory | `SamplesRef::Pairs` | none |
| HiRes | hardware where available; otherwise `Boxcar<S>` as specified above | none |
| Hardware Average | plain FPGA-averaged records | none |
| Software Average | ordered one-thread source fold, one `Waveform` per step to `replace` | source |
| Roll, scan | sweep so far as screen-resolution `Pairs`, `replace(Sweep)` | source |
| Zoom | second projection and plane set in the same deposit | none |
| Dots | skip spans; one difference pair per sample in `paint_band` | kernel choice |
| Persistence | `Blend::Window { min_records, max_age }`, `Decay { tau }`, `Infinite` | policy value |
| STOP, SINGLE done | `Frozen`; incoming deposits counted as `rejected.frozen` | state |
| Replay, preview, stopped mapping change | same `place` and `paint_band` via UI `replace`, mode-specific W per map | none |
| Measurements, decoders, math, history | per-batch `Waveform` FIFO, `stats`, `MeasRead`; never draw | tap |
| Mask, zones, segments, Bus trigger, Single | every record required: pool builds `RecordBatch` instead of `Deposit` | route tag in `BatchMeta` |
| XY, LA, Z input | outside this phosphor path | out of scope |

Sample form, source and blend vary independently. `fast_ok` shrinks to the batch route tag.
Overlays are separate: compose draws cursor, marker, label and graticule command lists over `&Picture` (ADR 0026).

### Minimum persistence and GPU hand-off

Keep the 2026-11-14 window shape: newest whole units until they hold N records, at most 3 units, at most 67 ms old.
The owner changed the unit to `Deposit` on 2026-11-18. Three ring reads per picture could omit records when reads
outpace pictures; three deposits do not impose that read-count limit because the UI takes at most one per step.
`BatchSpan` retains the minimum and maximum `BatchMeta::read_at` of all contributing batches. Coalescing updates
both; a new batch never refreshes the age of older contents. Minimum persistence uses `now - oldest_read_at` at
acceptance and on every live window rebuild. If it exceeds 67 ms, exclude the whole deposit, even if its newest
batch is younger. A deposit expired before first acceptance counts all its records as `rejected.age`; eviction
after acceptance leaves those records counted once as `deposit.shown`. Frozen pictures keep their specified lifetime.

The accumulator folds up to three deposits' difference planes in one pass, subject also to the
[combined hit bound](#difference-plane-capacity). At high rates the window is one deposit.
Age, deposit-count and capacity caps take precedence over reaching N records.

F2 defines and maintains `Picture::dirty` and the dirty-row `publish` contract. M9b consumes that contract for
GPU dirty-range uploads; F6 reuses it for window expiry, replace and grade changes. GPU protocol is unchanged.
`publish` copies dirty rows instead of comparing every row to the arena copy. W replaces per-channel `vmax` in F6.
`Picture::dirty` is the covering hull of every changed row, including rows cleared when old deposits expire or
are evicted. For disjoint old and new trace bands, it includes both bands and the gap between them, preventing
ghosts. Publishing uploads the unchanged gap too; widely separated bands can therefore cost a full-height upload.
This is the explicit exception to proportional GPU work in [ADR 0031](../adr/0031-performance.md).

Issue #216 applies this publication contract to the current `Phosphor` before F2: mutations stamp per-channel rows,
and every output/window keeps its own last publication revision. It copies the hull once into the existing shared
arena. Map identity, channel order and arena layout changes force a full initial copy. This does not implement
F2's accumulator ownership, deposit window or W grading.

## Performance estimates

Performance budgets below are unverified targets; unit timing must establish feasibility. Capacity bounds above
are arithmetic requirements. The picture period is 33.3 ms.

| Stage | Estimated budget at four channels | Basis |
|---|---|---|
| `read` | as today | unchanged |
| `place` + `paint_band` | <= 0.5 ms CPU/record at 10 us/div, 35,160 samples/channel; <= 0.25 ms at 1 us/div | target for shared kernel, without f32 copies or scoped spawns |
| Pool load | <= 70 % per A53 at 1800 records/s; expected about 11 % | 1800 x 0.25 ms = 0.45 core across four cores |
| Coordinator | <= 50 us/batch | two channel hops per tile, no allocation |
| Minimum-persistence blend | <= 3 ms UI, full-height traces, three deposits | 10.5 MB read plus 3.5 MB Picture write; requires about 4.7 GB/s effective throughput |
| Decay blend | <= 4 ms UI, one new deposit | 3.5 MB deposit read plus 7 MB Picture read and 7 MB write; requires about 4.4 GB/s |
| `publish` | <= 1 ms | target for dirty-row copies |
| UI step p99 | <= 15 ms | existing drawing target |
| Record to picture | <= one picture period plus batch deposit time | slot hand-off |

Blend traffic assumes four channels at 970 x 448, main only, and all rows touched. Minimum persistence rebuilds
Picture from up to three retained 3.5 MB plane sets each step; one pass still reads all three. Decay folds only
the new deposit and updates the existing u32 16.16 Picture in place. The 3 ms and 4 ms budgets assume sequential
NEON folds sustain the effective throughputs above, including prefix sums and decay arithmetic. These are
unverified throughput requirements, not measurements; prototype timing must validate them before acceptance.
Zoom adds another plane set per deposit and another Picture, so its combined blend budget needs separate unit evidence.

At 70 % on four A53, the assumed 0.25 ms CPU/record implies about 11,000 records/s at 1 us/div.
This is estimated capacity, not acceptance evidence. 100 kHz needs 28 us/record over four cores. Only short dense
records might achieve it, with slice timing and no per-record allocations. Sparse interpolation cannot promise it.
100 kHz is a direction, not an acceptance requirement. Unsustainable rate must be visible as capture dead time.

The placement budget assumes raw-word extremes at any stride, without f32 copies or `CODES_MAX_STRIDE`.
Kernel work follows the [difference representation](#difference-plane-capacity) and
[owned-band dispatch](#threads); it avoids worker-map clearing and summing.

### Tap capacity

For each read plan, let T = 1/30 s and bound completed batches in any interval of length T by
B = ceil(R_batch_max * T) + P. `R_batch_max` is a conservative maximum batch rate from the plan's minimum
arm/read cycle, not an observed average or a record rate. P bounds an already-read burst; with three DMA buffers,
use P = 3. Account for batches of one or two records: dividing a record rate by a typical batch size is insufficient.
These plan bounds are not yet verified on hardware.

Size the FIFO to 2B entries and drain up to B at each UI step. The FIFO covers one period's queued taps plus
one period's arrivals while the UI processes them. Reserve two more waveform slots for the coordinator copying
and the records stage consuming, so total tap storage is at most 2B + 2 slots. For example, a plan bounded at
1800 batches/s has B = 63, a 126-entry FIFO and 128 total slots, rather than a 90-batches/s drain ceiling.
This bound supports UI step starts at most T apart; a slower UI may still cause deliberate source back-pressure.
Include processing B taps in the UI step budget and verify the M12 rate floor with one- and two-record batches.

`memplan` must compute B, the FIFO capacity, the per-step drain limit and the byte bound from each read plan before
admitting it. Charge `(2B + 2) * maximum decoded record bytes` to decoded copies, including metadata and container
capacity. Budget overlap with outstanding old-plan taps on reconfiguration; keep their drain limit until they return.
The memory sweep must cover these bounds and reject a plan that cannot fit, rather than silently reducing B.

The unconfigured acquisition keeps its legacy queue limits. The explicit `f2.stage=configured-evaluation`
startup profile requires `f2.envelope_bytes`, `f2.tap_bytes`, `f2.tap_entries` and `f2.min_cycle_ns`.
It also requires positive bounds for the other owners in `f2.window_graphics_bytes`, `f2.dma_bytes`,
`f2.shared_bytes`, `f2.copies_analysis_bytes` and `f2.runtime_bytes`. The Handoff charges decoded capacity
and control storage against the tap partition, including raw newest replay and retained old plans. It can
lengthen the enforced cycle to fit the current geometry, subject to the useful-record floors of the four-channel
1 and 10 us/div Auto plans. An unfunded or unsupported staged plan uses the existing record path and reports
the effective route.

These declarations check arithmetic and ownership at the Handoff. The device still needs measured
exclusive shares, DMA and graphics overlap bounds, independent copy and analysis bounds, and verification
of whole-step cost and capture rate. No boot profile is supplied.

### Memory estimates

| Buffer | Size at four channels, 970 x 448 | Bound |
|---|---|---|
| Difference plane set | 4 x 970 x 449 x 2 B, about 3.5 MB | per set |
| Plane sets | open, slot, three window entries | five sets, 17.4 MB; twice with zoom |
| `Placed` arena | 64 records x 15.5 kB | about 1 MiB |
| Picture | 3.5 MB u16 or 7 MB u32 with decay | one main set; zoom adds another |
| DMA buffers | pool of three | as today |
| Newest-record taps | owned samples plus metadata, no DMA leases | 2B + 2 waveform slots total; [plan-derived bound](#tap-capacity) |

A full tap FIFO makes the pool wait for a slot; it never overwrites an unread tap. The UI polls
without waiting and hands records to retaining owners under their existing budgets before returning tap storage.
This pressure reaches acquisition through the finite DMA pool, just like deposit-slot pressure.

Five difference-plane sets are allocated in total, including recycled sets and sets being handed off. Peak
occupancy is one open set, one occupied slot and three retained window deposits. The UI validates the slot
deposit before changing the window. If accepting it would exceed the window's count, age or capacity caps, the
UI evicts and recycles old entries before moving the accepted deposit from the slot into the window. For a full
window needing one eviction, acceptance leaves three window sets, one open set, an empty slot and one recycled
set. The pool can move the open set into that slot and reuse the recycled set as the next open set.
Rejected deposits return their set
directly. Hand-off moves ownership; it never clones planes or allocates a sixth set. Without a recycled set the
pool waits. The same five-set limit applies with zoom, with each set containing both main and zoom planes.

For comparison, main bounds five producers to five `Acc` each. #186 states 49.7 MiB plus a 3.3 MiB snapshot.
Allocate enabled channels only. The pool clears recycled planes over touched rows; the UI does not.
No stage allocates after warm-up. Extend the existing counting-allocator tests to cover that contract.

## Back-pressure and conservation

Acquisition reserves a free buffer before arming the next read, including an armed-ahead read. If none is free,
it waits on the pool return channel in the same event wait as commands. Triggers during the wait are never read.
That is reported dead time. There is no read-and-discard overload path.

The deposit route has this conservation rule, checked by a test and reported on the state line. All terms are
in the `deposit` namespace and count only deposit-route records. Unqualified `lost.*` and `rejected.*` on this
page and in [picture](picture.md) abbreviate `deposit.lost.*` and `deposit.rejected.*`:

```text
deposit.read = deposit.deposited + deposit.lost.settings + deposit.lost.first + deposit.lost.second + deposit.lost.error
deposit.deposited = deposit.shown + deposit.rejected.stale_key + deposit.rejected.epoch + deposit.rejected.frozen + deposit.rejected.age
```

Every term counts records. A record with no bucket is a bug. Account for settings boundaries, incomplete hardware
averages, un-pause and roll queue clears. Do not count a held window record again each step: `deposit.shown` counts records
on first picture acceptance. Queued/in-flight records are pending until a terminal bucket is assigned; assert the
identities after draining, or include pending counts in a live snapshot.
There is no `lost.overload`. Key, epoch, frozen and age rejections are deliberate. The `RecordBatch` route also
waits for capacity and propagates back-pressure to the source, as [queues](README.md#queues) requires. Its reads
are `records.read`, with pending records and intentional identity rejections accounted separately under `records.*`.
Each read record enters only its batch's route namespace; newest-record taps do not increment `records.read`.
Record-route reads never enter `deposit.read` or its identity. Neither route drops read records for overload.

On screen, show `deposit.shown` per second beside trigger status. When available, capture ratio is
`(deposit.read + records.read) / triggers`, using the trigger count from
`hal/src/dho/counter.rs`. That counter's operation during batch acquisition is not yet verified on hardware.
Otherwise show dead time as one minus armed wall-time share. Mark any growth in `lost.*` except `lost.first` in the last
second. This rate display is part of the target, not a claim about the current UI.

### Counters

One line per stage in `:SYSTem:STATe?`, p50/p90/p99 over the last 256 observations, using the quantile ring from #186
(`ui/src/perfguard/ring.rs` on that branch). Replace `framecost`'s per-record clock calls and five sums with slice
rings.

| Counter | Unit | Proves |
|---|---|---|
| `deposit.place` | us/record, timed per slice | placement budget |
| `deposit.tiles` | us/slice | deposit budget |
| `deposit.batch` | ms, `read_at` to slot | record-to-picture latency |
| `deposit.coalesced` | batches/deposit | UI keeps up; 1 is normal |
| `pool.busy` | percent/worker | 70 % bound |
| `acq.wait_buffer` | ms/read and count | source back-pressure |
| `picture.blend` | ms | fold budget |
| `picture.records` | records/step, including zero | empty pictures; #186's `draw.frames_per_picture` |
| `picture.age` | ms | existing age metric |
| `deposit.read`, `deposit.deposited`, `deposit.shown` | records | deposit-route progress and conservation |
| `deposit.lost.settings`, `deposit.lost.first`, `deposit.lost.second`, `deposit.lost.error` | records | losses before deposit |
| `deposit.rejected.stale_key`, `deposit.rejected.epoch`, `deposit.rejected.frozen`, `deposit.rejected.age` | records | rejection before first acceptance |
| `records.read`, `records.*` pending and rejection counters | records | separate RecordBatch accounting |

### M12 acceptance

The owner's requirement is 30 fps and zero drops at four channels, Normal, at the fastest time bases.
Source waiting makes a rate floor essential. The unit round must show:

- fps >= 29.5 and `picture.records` p10 > 0;
- `deposit.shown`/s at least main's acquired rate: about 1770/s at 1 us/div and 400/s at 10 us/div on the present source;
- `acq.wait_buffer` count zero at those rows;
- UI step p99 <= 15 ms and every pool worker <= 70 %;
- no overload losses, and all conservation buckets accounted for.

These are future acceptance rows, not measurements of the design. Golden pictures and `draw_routes` equality remain
required while Standard exists. [F0-F7](migration.md#f0-f7-frame-pipeline) specify scope, files, risk and dependencies.

## Disposition of #186

Replace it; do not merge it. Its picture worker, per-channel reducers and persistence snapshot exist to sum worker
maps. F2 removes those maps. Its second version is not verified on hardware, adds five threads to four loaded A53
cores and still moves about 20 MB per picture. Task 0013 records additions of 138-385 production lines.

Salvage in F0: `draw.frames_per_picture`, the quantile ring, `BATCH_PICTURE` perf-guard rows as the F2 baseline,
enabled-channel-only allocation, and the test that completion without new records keeps the visible picture.

## Owner decisions

Approved 2026-11-18:

1. Wait at the source. Reserve a buffer before read; do not read and drop under load. Show capture rate and require
   the M12 rate floor. Read-and-drop was rejected because it spends IO on discarded data and normalizes loss.
2. The minimum-persistence three-unit cap counts deposits, not ring reads. A ring-read cap can leave records out
   whenever several reads arrive per picture period.
3. Close #186 without merging, following its [disposition](#disposition-of-186). Do not spend another unit round
   accepting code that F2 removes.
4. Put Peak, pairs and zoom on the deposit path immediately after M12b, in F5. Accept the changed Peak look:
   per-column spans replace joined segments. First verify whether Peak words are plain samples or min/max.
   Current code decodes them as plain samples (`hal/src/dho/acq.rs`); that is not hardware verification.
5. Treat 100 kHz as a direction. Accept at rates the present source can generate, with measured headroom,
   back-pressure and capture ratio. A 100 kHz requirement would need a bench source and another overhead pass.

All performance budgets, hardware counter behaviour and Peak word form remain not yet verified on hardware.
