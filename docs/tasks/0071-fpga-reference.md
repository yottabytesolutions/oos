---
id: 71
title: Public FPGA reference for implementers
kind: docs
status: active
release: none
owner: fpga-doc
depends: []
---

# Public FPGA reference for implementers

## Goal

Publish what is known about the acquisition FPGA as interface facts with their evidence (ADR 0013, ADR 0015), so
others can drive it and write more efficient host code. Review tier: standard.

## Plan

Reserved paths: `docs/hardware/fpga.md`, `docs/hardware/README.md`, `docs/README.md`, `docs/protocols/fpga-boot.md`,
`docs/protocols/fpga-image.md`, `docs/fork/hwzone.md`, `docs/fork/hwuart.md`, `docs/fork/hwserial.md`,
`docs/fork/core-offset.md`, `docs/fork/trigger-lockin.md`, `hal/src/dho/levels.rs` (comments only),
`docs/tasks/0071-fpga-reference.md`.

1. Check the private FPGA notes against the protocol pages, the OOS code and the unit logs: missing facts,
   contradictions, stale items.
2. Write `docs/hardware/fpga.md`: overview and block map, boot, PCIe/XDMA, throughput, data formats, function blocks,
   access rules, performance notes, hazards, open questions. Only facts verified on the unit or relied on by OOS code;
   link the protocol pages instead of repeating them.
3. Correct the protocol pages where the notes or the unit have newer facts; point fork pages at the new page.
4. Facts known only from analysis of the vendor image stay out; the owner decides on them from the withheld list in
   the task report.

## Status

- 2026-11-16: re-review applied: 0x4030 limited to the state field, DNA row and per-unit core offset dropped,
  rate wording instead of clock domains, marks cited from docs/README.md, upstream and assumed items marked.
- 2026-11-16: owner decision "publish confirmed parts": added clocks and time units, the registers OOS relies on
  with their confirmation, mask strobe rule, domain and skew, zone qualification, hardware average with 0x1004 bit 2.
  Pin map, PL clock structure, serial receivers and AUX OUT detail stay out: no hardware confirmation.
- 2026-11-16: branch t0071-fpga-doc from overhaul d9af73a9. Page written (one file, under 600 lines). fpga-image.md:
  PS DDR not shown to be fitted, slot 0x800000 is the self-test image. fpga-boot.md: Gen1 cap, factory image differs
  per unit, xdma completion modes. levels.rs and fork pages point at the public page. Next: review, then the owner's
  decision on the withheld items.
