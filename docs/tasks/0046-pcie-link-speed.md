---
id: 46
title: "Decide the PCIe link speed before the next 6.12 stable merge"
kind: chore
status: active
release: 0.2.0
owner: unitlab
depends: []
---

# Decide the PCIe link speed before the next 6.12 stable merge

## Goal

Know what the RK3399 2.5 GT/s cap costs the C2H path before a stable merge applies it.

## Plan

Reserved paths: none. Reserve paths when claimed.

Mainline commit 84d08ff902f1 ("PCI: rockchip: Drive at 2.5 GT/s, error other speeds", Cc: stable) is queued for
6.12 and in 6.18.63. Rockchip states 5 GT/s risks failure; the RK3399 datasheet gives 2.5 GT/s. The unit trains
Gen2 x2 at 5 GT/s (`max-link-speed = <2>` in kernel/dts/rk3399-rigol-dho900.dts). The change also touches
pcie-rockchip-host.c, which patches 0006 and 0007 modify.

Measure on the unit, Gen2 versus Gen1 (max-link-speed = <1> in a signed test package): C2H read() wall time for
4 MiB and for 4 ch x 10M, acquired frames per second, and frame time p99 from :SYSTem:STATe?. Unit work needs the
owner's go and a scope lease.

Decision: accept the cap if the loss is small; otherwise keep Gen2 by a local revert with the risk recorded, or
reduce per-read overhead first (persistent DMA mapping, docs/architecture/system.md). Kernel version policy: stay on
Rockchip develop-6.12 plus stable until Rockchip publishes a newer develop branch or mainline covers DMC and display.

## Status

- 2026-11-15: seeded by the coordinator from the kernel upgrade research. Blocks merging 6.12.116 or later.
- 2026-11-15: owner decision: accept the Gen1 cap now, do not risk the hardware. DTS max-link-speed = <1> on dho804 (fix/pcie-gen1) and overhaul. The unit measurement (Gen2 before, Gen1 after) is a record of the cost, not a gate.
