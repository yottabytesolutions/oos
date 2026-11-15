---
id: 54
title: "kexec start with a changed device tree hangs"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# kexec start with a changed device tree hangs

## Goal

Updates that bring another device tree start through kexec like same-tree updates.

## Plan

Reserved paths: none. Reserve paths when claimed.

Unit evidence (2026-11-15, private RE notes docs/logs/oos_pcie_gen1_20261115/ and oos_unit_round0_20261115.md): twice the kexec_load path (kexec-tools with --dtb, taken when the image's tree differs) hung in the new kernel before ramoops registered; the hardware watchdog reset the SoC about 20 s later and the previous build came back. The same-tree path (kexec_file_load) works. Capture UART2 (115200) during a kexec start on the unit (needs the owner to attach the serial adapter), compare the loader-change merge of the tree (hal/src/update/fdt.rs with_loader_changes) with the running tree, and check memory placement of the dtb/initrd in kexec_load. Until fixed, DT-changing updates install with --no-kexec and a reboot.

## Status

- 2026-11-15: seeded by the coordinator.
