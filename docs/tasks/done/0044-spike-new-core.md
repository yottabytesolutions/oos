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
- 2026-11-15: done. Report: `docs/tasks/notes/spike-new-core.md` on branch `spike/new-core` (6d5404f), kept
  unmerged as reference. Host results: deposit 49 us/frame (1,408 points) and 183 us/frame (12,512 points) at four
  workers against 13.9 and 7.7 us/frame for the current pipeline; 23-27 allocations and 12-16 MB per full step;
  trace IoU 76.8% (1 ch) and 50.5% (4 ch). Estimated 74-132 days to reach the current feature set.
- Decision (coordinator): keep the strangler migration (M1-M18). Do not replace the app. Reuse the spike's
  ownership/barrier tests, fake-clock cadence tests and zero-allocation compose guard where the M-steps need them.
