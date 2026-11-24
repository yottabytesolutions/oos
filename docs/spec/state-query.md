# :SYSTem:STATe? response schema

See the [frame counter reference](../architecture/frame-counters.md) for the six versioned frame pipeline groups.

Source baseline: merged `overhaul` c59b71d5, 2026-11-16. Extracted from source, not
verified on hardware in this task. This is the current diagnostic wire format
and the coordinator-decided version boundary for M15/unitlab. It does not
change any producer.
The SCPI dispatcher calls `cmd::stat_text(ui).trim()`
(`ui/src/scpi.rs:2333`); that producer is shared with the control FIFO `stat`
command (`ui/src/cmd.rs:140`). Transport framing is in [scpi.md](scpi.md).

Today the answer is a labelled, whitespace-delimited diagnostic line, **not
JSON** and not uniformly key=value. Literal parentheses, percent signs, slashes
and section labels are part of the wire syntax. There is no version field today.
Rows below use dotted names only to identify positions unambiguously: the dots
are not wire keys. Source means the actual model member, counter or diagnostic
window, rather than a presumed future metric. Extracted fields have keep
disposition except transitional shadow fields state-query-83..101 and 112,
which are removed with M2.3e and excluded from version 1. The version/stability
requirements at the end describe the M15 target, not today's producer.

The envelope, in producer order, is:

```text
fps F frames N records N run R status S t T step X ms (acq A draw D)
render X ms present X ms edges A B C D off SUM MAX
parts meas X analyse X pane X max X ms
[gpu layers X copy X wait X (upload X draw X read X) ms]
route=ROUTE interp MODE acquired N used N dropped N measured N compress k K
deep reads N dma X decode X record X measure X ms (N pts)
[gpu panel X [monitor X] ms]
pacing ... picture ... settle ...
rss BYTES_OR_UNKNOWN watchdog STATE [gpu_failures N gpu_restarts N gpu_retry STATE]
persistence on_OR_off last_crash none_OR_TIME_AND_TEXT
shadow acq=N mapping=N phosphor=N stats=N mask=N zone=N hw_average=N
history=N math=N view=N normalize=N outbox=N errors=N unreviewed=N
Full=N PayloadTooLarge=N SnapshotTooLarge=N DuplicateToken=N GenerationOverflow=N
[gen=N epoch=N stats_gen=N [resolved=REQUESTER/RUN/STAGE:EPOCH ...]]
stage intake ... stage change ... [stage intake+change ...]
stale_worker_push=N stale_ui_publish=N stale_queue_pop=N stale_pipe=N
queue_overflow=N fold_drops=N
acq_seg_start=N acq_seg_abort=N acq_roll_stop=N acq_roll_start=N
acq_batch_prearm=N acq_batch_disarm=N acq_configure=N acq_single_arm=N
stage_restart_autoset=N stage_restart_bode=N
diag intake.p50 ... diag intake.p99 ... diag poll.memory ... [other diag blocks]
graphics gpu_OR_cpu hdmi ...
[hw frames N dropped N fast X ms (place X draw X of it fold X, N frames a call) take N map WIDTHxHEIGHT]
```

Line breaks above are editorial; the response is one line except that unescaped
free text can contain spaces. `last_crash` is followed by the bounded crash
header, not a single token. Whitespace may double when an optional block is
absent. Section context distinguishes repeated labels: acq, frames, read,
record, step, max, ms, n and p99_ns. Existing external stress consumers also
match global regexes; their exact matching behavior is a compatibility
requirement (state-query-203), not a reason to require a parser rewrite.

Numeric notation: `f1` is decimal float printed with one fractional digit;
`f0` is a rounded float printed with zero fractional digits; `u64` is decimal
unsigned integer unless noted. No field includes an SI prefix. Timings in ms
and ns must not be interchanged. Main UI timing estimates are smoothed, not
histogram quantiles; atomic fields are loaded independently with relaxed order,
so the whole diagnostic answer is not an atomic multi-counter snapshot.


## Disarm counters

The current reply includes the following section. Each field is a saturating process-lifetime
decimal counter.

```text
disarm.v1 total=N unknown=N no_completed_record=N recorded=N dropped=N
```

| Field | Meaning |
|---|---|
| `total` | Batch disarms observed, the saturating sum of the three outcomes. |
| `unknown` | Disarms with missing or invalid status, a register read error, or unconfirmed STOPPED. |
| `no_completed_record` | Confirmed disarms whose post-pulse play-info word has DONE clear. The pulse can clear DONE after a record completes. |
| `recorded` | Confirmed disarms whose post-pulse play-info word has DONE set. |
| `dropped` | Retained batches discarded because their newest slot was inside the programmed capture range. This includes failed captures that needed no disarm. |

The range rule is independent of the reported outcome. Older matrix reports can omit the section entirely.

## UI summary and graphics timing

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-1 | `fps` | f1, pictures/s | ui.fps; recent presentation-loop rate, timers adapter updates it | `ui/src/system/device/timers.rs:80` |
| state-query-2 | `frames` | u64, UI frames | ui.frames; increments per layer composition/render, distinct from acquired waveform frames | `ui/src/ui.rs:3689` |
| state-query-3 | `records` | u64, records | ui.records; records taken into the UI record path | `ui/src/ui.rs:3126` |
| state-query-4 | `run` | enum Run/Stop | ui.m.run | `core/src/model/display.rs:57` |
| state-query-5 | `status` | enum Triggered/Auto/Wait/Stop | ui.m.status; differs from the abbreviations of TRIG:STAT? | `core/src/model/display.rs:63` |
| state-query-6 | `t` | f1, s | ui.now; UI session elapsed time, not Unix time | `ui/src/cmd.rs:192` |
| state-query-7 | `step` | f1, ms | ui.t_step; exponential smoothing factor 0.05 of step duration | `ui/src/system/device.rs:122` |
| state-query-8 | `(acq` | f1, ms | ui.t_acq; source intake/record conversion part of step | `ui/src/ui.rs:1422` |
| state-query-9 | `draw)` | f1, ms | ui.t_draw; phosphor-map drawing part of step | `ui/src/ui.rs:1424` |
| state-query-10 | `render` | f1, ms | ui.t_render; rendering duration | `ui/src/ui.rs:1425` |
| state-query-11 | `present` | f1, ms | ui.t_present; presentation duration | `ui/src/ui.rs:1426` |
| state-query-12 | `edges[0]` | u64, frames | ui.fast_edges[0]; edge found within col+16 samples of hardware pretrigger; analog fast path only | `ui/src/drawpipe.rs:122` |
| state-query-13 | `edges[1]` | u64, frames | ui.fast_edges[1]; edge farther away with nearest edge of wrong direction (unless Either slope); analog fast path only | `ui/src/drawpipe.rs:122` |
| state-query-14 | `edges[2]` | u64, frames | ui.fast_edges[2]; other found edge farther away; analog fast path only | `ui/src/drawpipe.rs:122` |
| state-query-15 | `edges[3]` | u64, frames | ui.fast_edges[3]; no refined edge found; analog fast path only | `ui/src/drawpipe.rs:122` |
| state-query-16 | `off[0]` | f0, samples | ui.fast_off.0; sum of absolute refined-edge offsets for edges[2] | `ui/src/drawpipe.rs:128` |
| state-query-17 | `off[1]` | f0, samples | ui.fast_off.1; maximum of those offsets | `ui/src/drawpipe.rs:129` |
| state-query-18 | `parts.meas` | f1, ms | ui.t_parts[0]; measurements; smoothed step part | `ui/src/ui.rs:1431` |
| state-query-19 | `parts.analyse` | f1, ms | ui.t_parts[1]; analysers and decoders; smoothed step part | `ui/src/ui.rs:1431` |
| state-query-20 | `parts.pane` | f1, ms | ui.t_parts[2]; second pane; smoothed step part | `ui/src/ui.rs:1431` |
| state-query-21 | `parts.max` | f1, ms | ui.t_parts[3]; brightest-pixel scans; smoothed step part | `ui/src/ui.rs:1431` |
| state-query-22 | `gpu.layers` | optional f1, ms | ui.t_gpu[0]; last panel helper frame layer composition timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-23 | `gpu.copy` | optional f1, ms | ui.t_gpu[1]; last panel helper frame copy timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-24 | `gpu.wait` | optional f1, ms | ui.t_gpu[2]; last panel helper frame wait timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-25 | `gpu.upload` | optional f1, ms | ui.t_gpu[3]; last panel helper frame upload timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-26 | `gpu.draw` | optional f1, ms | ui.t_gpu[4]; last panel helper frame draw timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-27 | `gpu.read` | optional f1, ms | ui.t_gpu[5]; last panel helper frame readback timing; block absent if t_gpu=None | `ui/src/cmd.rs:143` |
| state-query-28 | `route=` | enum batch/batch-funded/records/stopped/replay | current picture route | `ui/src/ui/step_draw.rs` |
| state-query-29 | `interp` | enum Exact/Fast | switches.interp; sparse interpolation policy | `ui/src/sparse.rs:161` |
| state-query-30 | `acquired` | u64, frames | drawpipe::Counters.acquired; process aggregate of frames acquired | `ui/src/drawpipe.rs:320` |
| state-query-31 | `used` | u64, frames | drawpipe::Counters.used; process aggregate of frames drawn into picture | `ui/src/drawpipe.rs:320` |
| state-query-32 | `dropped` | u64, frames | drawpipe::Counters.dropped; legacy overload and stale-work discard aggregate | `ui/src/drawpipe.rs:320` |
| state-query-33 | `measured` | u64, frames | drawpipe::Counters.measured; process aggregate of frames whose cheap measurements entered statistics | `ui/src/drawpipe.rs:320` |
| state-query-34 | `compress.k` | u32, dimensionless | compress::last_k(); last FPGA compression factor | `ui/src/compress.rs:45` |
| state-query-35 | `deep.reads` | u64, reads | deepmeas::cost::READS; accumulated raw deep rereads | `ui/src/deepmeas.rs:30` |
| state-query-36 | `deep.dma` | f1, ms | deepmeas::cost::DMA_US/1000; last DMA duration, not lifetime total | `ui/src/deepmeas.rs:42` |
| state-query-37 | `deep.decode` | f1, ms | deepmeas::cost::DECODE_US/1000; last decode duration, not lifetime total | `ui/src/deepmeas.rs:42` |
| state-query-38 | `deep.record` | f1, ms | deepmeas::cost::RECORD_US/1000; last screen-record creation duration, not lifetime total | `ui/src/deepmeas.rs:42` |
| state-query-39 | `deep.measure` | f1, ms | deepmeas::cost::MEASURE_US/1000; last worker measurement duration, not lifetime total | `ui/src/deepmeas.rs:42` |
| state-query-40 | `deep.(pts)` | u64, samples | deepmeas::cost::MEASURE_N; samples of last measurement job, all channels | `ui/src/deepmeas.rs:35` |
| state-query-41 | `gpu.panel` | optional f1, ms | ui.t_gpu_screens.0; helper time of last panel frame | `ui/src/cmd.rs:168` |
| state-query-42 | `gpu.monitor` | optional f1, ms | ui.t_gpu_screens.1 when Some; helper time of last monitor frame; monitor is every second frame | `ui/src/cmd.rs:167` |

`route=` includes `batch-funded` when the staged profile has admitted the current geometry or `records` while
admission is unavailable. Batch deposits have a separate newest-record tap for analysis and history.

## Pacing, composition and frame costs

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-43 | `pacing.pics` | usize, pictures | State.pics.len(), at most 300 recent observations | `ui/src/pacing.rs:18` |
| state-query-44 | `pacing.new` | f0/f0/f0, new frames per picture | p10/p50/p90 of State.pics frame counts | `ui/src/pacing.rs:374` |
| state-query-45 | `pacing.empty` | f0 with %, percent | 100 * zero-new-frame pictures / max(1,pics.len()) | `ui/src/pacing.rs:376` |
| state-query-46 | `pacing.gap` | f1/f1/f1/f1, ms | p50/p90/p99/max gaps between pictures with new frames; excludes empty ones | `ui/src/pacing.rs:366` |
| state-query-47 | `pacing.pass` | f1/f1/f1/f1, ms | p50/p90/p99/max frame-loop intervals; skip first interval | `ui/src/pacing.rs:379` |
| state-query-48 | `pacing.step` | f1/f1/f1/f1, ms | p50/p90/p99/max step duration from State.passes[*][1] | `ui/src/pacing.rs:380` |
| state-query-49 | `pacing.batch.wait` | f1/f1/f1/f1, ms | p50/p90/p99/max arm/wait-to-read-done durations State.batches[*].0 | `ui/src/pacing.rs:381` |
| state-query-50 | `pacing.batch.every` | f1/f1/f1/f1, ms | p50/p90/p99/max batch intervals; first skipped | `ui/src/pacing.rs:382` |
| state-query-51 | `pacing.batch.frames.p50` | f0, frames/batch | median State.batches[*].2 actual frame count | `ui/src/pacing.rs:383` |
| state-query-52 | `pacing.batch.full` | f0 with %, percent | fraction with actual count >= requested count | `ui/src/pacing.rs:384` |
| state-query-53 | `pacing.fill.p50` | f0, ms | median planned ring fill time State.fills | `ui/src/pacing.rs:385` |
| state-query-54 | `pacing.until_fill_end` | f1/f1/f1/f1, ms | p50/p90/p99/max zipped (batch wait - read duration); includes arming, first-trigger wait and fill | `ui/src/pacing.rs:388` |
| state-query-55 | `pacing.read` | f1/f1/f1/f1, ms | p50/p90/p99/max fill-end-to-read-done State.reads | `ui/src/pacing.rs:387` |
| state-query-56 | `pacing.drawn` | f1/f1/f1/f1, ms | p50/p90/p99/max read-to-workers-ready State.readies | `ui/src/pacing.rs:386` |
| state-query-57 | `pacing.ui.fast` | f1/f1/f1/f1, ms | p50/p90/p99/max queued-frame draw duration State.fast[*][0] | `ui/src/pacing.rs:390` |
| state-query-58 | `pacing.ui.merge` | f1/f1/f1/f1, ms | p50/p90/p99/max UI pipe merge duration State.fast[*][1] | `ui/src/pacing.rs:391` |
| state-query-59 | `pacing.views` | u64, view mappings | pacing::VIEWS; increments for new deposit-worker screen mappings | `ui/src/pacing.rs:108` |
| state-query-60 | `picture.deposit` | f0/f0/f0, frames | p10/p50/p90 deposit-kernel frames per composition | `ui/src/pacing.rs:408` |
| state-query-61 | `picture.record` | f0/f0/f0, frames | p10/p50/p90 record-path frames per composition | `ui/src/pacing.rs:409` |
| state-query-62 | `picture.weight` | f0/f0/f0, weighted frames | p10/p50/p90 of deposit + 3.7 * record for each composition | `ui/src/pacing.rs:410` |
| state-query-63 | `picture.age` | f1/f1/f1/f1, ms | p50/p90/p99/max newest merged batch age; entries with no age omitted | `ui/src/pacing.rs:412` |
| state-query-64 | `picture.stale` | u64, frames | pacing::STALE; obsolete frames actually reaching picture; distinct from rejected stale_* counters | `ui/src/pacing.rs:130` |
| state-query-65 | `picture.partial` | u64, batches | pacing::PARTIAL; batches whose frames reached the picture in more than one merge | `ui/src/pacing.rs:127` |
| state-query-66 | `picture.wanted` | u64, generation | pacing::WANTED; metrics mirror of published identity.gen, not an identity authority | `ui/src/ui/change/acquisition.rs:54` |
| state-query-67 | `picture.applied` | u64, generation | pacing::APPLIED; metrics mirror of worker-applied want_gen | `ui/src/board/dho/hw.rs:1881` |
| state-query-68 | `settle.configured/first/steady ms/empty` | zero..4 space-separated tuples, or - | Most recent first. Each tuple configured/first/steady/empty: rounded ms from settings change for first three, -1 if missing; final u32 empty-picture count. State.settled, ten new pictures in a row defines steady | `ui/src/pacing.rs:392` |
The per-record cumulative cost fields (state-query-69..75) are retired in F0 (#202).
Their clock reads are replaced by bounded claim-level rings described below. This is a version 0
migration; the append-only version 1 contract has not started.

Pacing windows retain at most 300 entries of each kind, independently; samples
are not necessarily aligned across windows (`ui/src/pacing.rs:18`, `:72`). The
percentile helper returns zero for no observations and otherwise sorts and uses
rounded `(len-1)*q` rank (`ui/src/pacing.rs:357`). Durations are sampled with
Instant, not the UI's `t`. These are diagnostic windows, not lifetime counters.


## Memory, helper recovery and persistent process state

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-76 | `rss` | u64 bytes or unknown | Watchdog.rss, latest RSS sample; bytes, not KiB or MiB | `ui/src/system/memory.rs:105` |
| state-query-77 | `watchdog` | enum ok/soft/recovering/stopped/exit75 | Watchdog state; precedence exit, recovering, stopped, warned, ok | `ui/src/system/memory.rs:105` |
| state-query-78 | `gpu_failures` | optional u64, events | gpu::recovery::Recovery.failures, helper failures in current recovery object | `ui/src/gpu/recovery.rs:16` |
| state-query-79 | `gpu_restarts` | optional u64, attempts | Recovery.restarts, retries actually taken | `ui/src/gpu/recovery.rs:22` |
| state-query-80 | `gpu_retry` | optional enum waiting/exhausted/idle | Recovery.retry and attempt budget; host runtime gpu status string may be empty before graphics reports | `ui/src/gpu/recovery.rs:41` |
| state-query-81 | `persistence` | enum on/off | Runtime.persistence; setup persistence status | `ui/src/ui/lifecycle.rs:30` |
| state-query-82 | `last_crash` | none or timestamp + header text | Runtime.crash; timestamp YYYYMMDDTHHMMSSZ and first bounded Panic/Oops line, may contain spaces; startup snapshot, not live counter | `ui/src/store/startup/crash.rs:14` |

Runtime concatenates watchdog, helper text, persistence and crash status
(`ui/src/ui/lifecycle.rs:30`). Recovery counters can reset when graphics
preference changes (`ui/src/system/device/graphics.rs:50`); do not treat them as
acquisition counts. The crash reader reads at most 256 bytes and strips control
characters (`ui/src/store/startup/crash.rs:59`). Text is unquoted, so consumers
locate the following `shadow` section with care and must not invent fixed token
positions after `last_crash`. The shadow section is transitional and absent
from version 1; it is not a permanent delimiter for crash text.


## Change-point shadow diagnostics and identity

**Transitional, removed at M2.3e:** state-query-83..101 (the entire `shadow`
block, including its error counts) and state-query-112 (`shadow_p99_ns=`) are
legacy migration diagnostics. Their table entries describe version 0 only;
none is part of version 1. M2.3e removes the old mechanisms and shadow tracking
([migration](../architecture/migration.md), M2.3e). The identity fields
102..105 and non-shadow stage timings 106..111 have keep disposition.

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-83 | `shadow.acq=` | u64, mismatches | Diagnostics.raw[0]; number of comparisons where new/legacy class acq membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-84 | `shadow.mapping=` | u64, mismatches | Diagnostics.raw[1]; number of comparisons where new/legacy class mapping membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-85 | `shadow.phosphor=` | u64, mismatches | Diagnostics.raw[2]; number of comparisons where new/legacy class phosphor membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-86 | `shadow.stats=` | u64, mismatches | Diagnostics.raw[3]; number of comparisons where new/legacy class stats membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-87 | `shadow.mask=` | u64, mismatches | Diagnostics.raw[4]; number of comparisons where new/legacy class mask membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-88 | `shadow.zone=` | u64, mismatches | Diagnostics.raw[5]; number of comparisons where new/legacy class zone membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-89 | `shadow.hw_average=` | u64, mismatches | Diagnostics.raw[6]; number of comparisons where new/legacy class hw_average membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-90 | `shadow.history=` | u64, mismatches | Diagnostics.raw[7]; number of comparisons where new/legacy class history membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-91 | `shadow.math=` | u64, mismatches | Diagnostics.raw[8]; number of comparisons where new/legacy class math membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-92 | `shadow.view=` | u64, mismatches | Diagnostics.raw[9]; number of comparisons where new/legacy class view membership differs, including approved differences | `ui/src/ui/change/diagnostics.rs:56` |
| state-query-93 | `shadow.normalize=` | u64, events | Diagnostics.normalized; comparisons where normalized shadow/legacy model disagree | `ui/src/ui/change/diagnostics.rs:101` |
| state-query-94 | `shadow.outbox=` | u64, events | Diagnostics.outbox; legacy/shadow staged proposal disagreement count | `ui/src/ui/change/diagnostics.rs:101` |
| state-query-95 | `shadow.errors=` | u64, events | Diagnostics.errors; total change errors, not SCPI error-queue length | `ui/src/ui/change/diagnostics.rs:101` |
| state-query-96 | `shadow.unreviewed=` | u64, events | Diagnostics.unreviewed; class differences without a reviewed oracle allowance | `ui/src/ui/change/diagnostics.rs:101` |
| state-query-97 | `shadow.Full=` | u64, errors | Diagnostics.error_counts for Error::Full; pending command list capacity exceeded | `ui/src/ui/change/diagnostics.rs:32` |
| state-query-98 | `shadow.PayloadTooLarge=` | u64, errors | Diagnostics.error_counts for Error::PayloadTooLarge; command payload bound exceeded | `ui/src/ui/change/diagnostics.rs:32` |
| state-query-99 | `shadow.SnapshotTooLarge=` | u64, errors | Diagnostics.error_counts for Error::SnapshotTooLarge; settings snapshot bound exceeded | `ui/src/ui/change/diagnostics.rs:32` |
| state-query-100 | `shadow.DuplicateToken=` | u64, errors | Diagnostics.error_counts for Error::DuplicateToken; duplicate stage request token | `ui/src/ui/change/diagnostics.rs:32` |
| state-query-101 | `shadow.GenerationOverflow=` | u64, errors | Diagnostics.error_counts for Error::GenerationOverflow; identity generation/epoch arithmetic overflow | `ui/src/ui/change/diagnostics.rs:32` |
| state-query-102 | `gen=` | optional u64, identity | observer.last.identity().gen(); hardware acquisition settings generation; absent before first publication | `ui/src/ui/change.rs:241` |
| state-query-103 | `epoch=` | optional u64, identity | observer.last.identity().epoch(); acquisition request epoch, including equal-settings rearm; absent before first publication | `ui/src/ui/change.rs:241` |
| state-query-104 | `stats_gen=` | optional u64, identity | observer.last.identity().stats_gen(); measurement statistics generation; absent before first publication | `ui/src/ui/change.rs:241` |
| state-query-105 | `resolved=` | repeated optional Requester/u64/u64:u64 | one per latest ChangeSet resolution: requester Autoset/Bode, run token, stage token, resolved acquisition epoch; not a lifetime list | `ui/src/ui/change.rs:244` |
| state-query-106 | `stage.intake.n=` | usize 0..256, samples | StageTiming.samples(); paired intake/change rolling sample count | `ui/src/perfguard.rs:141` |
| state-query-107 | `stage.intake.p99_ns=` | optional u64, ns | StageTiming.p99()[0], nearest-rank p99 intake incl queued intake | `ui/src/perfguard.rs:127` |
| state-query-108 | `stage.change.n=` | usize 0..256, samples | same paired StageTiming.samples() | `ui/src/perfguard.rs:147` |
| state-query-109 | `stage.change.p99_ns=` | optional u64, ns | StageTiming.p99()[1], nearest-rank p99 change duration | `ui/src/perfguard.rs:127` |
| state-query-110 | `stage.change.p99=pending` | optional literal pending | printed after stage change until >=50 samples; replaces quantile fields and combined stage block | `ui/src/perfguard.rs:155` |
| state-query-111 | `stage.intake+change.p99_ns=` | optional u64, ns | StageTiming.p99()[2], quantile of paired sums, not sum of quantiles | `ui/src/perfguard.rs:126` |
| state-query-112 | `stage.intake+change.shadow_p99_ns=` | optional u64, ns | StageTiming.shadow_p99(), quantile of shadow overhead, shown after >=50 samples | `ui/src/perfguard.rs:117` |

The class names come from `core/src/change/classes.rs:31`; they are case-sensitive
wire keys. The normalizing and staged-proposal comparisons increment their
separate counts in `ui/src/ui/change.rs:351` and `ui/src/ui/change.rs:228`. Errors quarantine requests and keep
the last publication (`ui/src/ui/change/diagnostics.rs:32`). Generations identify
different contracts and cannot be substituted for each other or for pacing
wanted/applied metrics mirrors. Stage token types are `core/src/change/requests.rs:8`.


## Rejected work and acquisition lifecycle counters

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-113 | `stale_worker_push=` | u64, frames | drawpipe::Counters.losses.worker_push; frames rejected under worker publication lock by current identity before handoff | `ui/src/board/dho/publication.rs:64` |
| state-query-114 | `stale_ui_publish=` | u64, frames | drawpipe::Counters.losses.ui_publish; queued frames removed at UI acquisition publication because identity is obsolete | `ui/src/ui/change/acquisition.rs:32` |
| state-query-115 | `stale_queue_pop=` | u64, frames | drawpipe::Counters.losses.queue_pop; obsolete frames rejected while draining/popping UI source queue | `ui/src/wave.rs:587` |
| state-query-116 | `stale_pipe=` | u64, frames | drawpipe::Counters.losses.pipe; obsolete frames rejected at deposit pipe admission/worker/merge sites | `ui/src/drawpipe.rs:696` |
| state-query-117 | `queue_overflow=` | u64, frames | drawpipe::Counters.losses.overflow; oldest frames evicted beyond QUEUE_MAX; independent of stale identity | `ui/src/board/dho/publication.rs:24` |
| state-query-118 | `fold_drops=` | u64, measurement samples | drawpipe::Counters.losses.folds; measurement samples discarded from invalid statistics-generation folds, sum of fold.n, not number of folds | `ui/src/drawpipe.rs:1920` |

All six wire names are serialized by `ui/src/drawpipe/losses.rs:17` and appended
by `ui/src/ui/change.rs:249`. They are independent reasons/sites, not disjoint
parts of one conservation equation. `queue_overflow` also increments aggregate
`dropped` (`ui/src/board/dho/publication.rs:43`). `fold_drops` deliberately counts
samples while stale identity counts frames. `picture stale` counts what reached
the screen, whereas these stale_* counters count rejection. Atomics are process
lifetime, initialized to zero, not reset by CLEAR or acquisition changes;
ordinary losses use fetch_add (u64 wrapping at overflow), unlike saturating
acquisition attempts (`ui/src/drawpipe.rs:343`).

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-119 | `acq_seg_start=` | saturating u64, attempts | AcquisitionCounters.events[Event::SegStart]; seg::start operation attempts; failures included | `ui/src/board/dho/lifecycle.rs:29` |
| state-query-120 | `acq_seg_abort=` | saturating u64, attempts | AcquisitionCounters.events[Event::SegAbort]; seg::stop attempts only with abort=true; normal completed-stop is excluded; failures included | `ui/src/board/dho/lifecycle.rs:39` |
| state-query-121 | `acq_roll_stop=` | saturating u64, attempts | AcquisitionCounters.events[Event::RollStop]; roll::stop calls; failures included | `ui/src/board/dho/lifecycle.rs:50` |
| state-query-122 | `acq_roll_start=` | saturating u64, attempts | AcquisitionCounters.events[Event::RollStart]; roll::start calls; failures included | `ui/src/board/dho/lifecycle.rs:45` |
| state-query-123 | `acq_batch_prearm=` | saturating u64, attempts | AcquisitionCounters.events[Event::BatchPrearm]; prearm wrapper calls, whether Single or continuous; failures included | `ui/src/board/dho/lifecycle.rs:56` |
| state-query-124 | `acq_batch_disarm=` | saturating u64, attempts | AcquisitionCounters.events[Event::BatchDisarm]; disarm wrapper calls; failures included | `ui/src/board/dho/lifecycle.rs:64` |
| state-query-125 | `acq_configure=` | saturating u64, attempts | AcquisitionCounters.events[Event::Configure]; seq::State.configure attempts; failures included | `ui/src/board/dho/lifecycle.rs:12` |
| state-query-126 | `acq_single_arm=` | saturating u64, attempts | AcquisitionCounters.events[Event::SingleArm]; Single arms: prearm(single=true) and batch(single=true,ahead=false); ahead batch does not double count; failures included | `ui/src/board/dho/lifecycle.rs:56` |

The eight attempt counters belong to task 0061/M2.3c and the acquisition worker.
Exact wire order is `ui/src/board/counters.rs:30`; event mapping is at `:12`.
Each counter saturates at u64::MAX, is monotonic for the process lifetime and
survives RUN/STOP, CLEAR, equal settings and generation/epoch changes. They count
attempts at the operation call site, including failures; they do not count
successful recordings, register writes, frames or settings generations. A single
request can cause several operation attempts. Host fakes use the same wrappers;
zero does not prove missing hardware or an idle UI. SINGLE also increments at
`ui/src/board/dho/lifecycle.rs:69` when not prearmed. Never sum `acq_single_arm`
with `acq_batch_prearm` as disjoint operations.

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-127 | `stage_restart_autoset=` | u64, stage restarts | proposals::Stages.restarts[0]; Autoset stage restarted when acknowledgement identity is superseded or no stage token remains | `ui/src/ui/change/proposals.rs:94` |
| state-query-128 | `stage_restart_bode=` | u64, stage restarts | proposals::Stages.restarts[1]; Bode stage restarted for that same condition | `ui/src/ui/change/proposals.rs:94` |

## Output selection

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-129 | `graphics` | enum gpu/cpu | ui.sys.gpu requested preference, distinct from optional actual GPU timing block | `ui/src/cmd.rs:171` |
| state-query-130 | `hdmi` | off or WIDTHxHEIGHT MODE | ui.hdmi_out dimensions in pixels and ui.m.hdmi debug name Mirror/Waveform/Layout; mode absent when off | `ui/src/cmd.rs:166` |

## Merged hardware fast-path suffix

Added by upstream and present at c59b71d5. Existing state-query-1..130 retain
their IDs, wire shapes and optional conditions. New fields are 131..140; none
of the old fields was removed by this merge. The suffix follows HDMI and is
present only when Source::hw_frames returns Some, so non-hardware sources
omit it; retained references do not themselves decide presence. It is unversioned like the rest of the current diagnostic line.

| Requirement | Wire field / position | Type and unit | Meaning and source | Evidence |
|---|---|---|---|---|
| state-query-131 | `hw frames` | u64, frames | ui.src.hw_frames().0 = Shared.decoded; board-decoded frame count, independent of UI frames | `ui/src/cmd.rs:183` |
| state-query-132 | `hw dropped` | u64, frames | ui.src.hw_frames().1 = Shared.dropped; board unseen frame losses, not aggregate pipeline dropped | `ui/src/cmd.rs:183` |
| state-query-133 | `hw fast` | f1, ms | ui.t_fast, smoothed fast-path time | `ui/src/cmd.rs:183` |
| state-query-134 | `hw (place` | f1, ms | ui.t_fast_ab.0, smoothed placement time | `ui/src/cmd.rs:183` |
| state-query-135 | `hw draw` | f1, ms | ui.t_fast_ab.1, smoothed drawing time | `ui/src/cmd.rs:183` |
| state-query-136 | `hw of it fold` | f1, ms | ui.t_fast_fold, smoothed statistics fold time | `ui/src/cmd.rs:183` |
| state-query-137 | `hw frames a call` | f0, frames/call | ui.fast_n, smoothed per-call frame count rounded to zero decimals | `ui/src/cmd.rs:183` |
| state-query-138 | `hw take` | usize, frames | ui.fast_take as usize; current adaptive take value, not a cumulative count | `ui/src/cmd.rs:183` |
| state-query-139 | `hw map width` | usize, pixels | ui.ph.w current picture map width | `ui/src/cmd.rs:183` |
| state-query-140 | `hw map height` | usize, pixels | ui.ph.h current picture map height | `ui/src/cmd.rs:183` |

Only the first frames/dropped pair are counters. Timings/count averages use the
UI fast path's smoothing; map dimensions and take are current values. Producers
are `ui/src/ui.rs:2956`, `ui/src/ui.rs:3528`, and `ui/src/wave.rs` hw_frames.
The suffix's repeated frames/dropped labels add matches to global regexes.
state-query-203 now compares the complete merged first-match and find-all
capture lists, including the appended match. This is a version 0 addition;
append-only stability still begins at M15 version 1.

## Separate HTTP workflow state used by the upstream validator

`tools/scpi_validate.py:110` state() reads GET /ui-state, **not** :SYST:STATe?.
The JSON comes from Ui::workflow_state (`ui/src/ui/workflow.rs`), serialized
with crate::json::J. It is not the labelled diagnostic line and has no
state_version key. These added IDs document that separate surface; they do
not renumber or replace diagnostic fields. All eight keys are emitted, in the
following producer order. Strings are JSON escaped, booleans are JSON bools,
and captures is converted from its integer source to an f64 JSON number.

| Requirement | JSON key | Type / units | Source and meaning |
|---|---|---|---|
| state-query-300 | mode | string live/reference/sim | session_is_sim first, then record_mode, else live |
| state-query-301 | running | boolean | m.run==Run |
| state-query-302 | busy | boolean | sim_generating, background generation pending |
| state-query-303 | measurementBusy | boolean | measurement_busy, background measurement pending |
| state-query-304 | source | string Rn or CHn, one-based | active_reference in record mode, else m.active |
| state-query-305 | captures | JSON number, captures | replay.captures as f64; replay counter, not diagnostic frames/records |
| state-query-306 | status | string TRIGGERED/WAIT/AUTO/STOP | m.status; SCPI TRIG:STAT? instead overrides Stop and reports TD for Triggered |
| state-query-307 | gate | string full-record/screen | m.meas_full_record |

The validator only
checks a subset (captures/running/status); it does not verify the diagnostic
schema, loss counters, identity, optional blocks or regex compatibility.

## Version and stability contract

**state-query-200 (change; coordinator decision): define version 1 at M15.**
Version 0 is today's answer without a version key, including transitional
migration diagnostics and the merged optional hardware fast-path suffix. M2.3e
removes state-query-83..101 and 112 before version 1 is defined; do not freeze those fields into the public schema. At M15, append
`state_version=1` to the surviving diagnostic line before its transport LF,
without wrapping, reordering or renaming its surviving fields. The field is a
decimal u32 schema identifier, not firmware or acquisition generation. It is not
emitted by c59b71d5. Coordinate the producer and consumer update at M15. The
supplied external-consumer patterns and additional format-derived fixtures in
state-query-203 define compatibility checks; complete consumer implementations
are not present in this worktree.

**state-query-201 (keep; coordinator decision): append-only from version 1.**
The stability rule begins with the version 1 schema defined at M15, after
M2.3e has removed transitional state-query-83..101 and 112. It does not prevent
that planned version 0 cleanup. From version 1, preserve labels, spelling,
case, type, unit, count semantics, section context, field order, tuple cardinality and
optional-field conditions. Never rename a field or reuse its position for a
different meaning. Add uniquely named fields at the end, with documented type,
unit, owner, source and absence rule, keeping all older fields. New counters
start at zero and state their reset/saturation behavior. Keep version 1 for
additive compatible fields; an incompatible alternative needs a separately
versioned query/format, with the old answer still available. Do not hide a
breaking unit or formatting change behind a higher version in this same query.

**state-query-202 (keep consumer compatibility): preserve existing global
matches and support section-aware readers.** External stress tools use global
regexes, including the exact patterns in state-query-203. Appending version 1
must preserve their matches and captures; section-aware parsing is a useful
choice for new readers, not a prerequisite for existing consumers. Readers
accept unknown appended fields, tolerate repeated whitespace and optional
GPU/identity/quantile blocks, and explicitly handle legacy version 0 and version
1, including the planned absence of transitional shadow diagnostics. Unsupported
versions fail the affected assertion with a schema message rather than assuming zero. Absent optional, unknown, pending and zero
are distinct values. Preserve integer counters exactly, never parse them through
float. Snapshot before/after with process identity to detect restart before
computing deltas. Reject negative acquisition deltas and mismatched units.

**state-query-203 (changed for merged suffix): schema/oracle fixtures.** M15 host fixtures must cover
CPU and GPU, HDMI off and all modes, no and present crash header, RSS unknown,
watchdog states, pre-publication absent identity, zero and repeated resolutions,
0/49/50/256 timing samples, empty/populated pacing windows, every loss cause,
all eight acquisition attempts including failures and saturation, both stage
restart counters, missing version and appended version 1 plus unknown fields.
Use exact source-format strings and a fake event schedule; validate both the
producer and consumer readers against them. Include a pre-M2.3e version 0
fixture with shadow fields and a post-M2.3e/M15 fixture without them; version 1
must never include the transitional fields. Merged-base fixtures also cover
the optional hardware suffix with distinct UI/cost/hw frame counts; the third
global frames capture is an upstream wire addition, not a version field.

The first three patterns below are exact compatibility patterns supplied in the
coordinator review, copied verbatim. The rest are additional fixtures derived
from the documented producer formats (`ui/src/cmd.rs:192`,
`ui/src/pacing.rs:432`, `ui/src/deepmeas.rs:40`, `ui/src/ui/change.rs:241`),
not claims about unseen consumer source. Run them as global searches on the
whole line, without first splitting into sections. Preserve the first match,
all matches in order, and capture strings before and after appending
` state_version=1` and further compatible fields. In particular, the frames
pattern matches UI `frames` and the appended `hw frames`; F0 retired the version 0 `cost frames`
section. The read pattern matches the pacing timing tuple rather than GPU `read X) ms`.

| Pattern (regex source) | Expected matching shape |
|---|---|
| `\bframes (\d+)` | UI frames first, optional hw frames last; F0 retired cost frames; preserve ordered captures |
| `read ([\d./]+) ms` | Pacing read p50/p90/p99/max tuple |
| `settle configured/first/steady ms/empty` | Exact settle label, including slash/space punctuation |
| `\bfps ([\d.]+)` | Leading picture rate |
| `\brecords (\d+)` | UI record count |
| `step ([\d.]+) ms` | Leading smoothed step timing; pacing step tuple does not match |
| `render ([\d.]+) ms` | Render duration |
| `present ([\d.]+) ms` | Presentation duration |
| `acquired (\d+) used (\d+) dropped (\d+) measured (\d+)` | Four aggregate pipeline counters |
| `gap p50/p90/p99/max ([\d./]+) ms` | Pacing new-picture gap tuple |
| `drawn ([\d./]+) ms` | Pacing workers-ready tuple |
| `age p50/p90/p99/max ([\d./]+) ms` | Picture batch-age tuple |
| `wanted (\d+) applied (\d+)` | Pacing generation mirrors |
| `settle configured/first/steady ms/empty ((?:-?\d+/-?\d+/-?\d+/\d+(?: -?\d+/-?\d+/-?\d+/\d+)*)\|-)` | Up to four settle tuples, including -1 sentinels, or - |
| `\bgen=(\d+) epoch=(\d+) stats_gen=(\d+)` | Optional published identity; absent before publication |

Keep the pattern strings intact in M15 compatibility fixtures, including
unescaped literal spaces and capture groups. Do not introduce earlier matching
labels or matching text in added fields that changes existing global match
selection. Fixtures cover CPU/GPU optional timing blocks, repeated labels,
empty settles and -1/populated settle tuples, and compare both first-match and
find-all results. This docs task runs no unit session and introduces no
executable producer/consumer tests.

**state-query-204 (keep): future fields are not current schema.** The planned
`xdma=` mode/CPU diagnostics, future memory channel bounds, per-stage metrics
and remote response latency mentioned in architecture documents are not emitted
by `cmd::stat_text` at this revision. From version 1, add them through the
append-only rule when implemented. Do not synthesize fields from documentation
promises or interpret
acquisition-attempt counters as DMA completion success.


## Deep-step diagnostics (task 0062 step 1)

The diagnostics append after `stage_restart_bode=`. Existing stage, loss,
acquisition and restart fields keep their order. Host StageTiming reports use
the same diagnostic formatter after their stage fields. The `diag` prefix is
distinct from the existing `deep reads` block.

Each completed StageTiming observation retains an additional paired split,
using the same ring index and eviction as intake/change (latest 256 steps).
`diag intake.p50` and `diag intake.p99` report `n=N intake_ns=I polls_ns=P
external_ns=E lock_wait_ns=W residual_ns=R` from the actual intake-ranked sample,
with `P + E + W + R = I` for every retained sample. `polls_ns` sums the timed
lifecycle calls, excluding their Shared lock waits. `external_ns` sums outer
external command/SCPI/panel intake, excluding nested polls and Shared waits;
nested external calls count once. `lock_wait_ns` sums UI Shared waits within
those intake intervals, excluding change/acquisition-stage locks. The residual
is intake minus the other three and includes lifecycle bookkeeping, timer and
metric recording overhead, and any unclassified elapsed time. External calls
queued before completion belong to that next completed step. Script preparation
outside any timed intake and memory-stopped steps without completion contribute
only call-ring observations. At u64 saturation components are capped in
poll/external/wait order so their sum still equals the represented intake.

The nearest-rank sample is chosen by intake time, with physical ring index
breaking equal-time ties. Its split components are not independent percentiles.
p50 is pending at n=0 and p99 at n<50, emitted as `diag intake.pNN n=N pending`.
The p99 sample's `intake_ns` equals the existing `stage intake` p99. The extra
paired ring occupies 8 KiB per StageTiming owner, with bounded scalar bookkeeping.

The per-call blocks begin `diag LABEL` and have
`n=N total=T p50_UNIT=V p99_UNIT=V max_UNIT=V`. Units are `ns` for elapsed
wall time and `bytes` for copied payload. `n` is the number retained (0..256),
`total` a saturating u64 process-lifetime observation count. Each independent
ring retains its latest 256 calls, including zeros. Nearest-rank p50 is pending
at n=0, p99 at n<50; pending is the literal `pending` in the value position
(e.g. `p99_ns=pending`). Empty max is 0. These per-call rings are neither
time-aligned nor additive. Neither CLEAR nor an acquisition identity change
resets diagnostic rings; process restart resets them.

| ID | Label after `diag` | Observation boundary | Unit |
|---|---|---|---|
| state-query-205 | `intake.p50` | Intake-ranked median step and its additive split | ns |
| state-query-206 | `intake.p99` | Intake-ranked p99 step and its additive split | ns |
| state-query-207 | `poll.memory` | Memory watchdog sample/recovery | ns |
| state-query-208 | `poll.relayout` | Layout refresh | ns |
| state-query-209 | `poll.meas_hold` | Held measurement key | ns |
| state-query-210 | `poll.workspace_hold` | Held workspace key | ns |
| state-query-211 | `poll.update` | Update service poll | ns |
| state-query-212 | `poll.wlan` | WLAN service poll | ns |
| state-query-213 | `poll.ntp` | NTP service poll | ns |
| state-query-214 | `poll.ssh` | SSH service poll | ns |
| state-query-215 | `poll.usb` | USB service poll | ns |
| state-query-216 | `poll.ch_hold` | Held channel key | ns |
| state-query-217 | `poll.counter` | Hardware reading/request handling, including Shared wait | ns |
| state-query-218 | `poll.dvm` | Hardware reading/request handling, including Shared wait | ns |
| state-query-219 | `poll.mask` | Hardware test-result handling, including Shared wait | ns |
| state-query-220 | `poll.zone` | Hardware test-result handling, including Shared wait | ns |
| state-query-221 | `poll.la_probe` | Logic probe detection, including Shared wait | ns |
| state-query-222 | `poll.log` | Logging tick | ns |
| state-query-223 | `poll.external` | One outer command/SCPI/panel intake including pending proposals; shares StageTiming's intake start/end, including nested polls/waits | ns |
| state-query-224 | `lock.ui.wait` | UI Shared wait at acquisition-source, publication, probe, recovery and calibration accesses | ns |
| state-query-225 | `lock.ui.held` | UI Shared hold at those same accesses | ns |
| state-query-226 | `lock.acq.wait` | Producer Shared wait at acquisition-thread requests/readings and batch/stream/segment publication, including deferred delivery on a worker | ns |
| state-query-227 | `lock.acq.held` | Producer Shared hold at those same accesses | ns |
| state-query-228 | `producer.clone` | One producer Arc::clone, excluding replacement/drop of the old destination | ns |

Sources: `ui/src/ui/lifecycle.rs`, `ui/src/ui/change/intake.rs`,
`ui/src/perfguard/{intake,deep,ring}.rs`. The existing acquisition metrics owner
has a 64 KiB inline diagnostic budget, with a test requiring at least 8 KiB
headroom. It retains no record payload. Host std mutexes may initialize OS
storage once on first use; warmed recording allocates nothing and does no sort.
Ring mutexes are separate from Shared. Lock wait ends just after Shared is
acquired; held ends immediately before its release, including scheduling delays
and the publication handle clone. Lock and copy samples are recorded after
releasing Shared. Clone samples carry only elapsed nanoseconds
until release, preserving the existing clone/publication order and lock scope.
Reports copy each ring under its metric mutex, then sort/format outside it;
blocks are best-effort snapshots, not an atomic process snapshot. Reading the
state itself adds a UI Shared observation after the report snapshot and the
outer query intake is visible on the next query. Polls skipped on memory stop
have no observation; a disabled poll that is called still has one. Scripted
prepared steps may skip lifecycle polls. No acquisition thread means acquisition
and clone rings may remain pending, distinct from measured zero cost.
Live debug-source locks, deposit locks and synthetic fixture writes are excluded.

Publication shares the acquisition's analog and LA buffers with the queue (task
0089). The retired `producer.clone_size` row measured a constant zero after the
Arc handoff and is removed. The checked-in device-test parser does not consume
that row; no perfmap scripts are present in this tree. Sample-sized allocation
censuses and pointer-identity tests guard sharing instead. This is host-verified,
not a fresh unit measurement. Compare stationary shallow/deep RUN windows after
>=256 UI steps and >=50 producer clones; at low delivery rates the clone window
spans much longer than intake. Clone timing now measures handle acquisition,
not sample-copy work.
If UI waits stay small, inspect the slow poll's own service/body instead. A
long acquisition hold without a long clone points to other work inside Shared;
a long UI hold points to UI work. Use total-count deltas over measured wall
time to separate increased clone frequency from increased cost per clone.
Do not subtract unrelated p99s or the EWMA `step` from these distributions;
matched repetitions and scheduling context are still needed for causal evidence.


## Draw sub-stage diagnostics (P0)

These seven blocks are appended after `diag producer.clone`, preserving all
existing field and block order. Each has the existing call-ring grammar
`diag LABEL n=N total=T p50_ns=V p90_ns=V p99_ns=V max_ns=V`, including literal `pending`
for p50/p90 at n=0 and p99 at n<50. Nearest-rank quantiles retain the latest 256
observations, including zeros; totals saturate over process lifetime. CLEAR and
acquisition identity changes do not reset them.

| ID | Label after `diag` | Observation boundary | Unit |
|---|---|---|---|
| state-query-230 | `draw.decay` | Persistence factor and main/zoom map decay | ns |
| state-query-231 | `draw.fast` | Newest-frame drain/draw, including deep raw record conversion, and fast-take bookkeeping, ending before worker merge | ns |
| state-query-232 | `draw.merge` | Worker pipe lookup, map merge, stale/status updates and measurement folds | ns |
| state-query-233 | `draw.records` | Whole bounded record loop, including acquisition/conversion, conditioning, deposition, history and completion; or stopped replay/redraw | ns |
| state-query-234 | `draw.grade` | grade_refs and main/zoom halving, excluding intervening finish-step observer work | ns |
| state-query-235 | `draw.render` | One panel render_layers call, including relayout and start-up early return; CPU composition/job construction, excluding presentation/helper execution | ns |
| state-query-236 | `draw.view_render` | One render_layers call within render_view_layers for the HDMI layout view; same CPU boundary as draw.render, excluding the outer view save/restore | ns |

The first five rings receive one sample per completed `Ui::step`; an unexecuted
sub-stage contributes zero. Their intervals are disjoint and additive elapsed
costs, not a partition of the entire step: setup, change, measurements/analyzers,
panes and other bookkeeping remain outside them. In particular `draw.records`
includes acquisition time already represented by the legacy EWMA `acq`; do not
add that EWMA again. Each render ring receives one sample per actual call for
its target, independent of step count; HDMI layout renders never enter the panel
ring, including start-up early returns. Individual
p50/p99 values are independent quantiles and must not be summed as a frame p99.
They are wall time, including scheduling delays, not CPU time or GPU time.

Sources: `ui/src/perfguard/draw.rs`, `ui/src/ui.rs`, `ui/src/ui/step_draw.rs`.
The compiled-layout test in `ui/src/perfguard/draw.rs` bounds `perfguard::draw::Timing`
below 17 KiB inline, including seven duration rings, F0's independent
`draw.frames_per_picture` ring and their bookkeeping. This owner is embedded in
each `StageTiming`; the bound excludes the rest of `StageTiming` and the separate
deposit timing rings. Each step also uses a fixed five-duration local.
Recording does not allocate or sort; reporting sorts copies.
Allocation guards and unchanged host layout/goldens are host
evidence only; timing overhead and budgets are not yet verified on the unit.

## F0 record accounting and stage rings (#202)

Host implementation, not yet verified on hardware. Process-lifetime counts use records, including
hardware-average inputs that are deliberately withheld. Roll and scan count published sweep snapshots,
not samples. `records read=N deposited=N shown=N` reports reads, deposits (worker and direct UI),
and hardware records reaching a picture. Demo playback is excluded from `shown` and direct deposits;
the older `used` aggregate still includes it. Independent relaxed atomic loads can observe work in flight.

`lost.settings`, `lost.frame0`, `lost.frame1`, `lost.average` and `lost.error` identify intentional
selection and short-read losses. Settings includes the existing worker-push, UI-publication and queue-pop
identity counters. `lost.buffer`, `lost.unclaimed`, `lost.budget`, `lost.queue_overflow`
and `lost.superseded` expose the current pipeline's remaining discards. F0 preserves these policies;
F3 removes the buffer and unclaimed overload sites through back-pressure. Sweep snapshots replaced by
a newer snapshot are `superseded`. Completed maps rejected after deposition count as
`rejected.epoch` when their epoch changed, otherwise `rejected.stale_key`.
`dropped` retains its pre-F0 meaning: buffer exhaustion, unclaimed work, stale queued work and
rejected maps. Intentional frame-zero/frame-one, average, budget and superseded-snapshot selections do not
increment it. `records ... not_used=N` is the saturating `acquired - used` gap, including those selections
and pending work. It is not a loss rate; the legacy `used` counter also includes demo playback.
The frozen accumulator and its rejection counter arrive with F6.

The quiescent ordinary live fast-route accounting is `read = deposited + source losses` and
`deposited = shown + map rejections`. Pending queues, batches and maps prevent testing those equalities
from an arbitrary running snapshot. Other record-only consumers retain their existing selection counters.

Every completed UI step, including zero-record and stopped steps, enters `draw.frames_per_picture`
(wire unit `frames`, counting acquired records). All shared diagnostic time rings also report p90.
`deposit.slice` samples every 64th worker/helper claim, including decode, drawing and cheap folds.
Each producer keeps its sampling cursor locally. Samples travel with the existing maps; each accepted picture
retains at most one sample, and only the UI updates the timing rings. `deposit.record` divides that sampled
claim's elapsed nanoseconds by its record count. `deposit.batch` measures feed-to-UI-merge age once per
nonempty picture using the newest accepted batch; it is not yet the future read-to-slot latency. All three use ns.
Deposited and measured counts accumulate locally and publish at existing batch/share boundaries.
There are no timing-ring locks or counter atomics in the per-record draw loop.
The future pool's separate place/tile, busy and wait-buffer observations require F2/F3 and are not synthesized here.

Each ring holds the last 256 observations, including zeros, with nearest-rank p50/p90/p99 and maximum.
No observations means pending; p99 stays pending until 50 observations. The lifetime `total` saturates;
ring eviction keeps working after saturation. Reporting sorts copies and does not reset the rings.
The rings allocate no memory per observation. Source: `ui/src/perfguard/ring.rs`,
`ui/src/perfguard/draw.rs`, `ui/src/claim_timing.rs`, `ui/src/drawpipe/losses.rs`.
