---
id: 86
title: Place background work and bound RUN clock floors
kind: feature
status: active
release: 0.2.0
owner: codex
depends: []
---

## Goal

Implement performance plan P5 and P8 without real-time scheduling, interrupt-mode
changes or governor changes. Review tier: unit. Hardware behavior is not yet verified.

## Plan

On `sweep/p1b-clock-floor-followups`, verify clock writes by read-back, add a
shared critical-trip guard, and switch between display and RUN GPU minima in
the existing controller. Use virtual time for the host memory-hold retry test.

Reserved paths: `docs/tasks/0086-perf-placement-clocks.md`, `ui/src/placement.rs`,
`ui/src/placement/`, `ui/src/http.rs` (accept-thread placement only),
`ui/src/http/listener.rs` (extract start to keep the existing file within its ratchet),
`ui/src/system.rs`, `ui/src/system/clocks/`, `ui/src/system/clocks.rs`,
`ui/src/sysconf/switches.rs`, `ui/src/ui/host.rs`, `ui/src/ui/lifecycle.rs`,
`ui/src/ui/change/acquisition.rs` (clock port call only),
`ui/src/system/device.rs` (startup IRQ call only),
`rootfs/initramfs/bin/oos-ui-loop`,
`hal/tests/rootfs_safe_start.rs` (clock recovery test only),
`ui/src/ui/host/tests/clocks.rs` (memory hold and non-hardware port tests),
`docs/architecture/README.md` (Threads, Placement and DMA only),
`docs/architecture/system.md`, `docs/spec/state-query.md`,
`target/t0086-evidence/`, `target/p1b-clock-floor-followups/`, `target/COMMIT_MSG.txt`.
The owner's explicit task assignment authorizes this claim on the supplied base.

1. Read P5/P8 and the 2026-11-15 unit evidence. Keep the GPU helper on cpu4,
   acquisition on cpu5, all existing governors and interrupt modes. Move web
   accept to A53 SCHED_IDLE using the existing background-thread helper.
2. Represent IRQ policy as data in placement. At startup discover XDMA MSI and
   eth0 IRQs from procfs, choose A53 destinations away from deposit0 (the first
   worker and assumed busiest core), and write explicit affinity lists. Leave
   VOP, panfrost and MMC untouched. Test with fake sysfs/procfs.
3. Add a platform clock adapter behind a port. Accepted hardware RUN raises
   policy4 minimum to 1416000 kHz and GPU minimum to 600000000 Hz only when
   listed as available, preserving any higher minimum. Capture previous values
   before writes; restore on STOP, teardown and panic. Signal teardown uses
   the existing stop request. Separate default-on system.conf switches disable
   either floor. Log successful applied values once per process.
4. Round 2: persist minima atomically before writes, recover at startup and
   supervisor abnormal exit, and retain failed recovery evidence. Add a guard for
   named CPU/GPU thermal zones using their lowest passive trip minus 5 C, with
   5 C re-arm hysteresis and one-second sampling. Make the panic hook nonblocking;
   log successful/empty IRQ plans and drop absent floors once at construction.
5. Round 3: throttle failed restore retries to one second with first-failure and
   eventual-success logs, validate on/off floor switches, identify thermal zones
   and readings/errors in transitions, restore after every supervisor exit,
   keep host tests hermetic and correct the remote-listener thread table. Add
   failing-before regressions where testable.
6. Test lifecycle, unavailable values, missing nodes, independent opt-outs,
   failed writes and restoration against a fake filesystem, debug and release.
   Run the gate's exact clippy step and `cargo xtask check --fast`; preserve
   complete logs and exits. No ratchet edits or commits.

Unit acceptance, coordinator under an owner-released scope lease:

- Read `/proc/<pid>/task/*/{comm,status,sched}` for oos-ui and oos-gpu. Check
  main/GPU cpu4, acquisition cpu5, deposit workers cpu0-3, and web-accept,
  web-read, web-png and web-send A53 affinity with SCHED_IDLE (nice 10 only
  where refused). No FIFO/RR/deadline policy.
- Capture `/proc/interrupts` and `/proc/irq/*/smp_affinity_list` before/after
  startup. XDMA MSI must avoid deposit0's core; eth0 must avoid cpu0 when a
  deposit worker uses it. Check VOP/panfrost/MMC affinities remain unchanged.
  Confirm MSI mode, no new timeouts and normal acquisition.
- Capture policy4 `scaling_available_frequencies`, `scaling_min_freq` and GPU
  devfreq `available_frequencies`, `min_freq` before startup. During RUN read
  back 1416000 kHz / 600000000 Hz (or preserved higher prior minima). STOP
  must restore exact prior minima; repeat RUN/STOP and exit while RUN,
  SIGINT/SIGTERM and the existing panic/exit qualification path. Under the
  coordinator's approved qualification procedure, include an unclean exit during
  RUN, confirm `/tmp/oos-clock-floors` exists before death and is removed only
  after verified restore, and capture supervisor/startup recovery logs. Test each
  opt-out independently and both off. Never inject a panic on a live unit
  outside the coordinator's approved qualification procedure.
- Before the 10-minute RUN at setup B, identify `cpu-thermal` and `gpu-thermal`
  by type, capture every passive trip and the logged drop/re-arm limits. Capture
  the live DT thermal cooling-maps and resolve GPU cooling-device phandles to
  node names; record which cooling devices `gpu-thermal` actually maps to,
  corroborating with its sysfs `cdev*` links, `type` and `cdev*_trip_point`.
  This mapping is pending, not assumed to cool only the GPU.
- Sample CPU/GPU temperatures and clock read-backs every 1 s over 10 minutes
  of RUN at setup B. Abort the unit check at the lower of 70 C and the first
  live passive trip in either zone, on unreadable temperature, or on any
  unexpected state. The 70 C ceiling comes from the documented platform tuning
  passive-trip candidate; live sysfs is authoritative if lower. The runtime
  guard drops both floors at passive minus 5 C (65 C for a 70 C trip), within
  one second plus one UI step; verify read-back and one log per transition.
  Re-arm only below passive minus 10 C (60 C for a 70 C trip). Do not heat the
  unit artificially to reach the thresholds. Record min/max temperatures,
  throttling, fps and acquisition errors versus baseline, and whether the guard
  actually tripped. Host tests cover transitions not reached on the unit.
- During the same setup B RUN, measure 100 requests each for `GET /`,
  `GET /update`, and `POST /update` with a fixed 5 MiB invalid package from an
  allowed client; rejection must leave update state unchanged. Capture request
  completion and upload-end-to-final-response latency separately from transfer
  time. Pass: page completion and post-upload response p95 <= 1 s, maximum <=
  2 s, no timeouts, expected HTTP responses, and no acquisition errors. Compare
  STOP baseline and confirm the tested web threads have A53 SCHED_IDLE. A real
  signed upload/install still requires the separate owner-approved procedure.


Read-only unit command templates, after acquiring the required lease. Capture
thread and IRQ snapshots before/after startup; run `clock_readback` before
startup, during RUN, after STOP and after each authorized exit-path check.
Policy 5 is SCHED_IDLE; policy 0 is SCHED_OTHER. In `/proc/.../stat`, field 19
is nice and field 41 is policy (account for the parenthesized comm field).

```sh
for p in $(pidof oos-ui) $(pidof oos-gpu); do
    for t in /proc/$p/task/*; do
        echo "$t"
        cat "$t/comm"
        awk '/Cpus_allowed_list/ {print}' "$t/status"
        awk '/policy|prio/ {print}' "$t/sched"
        cat "$t/stat"
    done
done
cat /proc/interrupts
for f in /proc/irq/*/smp_affinity_list; do echo "$f: $(cat "$f")"; done
clock_readback() {
    d=/sys/devices/system/cpu/cpufreq/policy4
    for f in scaling_available_frequencies scaling_min_freq scaling_cur_freq; do
        echo "$d/$f: $(cat "$d/$f")"
    done
    for d in /sys/class/devfreq/*.gpu; do
        for f in available_frequencies min_freq cur_freq; do
            echo "$d/$f: $(cat "$d/$f")"
        done
    done
}
clock_readback
# Run only while RUN is confirmed, with the lease renewed independently.
for z in /sys/class/thermal/thermal_zone*; do
    case "$(cat "$z/type")" in cpu-thermal|gpu-thermal) ;; *) continue ;; esac
    echo "$z: $(cat "$z/type")"
    for f in "$z"/trip_point_*_type "$z"/trip_point_*_temp; do
        [ ! -f "$f" ] || echo "$f: $(cat "$f")"
    done
    for c in "$z"/cdev[0-9]*; do
        [ ! -L "$c" ] || echo "$c -> $(readlink "$c"): $(cat "$c/type")"
    done
    for f in "$z"/cdev*_trip_point; do
        [ ! -f "$f" ] || echo "$f: $(cat "$f")"
    done
done
# Save the live DT thermal-zones subtree for offline phandle resolution too.
n=0
while [ "$n" -le 600 ]; do
    date -u
    clock_readback
    for z in /sys/class/thermal/thermal_zone*; do
        echo "$z: $(cat "$z/type") $(cat "$z/temp")"
    done
    n=$((n + 1))
    [ "$n" -gt 600 ] || sleep 1
done
```

## Status

- 2026-11-24: clock-floor review follow-up on `sweep/p1b-clock-floor-followups`,
  base `f8e62078`. Read-back confirms floor writes and failed applications retry
  on the guard cadence. Critical trips protect both floors with 5 °C of headroom
  and 5 °C re-arm hysteresis. The sole controller selects 400 MHz while drawing
  in STOP or Demo and 600 MHz in hardware RUN. Host tests cover the unit's
  thermal layout, frequency transitions, recovery and virtual-time memory hold.
  Format and both debug clock suites passed, with 44 tests per suite. The
  default and private-picture-counts release suites passed 2377 and 2384 tests
  respectively, including the CPU goldens. Strict standalone Clippy stopped
  in HAL and DSP on both targets (exit 101). The fast gate passed all 12 steps
  and the ratcheted Clippy and perf steps passed (exit 0). Evidence is under
  `target/p1b-clock-floor-followups/`. Hardware verification and cross-vendor
  review remain pending.

- 2026-11-16: round 3 author handoff ready, uncommitted. P2 and P3.1-P3.6
  addressed, including throttling repeated memory-hold restoration. Final debug
  and release commands all exit 0: clocks 32, host 17, lifecycle 4 and supervisor
  30 per profile, no failed or ignored tests. Clock/host groups overlap by three
  host clock tests. Evidence: `target/t0086-evidence/round3-final-{debug,release}-{clocks,host,lifecycle}.{command,log,exit}`
  and `round3-{debug,release}-supervisor.{command,log,exit}`. The intermediate
  release host run also reproduced the failing-before memory-hold regression
  (16 passed, one failed, exit 101); the final rerun above supersedes it.
  Exact unchanged xtask `steps::clippy` passes, exit 0, six host/five ARM crates,
  no new findings: hal 93 entries/177 total, dsp 115/262, core 3/46,
  ui 246/666, fpgad 7/19, xtask 0/0. Final fast gate exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 41 s`.
  All eight steps pass: fmt, size, architecture, mutable-statics, unsafe,
  method-exceptions, known-failures and agent-rules. Evidence:
  `round3-final-{clippy,fast}.{command,log,exit}`, the unchanged clippy step
  runner and complete logs under `target/check-logs/`. Relevant shell syntax
  and final diff whitespace checks pass, exit 0. All 22 Rust/script fingerprints
  in `round3-source-sha256.json` match the tested source. Task Status and
  `target/COMMIT_MSG.txt` are updated. Intentional Git changes are task 0086
  source, tests and docs. No commits, pushes, unit access, hook changes,
  dependencies or ratchet edits. Keep this active worktree for renewed Claude
  review, coordinator full/image gates and owner-released unit acceptance.
  Hardware behavior remains unverified.

- 2026-11-16: round 3 implementation complete; final validation running.
  P2: failed restores retain their receipt, retry at one-second guard cadence
  during polling and log only the first failure and eventual per-device success.
  STOP and teardown still attempt restoration immediately. The recurring
  memory-hold path uses the throttled STOP update, leaving forced restoration
  to teardown and panic recovery. Its regression failed before the lifecycle
  fix with 102 writes across 100 held frames and passes after with one.
  Evidence: `target/t0086-evidence/round3-red-memory-hold.{command,log,exit}`
  (exit 101 before). P3.1: supervisor
  recovery runs after every exit, including clean exits with a pending receipt.
  P3.2: CPU/GPU switches accept only on/off, default on when absent, and log
  unknown values while disabling the corresponding floor. P3.3: thermal drop
  logs identify each blocking zone and reading or error; re-arm logs both zones
  and readings. P3.4: applied-floor logs use eprintln. P3.5: the headless-host
  test uses a fixture rather than constructing a system controller and asserts
  clocks are absent. P3.6: remove HTTP from the SCHED_OTHER listener row.
  Failing-before evidence: `round3-red-review` exits 101 with four regressions
  (retry cadence, switch acceptance, switch diagnostics, thermal diagnostics);
  `round3-red-supervisor` exits 101 on clean-exit recovery. Complete commands,
  logs and exits are under `target/t0086-evidence/`. The existing applied-log
  test covers P3.4; P3.5 changes the test construction itself and P3.6 is docs.
  Debug clocks (31), host (16) and supervisor (30) all pass, exit 0. No unit
  access, commits, dependencies or ratchet edits. Renewed Claude review and
  integrated full/image gates remain pending.

- 2026-11-16: round 3 active on the existing accepted claim and uncommitted
  round-2 work. Address Claude round-2 P2 and P3.1-P3.6; run touched debug
  and release tests, exact clippy and the fast gate. No commits, ratchet
  changes or unit access. Renewed review and integrated gates remain pending.

- 2026-11-16: round 2 author handoff ready, uncommitted, on
  `t0086-perf-placement-clocks`. B1, S1, S2 and O1-O3 are addressed below.
  Final debug and release focused tests all exit 0: clocks 26, placement 4,
  switches 3, lifecycle 4, shutdown 5; the complete rootfs supervisor suite
  passes 30 in each profile. Total 72 tests per profile, none failed or ignored.
  Commands are preserved in `target/t0086-evidence/round2-*.command`; complete
  logs and actual exits in matching `.log` and `.exit` files. The final debug
  runner is `round2-final-checks.py`; `round2-tests.py` covers both profiles,
  with release clocks/placement refreshed by `round2-release-final.py` after
  the final source change. Source identity: `round2-source-sha256.json`, all
  21 Rust/script fingerprints verified unchanged after testing.
  Exact unchanged xtask `steps::clippy` passes, exit 0, with counts unchanged:
  hal 93 entries/177 total, dsp 115/262, core 3/46, ui 246/666, fpgad 7/19,
  xtask 0/0. No ratchet edits. Evidence:
  `target/t0086-evidence/round2-clippy-final.{command,log,exit}`, unchanged
  `clippy-step.py` runner, and complete logs under `target/check-logs/`.
  Final `cargo xtask check --fast` exits 0 and prints exactly
  `partial gate passed (not done): 8 steps in 36 s`. All selected steps pass:
  fmt, size, architecture, mutable-statics, unsafe, method-exceptions,
  known-failures and agent-rules. Evidence:
  `target/t0086-evidence/round2-fast-final.{command,log,exit}`. The initial
  failed fast-gate steps are recorded in the preceding implementation entry.
  Relevant shell syntax and final `git diff --check` pass, exit 0.
  `target/COMMIT_MSG.txt` is updated. All build output uses this worktree's
  `target`; no commits, pushes, hook changes, dependencies or hardware access.
  Intentional remaining Git changes are task 0086 source, tests and docs.
  Keep the worktree for coordinator-arranged renewed Claude unit review,
  integrated full/image gates and owner-released unit acceptance. Thermal
  behavior, live GPU cooling maps, clock read-backs and web/upload latency
  remain explicitly unverified on hardware. Task status remains active.

- 2026-11-16: round 2 implementation complete, final validation running.
  B1: atomic tmpfs receipt precedes every raise; startup restores and logs the
  true saved minima before reading a new baseline, regardless of opt-outs.
  Abnormal supervisor exits restore even without restart. Only verified
  restoration clears receipts; invalid or failed startup recovery retains the
  receipt and disables new floors. Recovery tests simulate a death without
  destructors, retry failures, corrupt receipts and staging-write failure.
  Staging uses exclusive creation after unlinking stale entries, with a
  symlink regression. The generated receipt has at most two entries (50 bytes).
  S1: one-second guard samples named CPU/GPU zones against each lowest passive
  trip minus 5 C, drops both minima, logs transitions and re-arms below another
  5 C of hysteresis. Missing trips/zones or unreadable temperatures inhibit
  floors. Lifecycle polling continues on rejected preparations. Hardware
  temperatures and live GPU cooling bindings remain unverified; the 10-minute
  RUN check now defines an abort temperature and the required live DT capture.
  S2: setup B page and 5 MiB rejected-upload response checks require p95 <= 1 s,
  maximum <= 2 s and no timeouts. O1: panic hook uses try_lock, with a locked
  panic/death recovery test. O2: each applied IRQ/CPU pair and empty plan logs
  at startup, with captured-output checks. O3: absent floors are filtered at
  construction and logged once, with a repeated-RUN captured-output check.
  The supervisor regression lives in `hal/tests/rootfs_safe_start.rs`; no
  edits remain in task 0055's reserved `hal/tests/safe_start/mod.rs`.
  The initial fast gate exited 1: size, architecture, mutable-statics, unsafe,
  method-exceptions and known-failures all rejected a redundant test #[path]
  attribute. Removing it restored default module discovery. That failed run
  is preserved as `target/t0086-evidence/round2-fast-initial.{log,exit}`.

- 2026-11-16: round 2 review fixes in progress, same accepted branch and
  uncommitted claim. B1: atomic tmpfs recovery journal before writes, verified
  startup and abnormal-exit recovery. S1: named SoC thermal zones, passive-trip
  margin and hysteresis. S2: explicit web/upload latency acceptance. O1-O3:
  nonblocking panic hook, successful/empty IRQ logging, absent-floor filtering.
  Re-run debug/release tests, exact clippy and fast gate. No commits or hardware
  access; renewed independent review and integration/unit gates remain pending.

- 2026-11-16: author handoff ready, uncommitted. Final exact `steps::clippy`
  passes, exit 0, across six host and five ARM crates, with zero new findings.
  Printed counts: hal 93 entries/177 total, dsp 115/262, core 3/46,
  ui 246/666, fpgad 7/19, xtask 0/0. The runner compiles unchanged xtask
  modules and calls the actual gate function; no substitute lint flags or
  ratchet changes. Evidence: `target/t0086-evidence/clippy-step-final.{log,exit}`,
  `clippy-step.py`, plus complete command logs under `target/check-logs/`.
  Final `cargo xtask check --fast` passes, exit 0, printing exactly
  `partial gate passed (not done): 8 steps in 62 s`. All eight selected
  steps pass: fmt, size, architecture, mutable-statics, unsafe,
  method-exceptions, known-failures and agent-rules. Evidence:
  `target/t0086-evidence/fast-gate.{log,exit}`. `git diff --check` passes;
  the 15 changed Rust files are fingerprinted in `source-sha256.json`.
  `target/COMMIT_MSG.txt` has one plain-subject section and `Task: 0086`.
  Intentional remaining Git changes are this task's source/tests and docs.
  No commits, pushes, hardware access, dependencies, hook changes or ratchet
  edits. Keep this worktree for coordinator-arranged Claude unit review,
  the full integration gate and the exact unit checks above. Hardware
  performance, restoration and thermal behavior are not yet verified.

- 2026-11-16: focused tests pass in debug and release, exit 0 for every
  command: final `clocks` filter 16 passed (14 controller/adapter tests plus
  two UI host tests); `placement::` three; `sysconf::switches::` three;
  `ui::lifecycle::` four; `system::shutdown::` five. Total 31 per profile,
  no failed or ignored tests. Commands use `cargo test --locked -p oos-ui
  --bin oos-ui FILTER -- --test-threads=1`, adding `--release` for release.
  Evidence: `target/t0086-evidence/{debug,release}-clocks-final.{log,exit}`
  and `{debug,release}-{placement,switches,lifecycle,shutdown}.{log,exit}`.
  The earlier clock-only logs cover a smaller suite and are superseded by
  `clocks-final`. Only two explanatory comments changed after the final
  behavioral tests; clippy and the fast gate cover the final source.

- 2026-11-16: expanded clock coverage to subprocess panic-hook restoration
  without unwinding, structured once-only success logs, read-back mismatch,
  unavailable restoration values and the memory watchdog early-return path.
  Headless/demo RUN cannot request hardware floors. Signal cleanup follows
  the existing orderly device teardown. No hardware, commits or ratchet edits.

- 2026-11-16: implemented the platform clock port, accepted-state RUN/STOP
  wiring, memory-hold/teardown/guard cleanup and pre-abort panic hook. Initial
  clock tests pass: 8 passed, exit 0 (`debug-clocks.log`, subsequently replaced
  by expanded final-suite output). Initial fast gate failed, exit 1:
  `gate FAILED in 89 s: size`, reporting `http.rs` 1264 -> 1272. Evidence:
  `target/t0086-evidence/fast-initial.{log,exit}`. Extracted listener startup
  into its own module rather than changing a ratchet. Final checks running.
  CPU0 as the busy deposit core is an assumption from the crowded baseline,
  not a per-worker measurement; owner clarification requested. The IRQ plan
  takes an explicit busiest-core argument and excludes it; startup currently
  supplies cpu0, selecting XDMA cpu3 and eth0 cpu2. Actual deposit affinity is
  the whole A53 mask, so placement does not guarantee exclusive worker cores.

- 2026-11-16: accepted owner assignment, branch `t0086-perf-placement-clocks`,
  base `8fafc719`, initially clean. Plan recorded before code. Read repository
  delivery, architecture and unit tuning evidence. This oscilloscope task is
  outside the OptiSource-only model. Cross-vendor unit review, coordinator full
  gate and hardware checks remain pending; no hardware access authorized here.
