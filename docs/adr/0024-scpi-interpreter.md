# One SCPI interpreter runs on the frame thread

- Number: 0024
- Category: code
- State: in force
- Date: 2026-11-18

## Context

Until 0.1.6 the servers read sockets on the frame thread with a 1 ms timeout, rounded by the kernel to 4-8 ms. Verified
on the unit: DHO924S over Ethernet, the open SCPI connection cut the picture from 30 to 16 fps (CHANGELOG 0.1.6). Raw
SCPI, VXI-11, HiSLIP and MCP still need identical program ordering and errors.

## Decision

Only Scpi::poll_program interprets SCPI. It runs on the frame thread against displayed state without UI locks. A request
is a complete program message. Programs execute in admission order without interleaving. Waiting queries become
per-client continuations; an adapter unable to retain one refuses with -221. Each intake serves at most 8 programs or
continuations and stops admission after 0.5 ms. Longer work goes to a worker and completes through *OPC.

Check: SCPI session tests with injected time check quotas, ordering and waits; intake perf measurements check
indivisible command cost.

## Alternatives

A parser per transport: rejected because commands, errors and fixes would drift across transports. An interpreter thread
with locked UI state: rejected because commands and frames would block each other. Blocking reads with a short timeout:
rejected because that path reduced the measured picture rate to 16 fps.

## Consequences

Transports own framing and connection lifetime. The frame quota cannot preempt an individual command; performance
evidence must cover it.
