# OOS ships its kernel and device tree inside recovery

- Number: 0014
- Category: system
- State: in force
- Date: 2026-11-18

## Context

OOS needs a board device tree and kernel drivers for its panel, DMA, network and video paths. Its Linux source is
assembled from the Rockchip BSP and kernel.org stable releases, with local patches, rather than vendor firmware.
Rebuilding recovery from identified sources leaves the stock kernel available as a fallback.

## Decision

The recovery image contains the OOS Linux kernel, device tree and initramfs. The kernel source combines Rockchip
`develop-6.12` at `470f9dccbdc42e7b8a824d0a5c5640a10e9457d2` with kernel.org stable through 6.12.115;
`kernel/base.sha256` identifies the archive and `kernel/series` lists local patches. The image also carries the
unchanged Xilinx XDMA module and wireless-regdb regulatory.db; kernel/README.md identifies their sources and licences.
No code from Rigol stock or modified firmware enters this build (ADR 0002). The stock kernel partition is untouched. Source and licence material accompany distributed images.

Check: `image-suite` (including kernel-base-provenance), image build checksum/provenance checks and review of kernel
sources and patches.

## Alternatives

Depend on the stock kernel: rejected because OOS needs its own drivers and reproducible source provenance. Overwrite the
stock kernel partition: rejected because recovery can carry OOS while preserving stock bootability.

## Consequences

OOS maintains its own device description and patches instead of relying on stock firmware.
