# 0012: Error policy

Status: proposed. 2026-11-14.

## Context

Errors are handled in several styles: `eprintln!` and continue, `unwrap`, silently ignored results. Some hardware
errors block the main thread.

## Decision

- One error enum per module, with `Display`. No `unwrap` or `expect` on input or IO.
- Errors return to their sender: SCPI error queue, HTTP status, toast.
- The acquisition thread retries hardware errors with bounded backoff and reports them in `Readings::status`.
- A broken invariant panics in debug and tests, and in release logs one line and recovers to a safe state.
- Log lines: `oos-ui <area>: <what> <values>`.

## Consequences

- Clippy denies `unwrap_used` and `expect_used` outside tests (0009).
- Error paths are tested with the fakes.
