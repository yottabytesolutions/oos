# Crates have one dependency direction

- Number: 0020
- Category: code
- State: in force
- Date: 2026-11-18

## Context

The scope is not always available, and register sequences must be inspectable without executing them. The original
three-crate boundary left domain types inside the UI, protected only by a module-path checker with gaps around aliases
and macros. A separate core crate lets dependency direction enforce the pure domain boundary.

## Decision

The workspace contains oos-dsp (dependency-free signal processing), oos-core (domain, depends on dsp), oos-hal (hardware
plans, boot and update), oos-ui (scope program), oos-fpgad (CLI on hal) and xtask (host tooling). The dependency
direction is dsp <- core <- ui, hal <- ui, hal <- fpgad. Core and dsp never name hal, libc or an async runtime. HAL
builds register-write lists as pure functions; application goes through Bus. Only ui::board::dho names oos_hal::dho. A
new crate or dependency edge changes this record and architecture.txt.

Check: `architecture`: crate, layer and confine rules in xtask/data/architecture.txt.

## Alternatives

A monolithic UI with direct register writes: rejected because host tests could not inspect plans independently. Keep the
domain as UI modules behind path rules: rejected because compiler crate dependencies enforce a stronger boundary than a
module checker with alias and macro blind spots.

## Consequences

The GPU C helper is separate from the Rust workspace (ADR 0026). Hardware plans remain inspectable without execution.
