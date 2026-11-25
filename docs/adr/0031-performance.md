# Performance work is bounded, event-driven and measured

- Number: 0031
- Category: code
- State: target: M3-M12, F0-F7 and performance-gate follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-24

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

Check through counting allocator tests, `perf`, review and the unit release round. Production sleep ratchet and required host perf
CI remain follow-ups.

### Pipeline counter budget

The original host guard capped the best elapsed ratio at 1.01 for diagnostics on versus off.
F0 staging raised it to 1.02 after repeated runs of the same binary reported ratios of
0.99706, 1.01651 and 1.01684. The 1.02 limit left about 0.316 percentage points above the
highest observation. The CPU p99 limit was 1.515 throughout.

The guard alternates diagnostics on and off after each picture and sums 16 pictures per side
in each of 101 rounds. Both sides reuse the same workers and buffers and wait for the same
batch completion. On Apple silicon, the process counter covers retired instructions across the
caller, coordinator and workers. The median paired instruction ratio filters occasional differences
in the scheduler's wake path. On hosts without the counter, rank paired rounds by combined CPU
time and compare the median ratio in the fastest quarter to reduce transient load noise.
The matched cost ratio cap is 1.01 and the independent CPU p99 ratio cap is 1.515.

The six workloads use one, two and four channels at 1 and 10 µs/div. Keep eight warmups,
picture and record counts, and deterministic pixel and pacing equivalence checks. Release
acceptance measures elapsed throughput and latency on the unit.

## Alternatives

Timer-paced sleep loops: rejected because they delay requests until the next poll instead of the relevant event.
Lock-free handoffs everywhere: rejected because complexity and unsafe need measured contention to justify them.
Kernel-only best-round benchmarks: rejected because they miss orchestration stalls and latency tails.

## Consequences

Existing allocation and primitive-ratio tests are in force. Ownership and pacing follow M3-M12; F0-F7 replace
worker-map reduction with the [frame pipeline](../architecture/frames.md). Its slice counters and budgets remain
targets until measured on hardware. Source waiting and capture-rate floors expose unsustainable throughput.
