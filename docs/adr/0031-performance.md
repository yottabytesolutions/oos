# Performance work is bounded, event-driven and measured

- Number: 0031
- Category: code
- State: target: M3-M12, F0-F7 and performance-gate follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The 2026-11-15 audit found trigger/ring polling every 500/200 us, a 33 ms frame timer, socket stalls up to 2 seconds and
page-flip waits up to 100 ms. Worker-map merging cost about 2.1 GB/s at 30 batches/s, and perf guards covered kernels
rather than orchestration tails. Ownership alone did not bound latency or repeated whole-map work.

## Decision

Each thread blocks in one event/deadline wait. No sleep loops. Hardware polling exists only when the hardware exposes no
event for the required condition, at the hardware period and interruptible by the same wait. Normal-boot DMA completion
uses interrupts; kexec currently needs polling because MSI completion is unavailable (docs/update.md). The UI thread
never waits for a client, disk or page flip. Remote requests can run [light steps](../architecture/README.md#waiting)
without composing: apply settings and reply without a picture. The deposit pool writes exclusively owned
column-difference tiles into one plane set, with no worker-map sum. The UI thread folds at most three deposits
in one pass per channel over touched rows (F2/F6). Hot paths allocate zero per frame after warm-up. GPU work is
proportional to change, with one exception: the [dirty-row hull](../architecture/frames.md#minimum-persistence-and-gpu-hand-off)
also uploads unchanged rows between disjoint changed bands and can span the full height. Unchanged pictures upload
nothing. No glFinish. Page-flip events pace full steps. Samples stay u16 until a consumer needs volts,
immutable after read and shared by Arc; no sample copy under a lock. Lock-free primitives stay in oos_hal::sync and
require a unit contention counter. Every budget has a number in architecture/README.md, a :SYSTem:STATe? counter and
an assertion. Ratio-based host perf guards are required CI; unit budgets are required release evidence.

Check: counting allocator tests, `perf` and unit release round; review. Production sleep ratchet and required host perf
CI remain follow-ups.

### Temporary F0 staging budget, 2026-11-19

The owner explicitly authorized recoverable overhead before frame cutover: "Widen allowed overhead if this
is the process before the cutover" and "add overhead only to recoup it later". In the private proposal,
only `perf_guard_pipeline_counters` changes best elapsed from 1.01 to 1.02 until F3 frame cutover.
CPU p99 remains 1.515, not 1.53. The six production-plan cases (1/2/4 channels at 1 and 10 us/div), 101
alternating rounds, 16 pictures per sample, eight warmups, actual picture/record assertions and deterministic
pixel/pacing equivalence remain. No sampler, instrumentation, profile, other guard or unit budget changes.

Repeated unchanged-binary observations include ratios 0.99706 and 1.01651; official attempt3 is 1.01684.
The separate counter-owner candidate yields 1.01039 and 1.01130 but does not establish causal improvement.
Two percent leaves about 0.316 percentage points above the official observation. A tighter 1.017 or 1.018
would closely fit this small selected sample. Two percent is a bounded engineering staging allowance,
not a confidence bound, noise correction or proof of no regression. Best-of-101 can hide sustained cost;
CPU p99 is a separate work-tail check and cannot prove wall-latency or unit throughput. Unreached cases
remain unverified. A >2 % result still fails; do not retry until green or widen again from these data.

Restoration owner: task 0013's F3 implementer and accepting coordinator. Before F3 frame cutover acceptance,
restore 1.01 in the Rust assertion, architecture row and contract fixtures; run all six unchanged cases with
CPU p99 <= 1.515 and actual picture/record checks, and retain raw paired evidence. Account for the recovered
budget explicitly in the task. Failure leaves cutover held, not the temporary cap permanent. Methodology
changes require separate justification and review. Existing cross-vendor review, integrated gate, matched
DHO804 A/B, M12 acceptance and route-deletion holds remain; historical 4CH regressions are not waived.
This amendment is a private owner-requested proposal, not public acceptance or release evidence.

## Alternatives

Timer-paced sleep loops: rejected because they delay requests until the next poll instead of the relevant event.
Lock-free handoffs everywhere: rejected because complexity and unsafe need measured contention to justify them.
Kernel-only best-round benchmarks: rejected because they miss orchestration stalls and latency tails.

## Consequences

Existing allocation and primitive-ratio tests are in force. Ownership and pacing follow M3-M12; F0-F7 replace
worker-map reduction with the [frame pipeline](../architecture/frames.md). Its slice counters and budgets remain
targets until measured on hardware. Source waiting and capture-rate floors expose unsustainable throughput.
