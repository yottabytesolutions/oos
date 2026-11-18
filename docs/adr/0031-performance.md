# Performance work is bounded, event-driven and measured

- Number: 0031
- Category: code
- State: target: M3-M12 and performance-gate follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The 2026-11-15 audit found trigger/ring polling every 500/200 us, a 33 ms frame timer, socket stalls up to 2 seconds and
page-flip waits up to 100 ms. Worker-map merging cost about 2.1 GB/s at 30 batches/s, and perf guards covered kernels
rather than orchestration tails. Ownership alone did not bound latency or repeated whole-map work.

## Decision

Each thread blocks in one event/deadline wait. No sleep loops. Hardware polling exists only when the hardware exposes no
event for the required condition, at the hardware period and interruptible by the same wait. Normal-boot DMA completion
uses interrupts; kexec currently needs polling because MSI completion is unavailable (docs/update.md). The frame thread
never waits for a client, disk or page flip. Remote requests can run [light steps](../architecture/README.md#waiting)
without composing: apply settings and reply without a picture. Workers pre-sum; the frame thread adds at most 3 maps per
step, independent of worker/channel count. Hot paths allocate zero per frame after warm-up. GPU work is proportional to
change; no glFinish. Page-flip events pace full steps. Samples stay u16 until a consumer needs volts, immutable after
read and shared by Arc; no sample copy under a lock. Lock-free primitives stay in oos_hal::sync and require a unit
contention counter. Every budget has a number in architecture/README.md, a :SYSTem:STATe? counter and an assertion.
Ratio-based host perf guards are required CI; unit budgets are required release evidence.

Check: counting allocator tests, `perf` and unit release round; review. Production sleep ratchet and required host perf
CI remain follow-ups.

## Alternatives

Timer-paced sleep loops: rejected because they delay requests until the next poll instead of the relevant event.
Lock-free handoffs everywhere: rejected because complexity and unsafe need measured contention to justify them.
Kernel-only best-round benchmarks: rejected because they miss orchestration stalls and latency tails.

## Consequences

Existing allocation and primitive-ratio tests are in force; ownership, pacing and whole-map bounds follow M3-M12.
