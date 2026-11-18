# Errors return to their sender and use typed values

- Number: 0030
- Category: code
- State: target: typed-error follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The original audit found errors printed and ignored, unwrap calls and silently discarded results; some hardware errors
blocked the main thread. String errors erase the distinctions a caller needs for recovery. In release binaries a panic
aborts, so it cannot substitute for reporting recoverable input or IO errors.

## Decision

Public fallible functions return an error enum with Display, one per crate area, not Result<_, String>. No unwrap
outside tests. Use expect only for an invariant just established, with that invariant as its message, never for input or
IO. Return errors to the sender through the SCPI error queue, HTTP status or a toast. Acquisition retries hardware
errors with bounded backoff and reports Readings::status. Detectable inconsistency is a recoverable error. Release panic
aborts. Mutex locks use oos_hal::sync::lock.

Check: Clippy unwrap_used and review; a shrinking Result<_, String> ratchet is a target follow-up.

## Alternatives

Result values containing only strings: rejected because callers cannot distinguish recoverable conditions. Print and
continue or panic: rejected because errors do not reach their sender, and release panic aborts the process.

## Consequences

Unwrap, expect, abort and lock-helper rules are in force. Typed errors and complete retry/status coverage remain
migration work.
