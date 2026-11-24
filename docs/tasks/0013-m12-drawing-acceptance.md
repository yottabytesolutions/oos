---
id: 13
title: "M12: acceptance and removal (D9)"
kind: chore
status: active
release: 0.2.0
owner: codex
depends: [12]
---

# M12: acceptance and removal (D9)

## Goal

Accept the Batch drawing path on the host and the unit, then remove the parallel Standard pipeline.
See [migration M12](../architecture/migration.md#m12-acceptance-and-removal-d9) for the acceptance criteria.

## Plan

Keep Standard as the host comparison path through the frame cutover. Compare matching raw and
corrected-code captures at one, two and four channels, and check pairs, dots, zoom, deep memory,
Roll, Scan, STOP, SINGLE and replay. Measure picture cadence, latency, capture rate, record
conservation and memory on the unit. Use the shared difference kernel for Batch and retain decoded
placement for modes that need it.

## Status

- 2026-11-24: The frame pipeline is the live path. The [current pipeline](../architecture/frames.md)
  describes its decoded fallbacks, deposit window and pacing. The
  [known issues](../known-issues.md) record the DHO924S observations from November 23 and 24.
  Matched unit acceptance against all M12 rows is still open.
- 2026-11-19: The consolidated upload implementation derives dirty-row hulls and retained consumer
  revisions from the same phosphor edits. Its host gate passed all 23 steps. The UI passed 1,871
  debug and 1,870 optimized tests, with 64 skipped in each profile. CPU goldens matched the
  reference. UI coverage was 90.4% against an 86.2% floor. Both allocation runs made 256 calls in
  each of six settings-observer regions with zero caller-thread allocations. Borrowing the supplied
  newest waveform reduced the warmed one-channel, 100,000-sample peak allocation above entry
  from 400,280 to 9,560 bytes. The four-channel row measured 9,752 bytes.
- 2026-11-19: In the four-channel host comparison, at 48 records per picture and 10 µs/div,
  Standard took 8.971/10.315 ms
  (p50/p99) to build a picture and Batch took 7.120/8.225 ms. At 1 µs/div the corresponding
  results were 1.554/1.688 ms and 1.694/1.856 ms. These host timings support the path selection
  and leave the hardware timing rows open.
- 2026-11-18: Batch with a shared difference kernel was selected. The extraction matched
  code and volt placement, folds and difference planes for line and dot rendering at one, two
  and four channels. The counter budget and its measurement history are recorded in
  [ADR 0031](../adr/0031-performance.md).
