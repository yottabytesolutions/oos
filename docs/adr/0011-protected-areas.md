# OOS writes only named card areas and protects stock FPGA slots

- Number: 0011
- Category: hardware
- State: target: protected-area evidence follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

The protected-area policy exists because damage to the stock loader, partition table or unit-specific FPGA golden image
can prevent boot or acquisition. The stock calibration files (`cal_*.hex`) exist nowhere else and cannot be recreated
from a generic image. These are recovery constraints, not permission to test destructive writes.

## Decision

OOS writes only the first 4 bytes of `boot` (KRNL/OOSK), `recovery` (its image), and `backup` (stock boot copy, rollback
slot and `/oos`). No other partition, IDB, vendor storage or partition table is writable. The stock data partition is
mounted read only. OOS calibration results go to `/oos`.

FPGA flash slots 0x000000 (unit-specific golden image) and 0x400000 (stock image) are never written. No PROG command may
cover any part of 0x000000 to 0x7fffff; a full-flash erase would cover both protected slots. Reading flash is allowed.
Slot 0x800000 is reserved for a future OOS bitstream, which requires an opt-in function and its own owner-approved
record before any write is implemented. Current OOS writes no FPGA flash. BOOT selects only slots 0x000000 and 0x400000.
There is no exception path to the protected-area list.

Check: protected_areas_before/protected_areas_after in tools/device-test.py; review. A host allowlist test and mandatory
protected-area evidence for releases and PRs touching update, boot, rootfs or HAL boot paths are the target follow-up.

## Alternatives

Allow protected-area writes when a feature needs them: rejected because the owner cannot reconstruct the original
calibration or boot chain. Use the stock FPGA slots for OOS: rejected; any future OOS image needs its own slot and
owner-approved decision, without an erase range overlapping stock.

## Consequences

No unit write is authorised without the owner releasing the unit for that task.
