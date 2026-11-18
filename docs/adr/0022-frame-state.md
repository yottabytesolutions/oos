# The frame thread owns UI state without locks

- Number: 0022
- Category: code
- State: in force
- Date: 2026-11-18

## Context

The frame loop owns the displayed state that commands and queries must observe. Moving an interpreter elsewhere would
put locks around each part of UI state it touches and make command/frame progress depend on lock scheduling. Transport
workers need owned snapshots and messages, not access to mutable UI state.

## Decision

The frame thread owns mutable application UI state. Only it executes UI actions and SCPI programs. Workers receive owned
requests or immutable snapshots and return values through bounded ports; they do not lock or mutate UI state. This rule
does not claim that the legacy board Shared lock is gone. CPU-heavy reply encoding runs on workers using snapshots.

Check: review only for ownership; netio layer/confinement rules and session tests check the remote boundary.

## Alternatives

Share mutable Ui behind locks: rejected because frame and command execution would wait for one another. Move only
queries to workers: rejected because a worker must read a coherent owned snapshot, not live mutable UI.

## Consequences

Acquisition sharing is a separate target (ADR 0036). Keeping UI execution synchronous avoids an executor in the frame
loop.
