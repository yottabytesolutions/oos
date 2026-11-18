# Tests do not depend on machine speed

- Number: 0009
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

Coverage instrumentation on a loaded host exhausted wall-clock deadlines in SIM, background measurement and pair tests
even when their results were correct. Waiting for elapsed wall time tested scheduling and instrumentation overhead
instead of the intended state transitions.

## Decision

Tests advance time explicitly through injected `Instant` or a real/manual Clock interface. They exercise the shipped
algorithm; cfg(test) does not select another algorithm. Tests waiting for a thread wait for its completion event,
channel or condition variable. Timeouts are hang guards of at least 60 seconds and name the missing event. Real duration
assertions belong in perf-guard, not unit tests.

Check: `test-time` scans test sleeps and deadline loops; its ratchet only shrinks. Review checks injected clocks and
event waits.

## Alternatives

Longer correctness deadlines: rejected because instrumentation and scheduling can still exceed them. A faster cfg(test)
algorithm: rejected because it would stop testing the implementation that ships.

## Consequences

Test results describe state transitions independently of machine speed. Manual tools and performance probes remain
explicit exceptions in the test-time inventory.
