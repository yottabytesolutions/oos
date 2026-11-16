---
id: 85
title: Measure the P0 frame-time breakdown
kind: feature
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Prepare one unit session to measure the P0 frame-time breakdown. Host part only;
review tier: host. No hardware access, commits, installation or publication here.

## Plan

Reserved paths: `docs/tasks/0085-perf-p0-measurement.md`, `docs/spec/state-query.md`,
`ui/src/ui.rs` explicitly (step and render timing only), `ui/src/ui/step_draw.rs`,
`ui/src/ui/change.rs` and `ui/src/ui/change/intake.rs` (timing accessor only), `ui/src/perfguard.rs`,
`ui/src/perfguard/draw.rs`, `ui/src/ui/tests/change_picture_contracts.rs`,
`ui/src/main.rs` (allocation-test timing export), `ui/tests/snapshots/hot_paths.rs`,
`tools/oos-stress/`, `tools/tests/test_perfmap.py`, `tools/tests/fixtures/perfmap/`,
`target/t0085-evidence/`, `target/COMMIT_MSG.txt`.
The owner's explicit new-task assignment authorizes this claim on the supplied
worktree. Existing task claims for other seams remain with their owners.

1. Add fixed-ring draw sub-stage samples to StageTiming, append their state-query
   diagnostics, and extract only what the size ratchets require. Preserve pixels,
   execution order, settings and acquisition behavior.
2. Build a stdlib-only, typed, read-only host collector using one paced SCPI socket
   and existing SSH/lease conventions. Preserve raw snapshots, counter deltas,
   identities and missing/reset observations in per-point JSON and a Markdown table.
3. Test parser fixtures, timing/state-query contracts and allocation recording.
   Run focused UI tests in debug and release, unchanged layout/goldens, Python
   tests, the gate's exact clippy step, and `cargo xtask check --fast`.
4. Document the P0 A-G session matrix and settle times, with H optional and requiring
   owner go. Record command exits and evidence; prepare one commit-message section.
   Cross-vendor review and the full integration gate remain coordinator work.

## Unit session

All unit behavior and timing below are **not yet verified on the unit**. Obtain
owner release and an existing task 0085 test lease before setting points. An
observe lease suffices for the collector alone. Use the documented `scope-lease`
acquire/check/renew/release flow, setting `OOS_SCOPE`, `OOS_LEASE_HOLDER`,
`OOS_LEASE_TASK=0085` and the acquired token. `perfmap.py` checks and renews that
existing lease; it never acquires, breaks or releases one. Release explicitly
at the end. Stop at the first unexpected unit state.

Use the same stable input signal and recorded wiring for A-G. Keep the signal
source independent of the setup reset, or reapply its recorded configuration
after every BASE. Feed the enabled
channels consistently; record missing wiring rather than comparing unlike loads.
Keep the published candidate, display size, renderer and background traffic fixed.
Record build identity (`*IDN?`), initial setup (query each changed setting, including `:SYST:GET? hdmi`), wiring and HDMI mode
in private host session notes. Do not install or reboot in this session script.

Send each setup command below over SCPI port 5555, at about 150 ms pacing.
Every point starts with BASE, then its listed overrides. A semicolon separates
absolute SCPI commands; it is not a shell command. Read `:SYST:ERR?` after setup;
proceed only on `0,"No error"`. Applying settings belongs to this operator script;
the collector sends only `:SYSTem:STATe?` and fixed read-only SSH commands.

BASE (one channel, Normal, 1 us/div, Auto depth, no measurements, vectors,
minimum persistence, color grading, zoom off, one scope view):

```text
:VIEW SCOP;*RST;:DISP:VIEW:COUN 1;:DISP:VIEW 1
:CHAN1:DISP ON;:CHAN2:DISP OFF;:CHAN3:DISP OFF;:CHAN4:DISP OFF
:CHAN1:SCAL 1;:CHAN2:SCAL 1;:CHAN3:SCAL 1;:CHAN4:SCAL 1
:TIM:VERN OFF;:TIM:SCAL 1e-6;:TIM:OFFS 0;:TIM:ROLL ON
:TIM:DEL:ENAB OFF;:ACQ:TYPE NORM;:ACQ:AVER 16;:ACQ:MDEP AUTO
:TRIG:MODE EDGE;:TRIG:EDGE:SOUR CHAN1;:TRIG:EDGE:LEV 0;:TRIG:SWE AUTO
:MEAS:CLE ALL;:DISP:TYPE VECT;:DISP:GRAD:TIME MIN;:DISP:COL ON;:RUN
```

For each table row, expand each comma-separated name/value to its own point.
Do not change setup during its two snapshots. Wait the listed settle time after
the final setting, then measure for 60 s. At least 256 completed UI steps should
have replaced the preceding setup's ring contents before snapshot 1; extend
settling if picture rate is below 256 divided by settle seconds. Snapshot 2
should have `n=256` and numeric p99 for the step and panel render stages.
`draw.view_render` needs actual HDMI layout renders; it stays pending when
that target is inactive and has its own call count. Slow producer clone
rings have a different horizon; record pending rather than treating it as zero.

| Group / point names | Overrides after BASE (one value per named point) | Settle |
|---|---|---|
| A-5ns, A-20ns, A-200ns, A-1us, A-10us, A-100us, A-1ms, A-10ms, A-20ms | `:TIM:SCAL 5e-9`, `2e-8`, `2e-7`, `1e-6`, `1e-5`, `1e-4`, `1e-3`, `1e-2`, `2e-2` respectively; repeat the full `:TIM:SCAL` header for each value | 12 s |
| A-50ms-roll, A-100ms-roll, A-200ms-roll | `:TIM:SCAL 0.05` / `0.1` / `0.2`; `:TIM:ROLL ON` (automatic roll threshold is 50 ms/div) | 20 s |
| B-200ns-normal, B-200ns-peak, B-200ns-avg16, B-200ns-hires | `:TIM:SCAL 2e-7`; then `:ACQ:TYPE NORM` / `PEAK` / `AVER` / `HRES`; Average additionally `:ACQ:AVER 16` | 12 s |
| B-10us-normal, B-10us-peak, B-10us-avg16, B-10us-hires | `:TIM:SCAL 1e-5`; same four acquisition sequences as above | 12 s |
| C-1us-1ch, C-1us-2ch, C-1us-4ch | `:TIM:SCAL 1e-6`; 1ch needs no channel override; 2ch adds `:CHAN2:DISP ON`; 4ch adds `:CHAN2:DISP ON;:CHAN3:DISP ON;:CHAN4:DISP ON` | 12 s |
| C-10us-1ch, C-10us-2ch, C-10us-4ch | `:TIM:SCAL 1e-5`; same three channel sequences as above | 12 s |
| D-auto, D-1M, D-10M | `:CHAN2:DISP ON;:CHAN3:DISP ON;:CHAN4:DISP ON;:TIM:SCAL 1e-3`; then `:ACQ:MDEP AUTO` / `1M` / `10M` | 20 s |
| E-min, E-100ms, E-1s, E-inf | `:DISP:GRAD:TIME MIN` / `0.1` / `1` / `INF` | 12 s |
| E-dots | `:DISP:TYPE DOTS` (compare E-min vectors) | 12 s |
| E-grade-off | `:DISP:COL OFF` (compare E-min color grading on) | 12 s |
| E-zoom | `:TIM:DEL:SCAL 2e-7;:TIM:DEL:OFFS 0;:TIM:DEL:ENAB ON` (compare E-min zoom off) | 12 s |
| E-zoom-dots | `:TIM:DEL:SCAL 2e-7;:TIM:DEL:OFFS 0;:TIM:DEL:ENAB ON;:DISP:TYPE DOTS` | 12 s |
| F-0meas | `:MEAS:CLE ALL` | 12 s |
| F-6meas | `:MEAS:ITEM VPP,CHAN1;:MEAS:ITEM VAVG,CHAN1;:MEAS:ITEM VRMS,CHAN1;:MEAS:ITEM FREQ,CHAN1;:MEAS:ITEM PER,CHAN1;:MEAS:ITEM RTIM,CHAN1` | 12 s |
| G-panel | Monitor physically disconnected; `:SYST:SET hdmi = Mirror` | 20 s |
| G-mirror | Monitor connected at the recorded resolution; `:SYST:SET hdmi = Mirror` | 20 s |
| G-waveform | Same connected monitor; `:SYST:SET hdmi = Waveform` | 20 s |
| G-layout | Same connected monitor; `:SYST:SET hdmi = Layout` | 20 s |

This is 43 windows, about 54 minutes of settling/measurement plus setup, checks
and wiring. Run G last so A-F use the same disconnected-monitor baseline.
The collector accepts a list of names for repeated windows of the **current**
setup; it does not switch between table rows. For example, after applying
BASE plus A-5ns, run on the host:

```sh
python3 tools/oos-stress/perfmap.py --out target/perfmap-session --seconds 60 --settle 12 A-5ns
```

Set `OOS_SCOPE` explicitly or pass `--host example.invalid`. The existing SSH
options `OOS_SSH_PORT` and `OOS_SSH_IDENTITY` are honored with strict host-key
checking. For D and G use `--settle 20`. One socket is used for all windows of
an invocation, with at least 150 ms between requests. Each point writes its own
JSON; `report.md` includes previous point JSON files in that dedicated output
directory. Existing point JSON is never overwritten. Use new names for repeats.

Each JSON preserves both raw STATe replies, parsed diagnostics, raw framed SSH
files, parsed counters, requested and actual intervals and deltas. CPU is in
USER_HZ ticks (also record device `CLK_TCK` when available); cpufreq residency
is in the kernel's time_in_state ticks, devfreq residency in milliseconds.
The Markdown table shows CPU ticks/s and draw p50/p99 in ms. Divide CPU ticks/s
by device CLK_TCK to obtain cores consumed; do not assume a host tick rate.
UI CPU rates use the process total, including reaped helper threads, rather
than a sum of sampled threads. JSON records the process delta as `delta.ui` and
the count of one-sided tids as `delta.expected_thread_churn`; scoped helper
births/exits are expected and do not invalidate that rate. Per-thread CPU,
faults, context switches, initial/final processor, per-CPU xdma
IRQs and frequency/transition deltas remain in JSON. Processor endpoints do not
count migrations between snapshots. SSH and SCPI observations are sequential,
not atomic; their own monotonic start/end times bound collection skew. Actual
intervals may exceed the requested duration if a read is slow. Missing files,
unsupported stats, counter resets and process restarts are explicit missing
or null observations. One-sided thread rows retain their unavailable delta;
long-lived thread identity changes and counter resets still flag the report.
Do not interpret them as zero cost. An SSH/SCPI failure stops the collector
without reconnecting or attempting recovery.

H is optional and requires separate owner go. There is currently no SCPI command
for CPU/GPU minimum clock floors. Do not invent one or put sysfs writes in
perfmap. If separately approved, the operator uses an approved reversible clock
procedure to compare BASE and D-10M at original clocks, policy4 minimum
1416 MHz, GPU minimum 600 MHz, and both floors; allow 20 s settling and a 10-minute
thermal observation at the combined floor. Reuse the corresponding BASE/D SCPI
commands above, and label windows H-original/H-cpu/H-gpu/H-both/H-restored.
Capture original values before that procedure and verify exact restoration
before H-restored. This host task supplies no clock writes, install or reboot
authorization. H is excluded from the 43-window A-G session.

Restore the recorded initial setup through SCPI after A-G, check errors, save
host results and release the lease. No register access, calibration writes,
clock changes or filesystem writes are made by the measurement commands;
mandatory lease bookkeeping is handled by the existing lease helper.

## Status

- 2026-11-16: Round 3 fixture sensitivity check rejects a swap of the first two
  diagnostic blocks at the new real-handler label-order assertion (expected
  exit 101). The fixture is restored byte for byte; the real-handler test then
  passes, exit 0. Source/fixture hashes match the passing debug/release, exact
  clippy and fast-gate candidate. Placement and Status-order negative controls
  reject the prior arrangements and pass the final candidate. Updated
  `target/COMMIT_MSG.txt`; evidence and final hashes are in
  `target/t0085-evidence/round3-*`. Task stays active for Claude fix verification
  and integration. No commits, pushes, ratchet edits or hardware access.
- 2026-11-16: Round 3 checks pass, all exits 0: 17 collector tests; debug and
  release perfguard 19 passed/one existing benchmark ignored, draw contracts
  six passed, state-query six passed, intake timing two passed, draw allocation
  one passed, existing hot paths one passed and observer census one passed.
  Exact unchanged `steps::clippy` passes across six host and five ARM crates:
  hal 93 entries/177 total, dsp 115/262, core 3/46, ui 246/666, fpgad 7/19,
  xtask 0/0. `cargo xtask check --fast` prints
  `partial gate passed (not done): 8 steps in 35 s`, exit 0. All eight selected
  steps pass; the timing accessor fits the existing size limits. Full gate,
  coverage, independent fix verification and unit measurements remain pending.
  Complete commands/logs/exits: `target/t0085-evidence/round3-*`.
- 2026-11-16: Round 3 fixes implemented. `delta.ui` uses the process stat pair;
  UI ticks/s no longer sums sampled threads. `delta.expected_thread_churn`
  counts one-sided tids while preserving unavailable thread rows and long-lived
  thread counters. Process restarts, counter resets and reused long-lived tids
  still invalidate the relevant observations. The real state-query host test
  compares the full ordered diagnostic label sequence with the fixture.
  `timing_mut` now sits directly beside `timing` in `change.rs`; size verification
  follows without ratchet edits. Status completion entries were reordered using
  their recorded exit times. The placement/order check rejects the prior layout
  and inversions, then passes the candidate. Initial collector regression run
  fails as expected (exit 1); both old report mutations independently fail the
  new tests. All 17 fixed collector tests and focused debug UI checks pass.
  Evidence: `target/t0085-evidence/round3-*`. Release, exact clippy and fast gate
  are running. No commits or unit access.
- 2026-11-16: Round 3 review fixes started on the existing uncommitted claim.
  Plan: use process CPU deltas for UI rates, count expected transient threads
  separately, bind fixture diagnostic label order to the production handler,
  colocate timing accessors if the unchanged size limits permit, and retain
  newest-first Status. Add regression evidence, run collector tests, focused
  debug/release UI tests, exact clippy and the fast gate. No commits or unit access.
- 2026-11-16: Round 2 checks complete on the uncommitted candidate at unchanged
  HEAD `8fafc7196d1ebc02a3dedd98c8c6d47775e55c6d`. Task remains active for
  independent fix verification and integration. No commits, pushes, ratchet
  edits or hardware access. Updated `target/COMMIT_MSG.txt`; current source
  hashes and command/log/exit evidence are under `target/t0085-evidence/`.
- 2026-11-16: Round 2 `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 36 s`. Selected fmt, size,
  architecture, mutable-statics, unsafe, method-exceptions, known-failures and
  agent-rules all pass. Exact unchanged `steps::clippy` exits 0 across six host
  and five ARM crates: hal 93 entries/177 total, dsp 115/262, core 3/46,
  ui 246/666, fpgad 7/19, xtask 0/0. Logs: `round2-fast.log` and
  `round2-clippy-step.log`, each with a matching `.exit` and `.command`.
- 2026-11-16: Round 2 Python collector tests: 14 passed, exit 0. Full
  `python3 -I tools/check_inventory.py python-tests` exits 1: 165 tools tests
  executed, with only existing `test_host_sockets` failing because TCP listeners
  are sandbox-denied. Separate privacy (69 tests) and UI-tool (eight tests)
  discovery passes, exits 0. The same verified host loopback session harness
  was retried and exits 1 before readiness with `Operation not permitted`;
  no setup/query was sent. Logs: `round2-python-tests.log`,
  `round2-python-privacy.log`, `round2-python-ui-tools.log` and
  `round2-loopback.log`. The fixture is a real production-handler reply, but
  TCP loopback recapture remains pending on a host permitting local listeners.
  Full integration gate, coverage, cross-vendor fix verification and unit
  measurements remain pending; the failed Python gate is not reported green.
- 2026-11-16: Round 2 focused tests pass in both debug and release, all exits 0:
  perfguard 19 passed/one existing explicit benchmark ignored, draw contracts
  six passed, state-query six passed, timing allocation one passed, existing
  hot paths one passed and observer census one passed. The timing allocation
  guard covers 1024 recordings of each panel and HDMI view ring plus step
  recording, including wrap, with zero allocations. Existing unchanged golden
  evidence below is retained; this round only changes diagnostic routing.
- 2026-11-16: Round 2 fixes implemented. The collector validates the real fps
  envelope and all seven draw rows, preserves duplicate diagnostic blocks as
  lists, stops blocks at trailing non-fields, reports SSH stderr on exit/timeout
  and uses explicit TypedDict capture/delta/snapshot rows. HDMI layout rendering
  has its own `draw.view_render` ring and state-query-236; state-query-231 names
  deep raw record conversion. The fixture is an unmodified production SCPI
  handler reply from the host regression, with provenance beside it; loopback
  capture remains pending because local TCP bind is sandbox-denied.
- 2026-11-16: Regression proof: the real-reply collector test fails with the old
  records prefix; the parser test fails separately with trailing-token scanning
  and duplicate overwrite restored; the host render test fails when HDMI samples
  are put back in the panel ring (expected exit 101). Fixed focused debug suites
  pass: perfguard 19/one existing benchmark ignored, draw contracts six,
  state-query six, timing allocation one, hot paths one and observer census one.
  All 14 collector tests pass. Complete round 2 evidence is retained under
  `target/t0085-evidence/round2-*`; release, exact clippy and fast gate follow.
- 2026-11-16: Round 2 review fixes in progress on the existing uncommitted
  claim. Addressing the real fps envelope/fixture, duplicate diagnostic blocks,
  separate HDMI view render ring, fast conversion wording, SSH stderr and typed
  collector rows. Local TCP bind is still denied by the sandbox, so the loopback
  harness cannot run here; preserve that limitation and obtain a real reply from
  the production SCPI handler in a host regression test. No hardware access or
  commits authorized for this round.
- 2026-11-16: Host implementation prepared for cross-vendor host review; task
  remains active. No commits, pushes, hardware access, installation, dependency,
  hook or ratchet changes. `target/COMMIT_MSG.txt` has one plain imperative
  subject/body and `Task: 0085`. Full integration gate, coverage, independent
  Claude review and unit measurements remain pending. The Python gate needs
  a host environment permitting local TCP listeners before acceptance.
- 2026-11-16: Final `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 37 s`. All selected steps pass:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Size reports three UI improvements; ratchets
  remain unchanged. Gate output/exit: `target/t0085-evidence/fast-final.log`
  and `.exit`. Exact unchanged `steps::clippy` exits 0 across all six host and
  five ARM crates: hal 93 entries/177 total, dsp 115/262, core 3/46,
  ui 246/666, fpgad 7/19, xtask 0/0. Runner and full output are retained as
  `clippy-step.py`, `clippy-step.log` and `.exit` in the same evidence directory.
- 2026-11-16: Final `python3 -I tools/check_inventory.py python-tests` exits 1:
  163 tools tests executed, with only the existing `test_host_sockets` error
  caused by sandbox-denied TCP listener creation. All 12 new perfmap tests pass,
  including mocked CLI JSON/report persistence, fixture parsing, counter
  reset/churn and the two-snapshot/socket-pacing contract. The gate stops before
  its remaining roots; separate unchanged discovery runs pass 69 privacy and
  eight UI-tool tests (both exit 0). Preserve the failure: `python-tests.log`
  and `.exit`; do not describe this as a passing Python gate. Optional loopback
  session-command validation is likewise blocked before any setup is sent.
- 2026-11-16: Unchanged debug goldens pass (20 cases, 40 fresh processes,
  88 exact-repeat captures); release `layout-check.sh golden-check` passes
  (263 cases, 526 fresh processes, 1242 exact-repeat captures), exits 0.
  Optional acceptance of all 43 SCPI setup sequences via a verified loopback
  host could not start: sandbox denies TCP listener creation (`Operation not
  permitted`), exit 1. No unit access occurred. Logs and command exits remain
  under `target/t0085-evidence/`; no references were changed.
- 2026-11-16: Focused UI tests pass in debug and release, all exits 0:
  perfguard 19 passed/one existing explicit benchmark ignored, draw contracts
  five passed, state-query five passed, new draw-recording allocation guard
  one passed, existing hot-path guard one passed and observer census one passed.
  Final focused reruns include the start-up render early return and the narrow
  timing accessor. The counting allocator observes zero allocations across
  1024 new draw step/render recordings, including wrap; this is not a claim
  that every legacy Ui::step path allocates nothing. Commands, logs and exits
  are retained under `target/t0085-evidence/`. `source-sha256.json` records
  final changed source hashes; `git diff --check` passes.
- 2026-11-16: Implemented the six appended fixed-ring draw diagnostics and
  read-only SSH/SCPI collector, fixture parsers and 43-window A-G session.
  Reserved `ui/src/main.rs` for the allocation-test export and
  `ui/src/ui/change/intake.rs` for a narrow timing accessor; extracted stopped
  redraw/render preparation to stay below existing size ratchets. Initial
  debug allocation retry, existing hot-path and observer allocation guards pass.
- 2026-11-16: Initial fast gate passed: `partial gate passed (not done):
  8 steps in 61 s`, exit 0. Debug perfguard, draw contracts and state-query
  tests pass. Allocation-test linking failed with disk full; the runner exited
  1 when disk exhaustion also prevented recording Cargo's exit. Preserved its
  log and removed only this worktree incremental cache before retry.
- 2026-11-16: Claimed by Codex on `t0085-perf-p0-timers`, base `8fafc719`
  (observed with git log). Plan written before code. Host-only assignment;
  `tools/oos-stress/` does not exist at this base, so model SSH/SCPI on
  `tools/device-test.py` and lease checks on `tools/scope_lease.py`.
