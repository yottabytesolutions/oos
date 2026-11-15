---
id: 44
title: "Spike: new core built fresh to the target architecture"
kind: chore
status: active
release: 0.2.0
owner: codex-b
depends: []
---

# Spike: new core built fresh to the target architecture

## Goal

Measure how fast a new core, built to the target architecture with leaf code transplanted from `ui`, reaches a
correct picture on the host. Input for the owner's decision between this hybrid and the step-by-step migration.
Timebox about 2.5 days. Throwaway unless the owner decides otherwise.

## Plan

Reserved paths: `spike/`, `docs/tasks/0044-spike-new-core.md`, `docs/tasks/notes/spike-new-core.md`.

The spike lives in its own Cargo workspace under `spike/` with path dependencies on `hal` and `dsp`, so it needs no
change to the root workspace, `xtask/` (task 0043) or the existing crates. It applies the gate's limits and lint set
to itself (same rustfmt.toml and lint table, size limits checked by hand or a small script) and reports them.
Part 1: settings, commands, change detection, demo batch source. Part 2: workers, kernels, accumulator. Part 3:
compose, picture comparison with the current app, report with a time log.

## Status

- 2026-11-15: claimed for codex-b on branch spike/new-core by the coordinator, base overhaul.
