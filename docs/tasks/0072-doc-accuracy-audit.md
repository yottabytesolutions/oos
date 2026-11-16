# 0072 Documentation accuracy audit

## Plan

Upstream review of PR #18 found two doc errors that would make a host program the hardware wrong (TPU event-limit
unit, 0x10e4 unit). Owner asked for a sweep of the hardware and feature docs.

1. Four Codex audits (parts a-d) compare every concrete claim with the code: S1 wrong, S2 inconsistent, S3 unclear.
   Reports: target/audit-{a,b,c,d}.md.
2. One Claude agent per part verifies each S1/S2 finding against code, tests and unit logs, applies corrections in its
   part's files only, and lists code findings and unit checks: target/audit-{a,b,c,d}-verified.md.
3. Coordinator pass: cross-part fixes, one commit, fast gate, integrate.
4. Port the docs/hardware/fpga.md and shared-protocol corrections to upstream PR #18.
5. Code findings become task 0073; unit checks go to the next unit round.

## Status

- Part a: verified (27 confirmed, 3 partly, 0 rejected), applied.
- Part c: verified (26 confirmed, 5 partly, 0 rejected), applied.
- Part b: verified (21 confirmed, 2 partly, 6 rejected; all 7 S1 confirmed), applied. 13 unit checks.
- Part d: verified (24 confirmed, 8 partly, 0 rejected), applied incl. self-cal/cold-boot cross-part fixes. 9 unit
  checks. adc-mode.md 3.2 qualifier added by the coordinator.
- Part b cross-part: docs/fork/specs/07-serial-triggers.md 1.2-1.4 superseded; specs/06-mask-unit.md:20 0x119c
  fields; specs/08-counter-dvm.md:52 lane is a literal index; docs/models.md:219 "LA not implemented" obsolete.
- Part b code finding (S2) ui/src/trig.rs:223-240: software I2C Data matcher ignores Direction (hardware receiver
  gets it, ui/src/board/dho/hw.rs:918-947).

Cross-part fixes for step 3:
- docs/protocols/self-cal.md:124-125: e=15 given as 65536 samples (2^(e+1)); elsewhere 2^e.
- docs/protocols/self-cal.md:474,516: "one ADC LSB per offset register step" superseded; rounds 11/12 measured about
  half a step.
- docs/protocols/horizontal-config.md 1.4: average-mode burst 32 vs the mem_size_avg comment in hal/src/dho/hz.rs:138.
- docs/protocols/cold-boot.md 3.9: 0x10a0 = 16 needs the "stock only, OOS does not write it" note (as C19).
- PR #18: "a burst of 10 points per channel stream" is wrong (A02): a burst is 10 stored words over all channels.

Code findings for task 0073 (part d adds: C1 medium hal/src/dho/selfcal.rs:827-832 vertical_payload indexes group 3
in a 12-block payload, panics at save after a full run; C2 low selfcal.rs:381 core offset loop assumes 1 step = 1 LSB
and no settle wait; C3 low selfcal.rs:21 adc_mean doc; C4 low fpgad/src/main.rs:833,845 PLL table claim; C5 low
ui/src/ui/selfcal.rs:156 All = 0..4 on 2-channel models; C6 low selfcal.rs:159-182 save not atomic):
- K2 medium hal/src/dho/adc.rs:61-67: CalAdc::load stops at the first CRC-valid cal_adc.hex even if its payload is
  not 0x738 bytes; cold boot (boot/cal.rs:94-101) tries the next location.
- low fpgad/src/main.rs:891-895: BOOT guard checks only xdma, not pcierockchiphost; --force skips it.
- low ui/src/ui/autoset.rs:19-26: comments give rates hz::prep does not produce.
- low hal/src/dho/coretrim.rs:24: set_offset doc says ADC steps; register steps are about half an ADC step.
- low hal/src/dho/boot/io.rs:18: "three image slots"; boot_message accepts two.
- low ui/src/board/dho/hw.rs:32: slow_mode comment says roll from 50 ms/div; ROLL_TDIV is 200 ms/div.
- low hal/src/dho/roll.rs:509-518: scan_trig_pos 0x4058 threshold "one column" is tdiv/10, a column is tdiv/100
  (settle with unit check U3 first).
- low hal/src/dho/boot/regs.rs writes 0x105c = 0x5555_5555, wpu.rs writes 0x5555.

- low hal/src/dho/hz.rs:136-137: mem_size_avg "two 32-burst blocks of margin" ambiguous (two blocks of 32 normal
  bursts = 128 average bursts).
- low hal/src/dho/auxout.rs:42: Source::Mask doc "OOS does not drive it yet" is stale.

Cross-part fixes applied (specs/06, 07, 08; horizontal-config 1.4: burst is 10 points in both modes, 32 is the stock
block size; fpga.md burst wording). models.md:219 already fixed by part b.

Unit checks: part a 9, part b 13, part c 7, part d 9 (lists in the verified reports).

Verified reports with the per-finding verdicts, code findings and unit checks: docs/tasks/0072-audit/part-{a,b,c,d}.md.
