# 0016: Performance principles

Status: accepted. 2026-11-15 (team-lead decision, from the Fable performance review and the Codex code audit).

## Context

The target architecture fixed ownership and correctness but left performance to the steps. The two reviews of
38fc26f found the same causes in today's code:
- **Sleep-polling everywhere.** The acquisition polls the trigger every 500 us (`seq.rs` 1032) and the ring count
  every 200 us (`seq.rs` 952); roll, hold and SINGLE sleep 9, 5 and 2 ms (`hw.rs`); main paces on a 33 ms timer with a
  busy last millisecond (`main.rs` 563, 610). Nothing wakes main when a batch unit or a remote request arrives.
- **Blocking on slow peers.** An SCPI or VXI-11 reply is written from the UI thread with a 2 s stall timeout
  (`scpi.rs` 46); `wait_flip` can block main 100 ms (`drm.rs` 390); every GPU composition ends in `glFinish`
  (`oos-gpu.c` 817).
- **Copies and repeated scans.** The deep newest frame is cloned under `Shared` and again on the fast path
  (`hw.rs` 2173, `ui.rs` 1945); four-channel deposits always decode to `f32`; workers publish full maps and main adds
  every share over every pixel (about 2.1 GB/s at 30 batches/s); grade, decay and maxima sweep the maps every step;
  the GPU path compares every row, copies changed rows and uploads row bands.
- **No budgets that fail.** Perf guards take the best of rounds and cover kernels, not orchestration, tails or
  contention.

## Decision

Eight principles, from the Fable review:
1. **One waiting model.** Every thread blocks in exactly one call (`epoll_wait`, `poll`, a channel receive) on its
   events and one deadline. No sleep loops. A hardware poll exists only where the hardware raises no event, runs at the
   hardware's period, and is interruptible by the same call.
2. **Interrupt completion for DMA.** C2H completion by interrupt where it works; a busy completion poll only as a
   measured, pinned fallback that the status line shows.
3. **Remote work never waits for a picture.** A remote request gets a light step: apply, reply, without composing.
4. **Main's work per step is bounded and independent of channels and workers.** Workers pre-sum their shares; main
   adds at most 3 maps per step.
5. **Zero allocations per frame after warm-up** on every hot path, guarded by a counting allocator in tests.
6. **GPU work proportional to change.** Dirty regions, a fenced buffer pool instead of `glFinish`, asynchronous
   snapshots on demand, accumulate, maxima and decay fused.
7. **The display paces the steps.** A step starts on the page-flip event, never on a timer, and a flip wait never
   blocks main.
8. **Every budget is a counter and a gate.** Per thread and stage, p50 and p99 in `:SYSTem:STATe?`, asserted by
   unitlab and by perf guards.

And five rules for the code:
- **Codes until volts are needed.** Samples stay `u16` codes through transfer, deposit, cheap measurements and
  records; a consumer that needs volts converts its own window, with the batch's calibration.
- **One ownership path per batch.** Sample storage is immutable after the read and shared by `Arc`; each record is
  built once; measurements and history share it. No clone of sample data under a lock.
- **Locks are short, and lock-free only where measured.** On per-frame paths no lock is held longer than a small
  copy. The default hand-off is a bounded `sync_channel` or a `Mutex` cell around a small copy; a latest-value cell
  whose reader must react within a step is paired with a wake (`eventfd`). Every hand-off has a contention counter. A
  hot path switches to a lock-free primitive (SPSC ring, triple buffer, seqlock) only when its counter shows
  contention on the unit. Those primitives live in exactly one reviewed module, `oos_hal::sync`, with their own tests,
  Miri runs (task 0034) and a contention counter; no other module implements one. This keeps contention off the hot
  paths without spreading `unsafe`.
- **Remote IO never blocks main.** Replies go into bounded per-connection output queues, written by the listener
  thread on `EPOLLOUT`.
- **Hot shared counters are per thread**, cache-line aligned, summed by the reader.

Details: [Waiting](../README.md#waiting), [Data path](../README.md#data-path-cost),
[Performance budgets](../README.md#performance-budgets).

## Consequences

- Idle CPU drops to the hardware polls that remain (trigger and ring fill, `seq.rs`), each at its hardware period.
- Latency of STOP, settings changes and remote requests is bounded by an event, not by a sleep.
- Kernels and orchestration change together: the drawing steps (M5 to M10) carry the data-path rules, and new steps
  carry the waiting model and the GPU path (`../migration.md`).
- Each principle has a counter and a gate. A change that breaks one fails a perf guard or a unitlab assertion, not a
  review.
