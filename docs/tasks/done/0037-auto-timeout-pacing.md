---
id: 37
title: "Review auto-timeout pacing"
kind: fix
status: done
release: 0.2.0
owner: codex-b
depends: []
---

# Review auto-timeout pacing

## Goal

Make auto-timeout pacing explicit and test its effect on responsiveness.

## Plan

Reserved paths: docs/tasks/0037-auto-timeout-pacing.md, ui/src/ui/host.rs,
docs/tasks/0012-m11-sparse-pacing.md (Status handoff only).
Branch: `t0037-auto-timeout`; base: `overhaul`.
The 2026-11-15 implementation assignment releases `ui/src/ui/host.rs` for the three host characterization tests.
Use M1 steps 1-2's merged `board::scripted::{BatchSource, Batch}` and `drawpipe::host::finish_batch` APIs.
Keep the UI fixture in frames mode and own a separate `Pipe` for the backlog test: M1 does not yet inject a pipe
into `Ui::step`. Exercise the pipe merge/delivery boundary explicitly, then let the UI consume the delivered
newest frame. Compare the combined maps with the frames route without introducing a global pipe test seam.
Do not claim this proves the integrated global-pipe wiring or producer timing. No held M1 paths are needed.
Run the changed tests in debug/release and only the fast gate, per the implementation brief and delivery policy.

### Current behaviour, read on the host

Source locations below refer to base revision `1d5fa1b63cbb2c94e4115a4b0fed5017f07353d9`.
This is code characterization, not a measurement of the FPGA or a hardware acceptance result.

1. **AUTO is an acquisition policy in the SCU, not a UI redraw timer.**
   `ui/src/board/dho/hw.rs:187` maps AUTO to `Settings::auto`; NORMAL and SINGLE disable it.
   `hal/src/dho/seq.rs:495` configures the auto-trigger block. The pure write-list builder at
   `hal/src/dho/scu.rs:320` sets the auto-enable bit and two timers in 4 ns ticks:

   | Timer | Value | Examples |
   |---|---|---|
   | t1 | clamp(20 x time/div, 10 ms, 500 ms) | 200 us/div: 10 ms; 20 ms/div: 400 ms |
   | t2 | max(holdoff, clamp(10 x time/div, 100 ms, 250 ms)) | 200 us/div: 100 ms; 20 ms/div: 200 ms; 1 s holdoff: at least 1 s |

   The FPGA decides when those programmed timers cause an accepted acquisition. The host code alone does not
   establish the exact t1/t2 FSM phase relationship, so do not present either timer or their sum as a measured
   record-to-record period. `auto_fast` supplies a zero time base to this builder, yielding 10/100 ms while still
   respecting holdoff (`hal/src/dho/seq.rs:497`). Autoset enables it during its search preparation and clears it
   again (`ui/src/ui/autoset.rs:162`, `ui/src/ui/autoset.rs:454`, `ui/src/ui/autoset.rs:460`).

2. **A timeout permits a forced acquisition when the TPU does not fire.**
   The SCU status distinguishes a TPU trigger from an accepted acquisition without a TPU trigger
   (`hal/src/dho/scu.rs:81`, `hal/src/dho/scu.rs:89`). This is separate from the user's Force key/`:TFORce`,
   which explicitly arms and pulses the force bit (`ui/src/board/dho/hw.rs:2014`). The host's acquisition-done
   safety timeout is also separate: max(2000 ms, twice the record duration + 500 ms)
   (`hal/src/dho/seq.rs:895`), checked with 500 us sleep polls (`hal/src/dho/seq.rs:1379`). An expiry there is an
   error, not an AUTO trigger; the acquisition loop backs off 200 ms (`ui/src/board/dho/hw.rs:2293`).

3. **Re-arm follows acquisition/read progress, not a display timeout.**
   The batch arms and waits for its first completed frame before starting the fill timer
   (`hal/src/dho/seq.rs:750`, `hal/src/dho/seq.rs:1249`). It then fills until the ring is full or `fill_ms`
   expires, and reads exactly that count (`hal/src/dho/seq.rs:1252`). AUTO affects the wait for frames and thus
   indirectly the next batch's start. It does not issue a new arm on every screen tick. Optional prearm starts
   the next batch before decode, after reads/status collection, only when average, SINGLE and replay needs allow
   it (`ui/src/board/dho/hw.rs:2206`, `ui/src/board/dho/hw.rs:1320`). Otherwise the next loop pass arms.
   STOP disarms and holds (`ui/src/board/dho/hw.rs:1950`). The `Shared::rearm` handshake is for another SINGLE
   scan sweep (`ui/src/wave.rs:827`, `ui/src/board/dho/hw.rs:1805`), not an AUTO-timeout acknowledgement.

4. **Fill planning and worker readiness are separate from AUTO.**
   The main loop runs `Ui::step` and composition on a 30 Hz schedule regardless of new records
   (`ui/src/main.rs:688`, `ui/src/main.rs:891`). `pacing::next_step_after` predicts that phase
   (`ui/src/pacing.rs:226`). `Fill::plan` uses measured arm/first-trigger/read rest, its deviation, and worker
   readiness plus a 3 ms margin (`ui/src/pacing.rs:237`, `ui/src/pacing.rs:267`). Startup fallback is 25 ms;
   minimum is 4 ms; with a known phase the clamp can allow up to one 33.333 ms frame, not merely 25 ms.
   Rest that cannot fit with the minimum fill selects 4 ms immediately (`ui/src/pacing.rs:279`).
   The loop feeds back pre-fill plus read time (`ui/src/board/dho/hw.rs:2031`, `ui/src/board/dho/hw.rs:2065`).
   Reducing fill cannot remove time spent waiting for the first forced acquisition.

   Workers draw previous batches while acquisition proceeds. Acquisition's helper runs only after the first frame,
   between ring-count reads, one claimed frame at a time, and stops when its estimated cost would overrun the fill
   (`ui/src/board/dho/hw.rs:2035`, `ui/src/drawpipe.rs:776`). It does not run during the first-trigger poll.
   Worker completion feeds the smoothed `ready_ms` (initially 3 ms; `ui/src/pacing.rs:211`). Main merges ready
   maps once per step (`ui/src/ui.rs:1987`). The producer uses the worker route only with more than one available
   frame and an accepting pipe (`ui/src/board/dho/hw.rs:2227`); a lone forced frame uses the UI queue.
   Shortening AUTO could increase production and worker load; it is not a cost-free redraw improvement.

5. **What the user sees.**
   Few new AUTO records means an intermittently updated trace despite continuing screen refreshes. At minimum
   persistence, the old trace stays until new data replaces it; positive persistence decays each step
   (`ui/src/ui.rs:1935`, `ui/src/ui.rs:2056`). Hardware status overrides software edge refinement so forced
   acquisitions display `AUTO`, real triggers `TRIG'D` (`ui/src/ui.rs:2158`, `ui/src/ui.rs:4012`). Without new
   records, `WAIT` appears only after max(0.5 s, 1.5 screen-record durations, 2.5 smoothed record intervals)
   (`ui/src/ui.rs:2152`). That status deadline neither forces data nor re-arms acquisition.

   Historical unit evidence, not reverified here: `docs/fork/stutter.md:79` reports roughly 170 ms per no-trigger
   AUTO record and about five new pictures/s, while `docs/fork/stutter.md:110` records the decision to leave AUTO
   stock. These observations are not universal timing guarantees derivable from the timer values.

6. **Host Demo does not emulate the SCU timeout.**
   `Source::Demo` advances by the supplied `dt_wall`, searches for a trigger, and immediately returns an untriggered
   record in AUTO if none is found; NORMAL/SINGLE return none (`ui/src/wave.rs:862`, `ui/src/wave.rs:947`).
   `Ui::step` makes up to six such acquisitions for Edge/Pattern, one for Video, two for other demo triggers
   (`ui/src/ui.rs:1943`, `ui/src/ui.rs:2013`). Thus a Demo screenshot or six demo records per step does not prove
   any physical AUTO timeout. An injected `Source::Hw` queue exercises consumption and status, not the producer.

### Intended pacing and relationship to M11

- Preserve the existing stock AUTO timer table, holdoff semantics and autoset's fast-auto exception. Code reading
  establishes no accidental host redraw/re-arm delay to fix in this task. The visible low update cadence without
  a matching trigger is real historical evidence, but changing it would reverse the recorded stock-policy decision.
  This plan does not select a shorter timer or silently make Demo a timed FPGA simulator. The first implementation
  increment is explicit characterization tests and durable documentation. A shorter AUTO policy needs an agreed
  numerical acquisition requirement and, if it changes an architectural decision, an owner-approved ADR before code.
- Keep rendering independent of trigger arrivals. The target remains 30 Hz full steps on vblank, with an absolute
  timer fallback on the host, and a published `Pacing { next_step, period }` to acquisition
  (`docs/architecture/README.md:477`). M3c/M3d own waiting, cancellation and the main event loop; M5 owns removal
  of global pacing state. Do not duplicate those migrations here.
- M11 owns sparse **real** event fill planning after M10. Its specification is the 24 ms rest case, a batch every
  <= 33.3 ms, and setup B at 5.1 V with <= 1 % empty pictures
  (`docs/architecture/migration.md:491`, `docs/architecture/drawing.md:299`). The drawing plan expressly excludes
  AUTO timeout (`docs/architecture/drawing.md:403`). M11 cannot promise a new batch every display frame when the
  first forced acquisition itself takes longer than a frame. Test sparse real events separately from no-edge AUTO.
- A disposable host probe called the unmodified `Fill::plan` using the existing `pacing.rs:510` simulation: 24 ms
  constant rest, 5 ms worker readiness, next start 0.5 ms after read, 60 batches. After 20 warm-up batches it chose
  8/9 ms fills and produced 32.5/33.5 ms read gaps. This is already roughly one batch/frame, not evidence of a
  fixed 25 ms fill or a reproducible 34-40 ms sparse-event gap. The literal M11 upper bound is nevertheless exceeded
  by whole-millisecond rounding. M11 must define and test deadline/rounding precision explicitly; do not call that
  small quantization effect proof of the historical stutter or fix it by altering AUTO timers.
- Preserve relevant target budgets: picture rate >= 29.5/s, main step p99 <= 15 ms, deposit p99 <= 2.7 ms/frame
  and <= 70 % load per little core (`docs/architecture/README.md:1093`, `docs/architecture/README.md:1105`,
  `docs/architecture/README.md:1110`). These are unit targets, not host wall-clock assertions. No additional
  queue, global timer or unbounded retained state is proposed.

### Test design, including failing-before evidence

Use `Host::fixture()`/`Ui::for_tests`, which disable OS services and supply fixed host metadata and
`Switches::default()` (`ui/src/ui/host.rs:60`). Supply alternative switches as values within that module's tests;
never modify process environment or read the developer's settings. `Host`'s Unix clock is a calendar seam, not a
monotonic acquisition clock: drive `Ui::step(dt)` explicitly (`ui/src/ui.rs:1863`), and pass synthetic `Instant`
offsets into `Fill`. M1's scripted-batch source bypasses acquisition production
(`docs/architecture/migration.md:126`); full producer timing/re-arm tests require M3.0's scripted `Device` seam.

| Test and location | Stimulus and assertion | Before/after meaning |
|---|---|---|
| `auto_timer_table_and_holdoff`, `hal/src/dho/scu.rs` tests | Table across lower/upper clamps, 200 us/div, 20 ms/div, intermediate values and holdoff > 250 ms; assert exact write lists/ticks and auto off for NORMAL. | Passing characterization expected. Extend `auto_trigger_200us`; preserve unrelated shadow bits. |
| `fast_auto_preserves_holdoff_and_mode`, `hal/src/dho/seq.rs` tests | Configure normal/fast AUTO and NORMAL/SINGLE; assert 10/100 ms fast values, long holdoff preserved, and enable bit follows sweep. | Passing characterization expected; retain `fast_auto_trigger_takes_the_shortest_times`. |
| `auto_records_do_not_clock_ui`, `ui/src/ui/host.rs` tests | Use a hermetic `Source::Hw` shared queue, no acquisition thread. Explicitly step through empty intervals, then inject a fresh frame and `trig_fired = Some((id, Some(false)))`; assert record count advances only on delivery, `AUTO` status, min-persistence map unchanged on empty steps. Exercise true TPU status and the WAIT boundary separately. | Passing characterization expected; extend coverage of `hardware_trigger_status_overrides_the_software_one`. This does not prove re-arm timing. |
| `demo_auto_is_immediate_but_normal_and_single_wait`, `ui/src/ui/host.rs` tests | Unreachable trigger level on Demo, fixed RNG/step sequence; assert AUTO records are untriggered on the first step, NORMAL/SINGLE make none, and a matching trigger still succeeds. | Passing characterization expected; explicitly guards against mistaking Demo cadence for SCU timing. |
| `sparse_rest_24ms_respects_period`, `ui/src/pacing.rs` tests, owned by M11 | Extend the existing `run` helper with 24 ms rest, 5 ms ready, multiple start phases and bounded rest jitter. Assert read gaps and ready-before-step deadlines after an explicit warm-up. Keep startup/no-phase and impossible-rest cases. | The strict <= 1000/30 ms gap assertion fails today on 33.5 ms gaps in the probe. Record the failing sequence, resolve precision in the M11 spec, then prove red/green there. Do not introduce a tolerance solely to turn it green. |
| `auto_delivery_survives_worker_backlog`, `ui/src/ui/host.rs` tests after M1 | Feed real scripted batches with explicit frame counts, completion acknowledgements/barriers and a held worker; step while no new batch is ready, then release it and verify delivered counts/status and unchanged golden pictures. Run frames and batch switches with fixture-owned pipes. | Characterization unless it exposes a reproducible defect. Use M1's actual post-merge API; no process-global pipe/environment coupling. |

No source tests were added or run in this planning pass. Do not claim any of the expected-green AUTO tests as a
failing-before regression. The strict Fill probe is evidence for an M11 boundary test, not a demonstrated AUTO bug.
If post-merge characterization exposes an AUTO defect, first add the smallest input sequence to the relevant test
above and record its failure on the exact parent. If a shorter timeout is chosen, the new pure write-list test must
pin the approved timer values, fail on stock values, and pass with the change; producer tests must then use M3.0 to
pin arm/wait/read/prearm ordering under that policy. An injected delivery schedule cannot prove that policy works.

Assert counts, status, maps, write lists and logical deadlines. Never assert correctness through `sleep`, elapsed
wall time or retry loops. Worker waits use explicit completion signals; a timeout may bound a hung test but cannot
serve as evidence of correct pacing. Run new tests in debug/release and the full `cargo xtask check` before a commit.
Preserve M1 golden references for this characterization-only increment. Any future visible change needs targeted
AUTO/WAIT golden pictures and a reason for changed references, plus cross-vendor Claude review of the exact revision.

### Prospective implementation paths and sequencing

These are future needs, not reservations against active work:

- Tests: `hal/src/dho/scu.rs`, `hal/src/dho/seq.rs`, `ui/src/ui/host.rs`. Documentation:
  `docs/tasks/0037-auto-timeout-pacing.md`, `docs/architecture/acquisition.md` and `docs/fork/stutter.md` to clarify
  acquisition timers versus redraw cadence without rewriting historical unit evidence. No production edit is
  justified yet. The plan commit message is `target/COMMIT_MSG.txt` (ignored local output).
- M11 handoff: `ui/src/pacing.rs` (Fill policy/tests), with the acquisition-loop owner handling any wiring after its
  migrations. Keep M11's code and acceptance in its own task/claim; task 0037 does not implement it early.
- M1 currently owns `ui/src/board/mod.rs`, `ui/src/board/dho/mod.rs`, `ui/src/board/dho/scripted.rs`,
  `ui/src/drawpipe*`, `ui/src/ui.rs`, `ui/src/ui/script*`; roll port owns `ui/src/board/dho/roll.rs` and
  `ui/src/board/dho/hw.rs`. Edit none of these during preparation. After both merges, refresh source citations,
  inspect their interfaces and request only the exact released paths needed for the tests. Reuse M1's fixture-owned
  pipe and acknowledgement facilities; do not build a competing harness or change Board contracts here.
- If implementation needs central test wiring, explicitly reserve `ui/src/ui.rs` only after M1 releases it.
  Any producer-policy change would need a separate explicit reservation of `ui/src/board/dho/hw.rs` (or its
  post-migration replacement), plus the responsible author's coordination. No roll rewrite is planned.

### Estimate: host work

Plan preparation: about half a day. After M1 and roll merge: 1-1.5 days for refreshed characterization, hermetic
tests and durable docs, plus about half a day for full gate and independent cross-vendor review. M11 work is excluded
and stays in its migration slot. A newly approved AUTO policy would need a fresh estimate after its numeric contract
and producer harness are available; allow provisionally 1-2 additional host days, not an implementation commitment.

### Unit acceptance, separate from host work

No hardware access or acceptance is needed for this plan. No hardware was used or verified. A tests/docs-only
increment preserving timers needs no new AUTO-policy unit round; it cannot claim the physical cadence was verified.

If AUTO values or re-arm/fill behaviour change, reserve about half a day of unit acceptance after the owner's
task-specific release and any required install go, under an appropriate lease. Compare no-edge AUTO, a valid edge,
NORMAL, SINGLE, manual Force and long holdoff at 200 us/div and 20 ms/div (select the batch path, not roll/scan),
then autoset, STOP/RUN and knob changes. Record the installed revision, setup, new-record gaps separately from screen
fps, status transitions, worker/drop counters and step costs. Check the approved AUTO deadline, preserved genuine
trigger/holdoff behaviour, stable held trace and the relevant budgets. Exact t1/t2 FSM timing remains unverified until
that measurement. Never infer install/reboot permission from this plan. M11 separately requires round 5's setup B at
5.1 V, <= 1 % empty pictures where the real event rate meets the drawing acceptance condition.

## Status

- 2026-11-15: Implementation validation on parent `1fe7150aa953b9ebcdd1e714aabd319c848696ee`, with only the
  three reserved files modified. Debug and release commands
  `cargo test --locked -p oos-ui [--release] auto_timeout:: -- --nocapture` each exited 0:
  3 passed, 0 failed, 0 ignored (582 filtered out); the filtered integration target ran 0 tests.
  Final `cargo xtask check --fast` exited 0: fmt, size limits, architecture, mutable statics and unsafe all `ok`;
  `partial gate passed (not done): 5 steps in 11 s`. Logs: `target/t0037-debug.log`,
  `target/t0037-release.log`, `target/t0037-fast-gate.log`. `git diff --check` passed; proposed message at
  `target/COMMIT_MSG.txt` passed its privacy scan with 0 findings. The M11 probe intentionally exited 101 as
  recorded in task 0012, then its scratch source/binary were removed. No production AUTO defect found.
  Standard cross-vendor review and the coordinator's integration full gate remain pending. No full gate,
  golden reference verification, hardware access, staging, commit, push or merge performed.
- 2026-11-15: Added the three requested characterization tests in the test-only `host::auto_timeout` module.
  `auto_records_do_not_clock_ui` pins record counts, held Min maps, AUTO/TPU status and the exact 0.5 s WAIT boundary.
  `demo_auto_is_immediate_but_normal_and_single_wait` pins first-step AUTO fallback, waiting NORMAL/SINGLE and
  matching edges (SINGLE records its trigger and ends in STOP). Initial test expectations used a threshold at the
  bottom of CH1's 0..2.5 V signal and expected TRIG'D after SINGLE stopped; corrected those expectations only.
  `auto_delivery_survives_worker_backlog` holds one real worker with channel acknowledgements for scripted raw
  batches of 3/13/27 frames, steps an empty UI, then checks exactly-once delivery, AUTO, all frames accounted for,
  zero drops and combined maps equal to the all-frames route. It uses a fixture-owned pipe and M1's return barrier;
  the UI fixture stays in frames mode, so integrated global-pipe wiring and acquisition production are not covered.
  Debug run: `cargo test --locked -p oos-ui auto_timeout:: -- --nocapture`, exit 0, 3 passed.
  Recorded the strict 24 ms-rest failure sequence in task 0012's Status; no sparse pacing test added here.
  Validation follows in the newer Status entry. No production defect found, production edit, golden change or unit use.
- 2026-11-15: Implementation assigned on `t0037-auto-timeout` at `1fe7150`; standard review tier.
  Reserved the paths above for the three requested host tests and M11's sparse-rest evidence only.
  Production code, timer policy, golden files and held paths stay outside this increment. No commit or full gate.
- 2026-11-15: Codex planning assignment on `t0037-auto-timeout`, base `overhaul` at
  `1d5fa1b63cbb2c94e4115a4b0fed5017f07353d9`. Claimed this task file only for the requested plan; implementation
  claims wait for M1 and roll integration. Read the review protocol, architecture pacing/budgets and M11.
  Characterized timer ownership and host limits. A disposable Rust probe of the unmodified Fill planner compiled
  and exited 0: 24 ms rest/5 ms ready yielded steady 8/9 ms fills and 32.5/33.5 ms read gaps. Scratch files removed.
  No source edits, hardware access, commit or full gate run; plan awaits independent review before integration.
- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0037-auto-timeout by the coordinator, base overhaul. Plan first; implementation after M1 and the roll forward-port merge.
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
