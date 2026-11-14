# 0012: Error policy

Status: accepted. 2026-11-14.

## Context

Errors are handled in several styles: `eprintln!` and continue, `unwrap`, silently ignored results. Some hardware
errors block the main thread.

## Decision

- One error enum per module, with `Display`. No `unwrap` or `expect` on input or IO.
- Errors return to their sender: SCPI error queue, HTTP status, toast.
- The acquisition thread retries hardware errors with bounded backoff and reports them in `Readings::status`.
- A broken invariant panics in debug and tests, and in release logs one line and recovers to a safe state through
  the stage's `reset()`. Every stage has `reset()` and a test that after it the stage behaves as a new one.
- Log lines: `oos-ui <area>: <what> <values>`.
- Mutex locks go through one helper, `oos_hal::sync::lock(&m)`, that takes the guard with
  `lock().unwrap_or_else(PoisonError::into_inner)`. The release profile has `panic = abort`, so a poisoned lock is
  never observed there; in debug and tests the panic that poisoned it already fails the run. `unwrap_used` stays
  denied.

## Consequences

- Clippy flags `unwrap_used` outside tests (0009). `expect` is allowed only for an invariant, with the invariant as
  its message; never on input or IO.
- Error paths are tested with the fakes.
