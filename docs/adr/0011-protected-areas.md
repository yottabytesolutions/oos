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

All raw card writes use `hal/src/update/card/writer.rs`, which owns the writable descriptor and checks the named
area and complete byte range before writing, syncing and reading back. Device identity follows the opened descriptor,
not its path spelling. Host boot tools and `oos-oneshot` submit bytes through `oos-update storage-write`.
Formatting and repair operate on a regular image of OOS data, then submit changed bytes through the same writer.
Legacy misc/BCB writes and whole-card clone writes are refused.

Check: host range, opened-device, CLI and boot-ordering tests; the fast gate's `protected-storage` rule rejects raw
storage-write primitives outside the writer. FPGA flash has no accepted range, including slot 0x800000 until a future
owner-approved implementation.

The protected-area before/after checks in tools/device-test.py and review
remain hardware evidence. Mandatory protected-area evidence for releases and PRs touching update, boot, rootfs or
HAL boot paths is still the target follow-up. This writer change is not yet verified on hardware.

## Alternatives

Allow protected-area writes when a feature needs them: rejected because the owner cannot reconstruct the original
calibration or boot chain. Use the stock FPGA slots for OOS: rejected; any future OOS image needs its own slot and
owner-approved decision, without an erase range overlapping stock.

## Consequences

No unit write is authorised without the owner releasing the unit for that task.
