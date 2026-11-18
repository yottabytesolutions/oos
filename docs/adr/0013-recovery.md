# OOS lives in recovery and stock stays bootable

- Number: 0013
- Category: system
- State: in force
- Date: 2026-11-18

## Context

The stock loader tries boot, recovery and then backup when an image fails its check. Retaining stock gives the owner a
route back even when OOS never starts. The OOSK-to-recovery switch was verified on the DHO924S during the 2026-11-07/08
boot work; image checks and the fallback chain are recorded in [boot.md](../boot.md#boot-chain).

## Decision

OOS boots from recovery. Stock boot is preserved; only its 4-byte magic changes (ADR 0011). KRNL selects stock; OOSK
causes the loader to fall back to recovery. The OOS one-shot start restores KRNL before enabling automatic restarts.
One-shot is the default; System > Boot mode > Persist keeps OOS selected. `backup` holds the stock boot copy, then the
rollback slot, then the `/oos` filesystem in its last 16 MiB.

Check: HAL boot/update tests; tools/device-test.py boot-selection and protected-area checks on the unit.

## Alternatives

Replace the stock boot image or loader: rejected because an OOS failure would remove the independent fallback. Persist
OOS by default: rejected because a build that never starts cannot restore the stock selection itself.

## Consequences

The stock loader is retained. The layout and power-loss limits are documented in docs/boot.md and docs/update.md.
