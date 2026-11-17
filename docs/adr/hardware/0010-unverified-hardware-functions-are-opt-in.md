# ADR 0010: A hardware function that is not verified on a unit is off by default

- Status: accepted
- Date: 2026-11-17
- Category: hardware

## Context

Host tests cannot prove that a new use of an FPGA block works on a unit. On a DHO924S (2026-11-15), autoset read stale
levels from the FPGA measurement unit and chose the wrong scale.

## Decision

- Merge unverified hardware functions switched off. An opt-in key in `/oos/system.conf` or a test environment
  variable enables them; the function becomes the default only after verification on a unit, documented with
  model and date (ADR 0015).
- Each temporary key has an issue for its removal. After verification, make the function the default and remove
  the fallback; after failure, remove the failed path. Issue tracking for existing keys is being implemented
  separately.
- Remove the `autoset.levels = unit` path; record-based levels are the supported autoset path. Code removal is
  separate from this documentation change.
- Keep the key inventory and verification state in the
  [system.conf reference](../../network.md#temporary-hardware-keys), rather than in this decision record.

2026-11-17 (#124): removed `trig.hw_status` and `meas.deep_raw` with their off paths. Trigger status from the FPGA
was verified on the unit 2026-11-13; raw deep measurements were verified on the unit 2026-11-15. Both now always use
the verified path.

## Consequences

The default behaviour is what ran on a unit. Temporary switches allow verification without changing other owners'
behaviour, but each adds a second path to test and must have a removal plan.
