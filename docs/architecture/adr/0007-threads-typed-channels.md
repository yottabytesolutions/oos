# 0007: Threads talk through typed channels

Status: accepted. 2026-11-14.

## Context

`board::HwShared` is `Arc<Mutex<Shared>>` (`board/mod.rs:449`). Commands, results and status share one lock.
Requests are flags that can be overwritten before the acquisition thread sees them. Lock order and hold times are
not documented.

## Decision

- Commands are bound or unbound. A bound `AcqCmd` (`Single`, `Force`, `Rearm`, `Final`, `Segments`, `SegRead`,
  `Levels`, `Slots`) carries the `(gen, epoch)` it belongs to and is served after that pair is applied, or
  acknowledged as `Superseded`. An unbound one (`AvgClear`, `SelfCal`, `CalCancel`) is always served in order. A
  SINGLE arm is a queued command, so no SINGLE epoch is lost in the latest-value cell.
- STOP and RUN are the `hold` level in `AcqWant`, not a command: run state is state and must never be superseded.
- One writer per kind (`../acquisition.md`, Port). In: `AcqWant` (latest value with `gen` and `epoch`)
  and `AcqCmd` (one-shot commands with ids, queued, acknowledged in `Readings`). Out: `Batch`, `Frame`, `MeasRead`,
  `Readings`.
- Metrics that change all the time stay atomics, one writer each, on the statics allowlist.
- Remote listeners send `RemoteRequest { command or query, wait, reply sender }` to main.
- The only mutexes are latest-value cells, each with one writer, held only for a copy or a swap, never across IO:
  - `AcqWant` (writer: `app/acq_client`; reader: the acquisition thread);
  - `DepositView` (writer: `app/acq_client`; readers: the deposit workers, at first claim);
  - `Readings` (writer: the acquisition thread; reader: main);
  - the composed-picture snapshot (writer: `present`; reader: the web PNG thread).
  Everything else moves through bounded channels or is moved by value (`Batch` into the pipe).
- Migrated after the change point (`../migration.md`, M5), since `gen` and `epoch` come from it.

## Consequences

- No shared mutable state between main and acquisition other than the cells above and the metric atomics.
- The acquisition thread can be driven by a test through its channels.
- `Shared` and its lock are deleted at the end of the migration step.
