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

Verify clock writes by read-back, add a shared critical-trip guard, and switch
between display and RUN GPU minima in the existing controller. Use virtual time for the host memory-hold retry test.

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
`docs/architecture/system.md` and `docs/spec/state-query.md`.

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
7. Reconcile PR 197 on the public 0.2.1 base. Keep the existing floor targets and
   cooling bindings. Track verified and pending rollbacks, retry independent
   startup recovery at sample deadlines, and publish sampled SCPI clock state.
   Extend the reserved clock controller and its fixture tests, the supervisor
   recovery and its test, and the state-query clock contract. Record host evidence
   in Status and keep hardware acceptance pending.
8. Resolve absent devices during receipt recovery in the controller and
   supervisor. Keep recovery pending when an existing device cannot be restored,
   and test both cases. Document how the clock status reports read-back mismatches.

Unit acceptance, coordinator under an owner-released scope lease:

- Read `/proc/<pid>/task/*/{comm,status,sched}` for oos-ui and oos-gpu. Check
  main/GPU cpu4, acquisition cpu5, and deposit workers cpu0-3. Capture actual
  affinity and policy for `netio` and `web-png`. The HTTP background placement
  criterion remains A53 SCHED_IDLE (nice 10 where refused). No FIFO/RR/deadline policy.
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
  STOP baseline and record the shared executor and PNG worker placement. A real
  signed upload/install still requires the separate owner-approved procedure.

### HTTP placement on the public 0.2.1 base

| Work | Current execution owner | Placement in source |
|---|---|---|
| HTTP accept, request/body reads and reply sends | `netio`, alongside raw SCPI, VXI-11 and HiSLIP | The executor has no explicit affinity or scheduling-policy call. |
| HTTP screenshot PNG encoding | `web-png` | Calls the existing `lower_priority` helper. |

The shared `netio` executor uses a current-thread Tokio runtime. The old
`web-accept`, `web-read`, and `web-send` threads no longer have independent
thread placement. The executor and PNG worker still need measurement on the
candidate. The HTTP socket placement criterion remains open.


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

- 2026-11-25: Clock-floor review follow-up. Startup and supervisor recovery
  resolve absent devices and retain the receipt when a present device cannot
  restore its saved minimum. The supervisor logs the reason per device.
  Both absence regressions failed before the fix and now pass. Formatting,
  all 12 fast-gate steps, 50 clock and 33 supervisor debug tests, and all four
  selected steps passed (exit 0). Release passed 3,838 tests including the
  golden comparison. Clippy passed, the plan matrix matched 77 entries, and
  perf passed 34 tests plus the allocation test's child run. The SCPI clock
  schema has no sample age field. Independent review and verification on a
  DHO924S remain pending.

- 2026-11-25: On branch `fix/t0303-clock-floors`, reconciled PR 197 against
  public 0.2.1 (`425470a3`). The controller keeps rollback ownership through
  unverified I/O and publishes sampled clock state. The controller and
  supervisor attempt saved devices independently. The HTTP placement notes
  reflect the shared executor and separate PNG worker. Formatting, all 12 fast
  gate steps, 54 clock and 32 supervisor debug tests, and all four selected
  steps passed. The release run passed 3,835 tests, including the golden
  comparison. The plan matrix matched 77 entries and 35 perf tests passed.
  Candidate review and verification on a DHO924S remain pending.

- 2026-11-24: Clock writes are verified by read-back. Failed applications retry
  on the guard cadence. Critical trips protect both floors with 5 °C of headroom
  and 5 °C re-arm hysteresis. The controller selects 400 MHz while drawing in
  STOP or Demo and 600 MHz in hardware RUN. Host tests cover the unit's thermal
  layout, frequency transitions, recovery and virtual-time memory hold. Formatting
  and both debug clock suites passed (44 tests each). The default and
  `private-picture-counts` release suites passed 2,377 and 2,384 tests, including
  the CPU goldens. Strict Clippy stopped in HAL and DSP on both targets (exit 101).
  The fast gate passed all 12 steps, and the ratcheted Clippy and perf steps
  passed (exit 0). Hardware verification and cross-vendor review remain pending.

- 2026-11-16: Failed floor restores retain their receipt and retry at one-second
  intervals, with one log on first failure and one on recovery per device. STOP
  and teardown still attempt restoration immediately. The memory-hold regression
  went from 102 writes over 100 held frames to one. Supervisor recovery runs
  after every exit. Invalid CPU/GPU switch values log a diagnostic and disable
  the corresponding floor. Thermal transitions report the zones, readings or
  errors. The web accept thread uses the A53 background placement. Debug and
  release each passed 32 clock, 17 host, four lifecycle and 30 supervisor tests,
  with three clock tests shared by the clock and host filters. Ratcheted Clippy
  and all eight then-selected fast-gate steps passed (exit 0).

- 2026-11-16: The clock adapter records previous minimums in an atomic tmpfs
  receipt before a raise. Startup and the supervisor recover them, including
  after process death. Recovery clears the receipt only after verified restore.
  An invalid receipt or failed startup recovery disables new floors. The receipt
  uses at most two entries (50 bytes). The guard samples the lowest passive trip
  of each named CPU/GPU zone once per second, drops both minima 5 °C below a
  trip, and re-arms below a further 5 °C of hysteresis. Missing or unreadable
  zones or trips inhibit floors. The panic hook uses a nonblocking lock attempt.
  Debug and release each passed 72 focused tests, including 30 supervisor tests.
  Ratcheted Clippy and all eight then-selected fast-gate steps passed (exit 0, 36 s for fast).

- 2026-11-16: The IRQ plan takes an explicit busiest-core argument. Startup
  supplies cpu0, selecting cpu3 for XDMA and cpu2 for eth0. That argument is
  based on the baseline workload. Deposit workers can run on the whole A53
  mask. The focused debug and release suites each passed 31 tests. Ratcheted
  Clippy and all eight then-selected fast-gate steps passed (exit 0, 62 s for fast).
  Live IRQ and GPU cooling-device mappings remain to be verified.
