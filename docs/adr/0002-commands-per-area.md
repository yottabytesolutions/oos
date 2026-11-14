# 0002: Settings change only through commands, one enum per area

Status: proposed. 2026-11-14.

## Context

Knobs, menus, SCPI, HTTP, MCP, VXI-11 and setup files all change settings, each by its own route. Validation and
side effects differ by route.

## Decision

- `Command` is a top-level enum of per-area enums: `Command::Trigger(TriggerCmd::Level(v))`, and so on.
- Each area enum lives in its group module next to the fields it changes.
- Every edge produces commands: input, menus, remote parsers, setup loading.
- Merge order per step: store, then remote, then input; arrival order within a source. The panel applies last and
  wins a conflict between two absolute values of the same field.
- Knobs produce relative commands (`ScaleStep(i32)`); `apply` clamps. A knob turn and a remote write in the same
  step both take effect instead of one overwriting the other.
- `apply` returns `Result<(), CmdError>`. The error goes back to the sender.

## Consequences

- One validation path per setting, testable without IO.
- Remote parsers map text to commands and nothing else. Adding an SCPI command does not touch settings code unless the
  setting is new.
- A command log is possible (replay a change storm in a host test).
- A flat enum with 150 variants is avoided. The area enums stay under the size limits.
