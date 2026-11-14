# 0007: Threads talk through typed channels

Status: proposed. 2026-11-14.

## Context

`board::HwShared` is `Arc<Mutex<Shared>>` (`board/mod.rs:449`). Commands, results and status share one lock.
Requests are flags that can be overwritten before the acquisition thread sees them. Lock order and hold times are
not documented.

## Decision

- One writer per kind (`architecture/acquisition.md`, Port). In: `AcqWant` (latest value with `gen` and `epoch`)
  and `AcqCmd` (one-shot commands with ids, queued, acknowledged in `Readings`). Out: `Batch`, `Frame`, `MeasRead`,
  `Readings`.
- Metrics that change all the time stay atomics, one writer each.
- Remote listeners send `RemoteRequest { command or query, reply sender }` to main.
- Migrated after the change point (`overhaul-migration.md`, M5), since `gen` and `epoch` come from it.

## Consequences

- No shared mutable state between main and acquisition other than atomics.
- The acquisition thread can be driven by a test through its channels.
- `Shared` and its lock are deleted at the end of the migration step.
