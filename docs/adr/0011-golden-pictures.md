# 0011: Golden pictures pin the drawing

Status: proposed. 2026-11-14.

## Context

Drawing regressions (dots, brightness, flicker) were found on the unit, hours after the change. Unit rounds are slow
and need the scope.

## Decision

- Host tests compose pictures from the Demo source and the fake board per display mode and time base, and compare
  them with reference PNGs in `ui/tests/golden/` with a per-pixel tolerance and a budget of differing pixels.
- A reference changes only in a commit that says why the picture changed.
- `gpu/test-host.sh` compares CPU and GPU output.

## Consequences

- Drawing changes show up in the gate.
- References need care when the LUT or fonts change. Regenerating is one command, reviewed like code.
