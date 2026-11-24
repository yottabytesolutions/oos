---
id: 48
title: "M1b: safe start and exit"
kind: feature
status: done
release: 0.2.0
owner: codex-b
depends: [45]
---

# M1b: safe start and exit

## Goal

A scope that cannot loop: restart limits, `--safe`, durable markers, bounded shutdown and kernel panic recovery.

## Plan

Reserved paths: rootfs/initramfs/etc/rcS, rootfs/initramfs/bin/oos-crash-save,
rootfs/initramfs/bin/oos-watchdog, rootfs/initramfs/bin/oos-ui-loop,
rootfs/initramfs/bin/oos-ui-exits, rootfs/board/dho/etc/oos-board,
hal/tests/rootfs_selfheal.rs, hal/tests/rootfs_safe_start.rs, hal/tests/safe_start/,
ui/src/main.rs (startup and shutdown only), ui/src/store/startup.rs,
ui/src/store/durable.rs, ui/src/store/startup/, ui/src/store.rs (module declarations only).
Part 2 extension authorized by the owner on 2026-11-15: ui/src/system.rs,
ui/src/system/, ui/src/layout.rs (reduce obsolete panel budget only), ui/src/mdns.rs, ui/src/http.rs, ui/src/http/shutdown.rs, ui/src/scpi.rs, ui/src/vxi11.rs,
ui/src/drawpipe.rs, ui/src/drawpipe/shutdown.rs, ui/src/gpu.rs, ui/src/gpu/,
ui/src/ui.rs (hot file, lifecycle and watchdog wiring only), ui/src/ui/lifecycle.rs,
ui/src/ui/sysinfo.rs, ui/src/ui/remote.rs, ui/src/ui/host.rs, ui/src/cmd.rs,
ui/src/sysconf/switches.rs,
ui/src/store/startup/crash.rs, ui/src/mdns/lifecycle.rs, ui/src/vxi11/ports.rs, ui/src/store/startup/interruption.rs.
The owner explicitly retains the listed held paths; no changes there.
Device-loop extraction uses ui/src/system/device.rs and ui/src/system/device/;
main.rs retains only extraction and lifecycle wiring changes under the owner's extraction instruction.
Held elsewhere: xtask/, ui/tests/golden*, ui/tests/soak.rs and tools/ golden tooling.

Specification: [M1b](../../architecture/migration.md#m1b-safe-start-and-exit),
[ADR 0017](../../adr/0017-safe-start.md),
[safe start and exit](../../architecture/README.md#safe-start-and-exit).
Branch `t0048-m1b-safe-start`, base `aac04cc`, contributor codex-b (Codex).
No commits, pushes or device access in this job. Prepare review units and their messages/file lists in
`target/COMMIT_MSG.txt`, separated by `-----`. Cross-vendor review and round 0 remain coordinator work.

### What 0045 already delivers

- `panic=10 softlockup_panic=1` in OOS_BOOTARGS, consumed by the image build. No boot partition changes needed.
- Independent hardware-watchdog kicker with bounded restart backoff and host regression coverage.
- Crash archive: panic/oops header filtering, whole dmesg plus bounded console, 1 MiB budget and 4 MiB reserve,
  durable copy/rename/readback, idempotent retries and one retained partial. Existing
  `hal/tests/rootfs_selfheal.rs` tests archive failure, corruption, fsync, rename and clear retries.
- It does not count abnormal exits, deduplicate a stale running marker with a crash, implement safe mode,
  delay setup persistence, bound UI shutdown, restart the helper or watch UI RSS. Crash archiving currently
  starts before trial bookkeeping; fix the ordering as part of the counter integration.

### Review units, in order

1. Extract the rcS UI loop into an image script and add the durable rolling-hour counter. Preserve trial
   rollback authority. Serialize archive/count/marker consumption before UI starts; count panic/oops after
   archive durability and before pstore clear, with durable event receipts. Keep at most the newest three exit
   times plus bounded retry receipts. An abnormal exit counts even with the off flag set. Counter failures
   log and choose safe mode rather than silently launching a saved setup.
2. Add storage-owned durable writes and startup state. Write the running marker before source/settings
   startup; safe mode quarantines persisted setup and removes the same-boot copy before restoration. Track
   candidate setup text every step with a monotonic clock; restart its 60 s timer at each change. Promote
   both setups before replacing running with stable. Clean power-off persists current settings before MCU
   commands; ordinary clean exit only promotes a stable candidate and removes markers durably.
3. Bound shutdown of the threads that exist now: stop main intake, stop/close remote connections (1 s),
   close deposit work and join current workers (1 s), persist eligible setup, stop helper (1 s then kill),
   release DRM. Acquisition cancellation remains M3.0/M5. Claim concrete adapter paths before edits.
4. Helper EOF/death renders on CPU at the next step, records failures and retries after 1/5/30 s,
   remaining on CPU after three failed restarts. Claim GPU/display/state-query paths before edits.
5. RSS watchdog last: configured soft/hard thresholds, STOP/reset buffer holders, bounded recovery then
   exit 75, with deterministic host memory samples. Claim stage/reset/config paths before edits.
6. Complete the remaining acceptance surface: last-crash details in state query/System, trial-panic integration,
   full counter power-loss modeling, and the image BusyBox matrix. Claim UI/query files before edits.

### Failing-before acceptance map

Planned names below are acceptance targets, not evidence of tests already run.

| Migration requirement | Test file and test name |
|---|---|
| Aborting stand-in reaches safe within three restarts | `hal/tests/rootfs_safe_start.rs::aborting_ui_reaches_safe_on_fourth_launch` |
| Three aborts 20 minutes apart; hourly expiry; exit 75 immediate | same: `slow_aborts_count_within_an_hour`, `old_exits_expire`, `memory_exit_selects_safe_immediately` |
| Trial disables safe; clean exits do not count | same: `trial_keeps_rollback_authority`, `clean_exits_do_not_count` |
| Panic/oops archived and counted once; console/pmsg count nothing | `hal/tests/rootfs_selfheal.rs::a_panic_is_counted_once`, existing `console_records_alone_archive_nothing` extended with counter assertion |
| Panic plus stale running marker counts once | `hal/tests/rootfs_selfheal.rs::panic_and_running_marker_are_one_exit` |
| Cut after archive/count/clear retries once | same: `counter_failure_keeps_pstore`, `clear_retry_does_not_count_twice`; `hal/tests/rootfs_safe_start.rs::stale_marker_retry_does_not_count_twice` |
| Archive budget/reserve, failure never stops boot | existing `the_budget_holds_and_the_newest_stays`, `the_reserve_skips_without_pruning_and_keeps_pstore`, `a_failed_archive_keeps_pstore_and_an_interrupted_one_is_kept` |
| Panic after trial bookkeeping falls back | `hal/tests/rootfs_safe_start.rs::trial_bookkeeping_precedes_crash_save`; existing update boot state-machine tests remain unchanged; actual reset in round 0 |
| Counter cuts at each durable step | `hal/tests/rootfs_safe_start.rs::counter_write_failures_preserve_retry_identity` |
| Candidate younger than stable_s is never restored, including same-boot restart | `ui/src/store/startup/tests.rs::abort_restores_last_stable_setup` |
| Timer restarts per settings change; later stable candidates persist | same: `each_candidate_gets_a_full_stability_interval` |
| Clean power-off persists even within stable_s | same: `clean_power_off_persists_current_candidate` |
| Marker/setup durable interruption at every step, old or new only | `ui/src/store/startup/tests.rs::cuts_never_mark_an_unwritten_setup_stable`, `ui/src/store/startup/tests.rs::cuts_keep_old_or_new_file` |
| Safe defaults, both saved copies bypassed, quarantine retained | `ui/src/store/startup/tests.rs::safe_start_quarantines_setup_before_restore` |
| Current shutdown ordering and bounds | future `ui/src/system/shutdown/tests.rs::existing_workers_stop_in_order`, `overrun_continues_without_freeing_live_buffers` |
| Killed helper gives CPU next step, bounded retries | future `ui/src/display/recovery/tests.rs::helper_death_draws_cpu_next_step`, `three_failed_restarts_stay_on_cpu` |
| RSS STOP/reset and unrecovered exit 75 | future `ui/src/system/memory/tests.rs::hard_limit_stops_then_exits_75` |

### Risks and verification

- BusyBox ash is the target shell, including its applets and file/directory `sync` behaviour. Host POSIX sh
  tests are useful but do not establish image compatibility; rerun with the image's BusyBox in a Linux runner.
- RTC/NTP steps can move wall time backwards. Keep future-dated entries conservatively; use uptime only for
  duration in one run, never as boot identity. Persist retry identity independently of hourly pruning.
- `/oos` is small and may be unavailable/full. Never clear crash records before the count commits. Startup
  storage failures must not claim a stable setup. Crash and UI counter writers must never race.
- Power loss can occur between any two operations, including after rename before directory fsync. Use a
  filesystem fake that drops unsynced operations for the Rust transitions, plus shell failure injection.
- The existing main function exceeds ratcheted limits. Extract startup/persistence into storage modules;
  reduce main instead of raising a limit. No xtask/data edits or new dependencies.
- Full acceptance requires debug/release tests, scope cross-check, the full `cargo xtask check`, independent
  Claude review, and coordinator round 0 (panic sysctl, restart-loop setup, update shutdown). Record all
  failures and unavailable checks. No hardware behaviour is verified by this host-only job.

## Status

- 2026-11-15: review round 2 unit fixes are prepared over `40c5ba6`, uncommitted, for renewed Claude
  review. Kexec undo starts the image supervisor only on exact-name absence; recovery requires a parse
  failure and retains two archives; PID-claim mkdir locking serializes writers and reclaims stale/empty
  locks. Quarantine stamps sort numerically. Shutdown waits share 2.2 s; memory STOP retains clock/toast
  and service polls; helper acknowledgments must span 10 s before retry-budget reset. ADR 0017's
  dw_wdt/CARDRESET residual-risk section is unchanged. Notes: `target/review2-findings.txt`; message and
  all 20 review paths: `target/COMMIT_MSG.txt`. No staging, commits, pushes, merges, device access,
  held-path edits or ratchet changes. Task remains active; another independent review follows.
- 2026-11-15: round 2 focused debug/release checks each pass 88 tests: 26 supervisor/counter, 23 archive,
  35 UI/storage/shutdown/helper and 4 updater. Logs: `target/review2-rootfs-{debug,release}.log`,
  `target/review2-ui-{debug,release}.log`, `target/review2-undo-{debug,release}.log`. The final fast gate
  exits 0 and prints `partial gate passed (not done): 5 steps in 11 s`
  (`target/review2-fast-gate-final.log`). Linux scope compilation exits 0
  (`target/review2-scope-check-final.log`). No full gate was run under the owner's fast-only instruction.
  Shell syntax, whitespace and message/addition privacy checks pass. BusyBox image execution, physical
  timing and device behavior remain unverified. Blocking kernel/storage IO can exceed process deadlines.
- 2026-11-15: failing-before evidence covers every round 2 finding: `target/review2-undo-before.log`,
  `target/review2-ui-before.log` (four failures, including 3.13 s shutdown), and
  `target/review2-ledger-before-final.log` (five failures, including timeout history loss, unbounded
  archives and the concurrent temporary-file race). Baseline script replay restored final scripts in a
  finally block. Broader archive checks first exposed shared fixture lock paths and host-wide sync stalls;
  fixtures now isolate temporary roots and fsync only the supplied paths. Initial results remain in
  `target/review2-ledger-after.log` and `target/review2-selfheal-after.log`; final logs above pass.

- 2026-11-15: review round 2 starts from committed `40c5ba6` under the owner's unit-tier instruction.
  Plan: reproduce each P2/P3 with failing-before tests; repair kexec undo supervision, parse-only ledger
  recovery and bounded archives, serialize ledger writers, sort quarantine stamps numerically, share a
  shutdown deadline below 2.5 s, keep service polls alive during memory STOP, and require 10 s helper
  stability. Run focused host tests and the fast gate only. Preserve ADR 0017 reset-evidence limits.
  Additional owner-assigned paths: fpgad/src/bin/oos-update.rs and its regression tests. Existing reserved
  paths cover all other code changes; reserve the safe-start/shutdown sections of the architecture README
  and ADR 0017 for matching implementation details. No staging, commits, pushes or device access; another
  review follows.

- 2026-11-15: round 1 fixes are prepared for renewed Claude review, uncommitted. The original part 2
  message sections are preserved; the added round 1 section lists all 19 review paths in
  `target/COMMIT_MSG.txt`. `target/review1-fixes.patch` isolates this delta over the preserved part 2
  source; `target/review1-findings.txt` maps every finding to changes, tests and limits. Source fingerprint:
  `target/review1-source.sha256`. No staging, commits, pushes, device access, held-path or ratchet edits.
- 2026-11-15: round 1 focused validation passes in debug and release: 33 UI/storage/shutdown/presentation
  checks and 44 supervisor/archive tests per profile. Final supervisor fixture results are
  `target/review1-debug-rootfs_safe_start-final.log` and `target/review1-release-rootfs_safe_start-final.log`;
  the initial host-sync failure remains recorded separately. Scope clippy on final source exits 0
  (`target/review1-scope-clippy-final.log`). The fast gate prints
  `partial gate passed (not done): 5 steps in 17 s`. Message privacy and added/new-line scans have zero
  findings; the unchanged whole-tree audit has 153 pre-existing findings. Whitespace checks pass.
- 2026-11-15: the post-fix full gate exits 1 and prints exactly:
  `gate FAILED in 525 s: tests (debug), tests (release), perf guards, coverage`.
  Formatting, size, architecture, mutable statics, unsafe placement and clippy pass. Both profiles report
  HAL 329 passed / 1 failed / 8 ignored, UI 569 / 21 / 29, xtask 40 / 1 / 0. Unit failure names match the
  prior part 2 result: socket permission failures and the scratch Git-commit test. This run's deposit
  worker performance guard fails at ratio 7.07 while checks overlap; a subsequent isolated
  `tools/perf-guard.sh` exits 0 and prints `perf guards: all passed`. Coverage stops at
  `wlan::tests::join_commands` (HAL Unix socket permission denial), so coverage floors are not verified.
  Logs: `target/review1-gate-final.log`, `target/review1-perf-isolated.log`. An earlier full attempt printed
  `gate FAILED in 642 s: clippy, tests (debug), tests (release), coverage`; its closure ownership error
  was corrected before the post-fix scope/clippy run. That attempt remains in `target/review1-gate.log`.
  The final host fsync fixture and safe-toast wording were refined during the broader check sequence;
  final focused checks, fast gate and scope clippy cover that source. No full-gate pass is claimed.
- 2026-11-15: remaining acceptance evidence: image BusyBox ash/timeout/sync execution, physical ext2/card
  errors, MCU power-off, physical power cuts and live RK3399 reset-reason sysfs/dmesg are unverified.
  No device access is authorized. The local pinned kernel source archive is absent and the pinned BSP
  source fetch failed; upstream dw_wdt inspection is documented separately from platform verification.
  New waveform/System persistence-off golden capture remains with the owner of held golden paths.
  Renewed independent Claude review, full gate/coverage in a suitable runner and coordinator round 0 remain.

- 2026-11-15: round 1 implementation preserves the part 2 source and changes no held paths. Storage startup
  errors now disable persistence and show a waveform/System status; power-off continues after save errors.
  Promotions back off 1/2/4 seconds to 300 seconds and report once per failure episode. Quarantine retains
  the first plus two timestamped copies. Ledger mode is read-only; damaged state is archived and replaced,
  selecting safe once; clear/recovery bypass awk. Crash-save has a 20 s process timeout; all ledger calls
  have 5 s, including crash archive counting. Archive counter failures retain pstore and signal the loop.
  Fresh loop-provided run IDs count early process aborts independently of stale markers. ADR 0017 records
  the owner's same-boot/previous-boot evidence rule and the limits of reset-reason evidence.
- 2026-11-15: the storage/backoff/quarantine/supervisor regressions fail before their fixes:
  `target/review1-storage-before.log` (4 failures), `target/review1-supervisor-before.log` (6 failures).
  Extracting the original shutdown exit policy also reproduces blocked power-off before its correction:
  `target/review1-power-before.log`. Focused after-fix results and full-gate results are recorded below when
  complete. A host-only intermittent early-safe launch passed in isolation; the supervisor fixture now
  fsyncs specified files/directories rather than invoking macOS whole-machine sync under a 5 s bound.
  Fault injection remains separate from that host compatibility shim. Final supervisor tests: 21 pass
  in debug and release, including archived previous-boot panic, evidence-free power loss and synthetic
  watchdog bit. First attempt at isolating the shim exposed persistent fault shims; the separate host-bin
  now leaves injected awk/sync failures scoped to their original invocation.

- 2026-11-15: round 1 fixes requested by the owner for the Claude review of part 1. Preserve all
  uncommitted part 2 work. Plan: add failing-before storage, power-off, backoff, quarantine and supervisor
  regressions; make storage optional, bound recovery subprocesses, recover corrupt ledgers and pass launch
  identities before process entry. Same-boot stale markers count; previous-boot markers count only with an
  archived panic/oops or an exposed watchdog reset bit. No evidence means power loss. Record that owner
  decision in ADR 0017. Review fixes get their own message/file-list section. No commits or device access.

- 2026-11-15: final part 2 source full `cargo xtask check` exited 1 and printed exactly:
  `gate FAILED in 528 s: tests (debug), tests (release), coverage`.
  Formatting, size limits, architecture, mutable statics, unsafe placement, clippy and perf guards pass.
  Both profiles report HAL 329 passed / 1 failed / 8 ignored, UI 561 / 21 / 29, xtask 40 / 1 / 0.
  The failing names are identical to part 1: socket permission denials plus the existing scratch Git-commit
  test, whose underlying failure is not exposed. Coverage stops at the HAL Unix-socket permission denial,
  so coverage floors remain unverified. Complete output: `target/part2-gate-final-source.log`.
  All 71 focused checks pass in each profile, including 35 UI checks, 35 rootfs script checks and the existing
  trial rollback test. The two introduced regressions also pass in both profiles. Scope check/clippy, host sh
  syntax and diff whitespace pass. Source fingerprint stayed unchanged during the final gate.
  Commit-message privacy and added/new-line privacy each have zero findings; whole-tree scan still has 153
  pre-existing findings.
  No part 2 commits, staging, pushes, device access, held-path edits or xtask ratchet edits. Task remains active:
  independent Claude review, allowed-runner full gate/coverage, M1 crash-notice golden capture, image BusyBox
  execution and coordinator round 0 remain outstanding. Acquisition cancellation remains M3.0/M5.

- 2026-11-15: GPU recovery distinguishes consecutive failed restarts from total restart count. An
  acknowledged picture restores the failure budget; process startup alone does not. Added host regression
  passes with the other five GPU tests. Final-source gate restarted after this policy correction.

- 2026-11-15: the second full-gate run exposed two introduced regressions beyond the known sandbox failures:
  the state reply no longer ended in HDMI information, and the panel-use budget still named main after
  extraction. Both existing tests now pass after preserving the reply suffix, using `Ui::scr` for startup
  canvases and removing main's obsolete budget allowance. No budget was increased. Before-fix evidence:
  `target/part2-gate-before-regression-fixes.log`; passing tests: `target/part2-state-suffix-after.log` and
  `target/part2-panel-budget-after.log`. A fresh full gate is running on the final source fingerprint.

- 2026-11-15: part 2 implemented without commits or device access. Device startup/intake/presentation
  is extracted from main into small process-wiring modules; main changes are extraction and shutdown wiring.
  Remote listeners/connections and deposit/measurement joins share 1 s group deadlines. A timed-out worker
  retains its captures. Setup persistence precedes helper EOF, a 1 s wait and kill with a bounded 100 ms reap;
  DRM release follows helper termination. Power commands now follow that sequence. Acquisition cancellation
  remains M3.0/M5. HTTP connection clones close when the worker ends, before handle reaping.
- 2026-11-15: helper failures select CPU presentation in the current step, record failures/restarts, and retry
  after 1/5/30 s; three failed restart attempts exhaust recovery until a graphics setting change. RSS is
  sampled once per second from the host port with monotonic policy time. `mem.soft_mb`/`mem.hard_mb` default
  to 768/1024 MiB. Hard-limit STOP requests no final raw read and resets current records, source publications,
  deposit mailboxes, analyser/view caches and unsent remote snapshots. Sustained excess, or missing recovery
  samples, exits 75 after 5 s while preserving abnormal markers. GPU starts are suppressed during memory STOP.
- 2026-11-15: completed archive sequence/time and a bounded dmesg panic/oops header from `/oos/crash` reach
  the state query and conditional System-page notice. Partial archives and console-only records are ignored.
  The host test checks both presentation data paths. New golden capture of the crash notice remains with
  the M1 owner because golden paths are held; no reference was edited.
- 2026-11-15: durability coverage now includes both possible namespace outcomes before directory fsync,
  retries of later setup promotions, all power-off/quarantine boundaries, killed ledger writers on both
  sides of file-sync/rename/directory-sync, and a partial temporary ledger. These are host models and process
  interruption tests, not physical power cuts. The existing trial rollback test is also rerun unchanged.
- 2026-11-15: BusyBox verification unavailable: this worktree has no `rootfs/out`, no BusyBox executable was
  found under `rootfs` or `boot`, and the sandbox denies the Docker socket. Consequently the image ash,
  its compiled applets/options, and file/directory `sync` behavior could not be executed or verified.
  Host sh syntax checks pass for rcS, oos-ui-loop, oos-ui-exits, oos-crash-save and oos-watchdog.


- 2026-11-15: part 2 begins from committed part 1 at `3b2f786`; `2d9496c` and
  `3b2f786` remain unchanged while Claude reviews them. Owner requests remaining shutdown,
  helper recovery, RSS, crash presentation, durability tests and BusyBox evidence.
  No commits, device access or held-path writes. Concrete part 2 paths recorded above before edits.

- 2026-11-15: final-source full `cargo xtask check` completed with exit 1, printed exactly:
  `gate FAILED in 575 s: tests (debug), tests (release), coverage`.
  Formatting, size, architecture, mutable statics, unsafe placement, clippy and perf guards passed.
  Debug/release fail at the existing HAL Unix socket test, 21 existing UI tests (including network listeners),
  and the xtask scratch Git commit test. Socket failures report PermissionDenied/Operation not permitted;
  the scratch commit's underlying cause is not exposed by that test. Coverage stops at the HAL socket test,
  so coverage floors are not verified. Full output: `target/gate-final-source.log`.
  Focused final checks: 33 script tests (10 supervisor/counter + 23 self-heal) pass in debug and release;
  9 startup/persistence tests pass in debug and release. Scope target check and scope clippy pass;
  sh syntax and diff whitespace checks pass. Message privacy check has zero findings; no changed/new file
  findings in the tree audit. Source and review messages remain uncommitted; task remains active.
  Remaining: bounded shutdown, GPU helper recovery, RSS watchdog, last-crash query/System presentation,
  complete trial/interruption acceptance, image BusyBox execution, independent Claude review and round 0.

- 2026-11-15: three additional retry regressions failed before their fixes and pass after: rearchiving a damaged
  record preserves the stable record-time/checksum identity; retrying an old panic preserves a newer UI run's
  receipt; a delayed first panic count matches the older run among the three retained exit entries. Evidence: `target/counter-regression-before.log`, `target/receipt-regression-before.log`, `target/delayed-panic-before.log`, and the host
  script test logs. Final-source full gate is running as `target/gate-final-source.log`; earlier full gate attempts
  are retained separately. All changes remain uncommitted.

- 2026-11-15: review unit 2 implemented locally: storage-owned running/stable markers before source creation,
  safe quarantine and restore bypass, per-candidate 60 s promotion to both files, and immediate clean-power-off
  persistence plus supervisor stop flag. Nine deterministic startup tests pass in debug and release, including
  data/directory durability cuts for writes, startup/quarantine, promotion and clean exit. Tests use a filesystem
  fake, not real power removal. Bounded worker shutdown is not implemented by this unit.
- 2026-11-15: review unit 1 implemented locally: extracted rcS supervisor, newest-three rolling-hour ledger
  with durable retry receipts, immediate safe after 75, trial suppression and serialized archive/count/clear.
  Archives freeze the original UI run identity so a later retry cannot consume a newer marker. Initial seven
  host acceptance tests failed before the helpers existed; all ten loop/counter and 23 self-heal tests now pass
  in debug and release under host sh. File-sync, rename and directory-sync failures are injected into the counter.
  Image BusyBox ash execution is not verified: the sandbox denies the Docker socket. Shell syntax check passes.
- 2026-11-15: no held paths or ratchets changed. The fast gate passes (5 steps), and the Linux scope target
  compiles and passes clippy. Commit-message privacy scan has zero findings; whole-tree audit has 153 findings
  outside the 13 changed/new files and zero in those files. Proposed two review-unit messages/file lists are in
  `target/COMMIT_MSG.txt`. No staging, commits, pushes or device access. Final full gate result is recorded above.
  Cross-vendor review and round 0 remain coordinator work.

- 2026-11-15: codex-b read the M1b/ADR 0017 and 0045 evidence; plan and failing-before acceptance map written before code. Confirmed clean worktree on `t0048-m1b-safe-start` at `aac04cc`; no device access or commits.

- 2026-11-15: seeded and claimed for codex-b by the coordinator; the M1b prerequisite (roll fix forward-port bb7dd9b)
  is merged.
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
