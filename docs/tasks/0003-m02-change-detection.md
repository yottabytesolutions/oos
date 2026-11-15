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

Reserved paths: `docs/tasks/0003-m02-change-detection.md`, `target/COMMIT_MSG.txt` (planning only).

Implementation follows [migration M2](../architecture/migration.md#m2-one-change-detection-point-d1a),
[change detection](../architecture/README.md#change-detection), and ADRs
[0001](../architecture/adr/0001-single-settings-owner.md),
[0002](../architecture/adr/0002-commands-per-area.md),
[0003](../architecture/adr/0003-one-change-point.md),
[0004](../architecture/adr/0004-explicit-stages.md),
[0011](../architecture/adr/0011-golden-pictures.md) and
[0014](../architecture/adr/0014-domain-crate.md). Source locations below refer to planning base
`6291e7080f8c91d2702d453052bccb75e0b1faf6`; proposed destinations and test names do not exist yet.
Refresh locations and claims after integrating predecessors. This assignment changes no source and makes no commit.
This revision incorporates the coordinator's decisions after Fable 5.2's architecture review (approve with changes).
Those decisions refine M2's acceptance below; do not infer authority to edit the migration specification here.

### Entry conditions and files

The coordinator's accepted planning claim is on `overhaul` at
`b20591e7b9856db88652a82334dcd6dbc6797a38`. Implementation waits for G1/task 0043, then tasks
0020, 0025 and 0027 in their documented serial order, and completed M1 references. M1b must pass before
the shadow unit round. Keep this task's existing dependencies; these additional migration prerequisites still apply.
Create a separate branch per numbered part from its integrated predecessor, retaining task 0003 as the owning record.
The present `t0003-m2` branch is for the plan. The coordinator must accept implementation reservations first.

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
| Analyzer source repair in `ui/analysis.rs:528` | `{stats, view}` for spec_src/eye_src repair, plus the triggering channel diff's classes | Compare after the corresponding repair, not before the old view stage; moving repair before classify is D3. |
| Relayout `dirty_map`, `ui.rs:1631`; dots/Z-input marks `ui.rs:6527,6544` | `{mapping}` for effective dimensions/dots; Z-input additionally `{acq, stats}` when raw needs or measured channel set change | Observe actual dimensions and raw-policy changes; add phosphor/view only where their field fixtures require it. Old broad clears use D1/D2. |
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
  without a new diff. Exact extra-old-clear counts are scenario-specific, not suppressed.
- **D2, STOP mapping:** settings_changed has no STOP dirty mark, while STOP redraw happens separately every step.
  The new mapping class requests preview even in STOP. Compare the stored record and visible trace, not dirty_map
  bit equality, and retain the legacy redraw weight in M2.
- **D3, normalization:** old offset clamps can be unmarked and zoom/analyzer repairs happen at other seams.
  The new point classifies effective repairs together before consumers; compare equivalent post-repair values
  and separately review any changed outcome caused by the fixed normalization order.
- **D4, request boundaries:** CLEAR and STOP-to-RUN gain an epoch even with equal settings; SINGLE and stage
  retries resolve requests rather than equating stamp changes with acq. Several requests may share an epoch.
  Epoch-only boundaries reject prearmed/queued old work without reconfiguration.
- **D5, internal timing:** autoset/Bode and completion settings act on the next step via outbox, with settings
  and token resolved together. Align old proposals with next-step application; unresolved tokens accept nothing
  and pending SINGLE STOP consumes no extra record.
- **D6, stale work:** pair/stats_gen rejection prevents late old records, results or folds from refilling
  current state. Pin deliberately delayed work and its exact drop counts; do not attribute drops to all classes.

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
| 5, M2.3a | Activate normalize before classify, retaining legacy notifications | Both external per-step clamp calls and migrated cross-group rewrites, together. Every effective normalize diff still feeds mark_changed for the acting legacy consumers until M2.3e; test RUN/STOP map/stat behavior. |
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
| Normalize, M2.3a; FB | `core/src/change/tests.rs::{normalize_is_idempotent,missing_channel_is_acq,clamped_offset_is_mapping}`; `ui/src/ui/tests/mapping.rs::clamped_offset_previews_last` | Second normalize changes nothing; 2-channel caps turn unavailable channels off with acq; offset clamp gives mapping/key change and preview. Sweep supported capabilities and cross-group clamp cases. |
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
producer coverage remains M3.0. Do not use scripted injection as evidence of hardware-loop correctness.

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
