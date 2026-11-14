# 0007: Threads talk through typed channels

Status: proposed. 2026-11-14.

## Context

`board::HwShared` is `Arc<Mutex<Shared>>` (`board/mod.rs:449`). Commands, results and status share one lock.
Requests are flags that can be overwritten before the acquisition thread sees them. Lock order and hold times are
not documented.

## Decision

- `AcqCmd` in, `AcqEvent` out, over channels. The acquisition thread takes the newest acquisition request. Explicit
  requests (rearm, segment read, DVM, self-cal, ...) are queued and not lost.
- Metrics that change all the time stay atomics, one writer each.
- Remote listeners send `RemoteRequest { command or query, reply sender }` to main.
- Migrated after the change point (step 1c), since the request type comes from it.

## Consequences

- No shared mutable state between main and acquisition other than atomics.
- The acquisition thread can be driven by a test through its channels.
- `Shared` and its lock are deleted at the end of the migration step.
