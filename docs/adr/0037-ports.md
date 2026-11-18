# Ports cover the IO the step depends on

- Number: 0037
- Category: code
- State: target: M16 and IO-port follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Board, HAL Bus and USBTMC Bulk already have fakes that support host tests. Direct DRM, panel, socket and clock calls
from the UI keep the complete step from running with fake IO. The boundary needs to cover those effects without wrapping
pure computations or duplicating file APIs that already accept paths and Read/Write.

## Decision

Ports cover Board, display::Present, input::Source, clock::Clock, instruments::Bulk and the IO layer. Each has a fake
next to it. No ports for pure code or file access already abstracted by paths and Read/Write. Adapters do not name one
another. Device stream adapters follow ADR 0023 when async removes blocking.

Check: `architecture` forbid/layer rules and fake-backed contract tests as each port migrates.

## Alternatives

Direct IO in the step: rejected because host tests cannot replace hardware, display or time consistently. Ports around
every pure function or file operation: rejected because they add indirection without replacing an effect that existing
value and Read/Write APIs cannot already isolate.

## Consequences

Board, Bulk and netio ports are in force. Remaining ports and complete adapter isolation are targets.
