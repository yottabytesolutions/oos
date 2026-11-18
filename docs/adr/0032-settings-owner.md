# Settings have one owner and change only through commands

- Number: 0032
- Category: code
- State: target: M13, M14
- Date: 2026-11-18

## Context

The original Model had 178 public fields with about 103 direct writer sites. Knobs, menus, SCPI, HTTP, MCP and setup
files used different paths, including string-keyed setters, so validation and side effects differed by source. A single
owner and typed commands make all settings writes observable and ordered.

## Decision

Settings is a struct of private groups in oos-core. Only the change stage writes it. Every source, including internal
writers and setup files, emits Command values with one enum per group; there is no string-keyed setter. Group apply
validates and returns Result<(), CmdError> to the sender. Cross-group constraints live only in idempotent
Settings::normalize(). Per-step order is internal, store, remote, panel; arrival order within each source. Knobs emit
relative commands.

Check: compiler visibility; group validation, merge-order and normalize-idempotence tests required by M13/M14.

## Alternatives

Public settings fields or string-keyed setters: rejected because writes escape common validation and ordering. One
command enum for the entire UI: rejected in favour of area enums that keep validation with each settings group.

## Consequences

Migration must route the existing Model setters through private groups and typed commands before retiring them.
