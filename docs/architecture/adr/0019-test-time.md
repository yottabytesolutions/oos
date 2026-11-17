# 0019: Tests do not depend on machine speed

Status: accepted. 2026-11-16. Authorized by the owner in task 0088.

## Context

Coverage instrumentation on a loaded host exposes scheduling assumptions in UI tests. Different SIM,
background measurement and pair tests exhaust their wall-clock deadlines despite correct results.

## Decision

No test passes or fails depending on how fast the machine is. Code that measures elapsed time accepts
`now: Instant` or a small real/manual Clock interface, without another crate. Tests advance time explicitly.
Production behavior remains unchanged. Tests exercise the shipped body, with clock and wait inputs supplied at
construction or through an explicit shared implementation. Production functions do not switch to a different
algorithm under cfg(test). Tests waiting for another thread wait on its event, using a channel,
condition variable, worker completion counter or a synchronous worker step. Timeouts serve only as hang
guards of at least 60 seconds and name the event that never happened. Assertions on real durations belong
in perf-guard, not unit tests.

## Consequences

Tests exercise state transitions independently of scheduling and instrumentation overhead. The fast gate
checks test source for sleeps and wall-clock deadline loops outside the reviewed helper. Its ratchet may
only decrease. Explicitly ignored manual tools and performance probes are recorded in the task audit.
