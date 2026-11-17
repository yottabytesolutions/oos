---
id: 3
title: "M2: one change-detection point (D1a)"
kind: feature
status: active
release: 0.2.0
owner: codex-d
depends: [2, 20, 21, 22, 24, 25, 26, 27, 28]
---

# M2: one change-detection point (D1a)

## Goal

Complete [migration M2](../architecture/migration.md#m2-one-change-detection-point-d1a). The migration document is the specification.

## Plan

M2.3e review round 4: keep the accepted claim and isolate the reference Eye
from pure PlaybackPosition VIEW reclassification; all other reference-eye VIEW
resets remain. Restore the upstream strict idle holds without an inserted step.
Pin the running non-fresh SIM statistics immediately after synchronous finish
and asynchronous poll, then gate installation until StatsReset acceptance while
retaining trigger-miss holds. Correct consumer timing accounting and import order.
Run focused debug/release tests with full SIM/eye/change/playback counts, the
exact gate clippy step with a zero-new base comparison, and the fast gate.
Report the known protected-base size/unsafe failures without changing ratchets.
Update Status and the single plain-subject draft message. No commits or hardware.

M2.3e review round 3: keep the accepted claim and fix all seven findings and
optional cleanups. First pin STOP value/analysis/job retention and the running
non-fresh SIM configuration reset with failing-before tests. Restore the strict
SIM acceptance helper and express task 0066 question 1's owner decision locally
in each affected test, including both ignored siblings. Keep transitions in G2
and sample consumers alone. Restore documentation and readable assignments;
move tests when size requires it. Record terminal autoset deferral and poll
ordering in Status and the difference inventory. Run focused debug/release,
soak/census, isolated timing, exact clippy (zero new findings) and the fast gate.
Update the imperative draft message. No commits or hardware access.
Additional reserved paths: `xtask/data/moves.txt`, `ui/src/ui/sim/` for review tests/test moves and
`ui/src/ui/eyepage/` for reset cleanup and test moves.

M2.3e coordinator follow-up: record playback position writes at their source as
presentation requests carrying both endpoints through the change point. Classify
only the matching driver delta as VIEW; user delay edits retain ACQ|STATS.
Pin N-frame accumulation and one user reset during playback, keeping both
upstream regressions unchanged. Trace Bode restoration and replace any fixed
step/timing assumption with a bounded condition wait, retaining all assertions.
Run Bode 30 times, focused debug/release, soak/census, isolated timing, exact
clippy (zero new findings) and the fast gate. Update the single draft-message
section. No commits. Additional reserved core paths: `core/src/change/{mod,shadow,mapping,presentation}.rs`;
existing request contracts/tests and UI reservations cover the remaining edits.

M2.3e claim: owner `codex-d`, branch `t0003-m23e`, base `11a8b2b9`, unit tier,
under the owner's assignment. Inventory every former reset trigger against the
accepted class table before edits. Switch statistics, history/segments, mask,
zone, hardware average, math and declared views to one accepted consumer per
effect; preserve run/completion and request semantics, keyed decode and the M4
worker-view key. Remove legacy marks and normalization notification bridge.
Retain independent shadow evidence until table/list acceptance is established.
Pin changed behavior with failing-before tests and unchanged behavior explicitly.
Render base and candidate scripted pictures directly; explain every difference.
Keep detection in G2 and reset work in its own measured phase without budget
changes. Run focused debug/release, soak/census, isolated timing, exact inherited
clippy comparison, size comparison and fast gate. One draft-message section;
no commits or hardware access.
Reserved paths: `ui/src/ui.rs` explicitly; `ui/src/ui/change/`,
`ui/src/ui/change.rs`, `ui/src/ui/tests/`, remaining mark callers under
`ui/src/ui/`, `ui/src/{cmd,scpi}.rs`, `ui/src/scpi/`, `ui/src/perfguard.rs`,
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.
Core class contracts and tests only if a missing former trigger is demonstrated;
no dependency, ratchet, golden reference or upstream rebaseline edits.

M2.3d claim: owner `codex-d`, branch `t0003-m23d`, base `d7418b0d`, unit tier,
under the owner's assignment. Switch mapping/phosphor only. Remove dirty-map state
and every writer (including upstream additions); retain observation-only legacy
signals and the normalization statistics bridge until e. Centralize accepted
picture invalidation and worker-view publication, preserving all old worker-key
inputs through M4 and ordinary Min/decay/STOP drawing. Pin changed and unchanged
behavior on the base before production edits. Compare base/candidate scripted
captures directly without re-blessing. Run focused debug/release, soak/census,
isolated timing, exact gate clippy and fast gate. No commits or hardware access.
Review-round plan: reproduce all seven findings before their fixes; audit each base dirty-map seam and effective field contract; separate detection and picture timing, derive the picture budget with the existing 4x A72 estimate, and retain unit storm/final-read acceptance. Also reserve `ui/src/perfguard.rs`, `ui/src/ui/tests/mapping/sessions.rs` and `ui/src/drawpipe/tests/publication.rs` for these fixes and regressions. No commits.

Round 3 plan: sample actual picture clears only, report count/p99/max, calibrate
both release storms with at least ten runs and the worst observed p99, re-render
base/final candidate, move review regressions to a named test module and assert
oracle counters. Reuse the accepted claim; additionally reserve
`ui/src/ui/tests/change_picture_contracts.rs`. Run focused debug/release, timing,
exact clippy and fast gate; update one draft-message section, no commits.

Reserved paths: `ui/src/ui.rs` explicitly, `ui/src/ui/change.rs`,
`ui/src/ui/change/`, `ui/src/ui/tests/{mapping,change,shadow}.rs`,
`ui/src/ui/tests/mapping/`, `ui/src/ui/{marks,autoset,analysis,deskew,offline,workflow,measure_pairs,trigger_controls,bus_candidate,workspace,sim}.rs`,
`ui/src/ui/meascatalog/backend.rs`, `ui/src/ui/bode.rs`, `ui/src/ui/analysis/window/tests.rs`,
`ui/src/ui/script/batch.rs`, `ui/src/drawpipe.rs`, `ui/src/drawpipe/feed.rs`,
`core/src/change/{mapping,values,display2}.rs`,
`core/src/change/tests/contracts/display2.rs`, `core/src/change/tests.rs`, `ui/src/drawpipe/tests/`,
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.
New helpers stay within these existing modules; no ratchet edits or upstream rebaseline.

Integration coverage failure repair: owner `codex-d`, branch `t0003-m2`, base
`0137955`, under the owner's assignment for batch integ-19. Trace partial SINGLE
completion and prove whether the 15 ms record budget changes completion or only
the step in which it happens. Reproduce the boundary before correcting the fixture;
pin same-step and separate-step delivery without relaxing completion/queue checks.
Run the regression 200 times in debug, release and the xtask coverage runner with
parallel CPU load, the complete UI library coverage suite, exact gate clippy and
`cargo xtask check --fast`. No commits or hardware access.
Reserved paths: `ui/src/ui/tests/outbox.rs`, this task, `target/COMMIT_MSG.txt`;
`ui/src/ui.rs` explicitly for a temporary zero-budget diagnostic only, restored
before final verification. Production changes require evidence of a completion race.

SINGLE/segment verification round: owner `codex-d`, branch `t0003-m2`, base `b53aee1`,
unit tier under the owner's assignment. Preserve completed STOP segment browsing across
CLEAR and pair changes without hardware reconfiguration; cancel discarded browse requests
and clear UI segment state after a real reconfigure. Pin both cases before fixes.
Carry optional terminated SCPI responses through VXI-11 and audit other remote callers,
with per-transport regressions. Run focused default-thread debug/release tests, the exact
gate clippy step and `cargo xtask check --fast`. No commits or hardware access.
Reserved paths: existing SINGLE reservations plus `ui/src/vxi11.rs`, `ui/src/vxi11/`,
`ui/src/main.rs` (host VXI response callback only), `ui/src/system/device/remote.rs`
(device VXI response callback only), `ui/src/http.rs` (MCP SCPI response distinction only),
`ui/src/http/tests/empty_tests.rs`, and the segment browse regression submodule.
Keep one section in `target/COMMIT_MSG.txt`.

SINGLE fix review round: owner `codex-d`, branch `t0003-m2`, base `36c2173`,
unit tier under the owner's assignment. Bind segmented runs to their start identity,
abort on every pair boundary, restart incomplete scan SINGLE sweeps, and share the
pure hold/eligibility/latch decisions with the host fixture. Reproduce the setup-change
segmented WAIT and empty segment-times SCPI reply on the fake path before fixing.
Exercise Recording/Reading boundaries, current-pair segment counts and hidden hardware
averages. Record accepted STOP-after-CLEAR and scan-RUN hold semantics plus unit smoke.
Run focused default-thread debug/release tests, soak/census, isolated release timing,
unchanged goldens, exact gate clippy and `cargo xtask check --fast`. No commits or
hardware access; keep one section in `target/COMMIT_MSG.txt`.
Reserved paths: `ui/src/board/dho/hw.rs` explicitly, `ui/src/board/dho/hw/`,
`ui/src/board/dho/roll.rs`, `ui/src/board/dho/publication.rs`,
`ui/src/ui.rs` explicitly, `ui/src/ui/change/transitions.rs`,
`ui/src/ui/change/acquisition.rs`, `ui/src/ui/tests/outbox/single_reconfigure.rs`,
`ui/src/ui/tests/outbox/single_reconfigure/`, `ui/src/board/scripted.rs`,
`ui/src/scpi.rs`, `ui/src/scpi/`, `ui/src/wave.rs`,
`docs/architecture/README.md`, `xtask/data/moves.txt`, this task and
`target/COMMIT_MSG.txt`. Extract only the existing segment worker into a generic
bus/reader seam so fake tests execute its real Recording/Reading states.

M2.3c unit-smoke finding: owner `codex-d`, branch `t0003-m2`, base `fbe6211`,
unit tier under the owner's assignment. The coordinator already merged task 0061
from `overhaul` (first parent `fcf8ead`); clean start, no merge needed here.
Keep worker SINGLE eligibility bound to the acquired pair, so an incomplete SINGLE
can replace an obsolete batch without a UI-drop-triggered arm. Preserve deferred
completion STOP and its publication ordering. Add failing-first scripted/fake
producer regressions for late old records, completed records, CLEAR, and roll/scan/deep
reconfiguration. Use `acq_single_arm` and stream lifecycle counters, add the unit
scenario, then run focused default-thread debug/release tests, soak/census, release
timing, unchanged goldens, the exact gate clippy step and the fast gate. No commits,
merges or hardware access; one section in `target/COMMIT_MSG.txt`.
Reserved paths: `ui/src/board/dho/hw.rs` explicitly,
`ui/src/board/dho/hw/single_fake.rs`, `ui/src/board/scripted.rs`,
`ui/src/ui/tests/outbox.rs`, `ui/src/ui/tests/outbox/single_reconfigure.rs`, `docs/architecture/README.md`, this task,
and `target/COMMIT_MSG.txt`. Additional seam: `ui/src/board/dho/roll.rs` for the shared stream poll to honor
STOP/rearm in scan as well as roll. No new UI completion state is needed: the
existing outbox applies STOP before publishing changed settings. Also reserve
`ui/src/ui/change/transitions.rs`: keep STOP held after CLEAR removes the retained
record, using the existing `cleared` state rather than another completion flag.
Reserve `ui/src/ui/change/legacy_keys.rs` to independently observe slow-mode changes;
the mode-only regression exposes a missing legacy acquisition observation.

M2.3c review round 1: owner `codex-d`, branch `t0003-m2`, base `3882594`,
unit tier under the owner's assignment. Apply P2-1 and P3-1 through P3-4;
record the accepted P3-5/P3-6 semantics and pending reviewer smoke in Status.
Move autoset after record/measurement folding, guard writer entry against settings drift,
skip idle writers, retain lifecycle polls on rejection, and narrow the sweep companion.
Reuse the accepted c reservations, including `ui/src/ui.rs` explicitly, and reserve
`ui/src/ui/autoset/recovery.rs` only if the existing begin-step fixture needs updating.
Pin failing regressions before fixes. Run focused debug/release with default threads,
soak/census, release timing, unchanged goldens, exact gate clippy and the fast gate.
Keep one section in `target/COMMIT_MSG.txt`; no commits or hardware access.

M2.3c implementation claim: owner `codex-d`, active on `t0003-m2`, base `adb72ff`,
unit tier under the owner's assignment. Complete remaining internal STOP and autoset/Bode
outbox writes, guard pending completion against extra records, and centralize accepted
run-transition consumers. Preserve the normalization notification bridge and all d/e
consumers. Keep stepping on rejected changes and preserve epoch-only rejection without
hardware reconfiguration. Hardware smoke runs independently and holds integration only;
its findings may require a revision. No hardware access or commits.
Reserved paths: `ui/src/ui.rs`, `ui/src/ui/change.rs`, `ui/src/ui/change/`,
`ui/src/ui/{autoset,bode,hwmask}.rs`, `ui/src/ui/tests/{outbox,epochs,shadow,roll}.rs`,
`ui/src/ui/host.rs` (existing SINGLE completion fixture only),
`core/src/change/{mod,shadow}.rs`, `core/src/change/tests/requests.rs`
(request-boundary sweep classification when external RUN overrides pending STOP),
`ui/src/ui/script/batch.rs` (only if completion accounting requires it),
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.
First pin parent-compatible completion regressions, then implement bounded command
capture and accepted run transitions; test focused debug/release with default parallelism,
soak/census, release timing and goldens with per-case reasons for changes. Run all exact
xtask clippy invocations with zero new/grown findings and the fast gate. Retain complete
logs and a single draft-message section. Independent unit-tier review and coordinator
full gate remain required.

M2.3b clippy follow-up: owner `codex-d`, active on `t0003-m2`, under the
owner's approved M2.3b assignment after the overhaul merge. Preserve behaviour
while extracting named test helpers, using struct update syntax and checked
division, and routing locks through the existing HAL helper. Verify the merged
change-test fix, run gate-equivalent clippy with zero new/grown findings,
focused debug/release tests and `cargo xtask check --fast`. No commits.
Run clippy separately before handoff: the fast gate does not select it.
Reserved paths: `ui/src/board/dho/{roll,tests}.rs`, `ui/src/drawpipe/feed.rs`,
this task and `target/COMMIT_MSG.txt`.

M2.3b verification round 2 at `d4f6b92`: owner `codex-d`, active on `t0003-m2`,
unit tier under the owner's assignment. Clear retained roll/scan analog and LA samples
and cursor on identity boundaries while retaining running hardware. Carry each retained
batch identity into STOP raw replay. Give counter-delta regressions isolated counters
and a matching isolated pipeline without serializing the suite. Check the batch regression
on `2c0bd5c`; run the default-parallel library suite five times per profile, focused tests,
soak/census, release timing, unchanged goldens and the fast gate. Expand the pending hardware
smoke with CLEAR and STOP-to-RUN in roll and scan. No full gate, staging, commits or hardware
access; one section in `target/COMMIT_MSG.txt`.
Reserved paths: `ui/src/board/dho/{hw,roll,publication,tests,replay}.rs`,
`ui/src/board/dho/mod.rs`, `ui/src/drawpipe.rs`, `ui/src/drawpipe/{test_scope,losses}.rs`,
`xtask/data/moves.txt` (loss counter extraction only),
`ui/src/ui/tests/{epochs,roll}.rs`, `ui/src/ui/script/batch.rs`,
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.

M2.3b review round 1 claim at `2c0bd5c`: owner `codex-d`, active on `t0003-m2`,
unit tier under the coordinator assignment. Apply P2-1 through P2-4 and P3-1 through P3-7.
Keep D4 epoch rejection without hardware reconfiguration; no D10. Quarantine failed
requests, retain the last publication and continue steps with bounded per-variant error logs.
Publish mask/zone wants atomically with the accepted pair after lifecycle polls; distinguish
stale drops by site from queue overflow and fold loss. Preserve SINGLE completion on
identity-only boundaries, report actual synchronous measurements, invalidate reset analyzer
work, and restart orphaned stage tokens with a bounded logged fallback. Record the
M2.3d worker-view bridge, parent timing and all nine hardware smoke additions in Status.
Reuse the M2.3b reservations and additionally reserve `ui/src/board/dho/publication.rs`,
`ui/src/ui/{hwmask,hwzone}.rs`, `ui/src/drawpipe/identity_tests.rs`,
`ui/src/ui/analysis/window.rs` (reset revision visibility only),
`ui/src/ui/analysis/window/tests/review.rs` and narrowly required regression modules.
Run focused debug/release, soak/census, release timing, unchanged goldens and the fast gate.
No full gate, staging, commits or hardware access; one section in `target/COMMIT_MSG.txt`.

M2.3b implementation claim: owner `codex-d`, branch `t0003-m2`, base `2279778`,
review tier unit, under the owner's assignment. Activate the resolved acquisition identities:
publish one consistent settings/route/gen/epoch snapshot before acquisition, reject old-pair
records before conversion and measurement, and gate asynchronous results and deposit folds
on stats_gen too. Replace independent stamp writers with stage tokens, resolving their
associated settings together at the next change point. Keep the normalize-to-mark_changed
bridge and all c-e reset/run-transition consumers at their current seams.
Reserved paths: `ui/src/ui.rs`, `ui/src/ui/change.rs`, `ui/src/ui/change/`,
`ui/src/ui/{autoset,bode,lifecycle}.rs`, `ui/src/ui/tests/{change,shadow,epochs}.rs`,
`ui/src/ui/script/batch.rs`, `ui/src/{wave,drawpipe}.rs`, `ui/src/board/mod.rs`,
`ui/src/board/dho/{hw,tests}.rs`, `docs/architecture/README.md`, this task,
`target/COMMIT_MSG.txt`. Add narrowly required helper/test modules within these areas
before editing and record any further seam here. Run parent-compatible failing regressions,
focused debug/release, soak/census/timing, justified per-case golden comparisons and the
fast gate. No full gate, staging, commits or hardware access.

M2.3a mask-source decision at `b3999bb`: retain ACQ|STATS|MASK, add D9 for the
deliberate generation boundary missing from legacy, and observe effective mask-source
changes independently of mask requests. Reserve `ui/src/ui/change/{oracle,legacy_keys,diagnostics}.rs`,
`ui/src/ui/tests/change.rs`, `docs/architecture/README.md`, this task and
`target/COMMIT_MSG.txt`. Pin one generation/statistics advance and MASK reset classification,
idempotence and source-specific oracle bounds. Keep acting record/reset consumers on their
scheduled M2.3b/e cutovers. Run focused debug/release tests, the three demo-mask cases
twice per profile with unchanged goldens and zero unreviewed differences, and the fast gate.
No staging, commits, full gate or hardware access; keep one commit-message section.

M2.3a review round 1 at `3908fbc`: apply the coordinator decisions P2-1/P2-2 and
P3-3 through P3-9. Reserve the existing M2.3a paths plus `core/src/model/{settings,acquisition}.rs`,
`core/tests/values.rs`, `ui/src/model/{bridge,platform_tests}.rs`,
`ui/src/ui/analysis/window/tests.rs` and the normalization comment in
`ui/src/board/dho/hw.rs`. Restore the intake analyzer source snap ahead of job submission,
remove analyzer STATS classifications, restrict D3 to actual repaired classes, type difference ids,
and reuse the legacy key compression value. Remove unused compatibility APIs and migrate their
tests to normalize. Add analyzer, RUN D7, hardware-mask source and accepted zoom readback regressions.
Run focused debug/release tests, soak/census, timing, unchanged goldens and the fast gate.
No full gate, commit, staging or hardware access; keep one commit-message section.

M2.3a implementation claim: owner `codex-d` (Codex), status active, branch `t0003-m2`,
base `8f47241`, review tier unit, under the coordinator's assignment. Round 1 below supersedes the analyzer snap deletion. Activate only normalization:
replace both per-step clamps together, migrate zoom/capability/analyzer-source repairs into the
fixed-order pure point, and bridge each effective normalization pass once to `mark_changed`.
Retain this bridge through M2.3d. Keep generation/publication, outbox and reset-consumer cutovers
for b-e. Record and test the decided DEFAULT, cursor and mask contracts; add acquisition/math
oracle observations and eliminate batch-harness unobserved statistics clears.
Also reserve `ui/src/ui/tables.rs` for the final event-navigation zoom clamp caller,
`ui/src/ui/script.rs` and `ui/tests/soak.rs` for the live-normalization census row.
Reserved paths: `core/src/change/`, `ui/src/ui/change.rs`, `ui/src/ui/change/`,
`ui/src/ui.rs` (normalization, DEFAULT and observation seams), `ui/src/ui/views.rs`
(zoom repair only), `ui/src/ui/script/batch.rs`, `ui/src/ui/tests/{change,mapping,shadow}.rs`,
`docs/architecture/README.md`, this task and `target/COMMIT_MSG.txt`.
Task 0052 is merged on this base. Additionally reserve only
`ui/src/ui/analysis/window.rs::update` to delete its relocated source-repair block,
and `ui/src/ui/analysis.rs::ana_adjust` only for its existing held-analyzer reset observation;
its worker scheduling/DSP code and other analyzer paths remain untouched.
Use existing host harnesses and unchanged golden references.
Run parent-compatible regressions first, focused debug/release tests, standard soak and allocation
census, debug/release captures and oracle inventory, explicit timing, and `cargo xtask check --fast`.
No full gate, commit, staging or hardware access.

Integration-gate repair at `131af5d` on `t0003-m2`, assigned to the existing M2 author.
Reserved paths: `ui/src/model/bridge.rs`, `ui/tests/support/counting.rs`,
`ui/src/ui/change/{diagnostics,oracle}.rs`, `ui/tests/host_script.rs`,
`ui/src/ui/tests/shadow.rs`, `xtask/src/arch/core_tests.rs`, this task and `target/COMMIT_MSG.txt`.
Also reserve `ui/tools/golden_run.py` to recognize shadow stderr beside pacing logs;
golden references and comparison thresholds stay unchanged.
Move bounded shadow diagnostics to stderr using the standard log prefix, remove unused
compatibility surface, add the counter Default, and pin the source-specific RUN oracle
difference. Run the fast gate, gate-equivalent host/ARM clippy and UI debug/release tests
including host scripts, soak and unchanged goldens. No full gate, commits or hardware access.

Merge-conflict resolution against G2: keep the soak-only allocator adapter and shared
counter bookkeeping; move the G2 hot-path census into the soak executable and retarget
the perf script. Combine G2's ratio/tail helper with M2.2's stage timing and retain the
intake Drop guard. Verify soak and core change tests in both profiles, G2 perf guards,
shadow regressions and the fast gate. Leave staging and merge completion to the coordinator.
Reserved paths (resolution): `ui/src/perfguard.rs`, `ui/tests/soak.rs`,
`ui/tests/support/{counting,mod}.rs`, `ui/tests/hot_paths.rs`,
`ui/tests/snapshots/hot_paths.rs`, `tools/perf-guard.sh`,
`ui/tests/golden/README.md`, `xtask/data/architecture.txt`, this task.

Also reserve `ui/src/ui/change/intake.rs` for the intake Drop guard.
M2.2 review fixes: retain the accepted M2.2 reservations and observe-only boundary at
`114b399`. Reserve `ui/src/ui/change/census.rs`, `ui/src/ui/script.rs` (host census entry),
and `ui/tests/snapshots/change_alloc.rs` for the warmed UI observer census. Keep analyzer view
paths held. Isolate the allocator static and unsafe implementation in the soak binary, prepare
an independently tested extraction patch against `0df21cd`, repair reachable oracle rows,
error counters, non-panicking shadow bounds and intake/timing accounting, then run focused
checks in both profiles and the fast gate. No commit, full gate or hardware access.

Reserved paths (M2.1): `docs/tasks/0003-m02-change-detection.md`, `target/COMMIT_MSG.txt`,
`Cargo.toml`, `Cargo.lock`, `ui/Cargo.toml`, `core/`, `ui/src/model.rs`, `ui/src/model/`,
`ui/src/{math,decode,zone}.rs`, `ui/src/ui/{la,views}.rs`, `ui/src/protocols/mod.rs`,
`xtask/src/{repo,workspace}.rs`, `xtask/src/arch/mod.rs`, `xtask/src/arch/core_tests.rs`,
`xtask/data/{moves,architecture}.txt`, `docs/architecture/README.md`.
No further caller reservations remain; compatibility exports covered their existing imports.

M2.1 implementation claim: owner `codex-d` (Codex), status active, branch `t0003-m2`,
base `18ac5c72ba5e268905ccd583af4cb00f91a0ec80`. The owner's implementation assignment releases
these narrow reservations after the predecessors. Implement numbered part 1 below only: pure domain move,
value injection, compatibility exports, move manifest and tested boundary/purity rules. Keep the reviewed
worker-view key, oracle, allocation and boundary contracts below for subsequent parts. Run the worker fast
gate and focused debug/release model, setup, worker, STOP, compression and M1 picture tests. No commits,
hardware access, or edits to the assignment's held paths.

Implementation follows [migration M2](../architecture/migration.md#m2-one-change-detection-point-d1a),
[change detection](../architecture/README.md#change-detection), and ADRs
[0001](../architecture/adr/0001-single-settings-owner.md),
[0002](../architecture/adr/0002-commands-per-area.md),
[0003](../architecture/adr/0003-one-change-point.md),
[0004](../architecture/adr/0004-explicit-stages.md),
[0011](../architecture/adr/0011-golden-pictures.md) and
[0014](../architecture/adr/0014-domain-crate.md). Source locations below refer to planning base
`6291e7080f8c91d2702d453052bccb75e0b1faf6`; proposed destinations and test names do not exist yet.
The original assignment was plan-only. Locations and claims are refreshed for M2.1 above; no commit is authorized.
This revision incorporates the coordinator's decisions after Fable 5.2's architecture review (approve with changes).
Those decisions refine M2's acceptance below; do not infer authority to edit the migration specification here.

### Entry conditions and files

The coordinator's accepted planning claim is on `overhaul` at
`b20591e7b9856db88652a82334dcd6dbc6797a38`. Implementation waits for G1/task 0043, then tasks
0020, 0025 and 0027 in their documented serial order, and completed M1 references. M1b must pass before
the shadow unit round. Keep this task's existing dependencies; these additional migration prerequisites still apply.
Create a separate branch per numbered part from its integrated predecessor, retaining task 0003 as the owning record.
The original `t0003-m2` planning branch is now assigned to M2.1 at the accepted base and reservations above.

| Files needed later | Purpose and current holder / sequencing |
|---|---|
| `Cargo.toml`, `Cargo.lock`, `ui/Cargo.toml`; new `core/` | M2.1 workspace and domain move; reserve explicitly. Crate creation runs alone, as integration.md requires; M3 preparation waits for its merge. |
| `ui/src/model.rs` | M2 owns the flat-model move and classification wiring; hot file, reserve explicitly. |
| `ui/src/ui.rs`, new `ui/src/ui/change.rs`, `ui/src/ui/tests/{change,epochs,outbox,mapping,shadow,views}.rs` | Change invocation, legacy consumer bridge and host regressions; reserve `ui.rs` explicitly. M1 has narrow wiring seams on this base; wait for its handoff. Add test modules through the existing `ui.rs:8878` test module, without `#[path]`. Use free functions in the bridge to avoid growing `Ui`'s inherent-impl spread. |
| `ui/src/{math,decode,zone,gfx}.rs`, `ui/src/protocols/`, `ui/src/ui/la.rs` | Move only pure settings/value definitions and tables the model names; retain rendering, protocol implementations and hardware access in UI. |
| `ui/src/board/mod.rs`, `ui/src/board/dho/{mod,hw}.rs`, `ui/src/board/scripted.rs` | Intent mapping, consistent request publication, frame identities and scripted fixtures. `hw.rs` is a hot file requiring an explicit reservation. G1 holds the Board boundary-test seam; M1 holds scripted-source seams until handoff. M3 hardware-loop edits remain serial with M2 wiring. |
| `ui/src/{wave,drawpipe,framestats,compress,deepmeas,pacing,cmd,scpi,http}.rs`, `ui/src/scpi/` | Remove duplicate decisions/publication, carry identities, observe resets and expose shadow counters. M1's worker barrier must land before editing `drawpipe.rs`. |
| `ui/src/ui/script/batch.rs` | M2.3b updates scripted batch/newest/queued-frame identities together; reads `ui.m.hw_stamp` at lines 79, 88 and 103. M1 owns this runner until handoff. |
| `ui/src/ui/{autoset,bode,analysis,eyepage,panes,marks,busview,views,windows,tables,afg,hwmask,hwzone}.rs` | Token/outbox writers, mapping handlers and declared reset consumers. Include every remaining mark caller discovered by the final deletion search. |
| `ui/src/main.rs`, `ui/src/store.rs`, `ui/src/store*` | Config/Context injection, setup loading and existing setters. M1b holds startup/shutdown in `main.rs` and store paths; coordinate a narrow handoff after its merge. Preserve its stable-candidate timer and durable writes. |
| `ui/tests/golden*`, `ui/tests/soak.rs` and M1's support/runner files | M1 holds steps 3-6 in parallel. Consume its documented reference families and harness contracts; do not reserve or edit its in-progress goldens or soak. After handoff, reserve the soak test seam for M2.2's counting-allocator regression, reusing G2 infrastructure when available. |
| `rootfs/initramfs/` | M1b holder; read-only for M2. No M2 rootfs change is planned. |
| `xtask/`, especially `src/repo.rs`, `data/{moves,architecture}.txt` | G1 and tasks 0020/0025/0027 hold this area. Obtain release after their merges for the narrow core registration/move manifest/boundary entries. Do not edit any ratchet or coverage baseline. |
| `docs/architecture/README.md`, this task | Update migration-gap notes and evidence as each part lands; permanent docs must describe the intermediate state accurately. |

### Design mapped to today's code

1. **M2.1, pure domain move.** Move flat `Model` (`ui/src/model.rs:963`), its defaults and pure methods,
   channel/trigger/decode types, math settings (`ui/src/math.rs:10`, `ui/src/math.rs:129`), decoder tables
   (`ui/src/decode.rs:6`), zone settings (`ui/src/zone.rs:10`), and LA settings/record values
   (`ui/src/ui/la.rs:95`, `ui/src/ui/la.rs:168`) into `core/src/model/` and pure leaf modules.
   Use a small `core/src/lib.rs`; split files without changing behavior. Keep compatibility exports in UI while callers migrate.
   Do not move the protocol registry, rendering or board code into the domain. Replace platform/Board reads
   (`ui/src/model.rs:6`, `ui/src/model.rs:19`, `ui/src/model.rs:30`, `ui/src/model.rs:2020`) and
   `la::available` (`ui/src/ui/la.rs:89`) with explicit capability values in `core/src/config.rs::Config`.
   Pass registry option lists, names, roles and pure protocol-setting results through a read-only `Context`:
   today's `PROTOS`, `Proto::module`, `dec_opts` and `bus_bit` call back into UI
   (`ui/src/model.rs:326`, `ui/src/model.rs:332`, `ui/src/model.rs:1815`, `ui/src/model.rs:1823`).
   Protocol objects and calls such as packet rendering (`ui/src/ui.rs:8701`) stay in UI adapters.
   Relocate UI constants used by field types (`ui/src/model.rs:1149`) to a pure definition or inject their values;
   keep palette/render helpers (`ui/src/model.rs:7`, `ui/src/gfx.rs:8`) on the UI side where possible.
   All cross-crate inputs are values, not callbacks that hide IO. `Config` here is capability injection;
   the general configuration/global-state cleanup remains M5. Record item moves using G1's exact key syntax.

2. **One point before acquisition/drawing.** Add `core/src/change/` with `classify`, `normalize`,
   `ChangeSet`, `Classes`, `Request`, `Generations`, `MappingKey`, `UiAcq`, `AcqIntent` and `DepositView`.
   `ui/src/ui/change.rs` bridges today's flat setters to that point in `Ui::step` (`ui/src/ui.rs:1863`).
   M2 guarantees outbox-first merge order only: apply pending internal commands before existing external inputs,
   then normalize, classify previous/current Model and UiAcq, resolve requests, publish, then run consumers.
   Preserve the existing relative order and transport behavior of store, remote and input paths. Their full
   four-source command merge/reply contract belongs to later command/stage migration, not this claim.
   Keep existing edge setters as a transitional input path; route `store::set` (`ui/src/store.rs:430`),
   SCPI (`ui/src/scpi.rs:1120`), HTTP/MCP through SCPI (`ui/src/http.rs:959`, `ui/src/http.rs:1093`),
   web knobs (`ui/src/http.rs:964`, `ui/src/cmd.rs:51`) and setup recall (`ui/src/ui.rs:7103`) through it.
   M13/M14 still own grouped settings/private setters and the complete per-area command conversion;
   M15/M16 still own transport and Session extraction. M2 does not duplicate those rewrites.

3. **Classify by construction.** Hand-write classifiers per future group: channels, timebase, trigger,
   acquire, display, measure, analysis and interfaces. Each helper stays below 60 code lines; no generating macro.
   An exhaustive `Model { ... }` destructure without `..` in `core/src/change/tests.rs::every_field_has_a_group`
   assigns every current field to one group. Verified on this revision: `ui/src/model.rs:963` has 194 top-level
   named fields (counting each struct field once, including outputs and run state), not the historical 178 in
   the spec. Keep 194 in the exhaustive test; recount after prerequisite merges rather than copying the spec.
   Destructure nested settings exhaustively too. Explicitly snapshot `Unrelated` field names for review.
   Exclude change-owned outputs (`hw_stamp`/its replacement, average-clear ids, generations) from the diff.
   Run state and its trigger-sweep companion are handled by transitions, not ordinary settings invalidation.
   Build `MappingKey` from the same mapping-field destructures: timebase/fine scale/delay, channel set,
   scale/offset/invert/coupling/bandwidth, trace mode, zoom and main/zoom dimensions. Do not maintain a second
   mapping-field list. Retain a separate transitional worker-view key covering every dependency in today's
   `drawpipe::Key::of` (`ui/src/drawpipe.rs:451`, `ui/src/drawpipe.rs:462`): all channel fields, timebase/delay,
   trigger source/line/type/slope/level/coupling/noise reject/LA trigger, acquisition mode, dots and dimensions.
   `Pipe::set_view` (`ui/src/drawpipe.rs:664-679`) refreshes the workers' Model clone only when that key changes.
   `MappingKey` alone omits trigger/acquisition inputs still used by workers; M2.3d must not delete this coverage.
   Build the worker key from the mapping key plus the remaining worker dependencies, preserving the old key's
   coverage. M4 may retire those extra fields only after batch-owned trigger snapshots supply every reader.

4. **Derived UI acquisition needs and publication.** Derive `UiAcq::of` each step from XY, protocol watch,
   mask reference, autoset/Bode activity and layout. Existing seams are `needs_raw` (`ui/src/ui.rs:1714`),
   `fast_ok` (`ui/src/ui.rs:2355`) and `relayout` (`ui/src/ui.rs:1612`). Process layout events first; one
   `Layout` value supplies panel/HDMI, pane, menu, main and zoom sizes, so dimension changes enter mapping.
   The change point produces board-neutral `AcqIntent`; `ui/src/app/acq_client.rs` maps it through pure
   `Board::want` (today `ui/src/board/mod.rs:86`). Publish the complete want, route, gen and epoch together
   through existing Shared until M5. Move publication out of `Source::acquire`
   (`ui/src/wave.rs:811`) and separate UI polls (`ui/src/ui.rs:1879`, `ui/src/ui.rs:1920`).
   Publish current mapping separately as `DepositView`, bridging `Pipe::set_view`
   (`ui/src/drawpipe.rs:664`) with the worker-view key above until M4. Route changes are `acq`; mapping is not
   another acquisition change signal. A trigger-level change must refresh the worker Model even with an equal
   MappingKey. This refresh key is a temporary pipe detail, not another hardware generation writer.
   The target fixes the workers' mapping at a batch's first claim. M2 owns key construction/publication and
   class-driven reset/STOP preview; M4 completes first-View pinning, published-batch key filtering, live preview
   and batch-snapshot conversion. M9 still owns whole-batch accumulator extraction. M2 uses a narrow bridge
   around today's picture consumers and does not claim those later steps complete.

5. **Generations and requests.** Private `Generations` has `gen` raised only on `acq`, `stats_gen` on `stats`,
   and `epoch` on request resolution, replacing the independent stamp bumps. Current writers are SINGLE arm
   (`ui/src/ui.rs:1883`), autoset start/advance/retry (`ui/src/ui/autoset.rs:172`,
   `ui/src/ui/autoset.rs:336`, `ui/src/ui/autoset.rs:376`) and Bode frequency/free-sweep stages
   (`ui/src/ui/bode.rs:468`, `ui/src/ui/bode.rs:661`). Autoset/Bode return unique
   `(requester, run, stage)` tokens via `Request::Epoch(token)`; no unresolved token accepts a record.
   Resolve next step, applying the stage's associated settings before its epoch; several tokens may share one
   epoch. Intake SINGLE, CLEAR and STOP-to-RUN get an epoch in the current change step.
   Consumers use exact resolved epoch, not today's `>= stamp` checks
   (`ui/src/ui/autoset.rs:237`, `ui/src/ui/bode.rs:580`, `ui/src/ui/bode.rs:675`).
   M2.3b owns the record generation filter. Carry consistent `(gen, epoch)` in acquisition output headers;
   filter records/measurements on the pair and
   measurement jobs/results additionally on `stats_gen`. Replace the `meas_n` convention in
   `MeasWorker`/`meas_poll` (`ui/src/ui.rs:902`, `ui/src/ui.rs:4941`, `ui/src/ui.rs:4979`), including worker
   folds entering `mstat_fold` (`ui/src/ui.rs:4906`), so an in-flight fold cannot refill reset statistics.
   An epoch-only boundary must invalidate prearmed/queued old work just as a gen boundary does; adapt the current
   acquisition application seam (`ui/src/board/dho/hw.rs:1632`, `ui/src/board/dho/hw.rs:1664`) without doing
   M3's closure split or M5's channel rewrite. Preserve STOP's final-read contract. Run the producer unit smoke
   immediately after M2.3b, before M2.3c. M4's specified old-generation-not-measured test becomes a regression
   test of this M2.3b behavior; give M4 the test and evidence, not a second generation-filter implementation.

6. **Normalize and outbox.** Fixed group order is channels, timebase, trigger, acquire, display, measure,
   analysis, interfaces. Move `Model::clamp_offsets` (`ui/src/model.rs:1865`), `fit_platform`
   (`ui/src/model.rs:2020`) and cross-group clamps such as `Ui::clamp_zoom` (`ui/src/ui.rs:1675`) into
   idempotent normalization. Include analyzer source repair (`ui/src/ui/analysis.rs:528`) in the audit.
   Remove both live per-step clamps (`ui/src/ui.rs:1878`, `ui/src/ui.rs:1888`) together when activating it.
   Clamped channels classify `acq`; clamped offsets classify `mapping` and preview.
   M2.3a bridges every effective normalize diff back through `mark_changed`, preserving its RUN-only dirty-map
   mark and unconditional statistics clear while legacy consumers act. Keep that bridge in M2.3b, M2.3c and
   M2.3d; after M2.3d the mark's picture side is inert and its statistics side still acts. Remove it only with
   the final legacy consumers in M2.3e. At each switch, one acting consumer owns each effect; no double reset.
   Internal SINGLE/segment completion (`ui/src/ui.rs:2135`, `ui/src/ui.rs:2142`, `ui/src/ui.rs:4803`),
   autoset/`auto_step` (`ui/src/ui/autoset.rs:216`) and Bode emit commands/requests into
   `ui/src/app/outbox.rs` for the next change step. Convert internal mask-stop writers too
   (`ui/src/ui.rs:2880`, `ui/src/ui/hwmask.rs:68`). Guard completion locally to avoid consuming extra SINGLE
   records while next-step STOP is pending. RUN-to-STOP keeps the picture; STOP-to-RUN restarts averaging.
   CLEAR and Create mask (`ui/src/ui.rs:1813`, `ui/src/ui.rs:2898`) are requests even when settings are identical.

7. **Consumers, not marks.** Replace `settings_changed`/`mark_changed` (`ui/src/ui.rs:6736`,
   `ui/src/ui.rs:1700`) and all 16 `dirty_map = true` sites with class consumers. `acq` drives publication
   and identity filtering; `mapping` drives preview from `last[0]` and mapping resets, with full batch-key
   rejection completed by M4; `phosphor` resets
   maps/weights; `stats` clears analysis/results/statistics and advances stats_gen; `mask` resets mask counts;
   `zone` resets zone drops; `hw_average` restarts hardware averaging; `history` clears history/segments;
   `math` resets held math/FFT state; `view` resets the declared temporal view state. No `decode` class:
   the decode cache stays keyed by record and decoder configuration.
   Centralize picture reactions around today's dirty-clear and STOP-redraw branches
   (`ui/src/ui.rs:1930`, `ui/src/ui.rs:2235`); mapping in STOP redraws the stored record without a blank frame.
   Preserve current kernels/weights, including STOP's legacy redraw weight until the later drawing steps.
   Declare `RESETS_ON` beside trend (`ui/src/ui/panes.rs:749`), eye (`ui/src/ui/eyepage.rs:496`),
   XY (`ui/src/ui.rs:3444`), cursors (`ui/src/ui.rs:3584`) and every other existing stateful view.
   Trend depends on stats/acq, eye on mapping and its view options, cursors on the timebase/view contract;
   test every class against each declaration. Today's XY rendering has no separate persistence buffer:
   document its stateless case and test that it retains no old mapped state rather than inventing a new buffer.

8. **Shadow before activation.** M2.2 keeps a separate previous Model/UiAcq, Generations and outbox.
   Normalize only a reusable shadow copy; old live clamps and all old mechanisms continue acting. Compare shadow
   classes to old observations using the oracle table below, normalized copy to the live post-clamp snapshot,
   and outbox proposals to old internal writes at the corresponding step. Count raw disagreements per class
   and log each field once, including the oracle row and reviewed difference id. Expose `shadow acq=N mapping=N ...`
   for all ten classes in `cmd::stat_text` (`ui/src/cmd.rs:130`), already used by `:SYSTem:STATe?`
   (`ui/src/scpi.rs:1145`). Shadow outputs are observe-only: no publication, reset, live settings or record
   acceptance uses them. Retain shadow comparison through the switches; delete old mechanisms only after round 0
   and continuing evidence that behavior matches the table, with differences only from the coordinator-reviewed
   list. Raw counters need not all be zero: assert their exact expected deltas for each scenario, and require
   zero unreviewed differences. Never suppress a mismatch or label every intentional difference an old-code bug.
   For the per-step previous snapshot and shadow normalize scratch, require capacity-reusing `clone_from` on
   heap-owning fields or update-on-change. Derived Model `Clone` alone does not guarantee a reusable `clone_from`.
   Allocate snapshots once, warm every nested String/Vec capacity, then update without per-step allocations on
   unchanged or already-warmed bounded changes. Reuse shadow diagnostics/outbox buffers as well.
   M2.2 must pass a counting-allocator test and the intake/change timing guard before the shadow unit round;
   carry both checks through M2.3, rather than discovering clone churn only after activation.
   Bound new state: one previous snapshot per detector and one shadow normalization scratch (<64 kB each,
   heap capacities included), Generations 24 B per detector,
   UiAcq/DepositView <1 kB each, ChangeSet <4 kB, intent <4 kB, and each outbox at most 64 entries.
   Specify a byte cap as well for command payloads before implementation (budget 64 kB per outbox);
   reject overflow explicitly, never silently drop commands or token resolutions. Field-log bookkeeping is a
   fixed table indexed by classified fields, not an unbounded set. Shadow state is removed at cutover.

### Shadow comparison oracle and reviewed differences

Old signals are observations of side effects, not class bits. Capture the initiating event, effective normalized
field diff, RUN/STOP state and request/token together with each observation. The expected new class set is the
union of the applicable rows and a separately reviewed field-to-consumer contract for multi-field operations.
Pin those exact unions in fixtures before M2.2's unit round; do not derive expectations
from the classifier under test. A bare `dirty_map` or `mstat.clear` cannot supply that union by itself.

| Old observable signal / seam | Expected new class set or non-class event | Comparison rule |
|---|---|---|
| `settings_changed` / `mark_changed`, `ui.rs:6736`: dirty_map only in RUN; mstat clear in RUN and STOP | Classes of the effective diff, not an unconditional `{mapping, stats}`. Scale/offset: at least `{acq, mapping, stats}`; grid brightness: `{}`; measurement list: `{stats}`. | Split map and statistics observations, include run state, and check each seeded consumer. Extra old clears are D1; STOP's absence of a dirty mark is D2. |
| `clamp_offsets`, `model.rs:1865` / `ui.rs:1878`, changes an offset without marking | `{acq, mapping, stats}` for an effective AFE-offset change, including mapping preview; `{}` if already in range | Compare pre/post effective values and normalized copies; missing old notification is D3. M2.3a-d retain the bridge; e removes it, as stated per commit below. |
| `fit_platform`, `model.rs:2020` / `ui.rs:1888`, marks on any repair | Unavailable channel/coupling repair: `{acq, mapping, stats}`; missing trigger source: `{acq, stats}`; active-channel-only repair: `{}`. Other repairs use their exact field union. | Pin each capability/repair case, including LA, decoder, generator and analysis sources; no blanket all-class repair. Extra old marks/clears are D1. |
| `clamp_zoom`, `ui.rs:1675`, called by zoom edits/recall/timebase changes | `{mapping}` for effective zoom geometry, plus any classes of the initiating timebase change | Compare effective zscale/zdelay after clamps and both plot sizes; earlier normalization is D3. |
| Analyzer source repair in `ui/analysis.rs:528` | `{math, view}` for spec_src repair, `{view}` for eye_src repair, plus the triggering channel diff's classes | Compare after the corresponding repair, not before the old view stage; moving repair before classify is D3. |
| Relayout `dirty_map`, `ui.rs:1631`; dots/Z-input marks `ui.rs:6527,6544` | `{mapping}` for effective dimensions/dots; Z-input additionally `{acq, stats}` by its field contract | Observe actual dimensions, Z source and raw-policy changes; an effective Z-source edit requires MAPPING and cannot be labelled D1 when its clear is missing. Only extra broad clears use D1; STOP absence uses D2. |
| Trigger/demo marks `ui.rs:5227,5413,5495`; autoset/Bode marks `ui/autoset.rs:504,596`, `ui/bode.rs:548,666` | Trigger-only changes: `{acq, stats}` and worker-view refresh; accompanying channel/timebase changes add `{mapping}` and their declared classes | Keep all old Key::of worker dependencies; dirty-map marks do not make every trigger field a mapping field. Stage timing is D5. |
| Recall/default/auto marks `ui.rs:7122,7896,7913`; history-navigation mark `ui/marks.rs:40` | Recall/default/auto: exact union of changed fields and derived UiAcq. Replay navigation: `{mapping}` preview event, without inventing `acq` for navigation alone | Same-value recall has no diff-driven classes; explicit requests still act. Seed every affected consumer; D1/D2/D5 as applicable. |
| SINGLE key mark `ui.rs:7889`, stamp bump `ui.rs:1883` and average restart | SINGLE request resolves epoch in the current step; `{phosphor, stats, hw_average}` plus other declared request resets | Compare request consumers even at equal settings; generic run/sweep diff is excluded. Old stamp is an epoch observation, not automatically `acq`; D4. |
| `hw_stamp` bumps: autoset `ui/autoset.rs:172,338,376`; Bode `ui/bode.rs:483,661` | `Request::Epoch(token)` and epoch resolution; accompanying setting diffs determine classes, including `acq` only when acquisition changes | Same-settings retry still has an epoch, possibly no class diff. Compare exact resolved token/pair, never infer a class from a stamp increment; D4/D5. |
| `Source::acquire` want/stamp publication (`wave.rs:811`), WANTED/configuration and raw/hold polls | `{acq}` on effective acquisition/route change; epoch boundary on request even with equal want; run transition separately | Compare one consistent want/route/gen/epoch publication, applied pair and boundary drops; no false reconfiguration for epoch alone. D4. |
| CLEAR `ui.rs:1813`: map/result/stat/mask/zone/average clears and last-record removal | Request classes `{phosphor, stats, mask, zone, hw_average, math, view}` with epoch; add history only if the request contract clears history | Pin each retained/dropped store explicitly; last-record removal is part of CLEAR, not generic mapping. Empty STOP display and same-value repeats required; D4. |
| RUN edge `ui.rs:1902`; segmented history/times reset `ui.rs:1904-1912` | STOP-to-RUN: `{hw_average}`, epoch, plus `{history}` when segments restart; RUN-to-STOP: no generic invalidation | Preserve picture/final read on STOP. Internal completion transitions occur next step; D4/D5. |
| Create mask `ui.rs:2898-2905`, mask count reset / explicit MaskReset `ui.rs:7011` | `{mask}` request at equal settings; mask-source/enable diff adds its declared acquisition classes | Compare count reset separately from mask construction and STOP requests; do not infer all ten classes. |
| Zone clear-id/drop reset, hardware average clear-id, math/FFT and temporal view resets | `{zone}`, `{hw_average}`, `{math}` or `{view}` respectively, unioned with applicable field/request classes | Audit each old writer and each view's RESETS_ON; output-id writes do not themselves re-enter classify. |
| Direct mstat clears for measurement removal/clear (`ui.rs:6863,6902`), recall (`ui.rs:7123`), clear (`ui.rs:1827`) | `{stats}` for measurement-list changes or explicit reset; recall is its effective field union | Distinguish intentional measurement reset from old unconditional clearing; D1. |
| Acquisition mode/average configuration, observed from the independent legacy `(acq, avg_count, avg_hw)` key | `{acq, stats, phosphor, hw_average}` for an effective mode/average diff | Row `acquisition` observes the real configuration change. Missing legacy average restart and STOP persistence invalidation are D7. No acting reset is switched in a. |
| Math operation, observed from all eight legacy MathCh operations | `{math, stats}` for an effective operation change | Row `math` observes the stateless recomputation boundary. Current math/FFT output is recomputed from the retained record without a held buffer; the extra reset class is inert here (D8). No fake old reset is reported. |
| `meas_n` and late job/fold acceptance (`ui.rs:4900,4945,4979`, `mstat_fold:4906`) | `{stats}` advances stats_gen; `{acq}` changes gen; epoch request changes epoch independently | All three identities gate results/folds; identity rejection is not itself a new class. D6. |
| `Pipe::set_view` key change (`drawpipe.rs:664-679`) and worker Model clone | Mapping changes refresh the mapping key; all old worker-key fields refresh the worker view, even for trigger-only `{acq, stats}` | Test batch-path worker behavior, not just key equality; preserve refresh coverage until M4. |
| Min-persistence clear, decay, STOP redraw and fresh-record replacement (`ui.rs:1930,1970,2057,2235`) | `{}` when driven only by acquisition/time; mapping preview when an effective mapping change requests it | Keep drawing operations outside settings resets. Do not count a normal Min clear as a phosphor-class disagreement. |

Before round 0 the coordinator reviews this table, the complete field/request class fixtures, and the following
expected-difference list with concrete storm steps, counter deltas and before/after evidence. These are proposed
differences, not blanket exemptions or approval already obtained. Any new difference returns for review before
unit use or cutover. Keep raw mismatch counts visible; acceptance is **matches the table, differences only from
the reviewed list**, including throughout M2.3.

- **D1, narrower invalidation:** old marks clear statistics for unrelated display/input changes and dirty maps
  broadly in RUN; classes reset only their declared consumers. Same-value writes/recall can cause old clears
  without a new diff. Existing broad analyzer average/waterfall resets likewise give exact extra-old
  math/view deltas, including at equal settings. Counts are scenario-specific, not suppressed.
- **D2, STOP mapping:** settings_changed has no STOP dirty mark, while STOP redraw happens separately every step.
  The new mapping class requests preview even in STOP. Compare the stored record and visible trace, not dirty_map
  bit equality, and retain the legacy redraw weight in M2.
- **D3, normalization:** old offset clamps can be unmarked and zoom/analyzer repairs happen at other seams.
  The new point classifies effective repairs together before consumers; compare equivalent post-repair values
  and separately review any changed outcome caused by the fixed normalization order. Allow D3 only
  for classes of the fields actually repaired in that pass, never for every class on the row.
- **D4, request boundaries:** CLEAR and STOP-to-RUN gain an epoch even with equal settings; SINGLE and stage
  retries resolve requests rather than equating stamp changes with acq. Several requests may share an epoch.
  Epoch-only boundaries reject prearmed/queued old work without reconfiguration.
- **D5, internal timing:** autoset/Bode and completion settings act on the next step via outbox, with settings
  and token resolved together. Align old proposals with next-step application; unresolved tokens accept nothing
  and pending SINGLE STOP consumes no extra record. Terminal autoset keeps the
  preceding statistics visible for one step, until terminal settings and their
  STATS reset are accepted together; old-pair results cannot refill them.
- **D6, stale work:** pair/stats_gen rejection prevents late old records, results or folds from refilling
  current state. Pin deliberately delayed work and its exact drop counts; do not attribute drops to all classes.

- **D7, acquisition-mode reset gap:** an effective legacy mode/average configuration change
  has no explicit hardware-average restart, and in STOP no persistence reset from the RUN-only
  dirty mark. Preserve those old effects until the respective d/e consumers switch. The oracle
  permits only new hw_average, or new phosphor while stopped, at the observed acquisition row.
- **D8, stateless math recomputation:** a changed operation invalidates no retained legacy
  math/FFT buffer because current drawing recomputes its output from the retained record.
  The math class therefore has no additional acting reset here. The oracle permits only new
  math at the observed all-channel operation seam; source/scale/other classes are not exempted.
- **D9, mask-source acquisition boundary (decided):** mask_src selects the hardware mask
  channel through mask_hw_poll and hw_merge, outside the legacy generation key. Changing it
  advances gen once and resets mask statistics: in-flight records were mask-tested against
  the old channel and must be dropped. Retain ACQ|STATS|MASK for hardware and demo sources.
  Only extra new ACQ and MASK at an independently observed effective mask-source change
  receive D9; mask requests alone and other classes do not. Raw disagreements stay counted.
  Acting generation/filter and reset consumers remain scheduled for M2.3b/e.

### Commit and merge order

Each row is a proposed commit; M2.1 and M2.2 may contain the two listed commits on their own numbered-part
branch. M2.3 has five separately gated, cross-reviewed parts/branches, a through e. Each switches its writer
and consumers together and merges only after independent Claude review and the full gate; do not bundle the
five switches into one review. Hold M2.3c for M2.3b's producer unit smoke. Run the full gate before every commit.
New or moved code meets ADR 0010; G1 permits only listed, non-growing legacy moves.

| Order / part | Commit intent | Deletes |
|---|---|---|
| 0, this revision | `Revise the M2 plan after architecture review` | Plan revision and duplicate Status claim only. No source deletion; draft message only, do not commit in this session. |
| 1, M2.1 | Register `oos-core`, Config/Context value contracts and gate membership | Nothing; existing behavior remains. No new external dependency. |
| 2, M2.1 | Move flat Model and required pure types; adapt callers to explicit values | UI-owned definitions of the moved model/types/tables and their domain-to-UI/Board/platform lookups. Keep only necessary compatibility exports. No invalidation mechanism removed. |
| 3, M2.2 | Add exhaustive classifiers, normalize, requests, tokens and reusable shadow state | Nothing in the acting path. Implement capacity-reusing clone_from or update-on-change and the counting-allocator regression here; legacy normalization/mark path is unchanged. |
| 4, M2.2 | Wire observe-only oracle comparisons/counters, host storm, timing and unit-round evidence | Nothing in the acting path; old clamps, marks and stamp writers still act. Require counting-allocator and p99 evidence, coordinator review of the oracle/difference list, then shadow round 0 before activation. |
| 5, M2.3a | Activate normalize before classify, retaining legacy notifications | Both external per-step clamp calls and migrated cross-group rewrites, together. Every effective normalize diff still feeds mark_changed for the acting legacy consumers until M2.3e; intake analyzer source snaps precede job submission and do not reach that bridge; test RUN/STOP map/stat behavior. |
| 6, M2.3b | Switch acq generation/epoch writers, publication and record/measure filters | hw_stamp bumps in UI/autoset/Bode; publication in Source::acquire and independent request writers; old stamp/generation decisions where replaced. Update scripted runner identities. Tokens and settings reach one change step. Retain normalize-to-mark_changed bridge. Run producer unit smoke before c. |
| 7, M2.3c | Switch remaining internal commands and run-transition consumers to outbox | Direct completion-to-STOP, autoset/Bode/auto_step settings writes and scattered run-edge reset effects. Retain normalize-to-mark_changed bridge, with one owner per switched effect. |
| 8, M2.3d | Switch mapping/phosphor writers and picture consumers | All 16 dirty_map assignments and dirty-map decision state; scattered settings-triggered map clears. Retain the complete old Key::of dependency coverage through the transitional worker-view key until M4. Retain normalize-to-mark_changed bridge for remaining statistics effects; disable its old picture side. Keep Min/decay drawing operations. |
| 9, M2.3e | Switch stats/history/mask/zone/hw_average/math/view, then remove legacy observation | settings_changed, mark_changed and every caller, including normalize's bridge; old meas_n convention; ad hoc resets now covered by classes. Remove shadow bookkeeping only after table/list acceptance; retain keyed decode and M4's transitional worker-view key. |

Verify deletions with repository searches for `settings_changed`, `mark_changed`, `dirty_map = true`,
`hw_stamp`, `clamp_offsets`, `fit_platform` and `meas_n`. List any retained compatibility naming and its
single permitted implementation; no writer or external per-step clamp may remain. Do not delete M4/M5/M9/M10
mechanisms merely because they are adjacent. Record moves in `xtask/data/moves.txt`; only the integrator runs
`cargo xtask ratchet --move` at merge. No ratchet edits on an author branch.

### Required test map

**FB** means the M2 specification requires failing-before evidence, not that a failure has been observed here.
All names below are proposed. For each behavioral regression, run an executable parent-compatible test against
the old path, retain the failing assertion, then pass it through the new path in debug and release. Missing APIs
or a compilation failure are not behavioral evidence. If today's code already passes a subcase, retain it as
characterization and report that limit; do not fabricate a failure or remove useful coverage.

| Specification / part | File and test name | Assertions |
|---|---|---|
| Each class resets exactly its consumers, M2.3b-e; FB | `ui/src/ui/tests/change.rs::{acq,mapping,phosphor,stats,mask,zone,hw_average,history,math,view}_resets_exact_consumers` (ten tests) | Representative fields: mdepth, delay, persist, meas, mask tolerances, zones, avg_count, segments, math op, eye options. Include overlapping expected class sets, seed every consumer and prove all unrelated state survives; do not assume each representative has only one class. Core per-group tests also mutate each classified field. |
| SCPI/HTTP equal knob, M2.3; FB | `ui/src/ui/tests/change.rs::scpi_and_knob_share_change_point`; `ui/src/http.rs::tests::http_store_set_matches_knob` | Same normalized Model, ChangeSet, publication and consumer effects after one step. HTTP exercises the existing MCP/SCPI `store::set` path and web knob path; no imaginary HTTP setter. Include setup load as an extra input route. |
| Mapping in STOP, M2.3d; FB | `ui/src/ui/tests/mapping.rs::stopped_mapping_redraws_last_without_blank` | Hold a populated last[0], change scale/offset/zoom/layout, step with no new frame, capture each step: same stored record, new mapping, nonempty trace. Running-preview acceptance remains M4. |
| Autoset/Bode epochs, M2.3b-c; FB | `ui/src/ui/tests/epochs.rs::{autoset_accepts_only_resolved_epoch,bode_accepts_only_resolved_epoch}` | Unresolved accepts nothing; reject older and different newer epochs and previous runs' tokens; accept exactly the resolved one. Settings and token resolve in one change step; same-settings retry still gets an epoch. |
| Normalize, M2.3a; FB | `core/src/change/tests.rs::{normalize_is_idempotent,missing_channel_is_acq,clamped_offset_is_mapping}`, `core/src/change/tests/normalization.rs::{capability_and_cross_group_repairs_report_effective_changes_once,analyzer_sources_follow_an_enabled_channel_before_classification,live_normalization_reports_only_effective_repair_classes}`; `ui/src/ui/tests/mapping.rs::clamped_offset_previews_last` | Second normalize changes nothing; 2-channel caps turn unavailable channels off with acq; offset clamp gives mapping/key change and preview. Sweep supported capabilities and cross-group clamp cases. |
| SINGLE done, M2.3c; FB | `ui/src/ui/tests/outbox.rs::single_done_stops_in_next_change_set` | Completion emits STOP without writing live settings; no extra record is consumed; next ChangeSet has the transition. Add `segments_done_stops_in_next_change_set` and `internal_writes_apply_before_external_inputs`. |
| Every view declaration, M2.3e; FB | `ui/src/ui/tests/views.rs::declared_reset_classes_match_every_view` | Table-drive every existing view over all ten classes, checking reset iff declared, plus unrelated changes. Explicit trend, eye, XY/stateless and cursor-timebase cases. Cover held spectrum/math state, not only visible panes. |
| Mapping key iff mapping, M2.2/3d; FB | `core/src/change/tests.rs::mapping_key_matches_mapping_fields`; `ui/src/ui/tests/mapping.rs::relayout_changes_mapping_key` | Mutate every mapping and nonmapping field, nested channels included; vary main/zoom dimensions, panes, menu width and HDMI. Check normalized effective changes, including fine timebase. |
| Worker-view refresh, M2.2/3d; failing-first for a mapping-only replacement | `ui/src/drawpipe.rs::tests::trigger_level_change_refreshes_batch_worker_view` | Warm the real batch workers, change only trigger level while MappingKey stays equal, publish/set_view, then feed a controlled batch whose refinement distinguishes the levels. Verify the workers use the new level and match the record/reference path, with M1's barrier. Demonstrate failure with the proposed mapping-only refresh, then pass with preserved worker-key coverage. The current Key::of path should pass as characterization; do not invent a parent failure. Mutate every other retained worker dependency in a table too. |
| No-diff requests, M2.3b-e; FB | `ui/src/ui/tests/change.rs::{clear_without_diff_resets_consumers,run_edge_without_diff_resets_consumers,single_edge_without_diff_resets_consumers,create_mask_without_diff_resets_consumers}` | Repeat each event with equal settings; correct consumer resets and epochs, same-step intake resolution; RUN-to-STOP preserves picture, STOP-to-RUN restarts average, CLEAR remains empty in STOP. Run/sweep changes must not trigger generic settings resets. |
| Unrelated review snapshot, M2.2; FB | `core/src/change/tests.rs::unrelated_fields_snapshot` | Exact checked-in list of Unrelated field names; outputs and run state are explicitly separate. New fields cannot disappear into `..` or an implicit fallback. |
| Host storm matches oracle, M2.2; contract and regression evidence | `ui/src/ui/tests/shadow.rs::scripted_change_storm_matches_reviewed_oracle` | Script knobs, SCPI/HTTP, setup, CLEAR/RUN/SINGLE, layout, autoset and Bode in RUN and STOP. Assert exact class sets/effects and per-class raw-counter deltas against independent table fixtures after each step; differences only from the coordinator-reviewed list. Include same values, unrelated changes, capability/offset repairs and mixed sources; require zero unreviewed differences, aligned normalize equality and next-step outbox agreement. |
| Normalize legacy bridge, M2.3a-d; regression for the intermediate commits | `ui/src/ui/tests/change.rs::normalize_diff_notifies_remaining_legacy_consumers` | A capability/offset repair without an edge setter still calls mark_changed once: RUN marks dirty while that side acts, STOP does not; legacy mstat clears in both. After d only the legacy stats side acts; after e classes replace the bridge. Pin behavior at each commit boundary. |
| Snapshot/shadow allocation, M2.2 and retained in M2.3; failing-first against naive clone | `ui/tests/soak.rs::change_snapshots_reuse_allocations_after_warmup` | Reuse M1's soak runner and G2's counting allocator in a dedicated test target. Count successful alloc/alloc_zeroed/realloc across threads, warm nested Strings/Vecs, and measure isolated snapshot refresh plus shadow normalize on unchanged and bounded repeat changes. Require zero new allocation/reallocation calls there; prove naive per-step Model clone fails. Keep whole-run plateau/frame accounting as a separate check; live-byte plateau alone does not prove zero churn. No production allocator or unsafe in core. |
| Intake/change timing, already M2.2 and retained through M2.3 | Shared G2 timing guard with M1's scripted storm runner | Record stage intake/change samples, at least 50 observations, including snapshot updates, live bridge and shadow normalize/classify/oracle work. Combined intake+change p99 <=0.5 ms; report host/profile and shadow overhead, and repeat on the unit in round 0 and after activation. Reuse the existing timing infrastructure; do not defer this check to M2.3. |
| Shadow isolation, M2.2; additional safety test | `ui/src/ui/tests/shadow.rs::shadow_is_observe_only` | Inject an intentionally different shadow class/normalize/outbox; only diagnostics change. Live Model, generations, queues, request publication, picture and results equal the old-path control. |
| Generation boundary, M2.3b; FB for replaced behavior | `ui/src/board/dho/tests.rs::epoch_only_boundary_rejects_old_work`; `ui/src/ui/tests/epochs.rs::old_pair_records_are_not_measured` | Prearmed and queued old work is rejected on epoch-only or gen change; published pair/route is consistent. Test pure application decisions at the current loop seam, not a premature M3 Device harness. M2.3b implements the record filter; M4 reruns old-generation-not-measured as a regression through its new conversion/batch path. |
| Late statistics, M2.3e; FB for replaced behavior | `ui/src/ui/tests/change.rs::{late_measure_result_is_dropped_after_change,late_fold_cannot_refill_reset_stats}` | Independently change gen, epoch and stats_gen; reject old jobs/results/folds, accept current identities and retain valid current values. |
| Exhaustiveness and bounds; new contract tests | `core/src/change/tests.rs::{every_field_has_a_group,change_values_stay_bounded}`; `ui/src/ui/tests/outbox.rs::overflow_is_explicit` | All 194 current top-level fields assigned, nested fields exhaustive, new state fits documented caps and full outbox produces an explicit error without losing queued commands/tokens. |
| Boundary rules, M2.1/2.2; deliberate-violation evidence | G1's boundary test harness in `xtask/`, using the merged paths | One compiled/reachable module fixture and deliberate forbidden import per new rule below, with the exact rule in the diagnostic; compliant fixture passes. Check actual module existence/reachability explicitly since the checker does not. Include empty leaf layers and actual exhaustive-destructure imports in the compliant tree. |

M2.1 is a refactor: preserve `model::tests::{four_channel_unit_keeps_everything,
a_setup_of_another_board_fits_this_one,every_generator_keeps_its_settings}` (`ui/src/model.rs:2322`),
setup serialization/round trips, and all M1 pictures. Keep existing
`drawpipe::tests::{batch_picture_equals_the_frames_mode,batch_picture_from_the_codes_equals_the_frames_mode,
drawing_from_the_codes_equals_drawing_the_decoded_frame,a_batch_is_delivered_with_its_frames}`
(`ui/src/drawpipe.rs:1620`, `ui/src/drawpipe.rs:1133`, `ui/src/drawpipe.rs:1170`, `ui/src/drawpipe.rs:1680`),
`ui::tests::{clear_empties_the_display_in_stop,segmented_recording_stops_after_n_records,
hardware_average_bypasses_the_blend_and_restarts}` and the compression/raw-consumer and STOP-final tests.
Preserve known-failure annotations until their owning fix; do not claim later drawing bugs fixed by M2.

### M1 picture gate and verification

Use the completed, reviewed M1 references recorded from pre-M2 `overhaul`, with provenance in its manifest.
Plan against M1's documented families, not provisional filenames: scripted Demo screens; lines/dots/Auto,
fast/slow timebases, 1/2/4 channels; raw/codes/live-pairs/deep-pairs through the real workers; Min/500 ms and
long persistence, colour grade, average, zoom, frozen/replay and synthetic sweep consumer cases.
Keep each source route's own reference. M1's scripted-batch captures have measurements off, so they cannot prove
M2 statistics correctness; the new tests above must do that. Roll/scan/segment producer coverage remains M3.0.
M2 to M6 preserve M1 references. Never regenerate them to hide a change-detection regression.
Use M1's reviewed per-region tolerance and differing-pixel budget, compare actual image diffs, retain galleries,
and keep repeat-process/debug-release determinism. An intended visible bug fix needs an explicit reason and
independent reference review before any exception; it does not authorize a drawing-kernel change in M2.

Run M1's host soak in the full gate: 10 000 steps per supported scripted mode, warm through step 1 000,
later cycle peaks within 1% of the plateau and exact planned drop/frame accounting. It includes workers but
does not exercise the real acquisition producer. M1 steps 3-6 are in progress; their acceptance is a prerequisite,
not evidence available to this plan. Keep `gpu/test-host.sh` parity and perf guards alongside CPU goldens.

After G1 and tasks 20/25/27 merge, use their full `cargo xtask check` inventory on every candidate, with the
worktree-local target, debug/release tests, coverage, ARM64 checks/link, boundaries, performance and GPU checks.
Retain complete output and exit codes, including failures. Add the combined intake+change p99 <=0.5 ms timing guard
in M2.2, including shadow/snapshot overhead, and retain it through M2.3 using G2's shared timing infrastructure
when available; coordinate the harness handoff rather than creating another benchmark framework.
Record the `stage intake`/`stage change` samples with at least 50 observations for reported quantiles.
No full gate, failing-before test or hardware result is claimed in this planning session.

### Boundary entries after G1

Coordinate these narrow edits with the released xtask holder. `xtask/data/architecture.txt` already has
`crate oos-core -> oos-dsp` and `crate oos-ui -> libc oos-hal oos-dsp oos-core`; retain/enforce them when
the crate is registered in the workspace and `xtask/src/repo.rs:17`. Do not add hal/libc to core.
For the proposed module split, add these rules when its imports meet them:

```text
layer core config ->
layer core change -> model config math decode zone la protocols
layer core model -> config math decode zone la protocols
layer core math ->
layer core decode ->
layer core zone ->
layer core la ->
layer core protocols ->
```

Here core's `protocols` holds settings metadata only, not the UI registry. `change` must name nested setting
types for exhaustive destructures, hence math/decode/zone/la/protocols as well as Model/Config. Finalize actual
leaf names and imports at the pure move; if a leaf legitimately depends on another pure leaf, name only that
dependency in its layer rule and review the matching violation test. Never leave existing pure modules unruled.
G1 already owns `confine ui oos_hal::dho -> board::dho`, replacing `board::boundary` without a period of
unenforced access. Retain that confinement; do not add redundant app/model forbids or another checker in M2.
For each new layer line above, add one deliberate-violation test using an actual reachable module: config to
model, change to a disallowed fixture module, model to change, and each leaf to model must fail with its rule's
diagnostic. Pair them with a compliant fixture and explicit module-existence/reachability assertions. The checker
does not validate that a named module exists; a passing rule for an absent leaf is no evidence of protection.
Core also forbids unsafe and applies crate-wide disallowed methods/types for environment, filesystem, network,
clock reads and thread/sleep, with no adapter exception. This uses the agreed lint configuration, without
loosening shared limits. Generations' mutators stay private to change. Do not enable the final settings/change
ownership rules prematurely: flat Model and legacy edge setters persist until M13/M14.
Add adapter-pair/stage restrictions only for modules actually made compliant, not absent target directories.

### Unit rounds, estimate and risks

**Required round 0 (shadow), after M2.2 and M1b.** Under task 0049's coordinated unit round and this task's
evidence, spin knobs, issue SCPI writes, load setups, and run autoset and Bode. Measure all ten shadow mismatch
counters, normalized-copy/live equality and outbox agreement; retain per-field first-disagreement logs and
before/after screenshots/statistics. Before this round, the coordinator must review the oracle table, exact
class fixtures and expected-difference list. Acceptance is matches the table, differences only from the reviewed
list; compare exact raw-counter deltas and require zero unreviewed differences. Keep that criterion through
M2.3 before removing legacy paths. Include repeated same-value requests and stage retries to exercise tokens,
not just changed settings. Record M2.2's combined intake/change p99, including snapshot/shadow cost.

**Required producer smoke, immediately after M2.3b and before M2.3c.** M1's scripted batches bypass the real
acquisition loop, so its host goldens/soak cannot validate the changed `hw.rs:1632-1664` epoch boundary. Under
task 0049's coordinated unit access, use a known input and the normal acquisition path, in batch and frames
routes, with shallow/raw and compressed/deep cases. Seed a batch armed ahead and queued old frames, then run
same-settings epoch requests (CLEAR, STOP-to-RUN, repeated SINGLE, autoset retry and Bode stage) and separate
acq/gen and route changes. Exercise roll/scan and segmented RUN/SINGLE/STOP as smoke cases too; their exhaustive
producer coverage remains M3.0. In both roll and scan, explicitly test CLEAR and STOP-to-RUN:
seed recognizable pre-boundary analog and LA samples, then verify that neither the empty
publication tick nor subsequent new-pair frames refill the cleared screen with those samples.
Require no roll stop/start or hardware reconfiguration for CLEAR's epoch-only boundary.
Also place CLEAR in the final-batch-to-STOP window and verify that a raw reread retains its
captured pair and is rejected if stale. Do not use scripted injection as hardware-loop evidence.

Record requested/applied/output `(gen, epoch, route)`, prearm/disarm and stale/queued-drop counts, configure
counts, accepted record/measurement/fold identities, acquired/published frames, errors and change-to-first-current
record latency over at least 20 boundaries of each kind. Require no output of an old pair after its application,
no old-pair record measured, explicit drop/frame accounting, no reconfigure for an epoch-only request, and a
consistent pair/route on every output. SINGLE must finish once per accepted arm, unresolved tokens accept
nothing, and STOP must retain its final-read contract. Compare acquisition rate and settling against the same
mode's pre-switch baseline and review any regression before c; do not invent a new performance tolerance.
Retain logs and screenshot/statistic comparisons with installed build/lease provenance. Unexpected producer
state stops the round and holds c; report it to the coordinator, without hand repair or automatic recovery.

**Required shared round 1, after M2-M4 plus M3c/M3d.** Measure knob-change settling over more than 20 changes,
clears/resets against baseline screenshots and measurement statistics; verify preview and stale behavior.
Probe compensation and setup B are the M3 smoke checks in that round. Do not substitute a host picture pass
or M2.2 shadow round for this later acceptance. M2.3's intake/change timing budget also needs recorded evidence.
Rerun M2.3b's old-generation-not-measured test through M4's new batch conversion path as regression coverage.

No device access is authorized here. Future rounds require the owner's release and task-specific install/reboot
go, signed packages through the documented update path (`oos-update install <pkg> --no-kexec --execute`),
the appropriate lease capability, renewal at least every two minutes and release afterward. The coordinator
records installed build, go, lease and artifacts. Observe OOSK re-arm and reboot spacing; stop on unexpected
hardware state. Hardware behavior in this plan is specified or assumed, not verified.

Estimate: **12-15 host days**, broken down below. Each M2.3 part has its own cross-vendor review and full gate;
the review/integration allowance covers author response, gate runs and five handoffs, not reviewer scheduling.

| Part | Host days | Included work |
|---|---|---|
| M2.1 | 2-2.5 | Pure move, capability/registry values, move manifests and tested boundary rules. |
| M2.2 | 3-3.5 | Exhaustive classes, oracle/difference fixtures, observe-only wiring, reusable snapshots, allocation/timing checks and host storm. |
| M2.3a | 0.5 | Activate normalize with the legacy notification bridge and intermediate-state regressions. |
| M2.3b | 2-2.5 | Consistent publication, token/pair propagation, producer boundary, scripted identities and record/measure generation filters; prepare producer smoke. |
| M2.3c | 1 | Remaining internal outbox/run transitions and pending-SINGLE guard. |
| M2.3d | 1-1.5 | Mapping/phosphor switch, STOP preview and retained worker-view refresh coverage. |
| M2.3e | 1-1.5 | Remaining consumers, late work rejection, declarations and legacy deletion. |
| Review/gate/integration response | 1.5-2 | Separate gated cross-reviews, corrections, deletion audit and final evidence/handoff. |
| Total | 12-15 | Host work only. |

Add about half a unit day for round 0 and half a unit day for the producer smoke after b; round 1 is shared
with M3/M4. Unit availability, prerequisite/claim waits and independent reviewer availability are outside the
host estimate. Re-estimate after the core move if actual coupling or reviewed differences exceed this allowance.

Risk is medium, as the spec states: every reset changes ownership. Main risks are misclassifying the 194-field
model, hidden protocol/platform calls crossing the pure-crate boundary, gen/epoch/route published separately,
unresolved or recycled stage tokens, late measure/fold results refilling cleared stats, double resets during partial
switches, extra SINGLE records while STOP is pending, and layout/STOP previews changing pictures or brightness.
The real producer's epoch/prearm/queued-work behavior is untested by M1's scripted source; the post-b unit smoke
is a blocking checkpoint, not coverage postponed until M3/M4. Dropping old worker-key trigger fields would leave
workers on a stale Model until M4; require the batch-path trigger-level regression before d. Snapshot/shadow
cloning can add allocation churn or step latency despite a stable heap plateau; test counts and p99 in M2.2.
Treating broad legacy side effects as exact class bits would produce a false shadow oracle; review the table and
expected differences before round 0, retain raw counts, and reject unreviewed differences throughout activation.
Losing normalize's legacy mark before e would skip still-acting consumers; maintain and test the bridge per commit.
Existing STOP redraw already works for some subcases: establish parent failures by execution, not assumption.
Pure moves may expose size/impl-spread findings; use G1's non-growing move manifest, never exemptions or ratchet edits.
M1b's persistence timing and parallel M1/xtask seams are integration risks; obtain handoffs before those writes.
M4/M5/M9/M10 retain their specified conversion, channels, whole-batch ownership and grading work.
Every candidate and conflict resolution needs independent cross-vendor review; the author never merges or pushes.

## Status

- 2026-11-17: M2.3e round 4 committed and rebased onto integ-27 (`b582fa27`),
  plus eleven follow-up commits: SCPI queries and message ends are accepted
  change points (IEEE 488.2 sequential), a switched analysis page builds after
  its resets, the Statistics switch requests StatsReset, integ-27's
  `cancel_measurement` owns the waiting record, tests for the two-level
  trigger repair and the dropped held SIM hit. Pane and eye-source goldens
  re-blessed with reasons in their commits. Host: oos-ui debug 1364 and
  release 1363 lib tests pass (45 ignored), goldens 263 cases pass in three
  release runs (two under a CPU burner), oos-hal 376 per profile; gate clippy
  step exits 0; plan-matrix identical to `b582fa27`. Not run: coverage and the
  full gate (task 0088), unit smoke. Awaiting cross-vendor review.

- 2026-11-17: a display layout change (Display > Windows, the lower pane) keeps
  the measurement statistics and the trend. Before M2.3e the broad settings
  mark reset them as a side effect; the change has no STATS class. Pane
  goldens p2/p3/p4 re-blessed at all three sizes for that reason (count, σ,
  min/max/mean and trend only). Test:
  `a_pane_layout_change_keeps_the_statistics_count`.

- 2026-11-16: e review round 4 final-source verification passes in debug and
  release: SIM 35/35 (all 11 bus cases enabled), Eye 44/44 with two existing
  ignored cases per profile, change 29/29, playback 12/12, timing-unit 8/8,
  outbox 36/35 and core change 28/28. Full commands, counts, exits and logs
  are in `target/m23e-r4-validation.json`; the Rust source receipt
  `target/m23e-r4-final-source.json` remains unchanged throughout validation.
  A fresh host/ARM clippy run on the saved HEAD source matches the prior base
  findings exactly. All 585 Rust/Cargo source/configuration files in that copy
  match `11a8b2b9` (`target/m23e-r4-base-verification.json`). Final candidate
  clippy and size comparisons each show zero new/grown findings and zero
  command errors across all six crates:
  `target/m23e-r4-{clippy,size}-comparison.json`. The unchanged scratch runner
  invokes the gate's exact `steps::clippy`; it exits 1 on inherited rebaseline
  findings (`target/m23e-r4-exact-clippy.log`). The fast gate exits 1 and prints
  exactly `gate FAILED in 33 s: size, unsafe` (`target/m23e-r4-fast.log`).
  These are the known protected-base `d7418b0d` failures; no ratchet, assertion,
  budget or golden was weakened, and no unrelated diff was changed. Final
  audit `target/m23e-r4-final-audit.json` confirms only the eight review paths
  changed, no staging, and zero added-text privacy findings; draft-message
  privacy and `git diff --check` pass. The draft message remains one section with a plain imperative subject and `Task: 0003`.
  Task stays active, uncommitted, awaiting independent review and coordinator
  full/image integration gates. Hardware is unverified; no staging or commits.

- 2026-11-16: e review round 4 retains the accepted `codex-d` claim on
  `t0003-m23e`, uncommitted at `11a8b2b9`. Reproduced the restored upstream
  strict Eye idle holds and immediate non-fresh SIM statistics failure against
  round 3: 8 pass, 3 fail, exit 101 (`target/m23e-r4-before-tests.log`). The
  final new Eye regression and extended SIM regression each independently
  fail on the round-3 production paths (exit 101): Eye is absent instead of
  retained, and SIM statistics are `[2, 2]` instead of `[1, 1]` immediately
  after finish. Logs: `target/m23e-r4-{eye,sim}-regression-before.log`.
  Pure PlaybackPosition reclassification now preserves only the reference Eye;
  other VIEW fields and requests still reset it, including an ordinary VIEW
  edit in the same accepted union. Both upstream strict idle holds retain two
  Eye records without an inserted step. Task 0066 question 1 applies only to
  SINGLE statistics. A qualified non-fresh running SIM capture now reuses the
  single pending slot until its requested stats_gen boundary is accepted;
  repeated polls retain old samples/statistics/Eye and queue the reset once.
  The extended regression covers synchronous finish and asynchronous post-G2
  poll, hit and miss, and one fold into the new generation. Misses queue no
  reset. Consumer time is subtracted from G2 only when separately recorded;
  the Eye reset constants follow the imports. No unrelated source paths changed.

- 2026-11-16: e review round 3 worker handoff remains uncommitted on
  `11a8b2b9`, active and awaiting independent review. The owner-requested seven
  findings and optional cleanups are implemented. Focused debug/release checks
  pass: change 28/28, SIM 35/35 (all 11 bus cases enabled), eye 44/44 plus two
  existing ignored cases, analysis 25/25, autoset 21/21 plus two existing
  ignored cases, outbox 36/35, timing-unit 8/8, playback 12/12 and core change
  28/28. Broad UI passes 756 debug and 755 release, with 21 ignored and the
  same two documented socket exclusions. Commands, exits and complete logs:
  `target/m23e-r3-validation.json`, `target/m23e-r3-refresh.json` and final
  focused verification `target/m23e-r3-verify.json`. The early source-snapshot
  assertions were invalidated by subsequent test cleanup; they are superseded
  by the unchanged final audit/verification receipt
  `target/m23e-r3-final-source.json`. The final test-only split retains all
  capture assertions while eliminating the new complexity finding. No shared
  assertion, lint, ratchet, budget or golden reference was weakened.

- 2026-11-16: final round-3 host scripts pass 14/14 and soak passes 11/11 in
  debug and release. All six warmed census rows have 256 calls and zero
  allocations in both profiles. Final isolated release G2 passes with paired
  intake/change p99 92,751 ns; picture has 90 samples, p99/max 470,375 ns;
  consumers have 81 samples and p99 7,500 ns. Mapping storm passes with paired
  p99 35,291 ns; picture has 256 samples, p99 41,333 ns and max 56,875 ns;
  consumers have 256 samples and p99 1,208 ns. Existing 500,000/750,000 ns
  budgets are unchanged. The owner's round-3 A/B evidence attributes the
  earlier timing/soak variation to host load and reports no picture/soak
  regression; these final runs independently pass their existing guards.
  Logs and commands are in `target/m23e-r3-refresh.json`.

- 2026-11-16: final exact gate clippy step exits 1 on the inherited rebaseline
  `Err` list (`target/m23e-r3-final-exact-clippy.log`). Host/ARM comparison
  against the production candidate base shows zero new/grown findings and no
  command errors in all six crates; size comparison also has zero new/grown
  entries. Evidence: `target/m23e-r3-final-{clippy,size}-comparison.json` and
  `target/m23e-r3-final-audit.json`. The final fast gate exits 1 and prints
  exactly `gate FAILED in 33 s: size, unsafe`. Its protected base is now
  `d7418b0d`, versus `856f5f74` in the earlier passing run's retained Git logs.
  The current range contains no upstream merge for the pending-rebaseline
  allowance, exposing inherited upstream size/unsafe findings. No unsafe syntax
  is added or removed relative to HEAD; no ratchet or Git ref was changed.
  This is a failed gate, not a pass. Full/image gates, coordinator integration
  and independent review remain outstanding. One plain imperative draft in
  `target/COMMIT_MSG.txt`; no staging, commits or hardware access.

- 2026-11-16: e round 3 additionally pins a rejected non-fresh SIM capture.
  A normal-trigger miss replaces no accepted record, so it must not queue
  StatsReset. The expanded configuration regression fails against the initial
  round-3 fix (exit 101, `target/m23e-r3-sim-miss-before.log`, stats_gen 1
  instead of 0 on the miss). Queue StatsReset only after the capture count
  advances; a hit still resets exactly once at the next change point. This
  retains no-hit acceptance and the strict sample/statistics/Eye hold checks.

- 2026-11-16: e review round 3 restores the eye module documentation and
  ordinary assignments, moves the stateless reset declarations into their test
  module and restores the MeasJob documentation. Page reset destructures every
  field exhaustively and delegates chart-label reuse to a small helper. The
  oversized eye/SIM files shed the unchanged raster-copy and generated-result
  evaluation helpers into their own modules; moves are listed in
  `xtask/data/moves.txt`, with no ratchet edits. The native-channel eye test
  extracts its full-frame fixture, making the test shorter than before this
  candidate. The two enabled SIM siblings accepted the capture-position VIEW
  boundary by inserting a step and expecting a one-record reference Eye before
  checking the idle hold. That masked the candidate's Eye policy regression:
  driver navigation destroyed the accumulated pool. Round 4 supersedes
  those expectations; task 0066 question 1 covers SINGLE statistics only.

- 2026-11-16: e review round 3 implements task 0066 question 1's owner
  decision of 2026-11-16: retain our SINGLE behavior, resetting accumulated
  statistics at the accepted change point. Restore the original strict
  `Accepted::assert_held`; each of the four affected upstream tests and both
  previously ignored siblings asserts its reset locally, citing that decision.
  Both siblings are enabled. The strict helper never skips statistics checks.
  The two STOP regressions fail on the review candidate (11 pass, 2 fail,
  exit 101, `target/m23e-r3-stats-before.log`); restoring the helper and enabling
  siblings exposes six SIM expectation failures plus the new running non-fresh
  reset regression (4 pass, 7 fail, exit 101, `target/m23e-r3-sim-before.log`).
  STATS now resets accumulated state/notifications only; current values,
  analysis, pending jobs and in-flight acceptance remain. CLEAR owns their
  removal/cancellation. Non-fresh running SIM payload installation queues
  StatsReset. Consumer timing ends before transitions, which remain in G2.

- 2026-11-16: e review round 3 records two step-order consequences explicitly.
  Terminal autoset intentionally leaves the previous accumulated statistics
  visible until its terminal settings reach the next change point, one step
  later; the existing terminal-autoset test pins retention followed by reset
  and rejection of old-pair records. This is D5, also added to the difference
  inventory. `begin_step` now precedes reference, catalog, SIM and eye polls.
  Those polls do not run when the memory watchdog stops the step. This makes
  accepted resets precede polling; it also suppresses polling under the
  watchdog's resource stop. Hardware remains unverified; no commits.

- 2026-11-16: e follow-up validation on unchanged HEAD `11a8b2b9`, still
  uncommitted and active, not accepted or done. Final production/test source
  receipt `target/m23e-decision-source.json` stays unchanged throughout final
  checks. Focused debug/release pass: change 26/26, analysis 25/25, outbox 36/35
  (one debug-only test), drawpipe 18/18 plus two explicitly ignored guards,
  perfguard 8/8, playback 12/12, SIM 24/24, Bode 5/5 and core change 28/28.
  Both upstream playback tests pass unchanged in both profiles. Broad UI is
  green: debug 751, release 750 passed; 23 ignored and only the two previously
  recorded local socket exclusions. Host scripts pass 14/14 per profile.
  Commands, exact exits and complete logs: `target/m23e-decision-validation.json`
  and final debug refresh `target/m23e-decision-refresh-debug.json`.
  Final scripted renders are deterministic over two repetitions: debug 20
  cases/44 captures; release 263/621. Pixels and metadata are identical to the
  earlier candidate, so the existing base/candidate reasons below still apply
  (three debug and 24 release differences, zero metadata differences).
  Receipts: `target/m23e-decision-render.json`, `target/m23e-decision-picture-{debug,release}.json`
  and `target/m23e-decision-picture-followup.json`. Golden references are unchanged.

- 2026-11-16: follow-up soak/census and timing are not uniformly green.
  Debug soak passes 11/11. The first release soak exits 101: raw's final
  strict live-byte sample grows by 816 bytes, matching the earlier recorded
  intermittent amount; its separate 1 % peak check passes. After all other
  jobs drain, three unchanged release soak runs exit 0, 101, 0. Both passing
  runs exercise all five modes for 1,000 steps each. The failed isolated run
  repeats the same raw final samples `[30971275, 30971019, 30971275, 30971019, 30971019, 30971835]`
  and additionally fails the allocator self-check at
  `ui/tests/snapshots/hot_paths.rs:33` (4 allocations vs 3). All six change
  census rows have 256 calls and zero allocations in every run, including
  the failures. Evidence: `target/m23e-decision-isolated-soak.json` and logs;
  the initial release failure is preserved in the validation receipt above.
  The strict snapshot failures remain unresolved; passing retries do not
  replace them and no bounds or assertions were changed.
  Initial isolated G2 timing exits 101 at `ui/src/ui/tests/shadow.rs:201`:
  raw picture p99/max 875,792 ns exceeds 750,000 ns; paired intake/change
  106,500 ns remains below 500,000 ns. Initial mapping timing exits 0.
  Ten further isolated rounds retain all results: G2 9/10 and mapping 10/10
  pass. Raw picture worst p99/max is 3,721,500 ns in failing round 10;
  paired intake/change always passes (repeat worst 88,666 ns). Mapping worst
  picture p99 is 102,041 ns and max is 1,991,000 ns. Timing verification
  remains unstable, not a clean pass. Receipts and full logs:
  `target/m23e-decision-isolated-all.json` and `target/m23e-decision-timing-repeat.json`.
  Detection/publication remain charged to G2 and consumers measured separately.

- 2026-11-16: follow-up exact gate clippy step exits 1 with the inherited
  rebaseline `Err` list in `target/m23e-decision-final-exact-clippy.log`.
  The scratch runner invokes the unchanged `steps::clippy`; all its supporting
  xtask modules were checked byte-for-byte against this tree. Host/ARM counts
  for all six crates show zero new/grown findings and no command errors vs
  production. Size comparison likewise has zero new/grown over-limit entries.
  Receipts: `target/m23e-decision-final-audit.json` and
  `target/m23e-decision-final-{clippy,size}-comparison.json`.
  `cargo xtask check --fast` exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 85 s`
  (`target/m23e-decision-fast.log`). One updated draft-message section.
  No staging, commits, ratchet/dependency/budget changes or hardware access.
  Independent cross-vendor review, full/image gates and unit smoke remain
  coordinator acceptance requirements; the soak/timing failures above also
  remain visible acceptance limitations. The playback policy question and
  intermittent Bode assertion are resolved by the follow-up below.

- 2026-11-16: e coordinator follow-up implements the presentation decision below.
  Replay/SIM records exact before/after main and zoom positions (including the
  driver's zoom clamp) as `Request::PlaybackPosition`. The change point maps
  only an uninterrupted matching driver delta to VIEW, including the mapping
  comparison. User writes before or after it and unrelated concurrent changes
  retain their ordinary classes. Requests expire at acceptance. The statistics
  consumer is unchanged by this follow-up. The new UI regression accumulates
  eight frames, resets once on a user knob delay edit, then accumulates again
  with stable gen/stats_gen. Disabling the source tag reproduces count 1 vs 2
  on its second frame (exit 101, `target/m23e-decision-playback-before.log`).
  Core tests pin main/zoom origins, mixed user writes, chained requests and
  expiration. Both upstream test bodies are byte-identical to HEAD; receipts:
  `target/m23e-decision-upstream-tests.json`.
  The unchanged full-record SIM test also exposed late source installation:
  first capture was visible with ten old live statistics and an unfinished
  reference measurement (`target/m23e-decision-sim-diagnostic.log`, exit 101).
  Source setup now records its replay request and a late driver waits for the
  next intake's setup/reset acceptance before presenting its first capture.
  Synchronous intake presentation keeps its existing behavior. The unchanged
  SIM test passes after this source-order correction.

- 2026-11-16: Bode root cause is the fixture's observation boundary, not a
  production timer defect. Full-depth demo results arrive asynchronously at
  `ui/src/ui/capture_memory.rs:131`. `bode_finish` marks the sweep stopped,
  while `change/proposals.rs:151` captures its restored settings as commands;
  `change/proposals.rs:112` applies them at the next intake. Completion on
  the old loop's 40th step exposed stopped-but-not-yet-restored settings to
  its assertion. The deterministic `completed_free_bode_restores_settings_on_the_next_intake`
  regression pins exactly that intermediate state and the following stable
  restoration. The existing test now exercises the same two simulated seconds
  and waits for the restoration condition with a 30-second deadline. Every
  original assertion remains. Production Bode behavior retains the documented
  next-intake boundary. Final Bode repetitions pass 30/30 debug and 30/30
  release: `target/m23e-decision-final-bode-debug.json` and the release repeat
  rows in `target/m23e-decision-validation.json`. This resolves the earlier
  intermittent Bode entry without weakening its checks.

- 2026-11-16: coordinator decision for e: replay/SIM playback stepping is
  presentation, not a settings change. Source-tagged playback position changes
  classify as VIEW only and statistics accumulate across frames. User delay
  changes (knob, menu, SCPI, recall, autoset) retain ACQ|STATS, including during
  playback. Implement at the driver/change point, never in the statistics
  consumer. Resume the existing accepted claim and uncommitted candidate;
  investigate Bode timing and validate as specified above. No commits.

- 2026-11-16: e worker evidence on unchanged HEAD `11a8b2b9`, uncommitted;
  task remains active, not accepted or done. Production/test source receipt
  `target/m23e-final-source.json` remains unchanged across final validation.
  Focused debug/release: change 26/26, analysis 25/25, outbox 35/34
  (one debug-only case), drawpipe 18/18 with two explicit ignored timing tests,
  perfguard unit 8/8 and core change 26/26 pass. Host scripts 14/14 and
  soak/census 11/11 pass in each profile. Each standard soak mode runs 1,000
  steps; every census row has 256 calls and zero allocations. Commands, exits
  and complete logs: `target/m23e-final-validation.json` and its log paths.
  An earlier release soak failed its strict plateau check by 816 bytes in
  pairs' last cycle (`target/m23e-handoff-integrations-release.log`); the final
  unmodified assertion passes. This is retained evidence, not a relaxed bound.
  No extended 10,000-step soak, full debug golden/image gate, unit access or
  independent review has been performed for e.
  Broad UI: debug 747 passed, two failed, 23 ignored; initial release 745
  passed, three failed, 23 ignored. Only the two known local socket tests are
  excluded. Replay count and full-record SIM current-value failures are the
  pending delay/STATS contract conflict below; both pass on production
  `11a8b2b9` (`target/m23e-final-base-playback.json`). The third release failure
  is `free_sweep_refuses_what_does_not_fit`, ending a capture without yet
  observing restored settings. It does not recur in ten isolated runs per
  version or three further broad release runs per version: base is green in
  those runs and candidate fails only the same replay/SIM pair. See
  `target/m23e-final-bode-repeat.json` and `target/m23e-final-ui-repeat.json`.
  The intermittent Bode failure remains unresolved; no assertion was weakened.

- 2026-11-16: e final scripted comparison against production `11a8b2b9`:
  debug 20 cases/44 captures and release 263 cases/621 captures, each rendered
  twice with identical pixels and metadata. No input, golden, manifest or
  tolerance changes. Final binaries/receipts are recorded in
  `target/m23e-final-render-{debug,release}.log`. Per-picture bounds, changed
  pixel counts and reasons: `target/m23e-final-picture-{debug,release}.json`.
  Metadata differences: zero. Picture differences: debug three, release 24.

  | Affected pictures | Profiles/text sizes | Reason |
  |---|---|---|
  | menus/12-stats, 13-cursor, 14-drag-offset | Debug large; release normal/large/larger | Effective STATS resets replace broad marks; unrelated/equal menu and cursor edits retain accumulated statistics (D1). Counts and derived statistics change, not waveform geometry. |
  | panes/p2-pane-results, p3-pane-results-cursor, p4-pane-trend | Release normal/large/larger | Pane/cursor changes retain measurement statistics and trend according to their declarations instead of blanket clearing. |
  | refs/50-stats | Release normal/large/larger | Unrelated reference/menu edits retain statistics under D1. |
  | eye/eye-source | Release normal/large/larger | Source reset and one refresh move from input to acceptance/analysis. Both versions show 13 records and 509 edges; the first pooled samples and derived jitter differ. The deferred-refresh regression pins the new phase and prevents duplicate accumulation. |

- 2026-11-16: e isolated release timing, after all other test/render/build jobs
  drained, passes both existing guards without budget changes. Raw storm:
  paired intake/change n=256 p99=68,916 ns against 500,000 ns; picture n=90
  p99/max=247,000 ns against 750,000 ns; consumer n=81 p99=237,542 ns.
  Mapping storm: paired p99=20,750 ns; picture n=256 p99=31,125 ns,
  max=102,792 ns; consumer n=256 p99=708 ns. Detection/normalization and
  publication, including scripted preparation, remain charged to G2;
  consumer work is measured in its separate actual-work ring. The provisional
  unit picture budget remains 3 ms and is not unit verified. Commands/exits:
  `target/m23e-final-timing.json`; complete guard output in its two logs.
  Final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 97 s`
  (`target/m23e-final-fast.log`). The exact clippy step exits 1 with the
  inherited rebaseline `Err` list (`target/m23e-finalverified-exact-clippy.log`).
  Independent measurements of all six crates using the unchanged gate
  implementations show no new/grown host/ARM clippy finding, no command errors
  and no new/grown size entry against the base. Comparisons:
  `target/m23e-finalverified-{clippy,size}-comparison.json`. No ratchet,
  dependency, hook, remote or budget changes. One draft-message section;
  no staging or commits. Cross-vendor review, policy resolution, the unresolved
  intermittent Bode check and unit smoke remain required before acceptance.

- 2026-11-16: e reset ownership audit: HISTORY consumes before segment polling;
  the transition owns `seg_run` and pair-only store resets, and skips stores
  already cleared by HISTORY. CLEAR and CreateMask no longer queue a second
  mask reset. Eye placement no longer clears a second copy of temporal state.
  The deferred refresh regression fails on production `11a8b2b9` with the exact
  final fixture (exit 101, `target/m23e-finalverified-deferred-before.log`) and
  passes after the change. Computation follows acceptance in the analysis
  phase, once; a second idle poll cannot count the retained record again.
  Native reference eye declares VIEW and owns source/raster changes by key;
  Auto Eye snapshots declare NONE and retain their complete source/settings
  key. Explicit clear invalidates both work revisions, rejecting late results.
  The generic live eye still declares MAPPING/VIEW. The class matrix exercises
  both native modes as well as all ten generic classes and NONE.
  The sixth census initially exposed 3,328 allocations in 256 resets, from
  default Arc/page replacement. Reusing unique Arc values and page buffers
  reduces all six rows to 256 calls and zero allocations. Logs:
  `target/m23e-consumer-census-{before,after}.log`. No ratchet or budget changed.

- 2026-11-16: additionally reserve `ui/tests/{host_script,soak}.rs` and
  `ui/src/ui/script.rs` for retired-diagnostic assertions and a sixth warmed
  allocation-census row covering the newly acting reset consumers. The original
  five observation rows exclude consumer work and cannot establish its allocation
  behavior. Script fixtures, golden references and all ratchets remain untouched.

- 2026-11-16: e implementation is in progress, not a completed handoff.
  Six initial consumer regressions fail on the old wiring (exit 101,
  `target/m23e-before.log`) and pass after the cutover. The expanded ten-test
  consumer suite passes debug (`target/m23e-consumers-once.log`), including all
  ten classes, repeated equal requests and script preparation consuming once
  before publication. Late measurements and folds retain b's independent
  gen/epoch/stats_gen tests. The upstream-specific audit adds per-row local
  measurement overrides and reference/SIM configuration changes: they use
  explicit StatsReset because their payload lives outside Model.
  A concrete contract decision is pending: replay/SIM playback changes delay,
  whose ACQ|STATS classes now reset accumulated statistics during capture
  advancement. The upstream accumulation/no-hit tests require retention. Do not
  weaken their assertions or classify the internal presentation as unrelated.
  The owner was asked whether presentation advances need mapping-only events or
  should retain the literal delay reset contract. No answer/acceptance inferred.
  Cursor-only layout changes retain temporal views under the existing accepted
  cursor decision. Native Auto Eye snapshots retain source/settings key reuse;
  explicit view resets get a revision so late snapshot completion cannot refill
  a cleared page. The table/list bookkeeping remains in test fixtures and named
  diagnostic metadata; legacy comparison is explicitly reported retired.
  No hardware access or commits. Task remains active and review has not started.

- 2026-11-16: e preserves the oracle tables, diagnostic counters and independent
  normalization/outbox checks pending explicit table/list retirement acceptance.
  The assigned row-9 deletion removes live legacy mark observation after the
  consumer switch; historical oracle fixtures remain review evidence. Existing
  external edit bookkeeping (bus-candidate cancellation and SINGLE operation
  receipts) has no reset effect and remains an explicit intake operation.
  Additional required caller reservations: `ui/src/http.rs`,
  `ui/src/drawpipe/tests/publication.rs`, `ui/src/scpi/actions.rs` and
  `core/src/change/{requests.rs,tests/requests.rs}` for an equal-settings explicit
  view reset. No worker-view/decode key removal, no class-table waiver.

- 2026-11-16: M2.3e former-trigger audit before production edits. The d table
  above remains the complete picture/worker-key inventory. The remaining effects:

  | Former trigger | Accepted class or explicit operation | Sole acting owner in e |
  |---|---|---|
  | Every screen/SCPI/HTTP mark, normalization repair, autoset/Bode mark, recall | Effective field union from exhaustive core fixtures; geometry/trigger/channel/acquisition/measurement changes carry STATS only where declared; unrelated/equal edits retain statistics (D1); repaired fields only (D3) | Accepted STATS consumer; no blanket notification bridge |
  | Measurement list add/remove/clear, pair/catalog changes, measurement gate/period configuration | STATS for effective list/config diff; explicit statistics reset remains StatsReset at equal settings | Accepted STATS consumer |
  | DEFAULT and Measure > Reset | Explicit StatsReset, including equal settings | Accepted STATS consumer |
  | CLEAR, SINGLE | Request classes including STATS and HW_AVERAGE; CLEAR also MASK/ZONE/MATH/VIEW; CLEAR removes last record and search state, retains history | Accepted request consumers; once per union |
  | STOP-to-RUN, repeated RUN request | HW_AVERAGE; HISTORY when restarting segments | Accepted class consumers; transition owns hold/final read only |
  | Segments/seg_hw/seg_ivl changes | HISTORY, including disabling segments; running pair boundary still rearms hardware segments; STOP completed browse retains times on identity-only CLEAR/mask edits | HISTORY clears stores, transition owns recording restart |
  | Create mask, load mask, MaskReset, mask enable/source/tolerance/invert | MASK request or field class; build mask stays construction, stop-on-failure stays outbox STOP | Accepted MASK resets software and hardware counters once |
  | Zone changes and CLEAR | ZONE field/request class | Accepted ZONE clears drops and hardware counter id once |
  | Acquisition mode, avg_count, avg_hw; RUN/SINGLE/CLEAR | HW_AVERAGE; includes previously missing acquisition-mode restart (D7) | Accepted HW_AVERAGE increments hardware clear id once |
  | Analyzer adjustments, source repair | MATH/VIEW exact field contract; source snap moves to normalize only; equal/unrelated edits do not reset | Declared held-state consumers |
  | Math operation/source/scale/filter/expression/FFT | MATH plus STATS; record math/FFT itself is stateless (D8) | No fabricated math buffer; held spectrum state reset explicitly |
  | Trend fresh-list convention | STATS or ACQ | Declared trend consumer; acquisition of a value only appends |
  | Live eye source/clock/rate/options and effective mapping | VIEW or MAPPING, except the accepted cursor-only layout contract | Declared eye consumer; data-driven range/UI adaptation remains computation |
  | Native reference eye and Auto Eye snapshots | Reference declares VIEW; snapshot declares NONE, retaining source/settings keys; explicit Clear/ViewReset invalidates both | Declared eye consumer plus keyed native computation; scope navigation preserves full-record history |
  | Spectrum average/waterfall, power/bandwidth/magnetic/motor/double-pulse results | MATH/VIEW as declared; late analyzer work revision rejects old completion | Declared held-state consumer, including hidden panes |
  | Cursor and XY | Stateless rendering; cursor mode remains unrelated; timebase uses current mapping | Empty reset declaration, no invented buffer |
  | Old meas_n cancellation, pending measurement, batch folds | Exact gen/epoch/stats_gen filters already landed in b; STATS drops pending notifications; current values/jobs remain held and identity filters reject stale results | Identity filter plus accepted statistics consumer |
  | Terminal autoset statistics | D5: terminal settings and their STATS reset wait for the next change point; previous statistics remain visible for one step | Outbox plus accepted STATS consumer; terminal-autoset regression pins both sides |
  | Bus-candidate cancellation and pending SINGLE operation after settings edits | Explicit effective-settings boundary, independent of resets | Detection/intake boundary, no mark wrapper |
  | Lifecycle memory reset | Resource release, independent of settings classes | Existing lifecycle reset retained |
  | Decode and worker-view caches | Record/config keyed decode; complete transitional WorkerViewKey through M4 | Existing keyed caches retained |

  Hardware not verified. Required unit smoke: statistics reset on each class
  (and retention on non-STATS classes); history/segments including STOP browse
  and disable/restart; mask/zone stop and reset; hardware average restart;
  math and every declared view including hidden state; both Standard and Batch
  draw paths; RUN/STOP/SINGLE including repeated requests and final read. Retain
  producer/c/d holds, lease/build provenance, record/arm/error/identity counters,
  per-step captures and unchanged G2/picture timing budgets.

- 2026-11-16: M2.3e claimed on clean `t0003-m23e` at `11a8b2b9` under
  the owner's assignment. Read delivery, row 9, bridge rules and recorded review
  decisions. Building the former-trigger coverage table before source changes.
  Unit-tier review and coordinator integration/image/hardware holds remain.
  No commits or hardware access.

- 2026-11-16: M2.3d round 3 worker handoff, no commits, unchanged HEAD
  `d7418b0d`. All five requested items are addressed. Fresh picture evidence
  supersedes `target/m23d-verified-pictures.json` from before round 2. Verify the
  archived base source byte-for-byte against d7418b0d, rebuild debug and release
  with its own `target/m23d-r3-base-build`, then render base and final candidate
  in fresh processes and isolated state directories, twice per capture.
  Debug: 20 cases / 44 captures per binary, all repeated captures deterministic,
  zero RGB or metadata differences. Release: 263 cases / 621 captures per binary,
  all repeated captures deterministic, zero RGB or metadata differences. There
  are no differing captures to justify and no golden references were changed.
  The matrix does not replace the focused Z-input, equal-settings session Replay
  and pending STOP-final Batch regressions, which pass on the final source.
  Commands: `cargo build --locked --manifest-path target/m23d-base-source/ui/Cargo.toml
  --bin oos-ui` with/without `--release` and isolated CARGO_TARGET_DIR, and
  `python3 target/m23d-capture.py {r3base,r3final} {debug,release}` for fresh renders.
  Build/render logs use those labels; direct comparison is
  `target/m23d-r3-pictures.json` and `target/m23d-r3-compare.log`. Source and binary
  SHA-256 receipts are in `target/m23d-r3-source.json`.

  Final focused debug and release each pass 42 tests, zero failures, two explicit
  timing ignores, with default test parallelism. Commands/exits/logs are in
  `target/m23d-r3-final-checks.json`. Both ignored release timing guards then
  pass explicitly and in isolation, exit 0 each, using the calibrated 750,000 ns
  host budget and provisional 3,000,000 ns unit budget. This adds run 11 of each
  storm: shared n=90, picture p99/max 296,875/296,875 ns, intake/change/paired p99
  78,958/4,625/81,541 ns; mapping n=256, picture p99/max 86,250/93,417 ns,
  paired p99 14,083 ns. Neither exceeds the 422,833 ns calibration worst, so the
  derivation below holds across all 22 runs. Full commands, reports and exits:
  `target/m23d-r3-final-timing.json` and its two logs. The mapping guard's exact
  zero-oracle assertion passes, with no unreviewed diagnostic.

  The exact unchanged gate clippy step runs all six host/five ARM invocations,
  exits 1 and prints `FAIL clippy (8.4 s)`
  (`target/m23d-r3-final-exact-clippy.log`). Its complete 198-finding list equals
  the round 2 list (`target/m23d-r3-final-clippy-equality.log`). The direct
  d7418b0d/candidate runner reports new=0/grown=0 for every one of six crates,
  zero compiler errors, exit 0 (`target/m23d-r3-clippy-comparison.log`); the final
  budget-only literals/comment update adds no finding. No ratchet or rule edits.
  `cargo xtask check --fast` exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 84 s`
  (`target/m23d-r3-fast.log`). Whitespace, source-receipt and added-text privacy
  checks pass. `target/COMMIT_MSG.txt` retains one section, with fresh capture
  evidence and the new derivation. No staging, commit, push, merge or hardware
  access. Task stays active for cross-vendor verification, coordinator full/image
  gate, the earlier socket checks on a permitted host, and actual unit storm p99
  plus Batch final-read acceptance. The 3 ms unit estimate supersedes the earlier
  1 ms provisional entries; it is not measured unit acceptance.

- 2026-11-16: M2.3d round 3 calibration supersedes the earlier mixed-step
  130,458 ns derivation and 1 ms provisional budget. The actual-clear-only ring
  retains the last 256 clears across idle steps, with an independent sample count;
  both state query and perf output report `stage picture n=... p99_ns=... max_ns=...`.
  Fewer than 50 clears report p99 pending, while max is already available. Storage
  remains 2 KiB plus one usize, observer-owned, without per-step allocations.
  Both new synthetic regressions fail with the old sampler (exit 101,
  `target/m23d-r3-timing-before.log`); idle steps, sparse clears, ring wrap and
  legitimate zero-duration samples are covered. The state-query regression pins
  60 actual clears across 120 steps and zero oracle disagreements.

  `change.rs`'s inline review module moved to
  `ui/src/ui/tests/change_picture_contracts.rs`; the test-only
  `mark_mapping_events` field and writer are gone. The settings-mark test now
  asserts exact reviewed oracle deltas. The mapping storm calls settings_marked
  after each offset edit and requires every oracle counter to remain zero;
  no unreviewed storm line is accepted. The shared storm now runs 1,000 steps
  to collect 90 actual clears, enough for p99, instead of padding 300 steps
  with idle timings.

  Isolated aarch64 release calibration ran each guard ten times, interleaved,
  with no builds/captures/gates running alongside. Each shared run has n=90
  (nearest-rank p99 is its max); each mapping run has n=256. All values are ns:

  | Run | Shared p99 / max | Main+zoom mapping p99 / max |
  |---|---|---|
  | 1 | 422,833 / 422,833 | 39,375 / 66,000 |
  | 2 | 332,875 / 332,875 | 67,875 / 115,417 |
  | 3 | 319,416 / 319,416 | 76,792 / 82,084 |
  | 4 | 302,917 / 302,917 | 69,667 / 97,541 |
  | 5 | 291,125 / 291,125 | 56,750 / 305,041 |
  | 6 | 251,333 / 251,333 | 32,458 / 49,959 |
  | 7 | 365,791 / 365,791 | 59,250 / 106,166 |
  | 8 | 310,667 / 310,667 | 37,000 / 76,667 |
  | 9 | 264,667 / 264,667 | 41,458 / 78,208 |
  | 10 | 273,542 / 273,542 | 68,833 / 93,041 |

  All ten shared calibration runs exit 101 at the old 250,000 ns picture
  assertion; all ten mapping runs exit 0, with zero oracle counters. Detection
  retains its independent 500,000 ns guard. Full commands, exits and raw reports:
  `target/m23d-r3-calibration.json` and `target/m23d-r3-calibration-{shared,mapping}-NN.log`.
  The worst observed p99, 422,833 ns, exceeds the reviewer's 164,500 ns and all
  dedicated-storm results. Derivation: 422,833 x 4 = 1,691,332 ns estimated A72;
  x 1.5 tail headroom = 2,536,998 ns; round up to 3,000,000 ns provisional unit
  budget, giving a 750,000 ns host guard. Both guards and the permanent performance
  documentation use these values (`target/m23d-r3-budget.json`). Unit storm p99
  remains the real acceptance item; no hardware access or unit performance claim.

- 2026-11-16: final review-round worker handoff, uncommitted on unchanged d7418b0d.
  All seven findings are addressed; the audit table below covers the base's
  dirty-map seams, key fields and oracle rows. Failing-before execution logs are
  `target/m23d-review-before-{mapping,events-timing,core}.log`: four picture/session
  failures, four emission/timing/script/STOP failures, and the literal Z contract
  failure respectively (all exit 101). The independent oracle-required-class and
  warmed-record negative controls also both fail their behavioral assertions,
  exit 101 (`target/m23d-review-negative-oracle-record.log`); controls were restored.

  Final focused debug and release each pass 41 tests, zero failures, two explicit
  timing ignores. Complete logs: `target/m23d-review-final-focused-{debug,release}.log`.
  Both timing tests then pass explicitly, isolated after builds/soaks, with 256
  samples: shared storm intake/change/paired p99 66,958/6,250/71,124 ns and picture
  21,792 ns; dedicated main/zoom mapping storm picture p99 52,208 ns, paired
  11,000 ns. Both assert picture <=250,000 ns and paired <=500,000 ns independently.
  The provisional unit picture budget is 1 ms, with hard unit storm acceptance
  still pending. Commands, exits and complete logs are in
  `target/m23d-review-isolated-timing-final.json`. No unit speed claim is made.

  Final-source host-script 14 and soak/census 11 tests pass in each profile;
  `target/m23d-review-final-integration-checks.json` records commands/exits/logs.
  Five release modes each complete 1,000 steps after 500 warmup steps, zero failed
  plateau cycles, no retained growth, and acquired/used/dropped 13,648/13,618/30.
  Each of the five warmed UI census rows reports 256 calls and zero allocations;
  snapshot reuse reports zero versus 220 naive calls, 6,579 bytes per snapshot.
  Debug retains the composed-soak release-only policy; its allocator/census tests
  do run. No extended 10,000-step/image or new golden acceptance is claimed.
  The broader implementation check also passed UI 736 debug / 735 release
  (23 ignores, two socket exclusions), drawpipe 18 / 2 ignores, and core 26 unit
  plus 3 value tests in each profile. Its UI run predates only the final oracle
  value-type encapsulation/rename and ignored timing-budget calibration; the
  final focused, integration and timing runs above cover the final implementation.
  Commands and evidence are in `target/m23d-review-final-checks.json`.

  Final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 82 s`
  (`target/m23d-review-fast-final.log`). The exact unchanged gate clippy step,
  including six host and five ARM invocations, exits 1 and prints
  `FAIL clippy (8.7 s)` (`target/m23d-review-exact-clippy-final.log`). All 198 reported
  entries match the earlier exact list. The isolated baseline comparison has
  new=0/grown=0 for all six crates, zero compiler errors; complete comparison logs
  are in `target/m23d-review-clippy-comparison.log`. Final-list equality is in
  `target/m23d-review-clippy-final-equality.log`. No ratchet, lint, allowance,
  dependency, hook or remote was changed. Source hashes are retained in
  `target/m23d-review-final-source.json`; whitespace and added-text/message privacy
  checks pass. One draft section remains in `target/COMMIT_MSG.txt`.

  Both socket cases were attempted on the final debug/release test binaries and
  exit 101: Bode's listener bind is PermissionDenied/Operation not permitted;
  bus-candidate's SCPI listener is unavailable. Full failures and commands are in
  `target/m23d-review-sockets.json` and its logs. No skip/ignore annotation was
  added. The coordinator must run both on a host that permits loopback sockets.
  No staging, commit, push, merge or hardware access. Task remains active for
  cross-vendor verification, coordinator full/image gate and the recorded unit
  smoke, including mapping-edit-storm p99 and Batch RUN-to-STOP final read.

- 2026-11-16: review-round timing decision: G2 detection ends before picture::apply;
  the main/zoom clear has its own fixed 256-sample ring (2 KiB), `stage picture`
  counter in :SYSTem:STATe? and adjacent perf output. Isolated calibration on the
  aarch64 release host measured picture p99 55,292 ns in a 300-edit storm that
  clears both maps every step; the mixed M1 storm reached 130,458 ns. Use the
  higher result, including colder and varied-layout clears. The existing
  A72/host estimate is 4x (`docs/fork/perf-plan.md`, Host measurements): 521,832 ns.
  Apply the perf guard's existing 50% tail headroom, giving 782,748 ns, then round
  up to a provisional 1,000,000 ns unit budget and a 250,000 ns host guard. Both
  storm tests assert this independent picture budget. Unit storm p99 is a hard
  acceptance item, not established by this estimate. The paired G2 guard remains
  500,000 ns. Calibration counter lines and derivation are retained in
  `target/m23d-review-timing-calibration.json`; those earlier raw captures were
  superseded by the final run. Complete final logs and both independent budget
  assertions are in `target/m23d-review-isolated-timing-final.json` and its logs.
  The final syntax round initially printed `gate FAILED in 86 s: size` (exit 1):
  the walker counts impls by spelling, so the new private Context constructor was
  grouped with unrelated Context types. Renamed it OracleContext; no rule was
  changed. The exact base/candidate size walker then reports new=0/grown=0 in
  all six crates, with ten UI entries shrinking (`target/m23d-review-size-delta-final.log`).

- 2026-11-16: M2.3d review-round implementation addresses findings 1-7 without
  commits. Z source now participates in MappingKey; its RUN/infinite-persistence
  regression exercises Standard and Batch. The literal core field contract also
  requires MAPPING. Record presentation observes its old map clear; offline entry
  and return to retained live settings issue an explicit Replay request even at
  equal settings. Script route policy updates an existing Prepared. Batch keeps
  merging while STOP final read is pending. settings_marked emits one Mark/MAPPING
  observation. Each finding has a failing-before host regression (logs and final
  validation follow). The oracle now keeps required effective mapping/phosphor
  contracts independently of the classifier: a missing required class increments
  unreviewed instead of receiving D1. Conditional layout inputs require MAPPING
  only when their effective Layout changes; unrelated broad clears still use D1.

  Base dirty-map/clear audit, including upstream writers and every MappingKey
  projection. `M` means MAPPING, `P` PHOSPHOR; these are picture classes only,
  unioned with the independently declared acquisition/statistics/view classes.
  The exhaustive 296 literal core field fixtures and nested worker fixtures pin
  the key/class relationship; host UI regressions pin accepted clear behavior.

  | Base fields or effective input | Class/key coverage | Old seam and oracle disposition |
  |---|---|---|
  | `ch[c].on/vscale/vfine/bias/skew/offset/coupling/bw/probe/invert` | M, MappingKey.channels | Mark/Recall/Stage/Normalize; required geometry, D2/D3 for old absence only |
  | `tscale/tfine/delay/zoom/zscale/zdelay` | M, MappingKey.timebase | Mark/Recall/Stage, wheel/zoom/fit; required geometry |
  | `dots` | M, MappingKey.display | Layout; equal dots is extra old D1, effective edit requires M |
  | `z_src` | M plus ACQ/STATS, MappingKey.display | Layout; required M, missing clear is a regression, never D1 |
  | `win_layout/ch_win/math_win/la_win` | M, MappingKey.display | Layout/workspace/relayout; direct key changes require M |
  | Main/zoom dimensions, panel/HDMI output, pane/menu geometry | M, MappingKey.layout | Layout, actual size comparison; required M |
  | `zoom_vfactor/zoom_vcenter` | M, Layout.vertical_zoom | Vertical wheel/drag/set_zoom; required M |
  | `persist/color_grade` | P, literal field classes | Mark/direct persistence clear; required P |
  | `acq/avg_count/avg_hw` | P, literal field classes | Acquisition/Mark/Stage; required P; missing old average/STOP reset remains D7 |
  | `draw_batch` | P, literal field class | Layout/draw-path switch; required P |
  | `deskew_ch` | P plus STATS, literal field class | Layout/direct clear; required P; extra old M is D1 |
  | `ref_win/density/strip_dock/split_pane/pane`, HDMI and bottom-bar choices | M when effective layout changes, through Layout | Raw settings are not unconditional geometry; actual Layout is required, extra old clear D1 |
  | `deskew_ref`, ref scale/offset/visibility, record timebase/source selection | M from effective timebase/Layout, or Replay on presentation/session transition | Analog phosphor does not store reference traces; extra analog clear D1; explicit presentation clear observed at Layout |
  | Trigger/demo/decoder choices, `ch[c].unit`, labels, brightness, cursor/analysis/measurement choices and same-value recall | M/P only for accompanying effective fields in the rows above | Mark/Trigger/Recall/Stage/Analyzer/Measurement; broad extra old clears D1; trigger-only worker refresh retained |
  | Initial construction; CLEAR; SINGLE; record presentation/session entry/return | Initial empty maps; CLEAR/SINGLE P; Replay M | Clear/Single/Layout request consumers act at equal settings and once per acceptance |
  | RUN-to-STOP, ordinary Min replacement, decay, STOP redraw, measurement-result arrivals | No new settings clear class | Run/Drawing/Measurement; drawing/final-read behavior retained, extra result-driven dirty marks D1 |
  | Publication/Reset/Mask/Math/MaskSource and explicit stage epoch | No implicit M/P | Their own declared consumers remain unchanged; any accompanying geometry/persistence uses this table |

  The post-d unit smoke additionally requires RUN -> STOP final raw read in Batch:
  an in-flight batch merges before the final current-pair raw record replaces it,
  then workers retire. Repeat Z-input edits with infinite persistence and repeat
  equal-settings record/session transitions. Hard performance acceptance: unit p99
  under a mapping-edit storm must meet the provisional 1 ms picture-clear
  budget and the existing 500 us G2 intake/change budget, retaining FPS and complete-step comparison.
  These hardware checks remain pending; no device access is authorized here.

- 2026-11-16: M2.3d worker handoff on unchanged `d7418b0d` plus this unstaged
  diff. Final broad UI selection passes 727 debug / 726 release, 22 existing
  ignores each, with default test parallelism. Exact commands:
  `cargo test --locked -p oos-ui --bin oos-ui [--release] ui:: -- --skip ui::tests::bode_sweep_drives_the_generator --skip ui::bus_candidate::tests::real_scpi_queries_keep_pending_candidate_and_force_uses_the_live_bus_path`.
  Logs: `target/m23d-final-ui-{debug,release}-socketless.log`, both exit 0.
  The first broad runs excluded only Bode and exited 101 (727/726 passed,
  one failed): the existing bus-candidate SCPI socket has no listening port
  in this sandbox. Both failures are retained in `target/m23d-final-ui-{debug,release}.log`;
  no test annotation, assertion or transport was changed to hide them.
  All other final commands exit 0: drawpipe 18 passed / 2 ignored, core 26
  unit + 3 value tests, host-script 14 and soak 11 in each profile. Commands,
  exits and complete logs: `target/m23d-final-checks.json` and its references.
  New coverage includes STOP redraw without a new record on both paths,
  accepted class unions, same-value CLEAR/SINGLE, no repeated clears, rejected
  mapping followed by acceptance, vertical zoom, same dots, unchanged Min/decay,
  retained statistics effects and accepted trigger-only worker publication.
  The controlled real-worker/reference regression passes; forcing its
  mapping-only negative control fails its new-level picture assertion as intended
  (exit 101, `target/m23d-mapping-only-negative-control.log`). Core's full old
  worker-dependency table also passes in both profiles.

- 2026-11-16: final picture verification renders d7418b0d and the candidate in
  fresh processes twice per case: debug 20 cases / 44 captures, release 263 /
  621. All 665 decoded RGB images and settings metadata match exactly; every
  repeat is deterministic. There are zero differing captures, so no per-capture
  exception or re-bless is needed. Evidence: `target/m23d-verified-pictures.json`,
  `target/m23d-{base-smoke,verified}-debug-captures.json`,
  `target/m23d-{base,verified}-release-captures.json` and their capture directories.
  Every stderr log has the same unreviewed event/field lines as its base:
  debug 0/0, release 58/58 lines across repeat logs (inherited upstream cases),
  zero differing logs (`target/m23d-oracle-inventory.json`). Golden references,
  manifests and tolerances remain unchanged; the redesign reference mismatch
  is still task 0066's separate re-bless work.

- 2026-11-16: final soak/census and isolated timing evidence uses the same Rust
  source hashes (`target/m23d-final-source.json`). Five release modes (raw,
  codes, pairs, deep-pairs, average) each complete 1,000 steps, warmup 500,
  with zero failed cycles and no retained growth. Each has exactly
  acquired/used/dropped 13,648/13,618/30. Debug retains the existing explicit
  release-only policy for this composed soak. Both profiles' five warmed UI
  census rows report 256 calls and zero allocations. Snapshot reuse has zero
  allocations versus 220 naive allocations, retaining 6,579 bytes per snapshot.
  Logs: `target/m23d-final-soak-{debug,release}.log`; summaries and cycle CSVs
  are under the per-mode directories printed there. No 10,000-step/image
  acceptance is claimed. Isolated aarch64 release timing, run sequentially
  after builds/captures/checks, passes on base and candidate with 256 samples:
  base intake/change/paired p99 73,959/5,249/78,084 ns; candidate intake/change/paired p99 71,417/234,875/260,501 ns
  (budget 500,000 ns). Complete samples are recorded in `target/m23d-isolated-timing.json` and its complete logs.
  The accepted picture clears now count inside change time; the old dirty
  clears ran later in Ui::step. This phase movement is explicit; it is not
  an on-unit frame-rate or complete-step performance claim. The required
  unit comparison against the latest baseline remains in the smoke list below.

- 2026-11-16: final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 90 s`
  (`target/m23d-fast-final.log` and its printed complete-log directory).
  Its upstream-pending entries remain task 0066 work; full/performance/coverage/
  image checks are not selected. A direct measurement with the unchanged gate
  size walker compares the complete base and candidate: zero new or grown
  size entries in every crate, ten UI entries shrink (`target/m23d-size-delta-final.log`).
  The exact unchanged gate `steps::clippy` was run, including all six host and
  five ARM invocations and protected-base comparisons: it exits 1 and prints
  `FAIL clippy (1.6 s)` for inherited upstream ratchet findings. Its pending
  comparison also fails on three inherited core enum_variant_names findings
  and the inherited segmented-test unnecessary_cast; these are not waived
  or repaired in d (`target/m23d-exact-clippy-final.log`). Measuring d7418b0d
  and the final candidate with that same clippy runner in separate target
  directories proves zero new/grown findings and zero compiler errors in all
  six crates (exit 0, `target/m23d-clippy-isolated-comparison.log`, with full
  command logs). Early shared-target comparisons duplicated diagnostics at
  old/new line numbers, and the first incomplete base archive lacked a required
  included public-key file; those exploratory failures remain recorded separately.
  No ratchets, lint limits, allowances, hooks, dependencies or remotes changed.
  Whitespace and added-text/message privacy checks pass (zero findings).
  `target/COMMIT_MSG.txt` contains one draft section. No staging, commits,
  push, full gate or hardware access. Task remains active for cross-vendor
  unit-tier review, coordinator integration/image checks and the listed unit
  smoke; existing producer/c integration holds are preserved.

- 2026-11-16: M2.3d implementation removes every `dirty_map` occurrence in source,
  plus the old drawpipe Key. `mapping_observed` is observation-only; mark_changed,
  settings_changed and normalize's statistics bridge remain until e. Accepted
  MAPPING/PHOSPHOR clear both maps once for their union. Min receives a local
  already-cleared result and retains its replacement/decay operations. STOP's
  existing same-record redraw owns its sole clear and legacy weight; running
  no-frame preview remains M4, as the reviewed acceptance map specifies.
  Accepted publication now owns worker enable/disable, including Standard/Batch
  switches and leaving the hardware source. WorkerViewKey preserves every old
  Key dependency; scripted record-only policy travels in Prepared (one bool,
  no extra snapshot or allocation). Explicit stale-batch injection and isolated
  host fixtures retain their controlled view construction, not normal UI writers.
  Upstream vertical zoom now enters Layout's mapping value (two u32 float bits);
  draw_batch classifies PHOSPHOR to preserve its old deliberate reset. Its core
  contract and unrelated-field snapshot are updated, not widened globally.
  Reservations also include `core/src/change/tests.rs`, its display2 contract,
  `ui/src/drawpipe/tests/publication.rs` and the existing Bode observation callers.

- 2026-11-16: base-compatible regression evidence with production unchanged:
  `target/m23d-before-valid.log` (debug) and `target/m23d-before-release.log`
  each exit 101: three failures, four passes. Failures demonstrate no accepted
  map clear without the old mark, immediate main-only persistence clear at input,
  and broad picture clearing for unrelated/trigger/same-value marks. The STOP
  scale/offset/zoom/layout acceptance and Min/decay characterization already pass
  on the base; no parent failure is claimed for them. Initial fixture attempts
  had a wrong Grid variant and indexed a 1-pixel inactive zoom map; corrected
  before this behavioral evidence. Their logs remain under the m23d-before prefix.
  Initial exact clippy exposed a new unused test-only script_prepare wrapper;
  it is now cfg(test). Upstream findings remain a separate baseline comparison.

- 2026-11-16: M2.3d claimed on clean `t0003-m23d` at `d7418b0d` by codex-d.
  Read the row-8 contract, all recorded coordinator decisions and task 0066.
  Upstream adds dirty-map writers beyond the historical 16; these are included
  in the deletion audit. Existing redesign golden failures and upstream size
  findings remain task 0066's separate rebaseline/re-bless work.
  Required post-d unit smoke (pending, not verified here): for both menu-selected
  Standard and Batch paths, change mapping while RUN and while STOP; test scale,
  offset, zoom (including vertical zoom), timebase/delay and layout with no new
  frame, retaining the same last record and a nonempty trace at every capture.
  Check persistence Min/500 ms/infinite, colour grade, mode/average changes,
  CLEAR and SINGLE: main and zoom phosphor maps/weights clear once at each
  accepted boundary, with no retained old mapping, lost clear or blank trace.
  Check trigger-only changes refresh real Batch workers without geometry changes,
  and Standard -> Batch -> Standard cannot merge a retained old worker picture.
  Confirm Min replacement and timed decay still work with and without new frames.
  Compare frame rate, complete step timing and intake/change p99 against the
  latest same-mode baseline, idle and during repeated edits; retain build,
  lease, settings, counts and per-step captures. Review regressions before
  acceptance. Hardware release/install go and the existing producer/c holds
  remain coordinator requirements. No unit access is authorized here.

- 2026-11-16: integ-19 coverage repair handoff at unchanged `0137955` plus the
  unstaged test/task diff. The cause is a test assumption about one UI step,
  not a production SINGLE completion race: its hardware fixture enables the
  15 ms wall-clock record budget. Yielding after the partial counts one accepted
  record and keeps SINGLE armed; the complete record queues STOP on a later step.
  The existing headless host removes this scheduling dependence only in the
  regression. Both same-step and separate-step deliveries retain all original
  record, queue, deferred STOP, hold and trace assertions; extra checks pin an
  empty waiting step and no further completion/queue consumption after STOP.
  No production Rust source changed, including the restored budget diagnostic.
  All 600 final-source focused executions pass: 200 debug, 200 release and 200
  via cargo-llvm-cov 0.9.1, with four concurrent CPU load workers and default
  test threads. Coverage runs use the xtask target and command from `ui`:
  `CARGO_TARGET_DIR=../target/xtask-cov cargo llvm-cov --locked --json --summary-only --lib -- ui::tests::outbox::partial_single_completes_once_and_leaves_later_records_queued --exact`.
  Each run emitted its own JSON summary; no report, instrumentation or test
  work was skipped. Per-run commands/exits/logs and CPU-load evidence are in
  `target/m2-coverage-stress/results.json` and `target/m2-coverage-stress/`.
  The complete UI lib suite ran once under the same runner, without test filters
  or extra skips: exit 101, `721 passed; 21 failed; 31 ignored; 0 measured;
  0 filtered out`, 109.22 s. The target regression passes. All 21 failures need
  TCP/UDP sockets unavailable in this sandbox: HTTP/listener initialization
  fails, with direct bind errors `PermissionDenied: Operation not permitted`.
  Complete log: `target/m2-coverage-stress/coverage-lib-001.log`.
  Exact gate clippy and fast-gate passes are recorded below. Final source hashes
  match every verification; `target/m2-coverage-verification.json` consolidates
  results. CPU workers stopped. One section in `target/COMMIT_MSG.txt`; no
  commits, staging or hardware access. The full suite still needs a socket-capable
  runner; independent review and the coordinator integration gate remain pending.

- 2026-11-16: final-source debug and release stress pass 200/200 each with four
  concurrent CPU load workers and default test threads. Full commands, per-run
  exits and logs: `target/m2-coverage-stress/results.json`. The exact unchanged
  `steps::clippy` passes all host/ARM invocations and protected-base ratchet
  comparisons (`ok   clippy (8.2 s)`), zero new/grown findings or compile errors;
  `target/m2-coverage-clippy.log` links complete command logs and
  `target/m2-coverage-clippy-dispatch.json` pins the scratch dispatch copy.
  Fast gate exit 0: `partial gate passed (not done): 8 steps in 22 s`
  (`target/m2-coverage-fast.log`). Coverage stress is running; its first command
  used an unsupported positional filter and exited before tests. Corrected to
  pass the exact test filter after `--`; the CLI error is retained separately.

- 2026-11-16: cause isolated to the live `Ui::step` 15 ms wall-clock record
  budget, enabled by `Host::fixture()`. After the accepted partial, a deadline
  yield leaves records=1 and RUN; the complete record and deferred STOP belong
  to later steps. No producer thread or scripted-source deadline runs in this
  fixture. A temporary zero-budget diagnostic reproduces the exact parent
  assertion (exit 101) and the revised test passes (exit 0). Production budget
  and all production sources restored byte-for-byte before stress verification.
  The headless test now exercises same-step and separate-step partial delivery,
  an empty intervening step, deferred STOP, retained trace/hold and the untouched
  third frame through another stopped step. Logs:
  `target/m2-coverage-{before,after}-zero-budget.log`. Stress/gates pending.

- 2026-11-16: claimed the owner-assigned integ-19 coverage regression at clean
  `0137955` on `t0003-m2`. Inspecting the hardware queue and record budget;
  completion/outbox and all retained queue/trace assertions remain required.

- 2026-11-16: SINGLE/segment verification handoff on unchanged `b53aee1` plus
  unstaged changes. Both P2 fixes are implemented. Recording/Reading alone abort
  at pair boundaries; completed STOP segments and times survive CLEAR and mask
  source changes without configuration or another arm. Browse reads use the
  current snapshot, reject and retry a pair change during DMA. Real hardware
  reconfiguration cancels pending requests and clears UI times/position on Idle,
  including when segments are disabled and a late click races that observation.
  Rejected final recording DMA does not advance its cursor into Done.
  VXI-11's host/device callbacks now use optional terminated SCPI responses;
  empty queries return a newline, while commands queue no reply. The remote
  caller search also found MCP's empty-to-`ok` substitution, now fixed using the
  optional unframed message so MCP keeps its existing JSON framing and reply text.
  The lossy `exec_line` wrapper is now test-only. No other production caller remains.

- 2026-11-16: final focused checks exit 0 on unchanged hashes of all 14 changed
  Rust files, with default threads (`RUST_TEST_THREADS` unset): 61 debug / 60
  release passed. Selection covers outbox/SINGLE, hardware worker fixtures, SCPI,
  VXI-11 and MCP, including all five new browse/transport regressions. Tests use
  the production TCP response encoder, VXI-11 RPC core and MCP handler; existing
  compound-query TCP and host VXI listener tests are explicitly excluded because
  socket binding is unavailable. Commands, counts and complete logs are in
  `target/segment-verification-final-checks.json` and
  `target/segment-verification-final-focused-{debug,release}.log`; source hashes
  are in `target/segment-verification-final-source.json`.
  The unchanged gate `steps::clippy` passes its actual host/ARM invocations and
  protected-base ratchet comparisons: zero new/grown findings, zero errors.
  Its target-only dispatch copy changes no check implementation or lint;
  `target/segment-verification-final-clippy.log` and its referenced command logs
  retain all 11 invocations. Fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 32 s`
  (`target/segment-verification-final-fast.log`). Initial fast gate printed
  `FAIL size` / `gate FAILED in 24 s: size` for HTTP 1115 -> 1116; nesting the
  test declaration fixed it. Initial exact clippy printed `FAIL clippy` for
  the unused production `exec_line`; restricting that wrapper to tests fixed it.
  These failures remain in `target/segment-verification-fast-initial.log` and
  `target/segment-verification-clippy.log`. No ratchets, allowances, dependencies,
  hooks, remotes, golden references or tolerances changed. The draft message has
  one section in `target/COMMIT_MSG.txt`. No staging, commits, push or hardware
  access. Task stays active for renewed cross-vendor unit-tier review and the
  coordinator's full/image gate and required unit smoke. Final Status/message-only
  fast-gate repeat is retained in `target/segment-verification-post-status-fast.log`.

- 2026-11-16: SINGLE/segment verification at `b53aee1`, owner `codex-d`, branch
  `t0003-m2`, unit tier under the owner's assignment. Items 1, 2, 4 and 5 are
  owner-confirmed. The two P2s reproduce before fixes: STOP browse becomes Idle,
  hardware reconfigure leaves UI times, VXI-11 returns error 15 and MCP returns
  `ok` for an empty query. All four regressions fail (exit 101) in
  `target/segment-verification-before.log`. Focused post-fix debug passes 24
  tests with default threads (exit 0), including existing active-recording and
  DMA-boundary cases. Final debug/release, exact clippy and fast-gate evidence pending.
  Bode empty-query fixtures explicitly clear the default generator address.

- 2026-11-16: arm-count interpretation for unit smoke: a worker that sees the new
  pair before the new UI run number arms once with the old number and rearms when
  the new number arrives. Two `seg_start` calls per boundary in that ordering are
  expected, so unit smoke arm-count deltas must allow both. The host worker
  regression now exercises this order in Recording and Reading for CLEAR and
  mask-source changes. Owner reports STOP browsing across these identity-only
  changes passed on the unit with M2.3b and confirms it as intended behaviour.
  This verification round uses no hardware; that prior unit observation is
  owner-supplied evidence, not a new smoke result.

- 2026-11-16: SINGLE review-round handoff at unchanged `36c2173` plus unstaged
  changes, owner `codex-d`, branch `t0003-m2`, unit tier. All 14 final check commands
  exit 0 on unchanged hashes of all 13 changed Rust files
  (`target/single-review-final-{checks,source,verification}.json` and referenced logs).
  Default-thread focused library tests pass 302 debug / 301 release, with eight
  existing ignores per profile. All 22 SINGLE-focused cases pass in both profiles,
  including the eight retained cases and 14 review-round additions. Existing
  socket-dependent Bode-generator and compound-query socket fixtures are excluded
  because binding is unavailable; the new SCPI regression exercises the exact
  production TCP message encoder, including empty and compound responses.
  Segment tests run real worker phases and DMA decoding against the HAL engine;
  the scan column test models the hardware counter and uses production boundary,
  restart and sweep-write decisions. Real stream chunk IO is still unit smoke.

- 2026-11-16: supporting final checks pass: core 25 units plus three value tests,
  host script 14 and soak 11 per profile. All five warmed UI census rows report
  256 calls and zero allocations; snapshots reuse zero allocations versus 210 for
  naive cloning, retaining 5,104 bytes each. Five release composed soak modes each
  complete 1,000 steps after warmup 500, with no retained growth, zero failed cycles
  and acquired/used/dropped 13,648/13,618/30. Goldens pass 20 debug / 263 release
  cases, 566 fresh processes and 1,330 exact-repeat captures. Independent decoded
  RGB and metadata match all 665 baseline captures exactly; all 566 stderr logs
  have zero unreviewed masks. No references, manifests or tolerances changed.
  Isolated aarch64 release timing (256 samples): intake p99 63,875 ns, change
  5,416 ns, paired intake/change 64,417 ns against 500,000 ns; shadow 24,250 ns
  separately. ARM check and all 11 exact gate clippy invocations pass with zero
  new/grown findings or compile errors. Fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 22 s`. Complete golden/soak evidence
  and timing are in `target/single-review-final-*`. Status/message-only updates
  are followed by a repeated fast gate in `target/single-review-post-status-fast.log`.
  Added-text/message privacy and whitespace checks pass; `target/COMMIT_MSG.txt`
  has one section. No staging, commits, push, hardware access or changes to ratchets,
  dependencies, allowances, hooks or remotes. Task remains active pending renewed
  cross-vendor unit-tier review, coordinator full/image gate and required unit smoke.

- 2026-11-16: review-round failing-first evidence retained. Original UI transitions
  and the original absent-run sentinel fail both setup-B/command-zero regressions
  (`target/single-review-setup-before.log`, exit 101). The identity-only worker test
  fails Reading versus Recording without a new UI run (`single-review-before-worker-pair.log`,
  exit 101). Hidden average and incomplete scan tests both fail with the old latch
  point and no sweep restart (`single-review-before-average-scan.log`, exit 101).
  The extracted unchanged SCPI response decision returns None instead of a newline
  (`single-review-before-scpi.log`, exit 101). Initial fixture import, ADC handover
  and unavailable-listener failures are not counted as behavioural evidence. The
  final socket-free regression runs the same encoder called by TCP polling.
  An expanded setup test exposed unapplied segment-history invalidation; the restart
  now applies that existing class and records the actual reset rather than suppressing
  diagnostics. A statistics-only control caught an initial full-Identity comparison;
  restart decisions now compare only `(gen, epoch)`. A DMA callback verifies captured
  identities reject old frames/times/browse results while retaining stale-drop counters.
  Initial fast gate failed only `size` (SCPI file 1484 -> 1495); extracting the message
  methods with a declared move passes size without ratchet edits. Initial exact clippy
  compiled all 11 invocations but found one new unused import; it has been removed.
  Final default-thread debug/release and supporting checks are running.

- 2026-11-16: review-round implementation and regression evidence in progress.
  The setup B fake uses CH2+CH3, 50 segments, 200 ns and Normal RUN before enabling
  hardware segments and pressing SINGLE. The UI previously advanced `seg_run` only
  on a RUN edge; enabling segments during RUN could leave command zero (the worker
  treated absent run as zero), and re-pressing SINGLE could reuse old Done status.
  Accepted running pair boundaries now refresh segment status before polling, and
  an absent worker run starts even at command zero. Segment configuration changes
  also apply their existing history invalidation to the restarted hardware recording.
  The SCPI query did not wait on segment state: its empty result was silently omitted
  by the TCP response encoder. Empty queries now produce a terminated response;
  compound replies retain empty members. Socket binding is unavailable in this
  sandbox, so the executable regression uses the exact production message encoder.
  No claim of reproducing the unit's four-minute timing or DMA behaviour is made.

- 2026-11-16: accepted behaviour and pending unit smoke additions (not verified on
  hardware here): STOP after CLEAR in RUN before the first record now holds the
  hardware, matching `acquiring()`; scan RUN honours hold, so STOP stops its stream.
  Check both on setup B and record `records` plus arm/stream-start/stream-stop deltas.
  Enable hardware segments after a setup change during Normal RUN, press SINGLE,
  and verify exactly 50 records followed by STOP without a persistent WAIT. Repeat
  with CLEAR (epoch only), mask-source change (generation only) and hardware
  reconfigure during Recording and Reading. Every replacement recording must contain
  50 segments of one current pair; no old timestamps, browse result or completion may
  leak through. Query `:ACQuire:SEGMents:TIMes?` during Recording (empty line is a
  valid empty list), during partial Reading (current available times), then `*OPC?`
  on the same connection. Each query must answer promptly without waiting for Done.
  Also change generation mid-sweep in scan SINGLE and check a complete current-pair
  sweep without a blank prefix. Retain existing producer/c smoke and integration holds.

- 2026-11-16: claimed the owner-assigned SINGLE review round at clean `36c2173`.
  Plan and reservations above precede implementation. Task remains active; no
  hardware access, commits or publication authorized in this worker.

- 2026-11-16: SINGLE-finding worker handoff at unchanged `fbe6211` plus the unstaged
  diff, owner `codex-d`, branch `t0003-m2`, unit tier. All 14 final check commands
  exit 0 with unchanged hashes of all eight changed Rust files
  (`target/m23c-single-final-{checks,source,verification}.json`, complete referenced logs).
  Focused default-thread library tests pass 274 debug / 273 release, eight existing
  ignores per profile; the existing socket-dependent `bode_sweep_drives_the_generator`
  fixture stays explicitly excluded. All eight new SINGLE tests pass in both profiles:
  late old-pair records before/after publication in shallow, roll, scan and deep memory;
  reconfigure after completion; CLEAR before/after completion; mode-only reconfigure;
  gen-only invalidation without configure; twenty SCPI presses; explicit RUN override.
  The twenty-press case has exactly twenty completed records, forty arm attempts
  (one obsolete and one replacement per press), and no post-completion arm. The fake
  uses the production SINGLE gate, configure/batch lifecycle wrappers, HAL engine and
  publication filters; stream records are synthetic complete snapshots. Real stream
  chunk IO and the on-unit timing race remain for smoke, not claimed as host coverage.

- 2026-11-16: final supporting checks pass on those same sources: core 25 units plus
  three value tests, host script 14 and soak 11 per profile. Five warmed UI census rows
  each report 256 calls and zero allocations; snapshots reuse zero allocations versus
  210 naive-clone allocations, retaining 5,104 bytes each. Release raw/codes/pairs/
  deep-pairs/average each complete 1,000 steps after warmup 500, with no retained growth,
  zero failed cycles and acquired/used/dropped 13,648/13,618/30. Debug retains the
  existing release-only composed-soak policy. Goldens pass 20 debug / 263 release cases,
  566 fresh processes and 1,330 repeat captures. Independent decoded RGB and settings
  metadata exactly match all 665 baseline captures; all 566 stderr logs have zero
  unreviewed event/field masks. References, manifests and tolerances stay unchanged
  (`target/m23c-single-final-{soak,golden}-evidence.{json,log}` where applicable).
  ARM check passes. Isolated release timing, 256 samples: intake p99 70,667 ns, change
  4,125 ns, paired intake/change 71,125 ns against 500,000 ns; shadow 34,542 ns separately.
  All 11 exact `xtask/src/clippy.rs` invocations pass with zero new/grown findings or
  compile errors (host hal/dsp/core/ui/fpgad/xtask; ARM for the first five).
  Fast gate exits 0: `partial gate passed (not done): 8 steps in 23 s`.
  Repeat fast-gate output after Status/message updates is retained in
  `target/m23c-single-post-status-fast.log`. Whitespace and added-text/message privacy
  checks pass. `target/COMMIT_MSG.txt` has one section. No staging, commits, merge,
  push, hardware access or ratchet/allowance/dependency/hook/remote changes. Task 0003
  remains active: renewed cross-vendor unit-tier review, coordinator full/image gate
  and required producer/c unit smoke still hold integration.

- 2026-11-16: failing-first and correction evidence retained. With the parent worker
  gate and stream poll, the new pre-fix selection exits 101, two failed / one passed:
  incomplete reconfigure stalls and completed scan can restart
  (`target/m23c-single-before-final.log`). The separate completed-CLEAR test exits 101
  before the hold fix (`target/m23c-single-clear-before.log`); mode-only observation
  exits 101 with one unreviewed acquisition disagreement before adding slow mode to
  the independent key (`target/m23c-single-mode-before.log`). Initial fixture compilation
  had two API mistakes, corrected before the rebuilt failing run. The first broader
  debug/release selections each exit 101 only on the new RUN-override fixture, which
  directly called the key handler instead of external-command intake; corrected to use
  `cmd::run`, with the accepted run priority unchanged. Initial exact clippy exits 1
  for one `items_after_test_module` finding on the test-only re-export; declarations
  now precede the test modules. All final commands above pass without suppressions.

- 2026-11-16: SINGLE finding implementation, host verified by the final handoff above. The worker's
  returned-batch latch now uses `(gen, epoch)` rather than epoch alone. This revises
  M2.3b P3-1's premise: a returned old-pair batch is not UI completion. A current-pair
  replacement is eligible only while acquisition is active; M2.3c completion remains
  an outbox STOP applied before new publication. Worker eligibility covers stale drops
  before UI publication as well as queue trimming, unlike a UI-drop-triggered re-arm.
  CLEAR before completion retries without configuring hardware. CLEAR after completion
  keeps STOP held through the existing `cleared` flag; the parent regression reproduced
  an extra arm when the last record disappeared. The stream poll now honors STOP/rearm
  for scan as well as roll; the parent completed-scan regression otherwise restarted
  after reconfigure. A mode-only test also exposed the independent legacy key omitting
  slow mode; that key now observes mode while retaining the sweep-companion exception.
  No added UI completion state, queue or hardware access.

- 2026-11-16: add to M2.3c's pending unit smoke (not verified on the unit here): on
  setup B shallow, batch route, repeat STOP -> `:SINGle`, then `:TIMebase:SCALe`
  about 30 ms later, x20. Record before/after `(gen, epoch)`, `records`, `acquired`,
  configured/error/stale counters and `acq_single_arm`. Every press must complete
  exactly one record of the new pair and reach STOP, with no stuck Wait beyond 2 s.
  A captured obsolete batch needs one replacement arm: `acq_single_arm` +2 when
  the old arm already occurred, versus +1 when the new pair was applied before any
  arm. One accepted completed record per press is mandatory; no further arm after
  completion. Follow with reconfigure after accepted record (including the one-frame
  pending STOP window): records and arms must stay unchanged. Check CLEAR before
  acceptance (new epoch, one completed record, no configure) and after completion
  (remain STOP, no arm). Repeat reconfigure-before/after acceptance in roll, scan
  and deep memory; allow the full sweep/deep-read duration when checking latency.
  For roll/scan `acq_single_arm` stays zero: use `acq_roll_start`/`acq_roll_stop`
  for stream starts/stops rather than equating a stream snapshot with a batch arm.
  Include scan-to-roll mode change and explicit RUN/SINGLE after completion to prove
  hold releases. Keep the existing producer and c unit-smoke/integration holds.

- 2026-11-16: claimed the owner-assigned M2.3c SINGLE reconfigure regression at clean
  `fbe6211`, owner `codex-d`, branch `t0003-m2`, unit tier. Task 0061 is already
  merged. Plan and exact reservations above precede code. No device access or commit.

- 2026-11-16: round-1 handoff at unchanged `3882594` plus the unstaged diff;
  all 13 final check commands exit 0 with unchanged hashes of all 11 changed Rust
  files (`target/m23c-r1-{checks,source,verification}.json` and complete referenced logs).
  Default-thread focused library tests pass 262 debug / 261 release, eight existing
  ignores per profile; the entry-assertion test is debug-only. The existing socket-dependent
  `bode_sweep_drives_the_generator` fixture remains explicitly excluded. Core passes
  25 units plus three value tests, host-script 14, soak 11 per profile. Five warmed UI
  census rows each report 256 calls and zero allocations in both profiles; snapshots
  reuse zero allocations versus 210 naive-clone allocations, retaining 5,104 bytes each.
  Release raw/codes/pairs/deep-pairs/average each complete 1,000 steps, warmup 500,
  with no retained growth or failed cycles and acquired/used/dropped exactly
  13,648/13,618/30. Debug retains its existing release-only composed-soak policy.
  Complete retained soak evidence: `target/m23c-r1-soak-evidence.json`.

- 2026-11-16: round-1 picture and performance checks pass on that same code:
  debug goldens 20 cases / 40 processes / 88 repeat captures, release 263 / 526 / 1,242.
  Independent decoded RGB and settings metadata exactly match all 665 baseline captures;
  all 566 stderr logs have zero unreviewed oracle event/field masks. No references,
  manifests, tolerances or exceptions changed (`target/m23c-r1-golden-evidence.{json,log}`).
  ARM check passes. Isolated release timing passes with 256 aarch64 samples: intake p99
  73,666 ns, change 5,001 ns, paired intake/change 73,958 ns against 500,000 ns;
  shadow p99 24,167 ns separately (`target/m23c-r1-release-timing.log`). All 11 exact
  invocations from `xtask/src/clippy.rs` exit 0, zero new/grown findings or compile errors:
  host for hal/dsp/core/ui/fpgad/xtask, ARM for the first five, using
  `cargo clippy --locked --all-targets --message-format=json --quiet` plus
  `--target aarch64-unknown-linux-musl` for ARM (`target/m23c-r1-clippy-*`).
  Fast gate exits 0 and prints `partial gate passed (not done): 8 steps in 22 s`
  (`target/m23c-r1-fast.log`); the early fast check printed the same passing result.
  Whitespace and added-text/message privacy checks pass. `target/COMMIT_MSG.txt` has one
  draft-message section. No staging, commits, push, full gate, image/coverage acceptance,
  hardware access, ratchet/allowance/dependency/hook/remote changes. Task remains active;
  cross-vendor unit-tier verification, coordinator integration gate and the recorded
  producer/c smoke remain required. Integration stays held; unit findings may require changes.

- 2026-11-16: round-1 failing-first evidence on `3882594` plus regression tests:
  terminal autoset repopulates statistics after its clear, NORM plus RUN drops ACQ|STATS,
  and rejected segment completion loses its times/STOP (`target/m23c-r1-ui-before-rebuilt.log`,
  exit 101, all three fail). The core NORM/RUN classification also fails, returning
  HW_AVERAGE alone (`target/m23c-r1-core-before.log`, exit 101). The initial UI fixture
  had a reversed measurement tuple/name and did not compile; corrected before reproduction
  (`target/m23c-r1-ui-before.log`). All four behavior regressions pass after the fixes.
  The first broader run found four autoset fixtures coupled to begin-step polling
  (`target/m23c-r1-focused-initial.log`, exit 101); they now execute the relocated writer
  or full step. Default-thread debug selection then passes 262 tests, eight existing ignores.
  The independent legacy want observation also distinguishes real sweep edits from
  Single-to-Auto companions using its run/sweep values, avoiding a false oracle mismatch
  for NORM/RUN. This is within the existing `ui/src/ui/change/` reservation.

- 2026-11-16: M2.3c review round 1 claimed at clean `3882594`, owner `codex-d` on
  `t0003-m2`; no P1. P2-1: align terminal autoset marks with deferred settings by
  running autoset after records and measurement folding. P3-1/P3-2: assert accepted
  settings at writer entry and skip idle autoset/Bode. P3-3: keep segment completion
  polling and STOP final-read requests during rejected steps; accepted boundary effects
  remain gated. P3-4: exempt only the Single-to-Auto sweep companion of RUN/SINGLE.
  P3-5 accepted: re-pressing SINGLE while armed restarts hardware averaging;
  RUN/STOP twice in one frame is a RUN boundary. P3-6 accepted: completion leaves
  one frame of RUN status before next-step STOP, under D5.

- 2026-11-16: reviewer M2.3c unit smoke remains pending, not verified on hardware:
  SINGLE in roll, scan and deep memory with a `:SING` + `:TRIG:STAT?` loop, checking
  one record and STOP latency; RUN within one frame of SINGLE completion; hardware
  segment completion then immediate RUN, checking `seg::start` and `seg_run`;
  hardware and software mask stop; hardware autoset with infinite persistence and
  statistics on; stepped Bode restoring tscale and generator; free-sweep Bode with a
  real generator; on-unit intake/change p99 idle and with autoset/Bode active.
  No device access is authorized in this worker; the existing producer-smoke and
  integration holds remain.

- 2026-11-16: final M2.3c handoff after the rejected-preparation correction, at unchanged
  `adb72ff` plus the unstaged diff. All 13 final check commands exit 0 on unchanged hashes
  of the 16 changed Rust files (`target/m23c-handoff-verification.{json,log}` and
  `target/m23c-handoff-source.json`). Focused default-thread library tests pass 256 per
  profile, eight existing ignores and the explicit socket-dependent Bode generator fixture
  exclusion; core passes 24 + 3, host-script 14 and soak 11 per profile. The strengthened
  rejected-resolution regression passes in both profiles. Five census rows each have 256
  calls and zero allocations; snapshots reuse zero allocations versus 210 naive allocations,
  retaining 5,104 bytes each. Five release soak modes complete 1,000 steps, warmup 500,
  with no growth/failed cycles and exact 13,648/13,618/30 acquired/used/dropped counts.
  Complete commands, environments, exits and logs: `target/m23c-handoff-checks.json`;
  retained-growth details: `target/m23c-handoff-soak-evidence.json`.

- 2026-11-16: final c picture/performance evidence after that correction: unchanged debug
  goldens pass 20 cases / 40 processes / 88 repeat captures; release passes 263 / 526 /
  1,242. All 665 baseline captures match exactly in decoded RGB and settings metadata;
  all 566 stderr logs have zero unreviewed event/field masks. No reference, manifest,
  tolerance or per-case exception changed (`target/m23c-handoff-golden-evidence.{json,log}`).
  ARM check passes. Isolated release timing has 256 aarch64 samples: intake p99 77,458 ns,
  change 8,833 ns, paired 81,792 ns against 500,000 ns; shadow p99 19,168 ns separately
  (`target/m23c-handoff-release-timing.log`). All 11 exact xtask clippy invocations pass
  with zero new/grown findings (`target/m23c-handoff-clippy-*`). Fast gate exits 0 and
  prints `partial gate passed (not done): 8 steps in 22 s`
  (`target/m23c-handoff-fast.log`). Earlier failures and their corrections remain below.
  Added-text privacy/whitespace checks pass and `target/COMMIT_MSG.txt` has one section.
  Task remains active. No staging, commits, push, full gate, image/coverage acceptance,
  hardware access, ratchet/allowance/dependency/hook/remote changes. Independent cross-vendor
  unit-tier review, coordinator integration gate and the parallel M2.3b producer smoke
  remain required; c integration stays held and smoke findings may require changes.

- 2026-11-16: final inspection found a rejected-preparation regression: unconditional
  internal capture could requeue unaccepted external fields even when auto_step did no work.
  The strengthened existing epoch regression fails before the correction (requested scale
  17 becomes 16, exit 101, `target/m23c-final-rejection-before.log`). A one-byte accepted-
  preparation flag now gates all internal captures and is cleared on quarantine. External
  edits stay at the intake edge after rejection. The step loop and last publication remain
  active. Final verification is rerun under the `m23c-handoff-*` log prefix; the preceding
  source-bound results remain recorded for the previous candidate.

- 2026-11-16: M2.3c worker handoff at unchanged `adb72ff` plus the unstaged diff,
  unit tier, still active. Final focused library tests pass 256 per profile with eight
  existing ignores and the explicit socket-dependent `bode_sweep_drives_the_generator`
  fixture exclusion; no thread-count override. Core tests pass 24 units plus three value
  tests per profile. Host-script passes 14 and soak passes 11 per profile. Five warmed
  census rows each report 256 calls and zero allocations in both profiles; snapshot reuse
  allocates zero versus 210 naive-clone allocations and retains 5,104 bytes per snapshot.
  Release raw/codes/pairs/deep-pairs/average each complete 1,000 steps with warmup 500,
  no growth or failed cycles, and acquired/used/dropped exactly 13,648/13,618/30.
  Debug preserves the existing release-only composed-soak policy. Commands, exits, source
  hashes and limits: `target/m23c-final-verification.{json,log}`; raw check records:
  `target/m23c-final-checks.json`, `target/m23c-final-source.json` and referenced full logs.
  The first final core-debug call reused the parent reproduction's generated test executable
  and failed (exit 101, `target/m23c-final-core-debug.log`). Removing only that exact owned
  artifact and rebuilding the unchanged current sources passes all 27 tests, exit 0
  (`target/m23c-final-core-debug-rebuilt.log`). Both outcomes remain in the evidence.

- 2026-11-16: final unchanged goldens pass: debug 20 cases / 40 processes / 88 repeat
  captures and release 263 / 526 / 1,242. Independent decoded RGB and settings metadata
  comparisons exactly match all 44 debug and 621 release baseline captures. All 566 stderr
  logs have zero unreviewed oracle event/field masks. References, manifests, tolerances and
  per-case exceptions are unchanged. Evidence: `target/m23c-final-golden-evidence.{json,log}`.
  ARM check passes. Isolated release timing passes with 256 aarch64 samples: intake p99
  58,501 ns, change 15,583 ns, paired intake/change 61,542 ns against 500,000 ns;
  shadow p99 18,876 ns separately (`target/m23c-final-release-timing.log`).

- 2026-11-16: final gate-equivalent clippy passes all 11 exact invocations from
  `xtask/src/clippy.rs`: host for hal/dsp/core/ui/fpgad/xtask and ARM for the first five.
  Per crate: `cargo clippy --locked --all-targets --message-format=json --quiet`,
  additionally `--target aarch64-unknown-linux-musl` for ARM. Deduplicated manifest-specific
  counts, with existing planned moves applied, show zero new/grown findings or compile
  errors. No ratchet changed. Complete JSONL/stderr logs, exits and comparison:
  `target/m23c-final-clippy-*`, `target/m23c-final-clippy-summary.txt`.
  Fast gate exits 0 and prints `partial gate passed (not done): 8 steps in 22 s`
  (`target/m23c-final-fast.log`). Initial failures remain recorded above. The final code
  hashes stayed unchanged throughout verification. `target/COMMIT_MSG.txt` contains one
  draft-message section. No staging, commits, push, full gate, image/coverage acceptance,
  dependency/ratchet/hook/remote changes or hardware access. Cross-vendor unit-tier review,
  the coordinator's full gate and the parallel M2.3b producer smoke still hold integration;
  smoke findings may require changes to c.

- 2026-11-16: the request-boundary regression also fails against unmodified `adb72ff`
  core/dsp sources with only the new test added in a scratch two-member workspace:
  `Classes(73)` rather than `Classes(64)`, exit 101 (`target/m23c-parent-sweep.{log,json}`).
  It passes on c. The scripted producer initially fed one extra batch after preparation
  consumed pending STOP (`target/m23c-host_script-debug.log`, exit 101); the final helper
  rechecks accepted run state before feeding, and the existing host SINGLE regression
  passes (`target/m23c-host-single.log`). Initial clippy found one collapsible condition
  and a legacy segment test grown past 60 lines; both are fixed without ratchet changes.

- 2026-11-16: c implementation queues SINGLE, software/hardware segment and both mask
  STOP writers. The bounded pending-completion flag prevents additional records and survives
  another proposal's quarantine; intake applies it before external inputs. Terminal autoset
  and Bode writes reuse the stage outbox and accepted snapshots. Free Bode consumes its
  retained record after deferred STOP, then queues restoration. Run/SINGLE averaging and
  segmented-run history now have one accepted-boundary consumer before publication.
  The narrow core reservation covers a request's sweep companion when external RUN
  overrides pending STOP without changing accepted run values; other effective fields
  and derived acquisition policies still classify normally. Deferred command application
  supplies an actual Stage oracle event under D5. D1-D9 and the a-d normalization bridge
  are unchanged. An oversized rejected payload releases its live capacity on restoration;
  otherwise it would permanently prevent following preparations. No extra snapshot is retained.

- 2026-11-16: the three initial parent-compatible c regressions compile and fail on
  `adb72ff` plus only their test wiring: SINGLE, software segments and software mask each
  write STOP immediately instead of waiting for the next change point (exit 101,
  `target/m23c-before.log`). The expanded focused regressions pass after the implementation,
  including roll with a late snapshot, partial SINGLE, external override, terminal commands,
  rejected payload recovery, and retained free-sweep analysis (`target/m23c-outbox-extra5.log`).
  Initial focused runs exposed six old same-step test expectations; the final fixtures
  assert the pending state and next accepted transition, preserving their original outcomes.

- 2026-11-16: M2.3c claimed at clean `adb72ff` on `t0003-m2`, owner `codex-d`.
  Read the c switch, bridge rules, D1-D9 and the b review-round decisions. The owner
  authorizes implementation in parallel with the b producer smoke; integration stays held.
  No hardware access is authorized for this worker.

- 2026-11-16: M2.3b clippy follow-up validation on the same unstaged code passes.
  `cargo test --locked -p oos-ui --bin oos-ui FILTER`, with filters
  `board::dho::`, `drawpipe::` and `ui::tests::change::`, and again with
  `--release`, exits 0 for all six runs. Each profile reports DHO 55 passed / 1
  ignored, drawpipe 17 passed / 2 ignored and change 11 passed / 0 ignored;
  zero failures. Existing tests retain all boundary, restart and snapshot assertions.
  Logs: `target/m23b-tests-{dho,drawpipe,change}-{debug,release}.log`.
  `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 22 s`; all selected steps pass,
  none fail. Log: `target/m23b-fast.log`, with complete command logs under
  `target/check-logs/16968-1794799404520833000/`.
  Clippy, full tests, perf, coverage and image steps are not selected by the
  fast gate; separate clippy/focused evidence is recorded here. Full integration
  gate and independent review remain with the coordinator. One section in
  `target/COMMIT_MSG.txt`; no commits, ratchet changes or hardware access.

- 2026-11-16: M2.3b clippy follow-up on `t0003-m2`, owner `codex-d`, base
  `2f492ad` plus unstaged changes. All 11 gate-equivalent clippy invocations
  from `xtask/src/clippy.rs` exit 0: host for hal/dsp/core/ui/fpgad/xtask and
  ARM for hal/dsp/core/ui/fpgad, with the manifest lint sets unchanged.
  Deduplicated counts compared against the existing ratchets with planned
  moves applied show zero new/grown entries. All four named files have zero
  findings, including the merged change-test fix from `d977abc`.
  Commands: per crate, `cargo clippy --locked --all-targets --message-format=json
  --quiet`, additionally `--target aarch64-unknown-linux-musl` for ARM.
  Logs: `target/m23b-clippy-*.jsonl`, `target/m23b-clippy-*.stderr`,
  `target/m23b-clippy-exits.json`, `target/m23b-clippy-summary.txt`.
  Focused debug/release tests and fast-gate results are recorded above.
  No ratchet edits, commits or hardware access.

- 2026-11-16: final M2.3b verification-round fixes are ready for independent review at
  unchanged `d4f6b92` plus the unstaged diff; task remains active. Five final default-thread
  debug library runs and five release library runs each report 674 passed, 21 failed,
  31 ignored, exit 101. All eight relevant regressions pass in every run: roll/scan
  boundary samples, captured STOP replay identity, nested/cross-thread counter isolation,
  batch identities, publication/queue-pop loss sites, roll STOP/rearm accounting,
  worker identity/overflow accounting and the legacy partial-merge pacing fixture.
  The same 21 socket-dependent fixtures fail in all ten runs because this sandbox denies
  socket binding, including the existing Bode generator fixture. No other failure remains
  in these final runs. The requested all-green unfiltered suites are therefore not claimed;
  rerun them in a socket-enabled environment. No `--test-threads` flag or suite-wide lock
  was used; RUST_TEST_THREADS was removed from the command environment.
  Complete commands, exits, failure names and source hashes: `target/m23b-r2-verification.json`;
  summary: `target/m23b-r2-verification.log`. Final logs are
  `target/m23b-r2-final-parallel-debug-{1,2,3,4,5}.log` and
  `target/m23b-r2-parallel-release-{1,2,3,4,5}.log`. Preliminary debug runs 3 and 5 exposed
  the separately corrected global pacing fixture race and remain recorded below.

- 2026-11-16: final focused library suites pass 247 per profile with 8 existing ignores
  and the explicit socket-dependent Bode fixture exclusion, default parallelism.
  Host-script passes 14 and soak passes 11 per profile. Each of the five warmed UI census
  rows reports 256 calls, zero allocations in both profiles; reused snapshots allocate zero
  versus 210 naive-clone allocations and retain 5,104 bytes per snapshot.
  Release raw/codes/pairs/deep-pairs/average each complete 1,000 steps, warmup 500, with
  no retained growth, zero failed cycles and later peaks equal to the warmed peak.
  Acquired/used/dropped are exactly 13,648/13,618/30 in every mode, all drops deliberately
  seeded. Debug retains the existing release-only composed-soak policy. Complete results:
  `target/m23b-r2-checks.json`, `target/m23b-r2-soak-evidence.json` and their referenced logs.

- 2026-11-16: unchanged goldens pass: debug 20 cases / 40 fresh processes / 88 repeat
  captures; release 263 / 526 / 1,242. Independent decoded RGB and settings metadata
  comparisons exactly match all 44 debug and 621 release baseline captures. All 566
  child stderr logs have zero unreviewed oracle events or field masks. No reference,
  tolerance, manifest or per-case exception changed. Evidence:
  `target/m23b-r2-golden-evidence.{json,log}`. ARM compilation passes. The isolated release
  timing guard passes, exit 0, 256 aarch64 samples: intake p99 67,084 ns, change 5,875 ns,
  paired intake/change 67,875 ns against 500,000 ns; shadow 22,833 ns separately.
  Logs: `target/m23b-r2-{arm,release-timing}.log`.

- 2026-11-16: final fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 22 s` (`target/m23b-r2-fast.log`).
  The earlier size-only failure and its correction remain below. Added-text and the
  single-message draft privacy checks report zero findings; diff whitespace passes.
  `target/COMMIT_MSG.txt` contains one message section. No staging, commits, push,
  full gate, image/coverage/10,000-step acceptance or hardware access occurred. No ratchet,
  allowance, dependency, remote or hook changed; only the loss-counter move manifest was
  extended. The corrected legacy pacing fixture is byte-identical on `2c0bd5c` and
  `d4f6b92` before isolation (`target/m23b-r2-pacing-provenance.json`). Independent
  cross-vendor unit-tier verification, socket-enabled library runs and the expanded real
  producer hardware smoke remain required; M2.3c remains held for that smoke.

- 2026-11-16: the third complete default-thread debug run exposed an additional
  legacy pacing race in `partial_merges_and_picture_age_are_counted`: stale count 0
  instead of 20. Concurrent UI publication can replace WANTED while its workers classify samples.
  Isolate only this existing pacing fixture in an exact-test child process; the suite
  and the new scoped-counter tests remain parallel. No production pacing behavior changed.
  Original failure: `target/m23b-r2-parallel-debug-3.log`. Repeat final debug runs after
  this test-only correction; retain the preliminary runs as evidence.

- 2026-11-16: M2.3b verification round 2 claimed at `d4f6b92`, owner `codex-d`,
  active on `t0003-m2`, unit tier. All three P2 fixes are present, uncommitted.
  Roll/scan clears all retained analog/LA samples and cursor on pair boundaries without
  changing rolling hardware state; STOP replay retains a Read with the batch identity.
  Four counter-delta tests use scoped counters and their own injected worker pipeline;
  concurrent tests continue normally. A nested/cross-thread isolation test also passes.
  Loss counters moved to `drawpipe/losses.rs` with move entries to keep the file ratchet;
  no ratchet or allowance changed. The first fast gate failed only size (`drawpipe.rs`
  979 to 985); the extracted layout passes, exit 0, printing
  `partial gate passed (not done): 8 steps in 22 s`.
  Logs: `target/m23b-r2-fast-{initial,layout}.log`.
  Focused initial regressions pass 70 with one existing ignore, default parallelism.
  Physical mutations disabling the sample discard and retaining the ambient frame tag
  each fail their new regression, exit 101 (`target/m23b-r2-red-{roll,replay}.log`).
  Hardware smoke now explicitly requires CLEAR and
  STOP-to-RUN in roll and scan plus CLEAR in the final-batch STOP replay window.

- 2026-11-16: unmodified `2c0bd5c` archive, isolated source/build target, reproduces
  the batch identity test failure under the default library runner: before counters
  `(0, 7199, 4)`, after `(27, 7397, 4)`, exit 101. Its full library suite reports
  658 passed, 22 failed, 31 ignored; the other 21 failures are socket permission errors
  from this sandbox, including the existing Bode generator socket fixture.
  The pre-fix `d4f6b92` run reports 669 passed, 23 failed, 31 ignored: the same 21
  socket failures plus both reported counter tests. Evidence:
  `target/m23b-r2-{parent,before}-parallel.log` and `target/m23b-r2-baseline.json`.
  This proves the batch race predates P2-1; it does not claim either suite green.

- 2026-11-16: final M2.3b round 1 worker handoff at unchanged `2c0bd5c` plus unstaged
  review fixes, task still active. Focused debug/release each pass 237, with 8 existing
  ignores and the inherited explicit `bode_sweep_drives_the_generator` sandbox skip.
  Additional diagnostics/hardware-mask/zone suites pass 7 per profile, including every
  error variant. Host-script passes 14 and soak passes 11 per profile. The final exact
  commands, exits and complete logs are `target/m23b-r1-final-checks.json` and
  `target/m23b-r1-final-diagnostics.json`. An earlier fixture incorrectly counted the valid
  first RUN request after initialization failure as an offending request; pinning the prior
  RUN state corrects that regression without suppressing legitimate later requests.

- 2026-11-16: final release soak runs raw/codes/pairs/deep-pairs/average for 1,000 steps each,
  warmup 500: all five report no retained growth, zero failed cycles and later peaks equal
  warmed peaks. Acquired/used/dropped are exactly 13,648/13,618/30 each, all 30 deliberately
  seeded. Debug retains the existing release-only composed-plateau policy. Reused snapshot
  observations allocate zero versus 210 naive-clone calls, retaining 5,104 bytes per snapshot.
  All five warmed UI census rows report 256 calls and zero allocations in both profiles.
  Summaries and artifact locations: `target/m23b-r1-final-soak-evidence.json`.

- 2026-11-16: final goldens pass unchanged: standard debug 20 cases / 40 fresh processes /
  88 exact-repeat captures; full release 263 / 526 / 1,242. Independent decoding verifies
  all 44 debug and 621 release captures exactly match baseline RGB and settings metadata.
  All 566 child stderr logs have zero unreviewed oracle events or field masks. No reference,
  tolerance, manifest or per-case exception changed. Evidence and run locations:
  `target/m23b-r1-final-golden-evidence.{json,log}`. ARM compilation passes. Final release
  timing passes, exit 0, 256 samples on aarch64: intake p99 61,999 ns, change 5,333 ns,
  paired intake/change 63,751 ns against 500,000 ns, shadow 26,418 ns reported separately
  (`target/m23b-r1-final-release-timing.log`). Publication costs remain included.

- 2026-11-16: P3-7 debug timing comparison on the same aarch64 host, 256 samples each.
  The parent is an untouched source archive with its own isolated build target. Both runs
  manually exercise the existing release-only guard in debug and exit 101; neither is a
  debug timing pass, and the 500,000 ns budget is unchanged.

  | Source | Intake p99 ns | Change p99 ns | Paired p99 ns | Shadow p99 ns |
  | --- | ---: | ---: | ---: | ---: |
  | Parent `2279778`, isolated comparison | 3,942,500 | 54,375 | 3,970,875 | 120,333 |
  | Reviewed M2.3b, existing debug observation | 4,008,458 | 47,125 | 4,030,625 | 128,249 |

  Logs: `target/m23b-r1-final-parent-debug-timing.log` and
  `target/m23b-timing-debug-alone.log`. The first parent run also failed at 4,104,833 ns;
  it overlapped evidence inspection, so the table uses the isolated rerun.

- 2026-11-16: final handoff fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 24 s`
  (`target/m23b-r1-final-fast-handoff.log`), all selected steps passing. Added-text and one-section
  draft privacy checks report zero findings (`target/m23b-r1-final-privacy.json`);
  diff whitespace passes. `target/COMMIT_MSG.txt` contains one message section.
  No staging, commits, push, full gate, image/coverage/10,000-step acceptance or hardware
  access occurred. No ratchet, limit, dependency, remote or hook changed. Independent
  cross-vendor unit-tier verification, coordinator integration and the expanded producer
  hardware smoke remain required; M2.3c stays held for that smoke.

- 2026-11-16: round 1 implementation applies all P2/P3 decisions below. The duplicate-token
  regression fails on `2c0bd5c` with two retained requests (exit 101,
  `target/m23b-r1-before.log`); it now quarantines them and recovers on the following step.
  New host regressions cover heap capacity beyond 64 KiB and shadow-init recovery, 20 mask
  creations plus 20 existing-mask enables with atomic publication, read-only zone polls,
  live/scripted poll ordering, synchronous measurement rejection, lifecycle analyzer revision,
  foreign CLEAR/gen recovery and separate loss counters. Bode consumes its two fresh settling
  records and completes the restarted stage within 0.13 s of simulated UI time. Autoset's
  measure-unit request resolves to a fresh epoch after the foreign CLEAR.
  HAL bus simulation counts segment starts and roll stop/start calls at the shared pure
  reconfiguration decision for CLEAR, RUN, SINGLE, autoset/Bode stages, D9 mask_src, fast_ok,
  actual settings, steady state and mode changes. This tests the host policy, not the live
  worker. The unchanged SINGLE completion epoch is exercised across 20 generation changes.
  These tests do not substitute for the expanded hardware smoke.

- 2026-11-16: size checks initially rejected autoset growth (`670 -> 683`, then `670 -> 671`),
  followed by `change.rs` at 411 and prepare_inner at 68. The final layout moves the existing
  scale helper with its move entry and isolates change rejection/initialization in
  `ui/src/ui/change/recovery.rs`; no ratchet, limit, dependency or architecture allowance
  changes. The corrected layout passes the fast gate, exit 0:
  `partial gate passed (not done): 8 steps in 22 s` (`target/m23b-r1-fast-layout.log`).
  Earlier failed gate logs remain `target/m23b-r1-fast-{initial,second}.log` and
  `target/m23b-r1-fast.log`; each prints `gate FAILED in 22 s: size`.
  Final verification is rerunning on this layout and the strengthened mask-enable regression.

- 2026-11-16: M2.3b review round 1 at `2c0bd5c` received with no P1; coordinator
  decisions accepted. P2-1: no D10; retain D4. Only effective want/mode changes tear down
  segments/roll and clear last_read. Epoch-only rejection does not reconfigure hardware.
  P2-2: failed shadow initialization/resolution must keep the step loop running on the
  last publication; drop/quarantine offending requests, bound stderr logs and count every
  error variant. This supersedes the earlier retained-mailbox regression and architecture text.
  P2-3: publish mask_want/zone_want inside acquisition::publish with the same step's pair;
  polls only read results. P2-4: separate worker-push/UI-publication/queue-pop/pipe stale
  identity counters from QUEUE_MAX overflow and lost folds, exposed in :SYSTem:STATe?.
  P3-1/P3-2/P3-3: preserve single_done on gen-only boundaries, notify measurements only
  when measured, and advance the analyzer revision on lifecycle reset. P3-4: lifecycle
  polls precede prepare on both live and scripted paths. P3-6: a foreign CLEAR/gen boundary
  restarts the active autoset/Bode stage with a fresh token and settling state; never broaden
  exact-pair acceptance. Recovery is one queued token and one following resolution step per
  boundary; the first four restarts per requester are logged and every restart is counted.
  P3-7: record a parent `2279778` debug timing next to the existing M2.3b debug observation.
  Reuse the accepted reservation, adding the narrowly required autoset recovery helper and
  host publication regressions under `ui/src/ui/{autoset/recovery,host/tests/publication}.rs`,
  and the helper move manifest. Hardware remains unverified and unreleased for this worker.

- 2026-11-16: P3-5 bridge item for M2.3d: two worker-view writers remain, the accepted
  acquisition::publish view and the legacy post-publication per-step Pipe::set_view seam
  (plus the scripted runner's matching seam). Consolidate them in d while retaining all
  trigger/acquisition dependencies of the worker key until M4. This review does not switch
  the mapping/phosphor consumers or remove the normalize-to-mark_changed bridge.

- 2026-11-16: add the reviewer's nine producer-smoke requirements to the existing M2.3b
  unit smoke under task 0049. These are required hardware evidence, not host acceptance:
  (1) record seg::start and roll stop/start counts for every boundary kind;
  (2) browse STOP segments through CLEAR, mask_src and autoset;
  (3) run each route steadily for 10 minutes with constant gen/epoch/stats_gen and zero stale drops;
  (4) change settings within 50 ms of SINGLE, 20 times, with exactly one hardware arm per press;
  (5) create/enable hardware mask 20 times, with counters starting at the new epoch;
  (6) CLEAR during autoset's unit stage and stepped Bode, recording bounded stall time;
  (7) change timebase in STOP with deep raw retained;
  (8) inspect diagnostic error counts and recall the largest setup;
  (9) inspect stale drops separately at worker push, UI publish, queue pop and pipe.
  Retain the existing requested/applied/output identity, route, lease/build provenance,
  raw/compressed/deep, prearm, valid-record, fold, final-read and unresolved-token checks.

- 2026-11-16: M2.3b worker handoff, uncommitted on the unchanged `2279778`, review tier unit.
  Final focused UI selection passes 221 tests, with 8 existing ignores, in debug and release.
  The existing Bode TCP-listener case is explicitly skipped under the inherited sandbox restriction.
  Core passes 23 unit plus 3 integration tests per profile. Host-script integration passes
  all 14 tests per profile (`target/m23b-host-script-{debug,release}-current.log`). Logs: `target/m23b-core-{debug,release}.log`,
  `target/m23b-focused-debug-current.log`, `target/m23b-focused-release-current.log`.
  M2.3b includes the rejected-resolution regression: duplicate queued tokens retain their mailbox,
  previous pair and settings publication, and consume no records. No failed resolution can publish
  new settings under an old pair. The final late-stage census also executes command capture and
  accepted-snapshot restoration, rather than only the old observation path.

- 2026-11-16: final standard soak passes all 11 tests per profile. Debug retains the existing
  release-only composed-plateau policy; release runs raw, codes, pairs, deep-pairs and average
  for 1,000 steps each, with warmup 500. All five report zero failed cycles and no end-cycle growth;
  each acquires/uses/drops exactly 13,648/13,618/30, with all 30 drops deliberately seeded.
  Later peaks equal warmed peaks in every mode. Both snapshot and hot-path allocation probes pass.
  Snapshots retain 5,104 bytes each; reused updates allocate zero versus 210 calls for naive clones.
  The five UI census rows each run 256 times with zero allocations in both profiles, including
  late-stage command capture and restoration. Logs: `target/m23b-soak-{debug,release}-current.log`,
  `target/m23b-live-stage-census-{debug,release}.log`; summary: `target/m23b-soak-evidence.json`.

- 2026-11-16: unchanged goldens pass: standard debug 20 cases / 40 fresh processes /
  88 exact-repeat captures (102.718 s); full release 263 / 526 / 1,242 (83.375 s), excluding builds.
  Independent decoded RGB hashes match all 44 debug and 621 release baseline captures exactly.
  Every compared capture also retains its baseline settings metadata; zero unreviewed oracle
  lines occur in either profile. There is no changed golden case to justify: no reference image,
  metadata, manifest or tolerance was edited. Logs: `target/m23b-golden-{debug,release}-current.log`;
  per-capture exactness, metadata and oracle evidence: `target/m23b-golden-evidence.json`.

- 2026-11-16: final isolated release timing passes, exit 0: 256 retained samples on aarch64,
  intake p99 72,542 ns, change p99 5,125 ns, combined p99 73,501 ns against 500,000 ns,
  shadow p99 23,875 ns (`target/m23b-timing-release-alone.log`). Publication and retained
  transition/bridge costs remain included in total intake/change accounting.
  An additional debug run of this explicitly release-only guard fails, exit 101: combined p99
  4,030,625 ns, intake 4,008,458 ns, change 47,125 ns, shadow 128,249 ns. The earlier debug
  attempt likewise failed at 3,966,542 ns. Logs: `target/m23b-timing-{debug,debug-alone}.log`.
  These failures remain visible; no debug timing pass or hardware timing acceptance is claimed,
  and neither the 0.5 ms budget nor the test's existing release-only contract was changed.

- 2026-11-16: final fast gate exits 0 and prints
  `partial gate passed (not done): 8 steps in 23 s` (`target/m23b-fast-handoff.log`),
  with no failed selected step. ARM64 check also exits 0 (`target/m23b-arm-final.log`).
  Earlier fast attempts printed `gate FAILED in 22 s: fmt, size` and
  `gate FAILED in 22 s: size`; an intermediate attempt also rejected the relocated IDS static.
  Formatting, narrow helper extraction and the actual function/static move entries fix these
  findings without ratchet, architecture allowance, dependency or limit changes.
  Commands/exits are in `target/m23b-checks.json` and `target/m23b-final-checks.json`;
  final supplemental focused/census logs are listed above. `target/COMMIT_MSG.txt` has one
  section. Added-text and draft-message privacy scans each report zero findings; diff whitespace
  checks pass. No staging, commits, push, full gate or hardware access occurred.
  Task 0003 stays active. Independent cross-vendor unit-tier review, coordinator integration
  and the explicitly listed producer smoke remain required; M2.3c is held for that smoke.

- 2026-11-16: M2.3b implementation is present, uncommitted. The change point publishes
  settings/generator/raw policy/route and one resolved identity; worker view publication precedes
  the Shared handoff. The producer snapshots that publication once and stamps batch/newest,
  queued, raw/final, roll/scan and segmented outputs from that applied pair. Epoch/gen changes
  invalidate prearm and old replay eligibility; an epoch alone does not force configure.
  Both queued record routes and segment display reject different pairs before conversion.
  Measurement/analyzer jobs and results and worker/helper folds gate on gen/epoch/stats_gen.
  Stats-only boundaries retain valid picture contributions and reject old folds, including
  mailbox coalescing in either order. Stage settings use a bounded reusable outbox and the
  existing accepted snapshot, with pending stage commands applied before external intake.
  Only token-associated stage writes switch here; remaining internal/run/reset consumers stay c-e.
  The normalize-to-mark_changed bridge is unchanged and its intermediate regressions remain.
  `Shared::stamp`, acting Model stamp bumps, Source::acquire publication and `meas_n` are gone.
  Compatibility names: Frame::cfg is gen and Frame::stamp is epoch, assigned from one applied
  identity; AutoRun::stamp is only the resolved epoch mirror, zero while unresolved. The flat
  Model::hw_stamp field remains dormant for the exhaustive 194-field contract; the legacy pipe
  stamp comparison and APPLIED-derived test headers are compiled only in test feed wrappers.
  M4 must rerun `ui::tests::epochs::old_pair_records_are_not_measured` through its new conversion
  path; it must not add a second generation-filter implementation.

- 2026-11-16: failing-before evidence on the approved source: the parent-compatible
  `old_pair_records_are_not_measured` compiled and exited 101 at its behavioral assertion:
  records advanced 0 to 1 after an old-generation frame was queued. The log is
  `target/m23b-before-record.log`. Final tests also cover epoch-only rejection, valid current
  records, unresolved/older/different-newer tokens and previous runs, delayed measurement
  results for all three identities, late folds and consistent scripted batch/newest/queue pairs.
  Existing consumer fixtures now explicitly publish/tag freshly acquired frames; the stale
  regressions call Ui::step directly and retain their deliberately old headers. No parent
  failure is claimed for new APIs or for already-passing characterization cases.

- 2026-11-16: claimed M2.3b only at `2279778`, retaining the approved M2.3a prefix
  (`3908fbc`, `b3999bb`, `2279778`) and all D1-D9 decisions. Working tree initially clean.
  The required module-registration seam is `ui/src/board/dho/mod.rs`.
  Also reserve `ui/src/ui/tests/producer.rs` for explicit current-frame identities in
  existing consumer fixtures; stale-frame tests keep calling Ui::step directly.
  Also reserve `ui/src/board/identity.rs`, `ui/src/board/dho/publication.rs`,
  `ui/src/drawpipe/{feed,identity_tests}.rs` and `xtask/data/moves.txt` for narrow publication helpers
  and non-growing legacy extraction; no ratchet or limit changes.
  Also reserve `core/src/change/shadow.rs` for read-only access to the accepted snapshot
  when staging only token-associated settings without a third Model copy.
  Also reserve `ui/src/drawpipe/host.rs`, `ui/src/ui/host.rs`,
  `ui/src/ui/script/{roll,host/soak}.rs`, `ui/src/ui/tests/roll.rs` and
  `ui/src/ui/analysis/{window.rs,window/history.rs,window/tests.rs,window/tests/review.rs,
  window/tests/review/verification.rs}` for existing producer/measurement fixture identity seams.
  The producer smoke holds M2.3c and is not yet verified on hardware. Under task 0049's
  coordinated lease and owner release, check the generation filter on real hardware in
  batch and frames routes, shallow/raw and compressed/deep, with armed-ahead batches and
  queued old frames. Exercise at least 20 boundaries each for CLEAR, STOP-to-RUN,
  repeated SINGLE, autoset retry, Bode stage, acquisition/gen and route changes. Require
  consistent requested/applied/output (gen, epoch, route), exact stale/drop accounting,
  no dropped valid records, no old-pair output after application or old-pair measurement
  or statistics fold, no reconfigure for epoch-only changes, and no acceptance by unresolved
  tokens. Check roll/scan, segmented RUN/SINGLE/STOP, STOP final read, one completion per
  SINGLE arm, hardware/software average restarts and mask-source generation filtering and
  mask behavior. Compare screenshots/statistics, frame/acquisition rate, settling latency,
  step timing and intake/change p99 against the same mode in the 20261115 baseline;
  intake+change p99 must remain <=0.5 ms. Record installed build/lease provenance, requested,
  applied and output identities, route, configure/prearm/disarm counts, acquired/published/
  accepted/dropped counts, errors and latency. Review every performance regression before c,
  without inventing a tolerance. Unexpected producer state stops the round without repair.

- 2026-11-16: D9 implemented and worker checks pass on `b3999bb` plus unstaged changes.
  Observe mask_src independently in the new bounded `mask_source` row (18 rows total),
  without inventing a legacy ACQ publication. Permit only extra new ACQ and MASK at that
  effective source edit; raw disagreements remain counted. The MASK allowance covers
  an isolated edit before any explicit create/reset request. Negative controls reject
  unrelated VIEW, mask requests alone and missing new ACQ/MASK behavior.
  `mask_source_advances_generation_once_and_requests_statistics_resets_as_d9` covers
  RUN/STOP and demo/fake HW: exact ACQ|STATS|MASK, gen/stats_gen advancing once with
  unchanged epoch, existing measurement-statistics clear and two idempotent observations.
  This pins the MASK reset contract; acting mask-count reset and stale-record filtering
  remain for M2.3e/b, rather than activating those consumers in a.
  Before the oracle fix the new regression failed (exit 101, two unreviewed ACQ/MASK
  differences); afterwards focused change/shadow/diagnostics/hwmask suites pass in debug
  and release: 34 passed, one existing explicit timing guard ignored per profile.
  All three `demo/{normal,large,larger}/mask` cases pass in both profiles, twice in fresh
  processes each: six ACQ diagnostics now D9, zero unreviewed event/field masks, and all
  24 decoded RGB captures per profile exactly match unchanged baseline hashes.
  `cargo xtask check --fast` exits 0: `partial gate passed (not done): 8 steps in 22 s`;
  all selected steps pass, no failed step. Commands/exits: `target/m23a-mask-checks.json`;
  regression/focused logs: `target/m23a-mask-{before,focused-debug,focused-release}.log`;
  picture evidence: `target/m23a-mask-evidence.json`; gate: `target/m23a-mask-fast.log`.
  `target/COMMIT_MSG.txt` has one section. No golden references, tolerances, ratchets or
  limits changed. No staging, commits, push, full gate or hardware access. Independent
  cross-vendor review and the coordinator's full integration gate remain pending.

- 2026-11-16: decided by the owner: changing mask_src is an acquisition-input change,
  retaining ACQ|STATS|MASK. In-flight records were mask-tested against the old channel
  and must be dropped at the generation boundary; mask statistics reset. Assign D9,
  the next difference id, to the deliberate new ACQ boundary absent from legacy.
  This resolves the six demo-mask diagnostics reported below; retain their raw counts
  and require zero unreviewed differences. Implement under the existing active claim on
  `t0003-m2` at `b3999bb`; generation/publication/filter and reset activation remain b/e.

- 2026-11-16: review round 1 worker handoff at `3908fbc` plus unstaged fixes, still active.
  Requested verification passes: core 23 unit + 3 integration tests per profile; focused UI
  170 passed, 8 ignored per profile, retaining the previously documented single Bode TCP-listener
  skip. Host-script 14, soak 11 and golden 5 passed per profile (one explicit update test ignored).
  Debug's composed plateau retains its existing skip policy; release runs all five modes for
  1,000 steps, warmup 500, with zero failed cycles, no retained growth and acquired/used/dropped
  13,648/13,618/30 each. Snapshot census: 2,000 observations, zero reused allocations versus
  210 naive-clone calls, 5,104 bytes per snapshot. All five UI census rows: 256 calls and zero
  allocations in both profiles, including the live repair/bridge and D3 evidence capture.
  Timing passes on aarch64 release: 256 samples; intake p99 59,084 ns, legacy change 9,041 ns,
  paired intake/change 63,458 ns against 500,000 ns; shadow p99 22,958 ns reported separately.
  Final fast gate exits 0: `partial gate passed (not done): 8 steps in 22 s`; every selected
  step passed. Initial fast gate exited 1, `gate FAILED in 25 s: size`, reporting
  `ui: file ui/src/ui/analysis.rs grew: 590 -> 595 (ratchet)`. Moving the source snap into
  the existing reset helper fixed that growth; no ratchet or limit was edited.
  Exact commands, environments, exits and full logs: `target/m23a-r1-checks.json`; final
  focused logs have `-final` suffix, and the gate log is `target/m23a-r1-fast-final.log`.
  P3-6's accepted readback is pinned by
  `ui::tests::change::zoom_knob_clamps_once_per_step_and_readback_follows_main_with_zoom_off`.
  The one-section draft remains `target/COMMIT_MSG.txt`. No staging, commit, push, full gate,
  image/coverage/10,000-step acceptance or hardware access. Independent cross-vendor review
  and the coordinator's full integration gate remain pending.

- 2026-11-16: round 1 picture/oracle evidence: debug 20 cases/40 processes/88 repeat captures,
  release 263 cases/526 processes/1,242 repeat captures. All 44 debug and 621 release baseline
  decoded RGB hashes match exactly; no reference, tolerance or per-case picture exception changed.
  Complete artifact locations, hash checks and soak summaries: `target/m23a-r1-evidence.{json,log}`.
  Restoring mask_src ACQ exposes six raw unreviewed ACQ events: `demo/{normal,large,larger}/mask`,
  each in both fresh processes (12 first-field traces carry the associated unreviewed bit).
  Debug has zero unreviewed events, and release has no other unreviewed event class/case.
  The old oracle's Want/raw/worker-route projection misses this independent hardware-mask
  configuration route; demo sources also have no hardware mask to reconfigure. Keep the
  required ACQ|STATS|MASK field contract and these raw diagnostics visible. No new difference
  allowance was invented; coordinator disposition of this oracle gap remains an acceptance limit.

- 2026-11-16: review round 1 implementation: P2-1 reports analyzer resets as MATH|VIEW
  (held FFT averaging is math; waterfall is view), classifies spectrum settings MATH|VIEW
  and eye/power settings VIEW, with no measurement STATS. P2-2 retains the intake source snap
  before analyzer job submission through M2.3e, while normalize still repairs sources when no
  setter caused the repair. Parent-compatible regressions fail on `3908fbc` (exit 101,
  `target/m23a-r1-before-ui.log`) for both the submitted off-source key and false STATS;
  both pass after the fixes. The RUN source-step regression seeds statistics, the picture
  and dirty_map; RUN/STOP cases pin all edited analyzer field contracts.
  P3-3 derives D3 allowances from actual pre/post-normalize field classes using the existing
  bounded scratch, with no new retained snapshot. The negative control leaves unrelated VIEW
  unreviewed beside a zoom repair. P3-5 uses a typed difference id and exhaustive names, preserving
  diagnostic bits. P3-8 now tests D7 in RUN as well as STOP and lists the real normalization tests.
  P3-4 deletes `Model::clamp_offsets`, `fit_platform`/`fit_platform_in` and `UiAcq::mask_reference`;
  all remaining callers were tests and now call normalize, including `core/tests/values.rs`.
  No deletion is deferred to M2.3e. The hardware normalization comment is corrected.
  The keys' compression value is reused by UiAcq instead of a second Board::want call.
  The mask_src extension is reversed: `mask_hw_poll` publishes `MaskWant.ch = mask_src`,
  `hw_merge` installs `hw_mask(w)`, and that sets `HwMask.ch = w.ch`. Thus mask_src retains
  ACQ|STATS|MASK even when the worker route/compression stays equal. This is source inspection,
  not hardware verification, and supersedes the earlier Status rationale removing ACQ.

- 2026-11-16: M2.3a review round 1 received against committed `3908fbc`
  (`Activate normalization with the legacy notification bridge`), no P1. Coordinator decisions
  accepted for implementation: analyzer source/settings edits preserve measurement statistics;
  keep intake source snap until M2.3e; bound D3 by repaired fields; remove unused compatibility
  helpers; use an enum for difference ids; add RUN D7 and real plan test names.
  Accepted behavior changes P3-6: zoom knob values clamp once per step, and zoom delay follows
  main delay with zoom off. P3-7 accepts the duplicate legacy mark after autoset writes.
  These intermediate behavior decisions remain active through the planned consumer cutovers.

- 2026-11-16: final M2.3a worker handoff, active and uncommitted on `t0003-m2` at
  `8f472413a17da5d222dc8dd0faff3d0b6ec5094d`. All eight final command groups exit 0;
  exact commands, exits and complete log paths: `target/m23a-final-checks.json`.
  Core passes 25 tests per profile (22 change/value unit tests plus 3 integration tests).
  Focused UI passes 165 with 8 ignored per profile, using only the explicit skip of the
  sandbox-denied Bode TCP listener recorded below. The initial broader selections in both
  profiles exited 101 for that one existing PermissionDenied failure; their logs remain
  `target/m23a-focused-{debug,release}.log`. No annotations were changed to hide it.
  Host-script passes 14, soak 11 and golden 5 (one explicit update command ignored) per
  profile. Debug's composed plateau is skipped by its existing policy. Release runs all
  five modes for 1,000 steps each, warmup 500: zero failed cycles, no retained growth,
  later peaks equal warmed peaks, and acquired/used/dropped 13,648/13,618/30 in each mode.
  Both allocation censuses pass in both profiles: 2,000 warmed snapshot/normalize observations
  allocate zero times (naive clone control 210 calls; snapshots retain 5,104 bytes each),
  and all five UI rows report 256 calls and zero successful alloc/alloc_zeroed/realloc,
  including effective live normalization repairs and their legacy notification bridge.
  Final release timing guard passes on aarch64: 256 samples, intake p99 100,666 ns,
  legacy change p99 4,042 ns, paired intake/change p99 102,999 ns against 500,000 ns;
  shadow overhead p99 129,458 ns is reported separately (`target/m23a-final-timing.log`).
  Fast gate exits 0: `partial gate passed (not done): 8 steps in 38 s`, with every selected
  step passing and full-gate steps not selected (`target/m23a-final-fast.log`).

- 2026-11-16: final picture/oracle acceptance evidence: release 263 cases, 526 processes,
  1,242 repeat captures; standard debug 20 cases, 40 processes, 88 repeat captures.
  Added debug coverage of eye/mask/math at all three text sizes: 9 cases, 18 processes,
  66 repeat captures (`target/m23a-debug-extra.log` and `.json`). This is supplemental
  coverage, not a claim to the full debug inventory. All 621 release and 77 distinct debug
  baseline RGB hashes match exactly. Every one of the 584 final child stderr logs has zero
  unreviewed oracle events or unreviewed field masks. No reference, capture or tolerance
  changed, so there are no per-case golden exceptions. Independent hash/oracle assertions,
  exact run paths and all five soak summaries are in `target/m23a-final-evidence.{json,log}`;
  standard hash checks also remain in `target/m23a-final-exact-goldens.log`.
  Added-text and draft-message privacy checks each report zero findings; diff whitespace
  checks pass. No ratchet, dependency or limit changed. The draft is one feature message in
  `target/COMMIT_MSG.txt`. No staging, commit, push, full gate, image/coverage/10,000-step
  acceptance or hardware access occurred. The mark_changed bridge stays through M2.3d;
  generation/publication and remaining b-e cutovers are unimplemented. Independent Claude
  unit-tier review and the coordinator's full integration gate remain required.

- 2026-11-16: the first full release inventory passes pictures but exposes six affected
  cases (18 repeated-process unreviewed event traces): mask source still has an unconditional
  acq class in the three mask scripts, and the three newly merged eye scripts change/reset
  analyzer state without an observation (two classes per process). Logs remain in
  `target/m23a-integration-release.log` and its printed run directory. Corrected mask_src to
  stats/mask: it selects data already acquired, not a new hardware channel. Its effective
  mask availability/worker route still comes from fast_ok. Hardware masking can also keep
  that route fast but disable live compression. UiAcq therefore carries the effective
  Board::want compression factor (one u32), with raw UI reads forcing factor 1, and compares
  that value as acquisition input. Tests pin equal raw factors with no generation change
  and compressed-to-raw factors with exactly one generation change, even at equal worker route.
  The existing analyzer reset is observed at ana_adjust's real spec_avg/waterfall clear.
  The seeded test proves those stores clear while generic mstat survives until e. Class
  observation represents analyzer results/temporal state, not a new mstat clear. Same-value
  analyzer edits retain exact D1 extra stats/view deltas. Restoring this producer adds one
  fixed row (17 total), without adding an oracle exemption for missing new resets.
  Independent decoded RGB checks on the first candidate match all 621 release and 44 debug
  current baseline hashes exactly (`target/m23a-initial-exact-goldens.log`); this merged base
  includes task 0052's three eye cases, so the full inventory is 263 cases, not the older 260.
  Final debug/release checks are rerunning after these capture-driven fixes.

- 2026-11-16: recorded M2.3 decisions in the executable oracle: D7 is the acquisition-mode
  reset gap, restricted to new hw_average and STOP phosphor at a real independent legacy
  acquisition/average-key change. D8 documents current math/FFT's stateless recomputation:
  the operation seam observes all eight channels and records the absence of a held-state
  reset, rather than inventing one. Its fixture changes Add to Sub on one retained record
  and verifies changed output with unchanged source samples and exact raw math deltas.
  The math-seam interpretation was offered for clarification; implementation uses this
  explicitly stated stateless contract. These remain unit-tier review inputs.
  Legacy cursor/mask classification and the batch clear were restored individually as
  executable controls, then put back automatically. All three parent-compatible tests fail
  their intended assertions (exit 101, `target/m23a-classification-before.log` and `.json`).
  The current mask fixture also queues a frame before toggling, consumes that record and
  verifies unchanged legacy stamp. Only the effective worker-route difference raises shadow gen.
  Initial broad debug selection passes 163 with 8 ignores, but exits 101 for the existing
  Bode generator TCP listener (`PermissionDenied`/`Operation not permitted`). Rerunning with
  only `--skip ui::tests::bode_sweep_drives_the_generator` exits 0: 163 passed, 8 ignored,
  `target/m23a-focused-final-debug.log`. No test annotation or production transport changed.

- 2026-11-16: M2.3a implementation: live normalization replaces both old per-step clamps
  together. All cross-group zoom callers, the view-slot clamp and task 0052's relocated
  analyzer-source rewrite in `analysis/window.rs::update` are removed. The latter is a narrow
  deletion on the merged task 0052 base, not a worker/DSP change. `normalize` reports a bool
  from effective scalar repairs, without a Model clone; a single bridge calls mark_changed.
  Normalization comparison now checks every settings field at that same pre-stage point,
  excluding only change-owned outputs. Later autoset/Bode writes are no longer mistaken for
  failed normalization. Outbox and all b-e writer/consumer switches remain unchanged.
  DEFAULT now explicitly requests/observes statistics reset; batch measurement removal uses
  stats_clear. Cursor's exact field contract is empty. mask_on is stats/mask only, and mask
  reference presence is removed from acquisition comparison. A software-mask toggle can still
  change fast_ok from workers to records, so only that derived route change adds acq/stats;
  with an already-slow route neither generation nor record acceptance changes in M2.3a.
  The census adds a fifth live-normalize/bridge row, alternating effective offset repairs.
  It retains the same snapshots and bounded mailboxes; the legacy key adds only an acquisition
  tuple and eight MathOp values. Oracle storage remains 16 rows, replacing the retired
  offset/platform/zoom seams with normalize/acquisition/math. Difference masks are now u16,
  with unreviewed in bit 15, to keep additional reviewed ids separate.

- 2026-11-16: regression evidence: the parent-compatible bridge and DEFAULT tests compile
  and fail before UI activation (exit 101, two failed assertions, `target/m23a-before.log`).
  Offset clamp retained old mstat and DEFAULT retained previous-setup statistics. They now
  pass, including RUN/STOP and a second idempotent normalization pass. Cursor temporal-state,
  mask route and math record-recomputation tests are additional independent contracts.
  The offset retained-record preview is characterization of the existing STOP redraw path;
  it does not claim the later running-preview or generation-filter changes.
  Initial fast gate exits 0: `partial gate passed (not done): 8 steps in 24 s`
  (`target/m23a-fast-initial.log`). Final-source checks follow after the census extension.

- 2026-11-16: M2.3a claimed on the clean assigned base under the existing M2 owner.
  Coordinator decisions are decided: DEFAULT invalidates previous-setup statistics (deliberate
  behavior change); cursor mode is display-only and must not reset trend/eye temporal state;
  mask_on itself is not acquisition. Only an effective raw/worker route change may add acq,
  with its publication seam tested separately. Acquisition-mode differences get an explicit
  D id and observed seam; math gets an observed recomputation/reset contract. Batch measurement
  removal goes through stats_clear. Require zero unreviewed oracle differences in debug/release
  captures before M2.3b. Plan and reservations above precede implementation.

- 2026-11-15: final uncommitted integration-repair verification. `cargo xtask check --fast`
  exits 0: `partial gate passed (not done): 8 steps in 22 s`
  (`target/m2-integ-fast.log`). All 11 gate-equivalent clippy invocations exit 0:
  from each of hal/dsp/core/ui/fpgad/xtask, `cargo clippy --locked --all-targets
  --message-format=json --quiet`, plus `--target aarch64-unknown-linux-musl` for the
  five scope crates. No new findings against the move-adjusted ratchets, including
  bridge dead code and Counting Default; existing listed findings remain. Commands,
  exits and raw JSON are in `target/m2-integ-clippy-exits.json` and
  `target/m2-integ-clippy-*.jsonl`; comparison result in `target/m2-integ-clippy-summary.txt`.
  `cargo test --locked -p oos-ui --test host_script --test soak --test golden`, repeated
  with `--release`, exits 0 in both profiles: host_script 14, soak 11, golden 5 with
  the explicit update command ignored. Debug uses the default 20-case smoke inventory
  and skips composed plateau work as designed; release runs the full golden inventory
  and the 1,000-step plateau for each of five modes. References are unchanged.
  Logs: `target/m2-integ-integration-{debug,release}.log`; commands and exits:
  `target/m2-integ-integration-exits.json`. Focused model/shadow/diagnostics tests pass
  24 with one explicit timing guard ignored in each profile (`target/m2-integ-focused-*.log`
  and `target/m2-integ-focused-exits.json`); the three M2 architecture tests pass
  (`target/m2-integ-xtask-core.log`). Broader `cargo test --locked -p oos-ui`, repeated
  with `--release`, exits 101 in both: `610 passed; 21 failed; 30 ignored`. These 21
  failures involve TCP/UDP listeners denied in this sandbox, with explicit
  `PermissionDenied` / `Operation not permitted` errors; they include HTTP, net/net6,
  SCPI socket, SNTP, Bode generator socket and VXI-11. Complete logs and command exits:
  `target/m2-integ-ui-{debug,release}.log`, `target/m2-integ-test-exits.json`.
  Full gate and coverage were deliberately skipped per assignment. No ratchet changes,
  golden updates, staging, commits or hardware access. `target/COMMIT_MSG.txt` contains
  one section. Renewed cross-vendor review and the coordinator's batch gate remain pending.

- 2026-11-15: integration-gate repair on `t0003-m2` at `131af5d`, under the existing
  M2 author claim and the owner's no-commit assignment. Both bounded diagnostic sites
  now use stderr with `oos-ui shadow:`; the existing first-field and row/class masks
  still limit output. `rootfs/initramfs/bin/oos-ui-loop` already appends stdout and
  stderr to the UI log. Host-script and golden runners recognize shadow diagnostics
  alongside pacing messages. Golden references and comparison thresholds are unchanged.
  The original host-script regression was reproduced (exit 101,
  `target/m2-integ-host-before.log`); it now passes, including a child regression that
  repeats RUN and grid-brightness changes and requires one event and one field log on
  stderr while stdout contains only the requested shot response.
  D4 review disposition: retain the reviewed RUN contract in the oracle table above.
  `Request::Run.classes()` declares `HW_AVERAGE` for every source and resolves an epoch;
  legacy `hw_avg_restart` only changes `hw_avg_clear` on hardware. P3's source guard
  corrected the old-effect observation, exposing the intentional extra request class
  on Demo/scripted sources. Such sources have no hardware average consumer to reset;
  no false acquisition reconfiguration follows. The classifier and oracle therefore
  remain unchanged. A new fake-Hardware/Demo test pins equal request classes, epoch 1,
  unchanged acquisition generation, hardware-only clear-counter increments, exact raw
  disagreement counts and zero unreviewed differences. Renewed independent review is
  still required; this disposition does not claim that review has already occurred.
  ARM production bridge dead code is removed: the unused `bus_bit` adapter is deleted,
  and the three test-only adapters are compiled only for tests. The shared allocation
  counter implements Default through its const constructor. Gate-equivalent clippy
  also exposed two stale M2 architecture-test calls after the overhaul merge; both now
  pass the execution context required by the merged APIs. Their three tests pass.

- 2026-11-15: resolved the four M2.2/G2 source conflicts. G2's hot-path test moved from
  `ui/tests/hot_paths.rs` to `ui/tests/snapshots/hot_paths.rs`, included by soak; the perf
  script now selects it from `--test soak`. Safe counter bookkeeping is shared, while the
  only allocator installation and unsafe System adapter remain in `ui/tests/soak.rs`.
  Architecture authorization is restored to that exact file. G2's 101-round best/p99
  guards and host budgets remain, alongside byte-identical M2.2 stage timing and intake
  Drop guard. Golden and host-script executables keep their default allocator.
  Focused checks exit 0: soak 11 in each profile (`target/merge-soak-{debug,release}.log`),
  core change 18 in each (`target/merge-core-{debug,release}.log`), UI shadow/timing/sampler
  21 in each with one explicit timing test ignored (`target/merge-ui-{debug,release}.log`),
  HAL sampler 4 with two timing tests ignored (`target/merge-hal-sampler.log`), and six
  Python budget tests (`target/merge-perf-budgets.log`). Commands: `cargo test --locked`
  with `-p oos-ui --test soak`, `-p oos-core change`, or
  `-p oos-ui --bin oos-ui -- ui::tests::shadow perfguard`, repeated with `--release`;
  HAL uses `--profile perf -p oos-hal --test perf_ratio`; Python uses
  `python3 -m unittest discover -s tools/tests -p test_perf_budgets.py`.
  `sh tools/perf-guard.sh` (the xtask perf step) also exits 0:
  `10 timing budgets and 4 zero-allocation budgets passed`;
  `perf guards: all passed (55 s)` (`target/merge-perf.log`). Its eight UI timing guards
  include M2.2's explicit storm: paired intake/change p99 72,168 ns against 500,000 ns,
  with shadow p99 reported separately at 46,125 ns. All four relocated G2 allocation
  probes emit 64 warmed frames with zero allocations.
  `cargo xtask check --fast` exits 0: `partial gate passed (not done): 8 steps in 23 s`
  (`target/merge-fast.log`). No ratchet edits, staging, commit, merge completion or unit
  access. Renewed cross-vendor review and the coordinator's full batch gate remain required.

- 2026-11-15: coordinator-assigned conflict resolution on `t0003-m2`, merging `overhaul`
  `9dba339` into M2.2 `958a985`. Use the existing claim and the resolution reservations
  above. No staging, commit, merge completion or hardware access is authorized.

- 2026-11-15: M2.2 review-fix verification passed on the final uncommitted source.
  Focused debug and release checks each pass: core 21 (18 change/value unit tests plus 3
  integration tests), UI shadow 16 with the explicit timing guard ignored in ordinary runs,
  timing 2, typed errors 1, mapping 2, soak 10. Commands and exit codes are in
  `target/m22-review-checks.json`; all 12 entries exit 0. The separate 300-step release timing
  storm also passes: intake p99 63,375 ns, legacy change p99 1,959 ns, paired intake+change
  p99 64,583 ns, shadow p99 25,249 ns. The legacy budget excludes prepare/classify, comparison,
  stage proposals and finish observation; rejected observations also retain separate accounting.
  Nested intake is balanced by a Drop guard on errors and unwinds. Shadow clamps use
  non-panicking min/max, and diagnostics count every error variant separately, including payload size.

- 2026-11-15: the new allocation census passes in both profiles: 100 warm-up steps followed
  by 256 actual `Ui::step` calls on fake Shared without producer threads, with 256 explicit
  stage captures. Keys::of (including hw::settings), Keys::changed, normalize/classify, compare,
  finish_step capture/proposals_agree and stage each report zero successful alloc/alloc_zeroed/realloc
  calls. Logs: `target/m22-review-census-{debug,release}.log`. The existing 2,000-observation
  warmed heap-payload snapshot census also passes. These results cover the measured observer
  fixture, not every legacy UI operation or input storm; the README states that limit.

- 2026-11-15: final picture checks pass unchanged: 20 debug cases, 40 processes, 88 repeat
  captures; 260 release cases, 520 processes, 1,230 repeat captures. All 44 debug and 615 release
  decoded RGB hashes exactly match the existing manifest (`target/m22-review-exact-goldens.log`).
  No reference, manifest or held view file changed. `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 23 s`; log `target/m22-review-fast-final.log`.
  The allocator-only patch was reapplied against `0df21cd`, then the exact requested
  `cargo test --locked -p oos-ui --test soak` ran inside its scratch checkout and passed 8 tests
  (exit 0, `target/split/move-test.log`). One earlier scratch invocation exited 101 because its
  target directory was outside that checkout; moving the build output inside it resolved the
  output-policy failure without source changes. Added-text and all draft-message privacy scans
  report zero findings; diff whitespace checks pass. No commit, push, full gate or hardware access.
  Independent review, full integration and the observe-only unit round remain coordinator work.

- 2026-11-15: M2.2 unit-tier review fixes claimed by `codex-d` on `t0003-m2` at
  `114b399`, under the coordinator's assignment. Observe-only holds remain in force; no P1
  was reported. The allocator static and unsafe System adapter now live only in `ui/tests/soak.rs`;
  support contains only safe reusable bookkeeping. The architecture unsafe allowance is scoped
  to that binary. `target/split/move.patch` extracts only this bookkeeping from `0df21cd`.
  It applies independently and the scratch checkout's soak target passes all 8 tests (exit 0).
  `target/split/move.msg` and `target/split/feature.msg` describe the coordinator's required
  base -> move -> feature reconstruction. `target/COMMIT_MSG.txt` is the same single feature message.

- 2026-11-15: M2.3 decisions to resolve before activation, with acting behavior unchanged here:
  DEFAULT setup: shadow classifies `stats`, while legacy keeps mstat. Cursor mode is classified
  `view`, likely an over-classification. `mask_on` is classified `acq`, which would advance gen
  on a mask toggle. Acquisition mode needs a reviewed D id; math op has no observed reset seam.
  Inventory noise: about 144 of 171 cases are batch-harness direct writes in `script/batch.rs`;
  route those through stats_clear or give them their own row before round 0. Outbox comparison
  is circular and detects missing mappings only; the post-view check is wrong in autoset/Bode
  stage steps. These remain review evidence, not accepted differences or new live behavior.
  Analyzer source repair in `ui/src/ui/analysis.rs` is outside the reserved paths and remains held.
  Its missing producer is an explicit gap for M2.3; the unreachable Analyzer arm/row is removed
  until that seam is released. Worker and Identity rows also had no producer and are removed.
  Zoom now emits its row from the actual clamp, with an exact-counter regression.

- 2026-11-15: M2.2 final worker verification on the uncommitted candidate passed all 12 recorded
  checks with exit 0. `cargo xtask check --fast` printed
  `partial gate passed (not done): 8 steps in 21 s` (exit 0). Full logs and commands/exit codes:
  `target/m22-fast-final.log`, `target/m22-checks.json`, and `target/m22-evidence.json`.
  Final M1 release pictures: 260 cases, 520 fresh processes, 1,230 exact-repeat captures;
  default debug pictures: 20 cases, 40 processes, 88 exact-repeat captures. Independent decoded-RGB
  verification matched all 615 release and 44 debug hashes exactly against the unchanged M1 manifest.
  `target/m22-exact-goldens.log` records both final runs. The layout oracle also passed its exact-class
  and zero-disagreement assertions in both profiles. Clippy has no new/grown findings (46 inherited
  core, 307 inherited UI; zero compile errors). Added-text/message privacy scans and diff whitespace
  checks passed. Held paths, M1 references, dependencies and ratchet files are unchanged.
  The final trace inventory retains 18 debug and 171 release cases with unreviewed first disagreements;
  those counts are evidence for review, not accepted behavior or cumulative raw-counter totals.
  Task 0003 stays active. No full-gate, extended-debug/10,000-step/image or hardware acceptance is claimed.

- 2026-11-15: M2.2 implementation is present on `t0003-m2`, uncommitted at `0df21cd`.
  New detector effects stay in shadow state: the live clamps, marks, stamp writers, acquisition publication, record acceptance
  and drawing kernels remain legacy. Exhaustive bindings cover 194 Model fields and nested settings;
  literal fixtures pin classes and the Unrelated list. The detector has 243 fixed field-log slots.
  The active decoder settings carry bus-trigger classes; stored decoder banks remain keyed cache inputs.
  Both previous/scratch snapshots reuse nested capacities, preflight retained growth, and stay below 64 KiB.
  Value outboxes retain warmed payload slots, reject entry/byte overflow, and compare proposals with old writes.
  All ten raw counters, normalized equality, proposal errors and unreviewed differences are visible through
  `stat`/`:SYSTem:STATe?`; first-field traces include oracle rows and difference-id masks on stdout.
  Normalize runs on the copy in the fixed eight-group order. Its full post-clamp comparison excludes only
  change-owned outputs and the four zoom/analyzer fields compared at their corresponding post-view seam.
  The prepared comparison is bounded below 16 KiB. The shadow point precedes legacy auto_step, and
  late stage settings/tokens resolve together at the next point. A same-final-value STOP/RUN intake
  sequence keeps its explicit epoch request; rejected resolutions retain their mailbox.
  Oracle rows retain the initiating STOP state across a later RUN from another input source.
  Every legacy dirty-map and measurement-clear seam records its actual effect and row before comparison.
  The transitional worker key preserves every old dependency; the acting pipe key is unchanged.

- 2026-11-15: host verification: core passed 20 tests in each profile (17 change tests plus 3 value tests).
  The broad focused UI selection passed 101, ignored 8, failed 0 in debug and release; the new explicit timing
  guard is one ignore and was run separately. All 110 xtask tests passed, including the compiled reachable
  change-layer violation fixture and purity probes. The allocation regression passed in both profiles:
  zero successful alloc/alloc_zeroed/realloc calls over 2,000 warmed snapshot/normalize updates plus typed
  proposal capture/replay, including a varying active field set. Both snapshots retained 5,104 bytes.
  A naive clone control made 210 calls for ten clones. The executable naive-per-step control exited 101 at
  the zero-allocation assertion (21,000 versus 0); the mapping-only worker control exited 101 at the real
  new-trigger-level record-path comparison. These are deliberate controls, not parent bug-fix claims.
  Standard release soak passed all five modes, 1,000 steps each, warmup 500: zero failed plateau cycles,
  later peaks equal warm peaks, and exactly acquired/used/dropped 13,648/13,618/30 in every mode.
  No full gate, coverage, image tier, 10,000-step soak or device result is claimed.

- 2026-11-15: the shared M1 storm supplied 300 steps for the intake/change timing guard on an aarch64 host,
  release profile. Its 256 retained paired samples report intake p99 64,542 ns, change p99 27,667 ns,
  combined p99 78,916 ns against 500,000 ns, and shadow p99 26,833 ns. Nested command/SCPI/panel timers
  count intake once; snapshots, normalize/classify, oracle comparison and outbox overhead are included.
  Log: `target/m22-timing-final.log`. Keep this guard through M2.3 and hand its fixed-ring helper to G2.
  Unit timing still belongs to round 0. Seeded oracle fixtures, repeated requests, actual autoset/free-Bode
  paths through a fake Shared producer, and shadow isolation pass. The broader M1 traces also expose
  unreviewed first disagreements: the final release inventory has 171 affected cases: 147 stats, 18 view, 13 hardware-average,
  3 math and 3 acq first row/class traces (cases can contain several classes). These are first-trace counts, not
  cumulative raw-counter totals. `target/m22-unreviewed-traces.json` records the case-family inventory;
  full stdout is retained beside each capture. Do not suppress these or treat them as accepted old-code bugs.
  Coordinator review of the field contracts, exact storm deltas and proposed D1-D6 list remains a hold
  before round 0 or activation. The tested seeded scenarios do not establish acceptance for all input storms.

- 2026-11-15: earlier fast-gate failures were corrected without changing any ratchet or limit.
  The first printed `gate FAILED in 14 s: size, architecture, mutable-statics, unsafe, method-exceptions, known-failures`:
  the shared cause was a forbidden test-module `#[path]`. Default module layout replaced it.
  The next printed `gate FAILED in 21 s: size`: added observation statements grew existing SCPI,
  autoset/Bode, key and constructor units. Small bridge helpers and the explicit command-function move
  removed that growth. The first picture attempt also rejected shadow stderr output; moving diagnostics
  to stdout preserved the harness's unchanged stderr contract. Both deliberate negative controls remain
  recorded separately from these corrected implementation failures.

- 2026-11-15: propose two commits so the pure allocator move stays separate, as AGENTS.md requires.
  Commit 1, `Extract the shared test counting allocator`: `ui/tests/support/counting.rs`,
  `ui/tests/support/mod.rs`, `ui/tests/{golden,host_script}.rs` (support export only), the allocator-only extraction from `ui/tests/soak.rs`, and the relocated test
  unsafe allowance in `xtask/data/architecture.txt`. `target/m22-counting-extraction.patch` passes
  `git apply --check` against an isolated copy of the assigned base files.
  `target/m22-implementation.patch` applies after that prefix; together they reproduce the complete diff.
  Commit 2, `Observe centralized change detection without driving consumers`: `core/src/change/`,
  `core/src/lib.rs`, `core/src/model/{settings,tables}.rs`, `ui/src/{cmd,drawpipe,main,model,perfguard,scpi}.rs`,
  `ui/src/scpi/actions.rs`, `ui/src/ui.rs`, `ui/src/ui/change.rs`, `ui/src/ui/change/`,
  `ui/src/ui/{autoset,bode,hwmask,lifecycle,marks,tools}.rs`, `ui/src/ui/script/host/soak.rs`,
  `ui/src/ui/tests/{mapping,shadow}.rs`, the remaining `ui/tests/soak.rs` changes,
  `ui/tests/snapshots/change_alloc.rs`, `xtask/src/arch/core_tests.rs`, `xtask/data/moves.txt`,
  the new strict change layer in `xtask/data/architecture.txt`, `docs/architecture/README.md`, and this task.
  The single job draft message is `target/COMMIT_MSG.txt`. No commit, push, merge or hardware access occurred.
  Independent Claude unit-tier review, the coordinator's full integration gate and shadow round 0 remain open.

- 2026-11-15: claimed M2.2 only on `t0003-m2` at `0df21cd` under the owner's assignment,
  owner `codex-d`, status active, review tier unit. No commit or hardware access authorized.
  Implement reviewed parts 2-8 in shadow only: exhaustive field contracts, normalize, bounded requests,
  reusable snapshots, independent oracle observations/counters, storm, allocation and timing checks.
  Reserved paths: `core/src/change/`, `core/src/lib.rs`, `ui/src/ui/change/`, `ui/src/ui/change.rs`,
  `ui/src/ui/tests/{shadow,mapping}.rs`, `ui/src/ui.rs` (observation seams only),
  `ui/src/ui/lifecycle.rs`, `ui/src/ui/{autoset,bode}.rs` (observation seams only),
  `ui/src/cmd.rs`, `ui/src/drawpipe.rs` (worker contract tests only), `ui/tests/soak.rs`,
  `ui/tests/support/{mod,counting}.rs`, `ui/src/perfguard.rs`, `xtask/src/arch/core_tests.rs`,
  `xtask/data/architecture.txt` (new strict change layer), `docs/architecture/README.md`, this task,
  and `target/COMMIT_MSG.txt`. Further paths only if strictly needed and recorded before edits.
  The allocator currently lives inline in soak.rs; extract it to the requested support/counting.rs.
  G2 shared quantile infrastructure is not present on this base; extend existing perfguard rather than
  introduce a separate framework. Held paths in tasks 0052/0055/0056 and every M1 reference stay untouched.
  Also reserve `core/src/model/settings.rs` (index the shared ladder with identical results),
  `ui/src/scpi.rs` (observe repeated SINGLE intake), and `core/src/model/tables.rs`: share the legacy time-scale ladder as a constant so shadow
  normalization can index it without allocating, with exact bit-equivalence coverage.
  Reserve `ui/src/scpi/actions.rs` for unchanged RUN/STOP/SINGLE parsing extraction, keeping the
  legacy hot file non-growing while observing repeated SINGLE requests.
  Additional required seams reserved before edits: `ui/src/model.rs` (capability value reexport),
  `ui/src/main.rs` (make existing perfguard timing module available to production observation),
  `ui/src/ui/hwmask.rs` (mask reset observation), `core/src/change/tests/`,
  `ui/tests/support/change_alloc.rs`. The test allocator unsafe allowance moves with its existing code;
  no production unsafe allowance is added. The final two-commit split is recorded above.
  Reserve `ui/src/ui/script/host/soak.rs` to expose an observer callback on the existing M1 storm,
  preserving its schedule, barriers and accounting. Timing samples include the external command intake.
  Reserve `ui/tests/{golden,host_script}.rs` for public common-support module exports needed by the
  shared read-only allocator census API; no fixture, script, assertion or reference changes there.
  Reserve `ui/src/ui/tools.rs` for observation of direct measurement-statistics clears and explicit
  no-diff statistics-reset requests. This is outside the held analysis view paths.
  Reserve `ui/src/ui/marks.rs` for the existing Dots dirty-map observation. All legacy dirty
  setters keep their old effects and order through a shared observation helper.
  Reserve `xtask/data/moves.txt` for the non-growing `cmd::run` to `execute` extraction: its public
  wrapper measures every early-return path. SCPI, panel/touch and the shared storm use nested intake
  timers so combined samples count nested command calls once. No ratchet baseline changes.
  The isolated allocator helper uses the normal module path `ui/tests/snapshots/change_alloc.rs`.
  Worker runs focused debug/release tests and fast gate only; full integration and independent review remain open.

- 2026-11-15: final-source M2.1 worker handoff on `t0003-m2`, uncommitted at the recorded base.
  Final standard debug pictures passed (exit 0): 20 cases, 40 processes, 88 repeat captures,
  101.781 s excluding compilation. All 44 selected decoded RGB hashes exactly match M1. Full release
  still matches all 615 M1 hashes exactly. Logs: `target/m2-golden-debug-final.log`,
  `target/m2-exact-goldens-final.log`; final debug artifacts: `target/golden/debug/run-mb5awwjm/`.
  The earlier complete debug characterization's separate source limit is recorded below.
  Final fast gate exited 0 and printed `partial gate passed (not done): 8 steps in 21 s`;
  all eight selected steps passed, all full/coverage/performance/image steps remain not-selected.
  Complete gate log: `target/m2-fast-final.log` and its printed check-logs directory.
  Final host clippy comparison found zero new/grown entries after moves (`target/m2-clippy-compare.log`);
  core API documentation also built without core warnings (`target/m2-core-doc.log`). Core tests were
  rerun after their lint fixes and passed 3 in each profile. `git diff --check` and the draft-message
  privacy scan passed; the latter printed `privacy: 0 finding(s)`. The exact commands are in
  `target/m2-commands.txt`. Verified every held path and M1 reference remains unchanged.
  Single proposed commit message: `target/COMMIT_MSG.txt`; no commit, push or device access occurred.
  Open: the owner decision on the inherited default-address/privacy conflict; 21 socket tests per
  broad profile require a host that permits local listeners; independent Claude unit-tier review and
  the coordinator's full integration gate remain required. M2.2 and all later M2 parts remain unimplemented.

- 2026-11-15: the full debug M1 characterization also passed (exit 0): 260 cases, 520 processes,
  1,230 repeat captures, 1,852.993 s excluding compilation; artifacts are
  `target/golden/debug/run-40wtznxi/`, log `target/m2-golden-debug.log`. This long run used the adapter
  construction before its final equivalent match-guard cleanup. Do not combine it with final-source
  evidence: the full release, focused debug/release, core, xtask, clippy and ARM64 checks use the current
  adapter. A final-source standard debug picture subset is now running in `target/m2-golden-debug-final.log`.
  The reviewed design text, including the worker key, oracle, allocation/boundary rules and unit rounds,
  is byte-for-byte unchanged. Exact worker commands are retained in `target/m2-commands.txt`.

- 2026-11-15: complete release M1 pictures passed, exit 0: 260 cases, 520 fresh processes, 1,230
  repeat captures, 129.670 s excluding compilation. Independent decoded-RGB SHA-256 verification matched
  all 615 manifest hashes exactly (zero differing baseline images), not just within tolerance.
  Logs: `target/m2-golden-release.log`, `target/m2-exact-goldens.log`; artifacts under
  `target/golden/release/run-nz4zivaz/`. No reference or manifest was changed. Full debug remains running.
  Host value sizes are Model 2,864 B (without its existing heap payloads), Config 112 B, Context 48 B,
  ModuleInfo 120 B and ModuleValues 32 B (`target/m2-layout.log`). Config/Context own no heap; the UI's
  immutable INFO vector has four production entries (480 B payload) for process lifetime. Core introduces
  no global state, queue, cache or per-step snapshot. The bounded snapshot/allocation work stays M2.2.

- 2026-11-15: M2.1 implementation is present, without commits. The 194 flat Model fields retain their
  order and types; a whitespace-independent comparison confirms its default implementation is unchanged.
  Pure settings, defaults, methods and LA record values are in `core/`; rendering, protocol objects and
  board-only generator/hysteresis queries stay in UI. Config and Context contain values only. Seven actual
  core modules have layer rules; the absent change module/rule remains M2.2. Move entries preserve original
  allowances; oversized pure methods were extracted rather than grown. No ratchet baseline was edited.
  No further caller edits were needed; provisional caller-import reservations listed below are released.
  The assignment's held files, acquisition loop and every M1 reference are unchanged.
- 2026-11-15: host checks so far: focused UI tests in debug and release each passed 86, ignored 7,
  failed 0 (model/setup, batch-worker equality, compression/statistics, LA, trigger refinement, CLEAR in STOP,
  segmented completion, hardware average, raw/deep consumer and STOP-final contracts). Core value tests
  passed 3 in debug and release. Boundary/purity tests passed 3, including compiled violating/compliant
  fixtures, actual module reachability and forbidden outside observations. All xtask tests passed 110 after
  updating the member-list fixture. Host core/UI clippy compiled with no new/grown findings after applying
  the move manifest; ARM64 core/UI check passed. Complete logs are `target/m2-*.log` and
  `target/m2-*-clippy-final.jsonl`. Full M1 debug/release pictures are still running.
- 2026-11-15: broad UI debug and release runs each exited 101: 582 passed, 21 failed, 29 ignored.
  The 21 failures are existing socket-based HTTP/net6/net/SCPI/SNTP/Bode/VXI11 tests: local listeners
  are unavailable or report Operation not permitted under this sandbox. Full output is retained in
  `target/m2-ui-{debug,release}-tests.log`; no tests or known-failure annotations were changed to hide them.
  The latest fast gate passed: `partial gate passed (not done): 8 steps in 27 s` (exit 0),
  `target/m2-fast-final.log`. Full integration, coverage/performance/image suites and independent Claude
  unit-tier review remain coordinator work. No hardware was used.
- 2026-11-15: the tree privacy audit exited 1 with 151 findings, including the inherited default address
  moved verbatim from Model::default. The owner has been asked to resolve the direct conflict between
  unchanged defaults and AGENTS.md's no-LAN-literals rule. No exception, scrub bypass or default change
  has been made. The audit's redacted findings are in `target/m2-privacy-tree.log`.

- 2026-11-15: reserved `xtask/src/workspace.rs`'s existing member-list test before updating its
  five-crate fixture for the new registered core member. Its deliberate unlisted member becomes `extra`;
  production workspace parsing is unchanged. The initial full xtask suite had 109 passes and this one
  stale-fixture failure; the M2.1 boundary/purity tests passed.

- 2026-11-15: M2.1 additionally reserves `ui/src/ui/views.rs` to reexport its existing view-slot
  constant from the domain, and caller import seams in `ui/src/{board/mod,board/record,board/dho/hw,
  board/dho/mod,board/dho/scripted,cmd,compress,deepmeas,drawpipe,fastdraw,framestats,http,main,scpi,
  sparse,store,trig,trignear,wave}.rs` only where compilation requires the compatibility traits.
  These are strictly type-move support; the held pacing, roll, perf/allocator, doctor and GPU files stay untouched.
  New `core/clippy.toml` owns the crate-wide purity lint list. No ratchet baselines will be edited.

- 2026-11-15: codex-d (Codex) claimed M2.1 implementation on `t0003-m2` at
  `18ac5c72ba5e268905ccd583af4cb00f91a0ec80` under the owner's assignment. Status active.
  Recorded the implementation plan and narrow reservations before source edits. The working tree was clean.
  M1 goldens, M1b, G1 and gate predecessors are merged per the assignment. Unit-tier cross-vendor review
  and the coordinator's integration gate remain required; this worker makes no commit or hardware claim.

- 2026-11-15: codex-d (Codex) prepared the plan on `t0003-m2`, source base `overhaul`
  `6291e7080f8c91d2702d453052bccb75e0b1faf6`. Applied the coordinator's accepted planning claim from
  `b20591e7b9856db88652a82334dcd6dbc6797a38`; implementation waits for the documented predecessors and claims.
  Read the specifications, ADRs and current call sites; mapped tests, deletions, M1 gates and unit rounds.
  Planning checks passed: `git diff --check`; all 82 cited source locations exist; task structure and commit
  subject checked; privacy scans of the plan text and draft message each reported `privacy: 0 finding(s)`.
  The full gate and behavioral tests were not run for this plan-only assignment; no runtime pass is claimed.
  Plan only: no source change, commit, push or hardware access. Draft single plan message: `target/COMMIT_MSG.txt`.
- 2026-11-15: claimed for codex-d on branch t0003-m2 by the coordinator. Plan first; implementation after G1
  and tasks 0020, 0025, 0027 merge and M1 is complete.
- 2026-11-14: seeded from task 0001. Not yet claimed.
