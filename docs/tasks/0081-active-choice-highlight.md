---
id: 81
title: Active choice highlighting in current menus
kind: fix
status: active
release: 0.2.0
owner: Codex
depends: []
---

## Goal

Pull forward menus-1 (H1) from `e9f7c30b:docs/spec/menus.md` into the current shared menu renderer.
Committed options retain a filled cue with contrasting text independently of focus, including collapsed and
expanded pickers. Read stable option identities rather than display labels. Review tier: unit, as assigned.

## Plan

1. Inspect shared controls, option domains and the legacy highlight inventory. Add shared identity readers without
   changing menu tables, settings mutation or the spec-first rebuild.
2. Apply the cue to segmented controls, collapsed pickers and option lists. Keep numeric buffers and actions
   unselected, and keep focus separately visible. Preserve the upstream bottom bar and buttons.
3. Add behaviour and rendering regressions for the control kinds, focus changes and colliding display labels.
4. Run focused debug/release tests, the exact clippy gate step and size/fast checks. Re-render scripted pictures,
   justify each changed capture, bless only affected captures and retain six representative PNGs under target/.
5. Prepare a single-section `target/COMMIT_MSG.txt` and hand off for independent cross-vendor review and integration.
   No commits, pushes, merges or hardware access in this worker job.

Reserved paths: `docs/tasks/0081-active-choice-highlight.md`, `ui/src/ui.rs` (menu rendering/control paths and module
wiring only, as assigned alongside the other lane), new `ui/src/ui/active_choice*` modules, menu identity helper
paths strictly required by private state, affected `ui/tests/golden/` captures and worktree-local `target/` evidence.

## Status

- 2026-11-16: Review corrections on uncommitted base `15efe011`: Identify uses `Ctl::Seg(vec![cur], None)` again.
  Its tap regression pins the existing no-change/menu-kept behavior and unselected presentation, with long text
  retaining the segmented font fallback. Pairs source headers retain dark contrasting text on the active fill and
  a channel-coloured edge, pinned by a rendering regression. LA import order is restored; the equivalent `0xff`
  spelling keeps the exposed GROUPS declaration on one line and the legacy file within its ratchet.
- 2026-11-16: Option-order regression covers 813 selection attempts across the expanded menu fixture, including
  Trigger, Decode, Bus, Logger and additional system/catalogue pages. Compare `options()[index()]` with `value()`;
  explicitly normalize the existing `XY`/`Xy` spelling, remote `.local` suffix and explanatory DHCP display text.
  These display exceptions do not duplicate option orders. A negative control swaps ChOn's index mapping and fails
  the round-trip assertion (exit 101, `target/0081-review-order-drift.log`), then restores the mapping.
- 2026-11-16: Complete active-choice before run against the `15efe011` renderer exits 101: 18 tests, 8 failing and
  10 passing (`target/0081-review-base-active-choice.log`). The test-only transplant retains the new identity
  readers and helper visibility but leaves base control, tap and rendering code untouched; reproducible patch:
  `target/0081-review-base-transplant.patch`, explanation: `target/0081-review-base-transplant.txt`.
  Not every test fails on the base: reader-only invariants and already-correct behavior pass, including the new
  round-trip test. The changed-cue regressions fail. Both new tap/colour regressions also fail against the submitted
  pre-review source (exit 101, `target/0081-review-before-fix.log`). The initial literal-value probe found the
  existing display exceptions above (`target/0081-roundtrip-probe.log`); it is not a product regression.
- 2026-11-16: Review check evidence: final exact clippy step exits 0 with zero new findings across all six crates
  on host and applicable ARM targets (`target/0081-review-clippy.log`, detailed command logs under
  `target/check-logs/37218-1794851461444796000/`). The scratch driver matches every repository gate source module;
  only its main dispatch differs. Final `cargo xtask check --fast` exits 0 and prints
  `partial gate passed (not done): 8 steps in 45 s` (`target/0081-review-fast.log`). First fast run exits 1 for
  one-line growth in LA and workspace (`target/0081-review-fast-first.log`), printing
  `gate FAILED in 45 s: size`; both are fixed without ratchet edits.
  Final debug filters pass: active_choice (18), menu (26), picker (13), every_option_selects (1), all exit 0;
  complete logs: `target/0081-review-debug-{active_choice,menu,picker,every_option_selects}.log`.
  Final release filters also pass: active_choice (18), menu (26), picker (13), every_option_selects (1), exit 0 each;
  logs: `target/0081-review-release.log` and `target/0081-review-release-{menu,picker,every_option_selects}.log`.
  Final source fingerprints: `target/0081-review-source-sha256.txt`. `git diff --check` passes and the index remains
  empty on `15efe011`. `target/COMMIT_MSG.txt` retains a plain imperative subject with no Markdown heading.
  Baseline scratch source/build and the superseded test prototype were removed after retaining reproducible test
  transplant and failure evidence. Required logs and active build caches are preserved. No additional golden
  references changed during this review round; Pairs is covered by the new raster regression.
  This evidence supersedes the earlier handoff's source validation. Task remains active for the coordinator's
  review verification and integrated default full gate. All worker check processes have exited. All task source,
  task documentation and the existing 123 PNG/manifest updates intentionally remain uncommitted. No hardware access.

- 2026-11-16: Review round requested by the owner, with no blocking findings. Plan: restore Identify's segmented
  tap/font path without a committed cue; add option-order and Pairs colour regressions; restore LA import order;
  rerun the complete active-choice filter against the base renderer and retain the log, then focused debug/release,
  exact clippy and fast checks. Keep all changes uncommitted; coordinator retains integration and full-gate ownership.
- 2026-11-16: Deferred follow-ups, owned by the coordinator for assignment in the spec-first menu rebuild:
  - `0081-F1`: 36 submenu rows need H1 inline summaries and selected-state cues: menus-105, menus-106,
    menus-109, menus-110, menus-111, menus-112, menus-113, menus-114, menus-115, menus-116, menus-117,
    menus-118, menus-119, menus-120, menus-122, menus-123, menus-124, menus-125, menus-126, menus-127,
    menus-131, menus-135, menus-146, menus-194, menus-208, menus-218, menus-221, menus-222, menus-223,
    menus-284, menus-310, menus-311, menus-312, menus-445, menus-458, menus-520.
  - `0081-F2`: menus-107 SlopeAct needs the inline active slope and selected-state cue, retaining its tap behavior.
  - `0081-F3`: menus-134 LockAct needs the inline current touch state and selected-state cue, retaining its tap behavior.
  - `0081-F4`: replace the pre-existing label lookup in `Ui::select_option` (`ui/src/ui.rs`, option-position lookup)
    with selection by index. NTP preset/custom truncated-label collisions can select the wrong raw server even though
    H1's identity reader highlights the correct current server. Preserve the collision fixture as a regression input.
  These are local follow-up IDs under task 0081, not newly allocated numbered task claims. Specification inventory:
  `e9f7c30b:docs/spec/menus.md`. This renderer patch does not claim these deferred H1 requirements are complete.

- 2026-11-16: Worker handoff ready on unchanged HEAD `15efe011`, with all task changes intentionally uncommitted
  and the index empty. Final post-bless comparison exited 0 and printed
  `PASS: 263 cases, 526 fresh processes, 1242 exact-repeat captures` (release, full inventory, 133.201 s).
  Complete output: `target/0081-golden-handoff.log`; final artifacts:
  `target/0081-final-goldens/golden/release/run-tdonwjif/`. This run uses the final source and release binary.
  Only the 123 justified menu PNGs and their 123 manifest hashes changed; no metadata, closed-menu captures,
  gate files or ratchets changed. `target/COMMIT_MSG.txt` has one section and passes privacy scanning.
  Task stays active for the coordinator: arrange cross-vendor unit review, run the default full gate on the
  integrated candidate, and complete the merge lifecycle. No commits, publication, hardware access or full-gate
  claim in this worker job. All task-owned checks and render processes have exited.

- 2026-11-16: Final source validation: 15 active-choice regressions pass in debug and release (exit 0 each),
  including the retained-reference/LA case that must not select an unrelated analogue source. Commands:
  `cargo test --locked -p oos-ui --bin oos-ui active_choice` and the same with `--release`.
  Logs: `target/0081-active-debug-final.log`, `target/0081-active-release-final.log`.
  The same final Cargo test executables also pass the `menu` (25), `picker` (13) and `every_option_selects` (1)
  filters in both profiles; logs are `target/0081-{menu,picker,options}-{debug,release}-handoff.log`.
- 2026-11-16: Final exact clippy step exited 0 with zero new findings across host and ARM for all six crates
  (`target/0081-clippy-handoff.log`; full logs `target/check-logs/30499-1794850262690649000/`).
  Final `cargo xtask check --fast` exited 0 and printed `partial gate passed (not done): 8 steps in 34 s`
  (`target/0081-fast-handoff.log`, full logs named at its start). All eight selected steps passed; the default
  and image-only steps were not selected. Size limits and ratchets did not grow; no gate, lint or ratchet files
  changed. Hooks remain `.githooks`. `git diff --check` and commit-message privacy scanning pass (0 findings).
- 2026-11-16: Rendered and visually inspected six menus: Channel, Trigger, Math, Spectrum, Windows and Eye.
  Final release PNGs exactly match the inspected images, with readable dark selected text and a separate outer
  focus outline, including focus on numeric rows. Gallery: `target/0081-visual/index.html`; PNGs and script beside
  it. The final release build exited 0 (`target/0081-build-verified.log`); binary and source fingerprints are
  `target/0081-release-binary.sha256` and `target/0081-source-sha256.txt`. Superseded initial PNG runs were removed;
  their reports remain under `target/0081-initial-render-evidence/`. Required failed/passing check logs and build
  caches are retained. No hardware was accessed.

- 2026-11-16: Scoped golden approval for H1: 123 changed PNGs (41 captures at Normal, Large and Larger);
  all 498 remaining captures are pixel-identical. Every change has `menu_open=true`; changed pixels stay between
  y=91 and y=514, leaving the top and bottom bars unchanged. Metadata and capture contracts are identical.
  The twice-rendered 263-case inventory has 621 captures. Expected pre-bless comparison failures are retained in
  `target/0081-golden-final-before-bless.log` and `target/0081-final-goldens/golden/release/run-rx373t0r/`.
  Exact paths, changed-pixel counts and bounding boxes are in `target/0081-changed-captures.json`.
  The following per-capture reasons apply separately to `ui/tests/golden/demo/normal/`, `large/` and `larger/`.
  Only these PNGs and their RGB hashes in `manifest.txt` are approved for replacement; no `.meta` changes.

| Capture beneath each text-size directory | Reason |
|---|---|
| `basic/02-auto.png` | Channel selector cues and separate focus outline in the menu left open by the channel keys. |
| `colorgrade/a0-colorgrade-menu.png` | Display picker values retain their filled active cues beside the selected colour-grade toggle. |
| `decode/70-uart-menu.png` | Committed decoder protocol, baud and source choices retain selection independently of row focus. |
| `decode2/72-i2c-menu.png` | Committed I2C decoder choices use the shared picker fill and focus outline. |
| `math/40-math-sub.png` | Math category, operation and any visible window picker keep their committed fill and separate focus outline. |
| `math/41-fft.png` | Math category, operation and any visible window picker keep their committed fill and separate focus outline. |
| `math/42-math-intg.png` | Math category, operation and any visible window picker keep their committed fill and separate focus outline. |
| `math/43-math-lowpass.png` | Math category, operation and any visible window picker keep their committed fill and separate focus outline. |
| `math/44-fft-flattop.png` | Math category, operation and any visible window picker keep their committed fill and separate focus outline. |
| `menus/10-ch1menu.png` | Channel option pickers keep their active value fill after focus moves. |
| `menus/11-measadd.png` | The catalogue source selectors use the same filled current-source cue. |
| `menus/16-persist.png` | The channel menu left open after setting persistence retains its option cues and separate focus outline. |
| `menus/17-about.png` | System picker cues behind the About overlay remain selected independently of focus. |
| `picker/d0-picker-trigtype.png` | The expanded type list keeps the active type filled and draws focus as an outer outline. |
| `picker/d1-picker-runt-set.png` | The collapsed trigger type shows Runt as a filled committed value. |
| `picker/d2-picker-math.png` | The expanded math option list separates the active option fill from focus. |
| `picker/d3-picker-fft-set.png` | Collapsed math category, operation and window choices remain filled after selecting FFT. |
| `picker/d4-picker-quick.png` | The expanded Quick key option list has a distinct outer focus outline. |
| `picker/d5-picker-quick-set.png` | The collapsed Quick key selector keeps Clear filled after the picker closes. |
| `refs/51-ref.png` | The reference on-button, slot and populated colour selector show their committed states. |
| `refs/52-xy.png` | Display picker values remain filled while the XY format is selected. |
| `startmenu/g0-startmenu.png` | The focused start-menu button gets an outline distinct from its existing running-state fill. |
| `startmenu/g1-startmenu-history.png` | The start-menu focus outline remains separate from existing History and running-state cues. |
| `storage/b0-storage-menu.png` | The stepped setup-slot selector keeps its committed slot filled, including after the storage action. |
| `storage/b1-storage-setup-saved.png` | The stepped setup-slot selector keeps its committed slot filled, including after the storage action. |
| `storage/b2-storage-csv-saved.png` | The stepped setup-slot selector keeps its committed slot filled, including after the storage action. |
| `storage/b5-storage-recalled.png` | The stepped setup-slot selector keeps its committed slot filled, including after the storage action. |
| `triggers/80-trig-edge-menu.png` | The edge trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/81-trig-pulse-menu.png` | The pulse trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/82-trig-slope-menu.png` | The slope trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/83-trig-video-menu.png` | The video trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/84-trig-pattern-menu.png` | The pattern trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/85-trig-duration-menu.png` | The duration trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/86-trig-timeout-menu.png` | The timeout trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/87-trig-runt-menu.png` | The runt trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/88-trig-window-menu.png` | The window trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/89-trig-delay-menu.png` | The delay trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/90-trig-setuphold-menu.png` | The setuphold trigger keeps its committed Type picker filled with a separate focus outline. |
| `triggers/91-trig-nthedge-menu.png` | The nthedge trigger keeps its committed Type picker filled with a separate focus outline. |
| `xy/54-xy.png` | Display picker values remain filled while XY is selected, independently of focus. |
| `zoom/60-zoom.png` | The open horizontal menu keeps committed choices filled with the separate outer focus outline. |


- 2026-11-16: Earlier source checks: `cargo test --locked -p oos-ui --bin oos-ui active_choice` passed
  14 tests (exit 0, `target/0081-debug-verified.log`); the `menu` filter passed 25 tests including all menu layouts
  (exit 0, `target/0081-menu-debug.log`). The exact `steps::clippy` gate implementation passed host and ARM for
  all six crates with zero new findings (exit 0, `target/0081-clippy-verified.log`, complete command logs under
  `target/check-logs/71411-1794849881370441000/`). A scratch copy of xtask under `target/0081-clippy-step/` invokes
  the unmodified step directly; only its main dispatch differs. Repository gate code and ratchets are untouched.
- 2026-11-16: H1 inventory review also covers stepped RefSel, RefColor and StoreSlot despite their legacy Value
  kind, plus populated reference on-buttons. Numeric keypads and one-shot findings remain unselected. Displayed
  analyser cues respect XY precedence. Colliding NTP labels use raw server identity and select exactly one row.
- 2026-11-16: Intermediate checks: second fast run rejected one added line in legacy `la.rs` (566 -> 567);
  removed the extra blank line, retaining its size. First exact clippy run rejected one test over 60 lines;
  split its menu inventory fixture, then reran cleanly. Logs: `target/0081-fast.log`, `target/0081-clippy.log`.

- 2026-11-16: Shared identity readers and selection painter implemented. Existing helper visibility only widened to
  the parent for `bode.rs`, `bus.rs`, `bwtest.rs`, `la.rs`, `remote.rs`, `sim.rs`, `update.rs`; reserved additionally
  `ui/src/ui/meascatalog.rs` and `ui/src/ui/workspace.rs` rendering paths for their compact source selectors.
  `ui.rs` edits stay in module wiring, control presentation, picker initial focus and menu drawing.
- 2026-11-16: Failing-before evidence in `target/0081-before.log`: collapsed picker and suffixed remote-name
  selection fail against the old renderer. Initial fixture also named Display instead of Windows for the density
  row; corrected the fixture. First fast gate exited 1: `#[path]` modules rejected by source discovery in size,
  architecture, mutable-statics, unsafe, method-exceptions and known-failures. Switched new modules to ordinary
  directory layout. No ratchets or rules changed.

- 2026-11-16: Codex accepted the owner's task 0081 assignment on branch `t0081-active-highlight`, base `15efe011`.
  Plan recorded before code. Working tree initially clean. Delivery worker scope is fast gate plus focused checks;
  coordinator owns cross-vendor unit review and final integrated full gate. No hardware access requested or used.
