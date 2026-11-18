# Threads share named cells and bounded channels

- Number: 0036
- Category: code
- State: target: M5 and F3
- Date: 2026-11-18

## Context

The legacy board shares commands, results and status in Arc<Mutex<Shared>>. Request flags can be overwritten before the
acquisition thread observes them, and lock order and hold times were undocumented. A message-count limit alone also
fails to bound memory when each retained message can own a large acquisition payload.

## Decision

Mutable sharing is limited to one-writer latest-value cells AcqWant, DepositView, Readings and composed-picture
snapshot, plus metric atomics on the architecture.txt statics allowlist. Hold cell locks only for copy or swap, never
across IO. Other data moves by value or bounded channels; each channel states its bound and overload action. RUN/STOP is
a level in AcqWant, not a queued command. Before arming a read, acquisition reserves a free DMA buffer.
When none is free, it waits for buffer return in the same event wait as commands. A full deposit slot coalesces
whole batches within count bounds, then waits; finite buffers propagate pressure to the source. No read record is
dropped for overload. Report wait time, capture rate and the conservation buckets in the
[frame design](../architecture/frames.md#back-pressure-and-conservation). One-shots carry gen/epoch and are served or
acknowledged as superseded.

Check: `mutable-statics` ratchet/allowlist and review; the named-cell Mutex boundary check is a target follow-up.

## Alternatives

One broad Shared mutex: rejected because unrelated commands, results and status contend without clear writers. Queue
RUN/STOP edges or overwrite one-shot flags: rejected because acquisition can miss intent; levels and explicit
acknowledgements preserve it. Unbounded channels: rejected because retained acquisitions can exhaust memory.

## Consequences

Legacy Arc<Mutex<Shared>> remains until M5. A channel bound includes retained payload bytes, not just message count.
F3 changes acquisition overload from read-and-drop to source waiting; command wakeups must remain responsive.
M12a requires the capture-rate floor so zero drops cannot pass by acquiring less.
