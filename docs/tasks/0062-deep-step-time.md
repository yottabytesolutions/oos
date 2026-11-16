---
id: 62
title: Explain deep-record main-step time
kind: chore
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Explain the copied M2.3b deep-run timing before M2.3c acceptance, using host evidence.
Review tier: standard. No commits or hardware access.

## Plan

1. Map the actual intake timer and compare `2279778` with `adb72ff`, separating
   per-step, per-frame and producer-lock work. Read copied unit evidence only.
2. Exercise the real intake with synthetic 4-channel 10 M compressed records at
   the board's deep-read factor, on isolated source archives of both commits.
   Use the existing StageTiming and paired host perf harness; retain complete logs.
3. Attribute measured work, distinguish increased delivery rate from new cost,
   and specify the remaining unit measurement if the host cannot explain the delta.
4. Implement only a clearly local safe fix; otherwise map the improvement and
   regression budget to M5/M5b/M7. Run focused debug/release tests, release timing,
   the exact clippy step and `cargo xtask check --fast`.

Reserved paths: `docs/tasks/0062-deep-step-time.md`,
`ui/src/ui/tests/deep_intake.rs`, `ui/src/ui.rs` (test module declaration only),
`ui/src/board/scripted.rs`, `ui/src/board/dho/scripted.rs` (test-only fixture),
`target/t0062/`, `target/COMMIT_MSG.txt`.
The owner's new-task assignment authorizes this claim and the test-only hot-file seam.
Existing production reservations, ratchets and copied unit evidence stay untouched.

## Findings

### What the timer measures

At both `2279778` and `adb72ff`, `change::begin_step` times only
`lifecycle::begin_step` as its local intake slice. That runs lifecycle/service
polls: memory, relayout, update/network/USB, held keys, counter/DVM, mask/zone,
LA probe and logging. Counter/DVM/mask/zone/LA polls read Shared under its lock.
`change::intake` also accumulates external command, SCPI and panel handling;
its nesting counter avoids double counting nested commands. Those queued times
are added to the next completed step. Consequently intake is elapsed time,
including lock waits, OS service work, query handling and descheduling.
It is not a timer for every acquisition-related operation on main.

The change slice starts afterward: legacy settings/normalization, transitions,
publication and observation. Separately measured shadow work is subtracted
from change. StageTiming keeps the last 256 completed paired samples and uses
nearest-rank p99; combined p99 is over each sample's intake + change sum.
The unit's `step 14.1 ms` is a 0.05 EWMA around `Ui::step` in
`system/device.rs`, whereas intake p99 spans roughly 8.5 s at 30 Hz.
These are different statistics; neither can be subtracted from the other.
GPU upload/draw is separately instrumented and must not be added to intake.

### Before and after the cutover

| Work and source seam | M2.3a (`2279778`) | M2.3b (`adb72ff`) | Size dependency |
|---|---|---|---|
| Live normalize, `change/normalize.rs` | Already active, before classification | Retained | Settings only, no sample traversal |
| Acquisition and generator wants | `Source::acquire`, independent generator publication | `change/acquisition.rs::publish`, after transitions, one Shared lock | Settings, not record depth |
| Mask/zone wants | Derived by lifecycle polls | Polls read results; wants derived during change publication | Mask columns / zone polygons, off in this deep scenario |
| Worker view, `drawpipe::Key::of`, `feed::set_view` | Fixed channel/settings key, Model cloned on key changes | Same key, additionally refreshes on stats identity | Four Channel values/labels, not frame samples |
| Identity tagging, `publication::tag` and frame constructors | Stamp plus pacing gen mirrors | Pair from one applied publication | Moves Vec descriptors and writes scalar fields; no sample clone |
| Publication retain | Absent | `change/acquisition.rs::publish` retains matching pairs | O(queue length), up to QUEUE_MAX=96; stale payload destruction can release samples |
| Queue drain/pop, `wave.rs` | Drain all but last, pop into conversion | Additional exact-pair retain/rejection and loss counters | O(queued frames); no traversal of kept samples |
| Producer newest-frame publication | `hw.rs::push_frames`: `g.frame = f.clone()` under Shared | Extracted `publication.rs::push_frames`: same clone, plus pair filtering / overflow counter | Copies all four sample Vecs under lock |
| Record conversion, `wave::hw_record`, `board::to_record` | Move Frame into Arc, make 3600 screen points | Same | Existing reduction of deep pairs; after intake/change |
| Main drawing, `ui.rs` record loop | `raw.frame.clone()` clones Arc, then deposit reads frame samples | Same | Existing O(samples) draw; Arc clone itself is constant cost |
| Loss accounting, `drawpipe::Counters::add` | Existing relaxed atomic adds | Additional relaxed atomic adds per rejection/site, including zero-count adds | No sample traversal |

Relevant current lines: `ui/src/ui/change.rs:121`,
`ui/src/ui/lifecycle.rs:206`, `ui/src/perfguard.rs:74`,
`ui/src/ui/change/acquisition.rs:19`, `ui/src/drawpipe.rs:474`,
`ui/src/drawpipe/feed.rs:150`, `ui/src/board/dho/publication.rs:25`,
`ui/src/wave.rs:781`, `ui/src/wave.rs:799`, `ui/src/ui.rs:1967`.
Historical line numbers differ; the named seams were compared at each exact revision.

### Workload and host limitations

The copied unit report has 4 channels, 10 M depth and 1 ms/div, batch mode,
no measurements or deep rereads (`deep reads 0`). The host production `batch_plan` for those settings gives
k=16 and no raw window: 625,000 decoded points per channel, 5 MB wire data,
10 MB f32 payload, rate 312.5 MS/s and dt=51.2 ns. Its 4 MB target clamps
frames per batch to one. The worker handoff requires `avail > 1`, so the
newest deep record goes through the ordinary queue/record path even when the
configured pipeline says Batch. `compress k 1` in the copied diagnostics is
not evidence that this deep frame was raw: `note_batch_k` reports b.compress
only for `raw_newest`, which excludes the deep-compression case. The copied
logs do not independently capture the deep frame header/k; k=16 is established
by the matching production plan on the host, not a new hardware measurement.

The new test fixture calls actual settings, batch_plan, scripted wire fill and
board decode, then feeds Shared and calls real `Ui::step`. It pins both the
625,000-point raw frame and the 3600-point screen record. The golden-script
DeepPairs fixture deliberately uses 12,500 points; it remains unchanged.
The timing run feeds 38 records into 300 logical 30 Hz steps, one every eight
steps (3.75/s), with copying/decode setup outside the main-step timer.
The paired host harness alternates shallow/deep resident payloads through the
real intake/change seam for 101 rounds of 20 calls, using process CPU p99.

A separate controlled experiment synchronizes a producer thread that clones
the same 10 MB payload under Shared with the real intake/change seam. This
replays the existing critical section, not the live acquisition thread or a
claim that its scheduling matches the unit. It establishes that elapsed
intake includes this wait even when main does not inspect sample data.

The host has no DMA, actual board producer, GPU helper, core placement,
unit service polls, vblank or real-time delivery cadence. Logical dt does not
pace wall time. Cold faults and scheduler outliers remain visible in the logs.
Thus these experiments can identify depth-dependent main work and a lock-wait
mechanism, but cannot prove why the specific unit run changed from 4.4 to
14.1 ms. In particular the host does not reproduce a new 12 ms intake body.
Do not promote the demonstrated lock mechanism to a verified unit cause.

### Measured release comparison

Final-source aarch64 host runs are serial AB then BA, with compilation complete
before measurement. Each cell below is round 1 / round 2. Times are microseconds
unless marked ms. These are host event samples, not new unit observations.

| Observation | `2279778` M2.3a | `adb72ff` M2.3b |
|---|---:|---:|
| Intake p99, last 256 steps | 4.750 / 8.042 | 7.625 / 7.458 |
| Change p99 | 4.083 / 6.792 | 6.792 / 6.875 |
| Paired intake+change p99 | 8.792 / 14.916 | 13.293 / 14.457 |
| Shadow p99, separate | 15.750 / 28.832 | 24.541 / 32.084 |
| Mean new-record full step, ms | 10.077 / 10.062 | 10.035 / 9.490 |
| Mean idle full step | 58.068 / 74.996 | 62.811 / 58.880 |
| Resident shallow/deep best paired seam | 2.575:2.575 / 2.525:2.527 | 2.683:2.639 / 2.702:2.668 |
| Resident shallow/deep p99 process CPU | 4.850:4.800 / 3.000:3.100 | 3.300:3.500 / 4.100:3.350 |
| Producer clone under lock, mean | 171.026 / 158.672 | 153.725 / 158.797 |
| Intake p99 with controlled clone overlap | 209.833 / 197.250 | 176.416 / 192.000 |

Both final comparisons pass their behavioral/accounting assertions (two
explicit timing experiments per revision per round). Deep resident data
causes no proportional growth in intake/change, and the full new-record step
shows no M2.3b per-record regression in this host workload. The controlled
copy raises intake by roughly 0.2 ms on both versions without any new main
sample processing. Earlier exploratory runs also show millisecond scheduler
outliers in both revisions; they are retained as `*-timing-{1,2}.log`, not
silently mixed into the final source comparison or treated as a unit cause.

Reproduce with `python3 target/t0062/reproduce.py`. It archives both exact
revisions and adds the test-only probe/fixture; production timing, lifecycle,
wave, hardware and publication sources are byte-checked against Git in
`target/t0062/production-provenance.json`. Complete final logs:
`target/t0062/{2279778,adb72ff}-final-timing-{1,2}.log`;
`final-timing-exits.json` records all four exit codes (0).
The current worktree also passes the same deep diagnosis with intake p99
5.625 us, change 5.415 us and paired 11.373 us; its controlled overlap has a
0.986 ms intake p99, illustrating elapsed scheduler/lock variation rather
than providing a new production delta.

### Rate versus per-record cost

The reported delivery rate increased 19.4% (3.1 to 3.7/s). One existing 10 MB
clone therefore grows from 31 to 37 MB/s of logical copied payload, without
any increase in cost per record. Decode, conversion, deposit and GPU dirty
work may also run more often. At steady state a rate-only explanation would
scale only the record-dependent part by 1.194, not multiply every step by
3.2. The actual report gives smoothed snapshots, not matched stationary means,
so this arithmetic is a diagnostic constraint rather than a regression proof.
M2.3b adds pair predicates, queue passes and counters, not a new 10 MB copy.
The copied deep diagnostics have stale_ui_publish=0 and queue_overflow=0;
stale_worker_push=11 and stale_pipe=647 are cumulative boundary losses.
They do not establish steady deep-record drop/destruction cost.

### Improvement and acceptance plan

No production fix is justified by this host comparison. Moving a clone out
of Shared in isolation must revalidate identity and newest-frame ordering
across the second lock; changing Frame storage or publication ownership is
M5 work, rather than a safe local edit in this investigation.

1. Before M2.3c acceptance, arrange an owner-authorized unit measurement with
   fixed setup and the same cold-boot/placement/governor conditions at each
   revision. For every stage event, record the paired sequence, queue length,
   gen/epoch, actual k, point count, delivered/dropped frames and new-record flag.
   Split intake into external-command time and each lifecycle poll; split each
   Shared acquisition into wait and held time. At producer push, measure lock
   wait, sample clone, old-frame destruction, queue overflow and total held time.
   Keep thread CPU alongside elapsed time and correlate these with intake tails.
   Capture at least 60 s and report idle versus new-record p50/p99 separately.
   Measure each old/new revision at both 3.1/s and 3.7/s delivery if pacing can
   hold those rates. Keep GPU upload/fence/dirty-row timing separate. This
   distinguishes a producer clone wait from OS polling, scheduling, query load,
   stale destruction or a true new per-record cost. M2.3c remains held until
   the cause has that evidence or the owner explicitly accepts the remaining gap.
2. M5 already owns removal of Shared and immutable Arc<SampleStore> views.
   Remove the sample clone under the handoff lock, preserve pair checks, and
   copy only retained ranges once off main. Expected gain is exactly one
   10 MB payload copy per deep handoff (37 MB/s at 3.7/s), its allocation/free
   work and its contribution to main lock waits. It does not promise to remove
   all 12 ms of unit intake or the separate GPU upload cost.
3. Add M5's planned counting-allocator regression using this production-shaped
   fixture: after buffers are prepared, publishing the newest 4 x 10 M view
   must allocate/copy zero sample bytes on producer or main. Hold the old view
   through the next publish and verify its data and pair survive, so moving the
   clone to a different stage cannot satisfy the test. Add an overlap test that
   stalls off-main retained copying while intake still finishes within the
   existing 0.5 ms paired intake/change budget on the unit. The host ratio guard
   compares deep and shallow handoff orchestration with 1.25 best / 1.875 p99
   CPU limits; unit elapsed lock-wait evidence is needed in addition to CPU.
4. M5b enforces byte bounds and nonblocking overflow so a dropped deep payload
   cannot put unbounded freeing/copy work on main or starve the pool. Test
   saturated deep queues with slow consumers and exact losses/identity checks.
   M7 removes remaining O(samples) main deposit through the shared lines/code
   kernels; profile that separately from records and GPU. M7 is not the fix for
   the lifecycle intake timer, and M5b alone does not remove the current clone.

## Status

- 2026-11-16: host investigation and standard-review handoff prepared, uncommitted.
  No new O(samples) intake body or new per-record slowdown reproduced between
  `2279778` and `adb72ff`. The existing 10 MB producer copy and resulting intake
  wait are demonstrated, but the particular unit regression's cause remains
  unverified. No speculative runtime fix made. The Findings retain the M5/M5b/M7
  plan, expected copy saving, proposed perf budgets and exact unit measurement
  needed before M2.3c. Task remains active for review; no hardware acceptance.

- 2026-11-16: final-source focused debug and release each exit 0: 1 passed,
  0 failed, 2 explicit diagnosis tests ignored. Historical final release
  behavioral/accounting checks and both timing experiments pass on both revisions,
  two serial rounds, all exits 0. Current explicit deep timing also exits 0
  (2 passed). The unchanged existing release intake/change guard exits 0:
  intake 59,375 ns, change 60,625 ns, paired 116,041 ns against 500,000 ns;
  shadow 65,750 ns separate, 256 samples. Logs and command list are under
  `target/t0062/`, including `focused-{debug,release}-final-source.log`,
  `current-timing-final-source.log`, `existing-timing-final-source.log`,
  `commands.txt` and `reproduce.py`.

- 2026-11-16: exact `xtask::steps::clippy` passed against this worktree, exit 0,
  through a test-only scratch xtask wrapper (no gate-source change). It runs
  the real host/ARM invocations, parsing, moves and ratchet comparison: no new
  or grown findings, xtask zero findings. Full log: `target/t0062/clippy-step-final-source.log`;
  complete child logs: `target/check-logs/95486-1794817986497520000/`.
  `cargo xtask check --fast` exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 34 s`, with all selected steps
  passing. Log: `target/t0062/fast-final-source.log`; child logs:
  `target/check-logs/95693-1794817991467082000/`.
  Clippy/full tests/perf/coverage/image are not selected by the fast gate;
  only the separately named clippy/focused/timing runs are claimed here.
  Independent cross-vendor review and the coordinator's full gate remain open.

- 2026-11-16: initial diagnostic compilation rejected a borrowed mpsc Receiver
  in the scoped producer closure; moving that closure's channel ownership fixed
  it. The first exact clippy attempt failed on one new items_after_test_module
  finding in the adapter helper. Moved the helper ahead of tests without lint
  suppression; both later exact step runs pass. Earlier logs remain retained.
  Diff whitespace and added-source/task/message privacy checks pass. One section
  in `target/COMMIT_MSG.txt`. No commits, staging, push, hardware, ratchet,
  dependency, lint-limit, golden reference, remote or hook changes.

- 2026-11-16: mapped intake to lifecycle polls, not record conversion. Both versions
  clone sample vectors in producer publication under Shared. M2.3b moves acquisition,
  generator, mask and zone publication into the separately timed change slice.
  The copied `compress k 1` is a live-compression diagnostic; production deep
  `batch_plan` selects k=16. New adapter-owned test fixture uses that actual plan:
  4 x 625,000 decoded f32 pairs, 5,000,000 wire bytes, 312.5 MS/s,
  dt=51.2 ns, 10,000,000 original points per channel. The existing M1 deep
  fixture instead targets 12,500 decoded points and is deliberately unchanged.
  Focused debug real-intake/record-conversion test passed (1 test, exit 0).
  Historical archives preserve each production revision, adding only the fixture,
  test declaration and identical probe with parent-only legacy identity and
  absent test-scope API shims.

- 2026-11-16: claimed under the owner's assignment on `t0062-deep-step`, base
  `dca35ed` (M2.3b included), author Codex. Tree initially clean. Plan written
  before code. Independent cross-vendor review and integration remain open.
