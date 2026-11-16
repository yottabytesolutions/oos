# Audit part b: second check

Date: 2026-11-16. Input: `target/audit-b.md`. Each S1/S2 finding was checked against the cited doc line, the code,
its tests and the unit logs. Extra evidence used for the serial and AUX findings: the netlist-simulation record of the
FPGA in the private private RE notes repo (`docs/fpga.md`, sections "Serial trigger" and "AUX OUT"), which the fork
pages were written from.

## Verdicts

- B01 CONFIRMED. `tpu.rs:491-496` leaves the offset out for AC and LF reject; trigger.md 2.5 has the unit result.
  Fixed trigger.md 2.3 formula and 6.1.
- B02 CONFIRMED. `tpu.rs:486-490` also rewrites 0x208c on a source change while filtering. Fixed 2.5 sequence; that
  the filter needs the channel is kept as assumed (section 8 item 3), not stated as fact.
- B03 CONFIRMED. `tpu.rs:355-358` masks `(src-1)&3`; D0 (src 9) would give 0x8020 unmasked. Fixed 10.1 and all
  seven table rows.
- B04 CONFIRMED. `seg.rs:16-22`, `:39-42`; unit-dho804.md rounds B3 (0.8 ns, header(k) = latch(k-1)). Fixed
  trigger.md frame header line.
- B05 CONFIRMED. `mask.rs:167-188` clamps n to 1..1023 and to the record. Fixed hwmask.md (what it does, assumption
  row, `cols` row) and hwzone.md (columns, assumption row).
- B06 CONFIRMED. `mask.rs:308`, `zone.rs:113-114` subtract `map.skew` (6.5). Fixed hwmask.md round 5 note and the
  summary; hwzone.md marks the skew as not yet verified for the zone.
- B07 CONFIRMED. `dvm.rs:111-125`: mean = sum / 2^28, cfg reads SQ_HI with bits 7:0 replaced. Fixed step 3.
- B08 PARTLY. Code writes bit 12 = 0 for all three (`tpu.rs:507,543,558`). The table value 1 is the stock driver's,
  not wrong as such; nothing settles which value the receivers need. Annotated table rows and the 5.1 note, marked not
  yet verified on hardware. Unit check U1.
- B09 REJECTED. hwserial.md is right: A = clock, R = data, B = CS is the netlist-simulation result and the stock
  routing (`SetSourceA(clk)`, `SetSourceR(data)`). specs/07 section 1.4 is the older static reading. Added a
  supersede note to hwserial.md. specs/07 itself needs the fix (not in my list).
- B10 REJECTED. hwuart.md fields match the netlist simulation ([20:0], 0x221c [10:0], [25:23] relation, 0x2104[2:0]
  data bits - 1). specs/07 1.2 is stale. Added a supersede note and the rounding rule to hwuart.md.
- B11 REJECTED. hwserial.md packing matches the netlist simulation (0x21a0 [7:0]/[15:8]/[23:16], first byte most
  significant). specs/07 1.3 is stale. Covered by the same supersede note.
- B12 REJECTED. auxout.md matches the netlist analysis and already says "Not measured on the unit yet" (`auxout.md:8`).
  specs/06-mask-unit.md:20 ([23:2], "enable") is the stale side. Unit check U5.
- B13 CONFIRMED. `la.rs:98-103` and `hz.rs` give m = 2 for mask 3; la.md 3.1 has the unit result. Fixed T3, section
  9 item 2, section 10 row, section 13.
- B14 CONFIRMED. `la.rs:261-268`: READ_MAX = 40,000,000. Fixed section 10 item 2.
- B15 CONFIRMED. The page top reports OOS acquisition, raw reads from word 0, deep reads and roll spacing on the unit.
  Fixed 4.2 intro, section 10 heading and item 1, section 13 heading. Open items kept: non-zero raw start, raw TX
  length, LA-analog skew.
- B16 REJECTED for la.md. la.md and the code are right. The wrong line is `docs/models.md:219` ("not implemented on
  any model"), not in my list.
- B17 CONFIRMED. `ui/src/ui/sertrig.rs:44,130-135,157-160` offer polarity and stop bits; `hw.rs:817-822` passes them.
  Fixed hwuart.md "What OOS does" and bench step 6.
- B18 CONFIRMED. `hw.rs:873-877` keeps the receiver without CS when a timeout is set. Fixed hwserial.md step 3.
- B19 CONFIRMED. `ui/src/trig.rs:223-240` ignores direction for I2C Data; `addr_ok` applies it only to addresses.
  Added a row to serial-ui.md. Code finding C1.
- B20 REJECTED. dvm.md and `dvm.rs:59-70` agree; the lane field is a literal lane. specs/08-counter-dvm.md:52 is the
  stale side (not in my list). Four channels are already stated as untried (`dvm.md:53`). Unit check U7.
- B21 CONFIRMED. Round 5 settled the scale and the CH2-alone lane; round 8 and core-offset.md round 13 did the
  per-lane check. Rewrote the open items as history.
- B22 CONFIRMED. `coretrim.rs:34-36`: target 0.6, accept 1.5; round 13 lanes -4.92..-3.12 mV against -5.4 mV software
  at 500 mV/div (about 2 steps). Replaced the +-0.5 step bound. Unit check U8.
- B23 CONFIRMED. The round-19 grid covered 200 us/div and 1 ms/div, one channel. Restricted the claim, rest marked not
  yet verified on hardware. Unit check U9.
- B24 CONFIRMED. hwmask.md round 20 itself reports the 10 us pass at 1 ms/div 10k. Added the exception to step 2.
- B25 CONFIRMED. hwzone.md status and unit-dho804.md round 8 ("latch frames stay 0"). Fixed the hardware bullet.
  Unit check U10.
- B26 CONFIRMED. `hw.rs:668-733` (flush batch, 5 s retry, second block persists). Added the later behaviour to the
  hwzone.md stall guard section; the flush clearing the state machine stays assumed.
- B27 PARTLY. la.md 4.2 step 1 was wrong for OOS (`la.rs:199-214` writes whole words). The register table at
  `la.md:55-56` describes the stock field layout and is right as such; left unchanged. Fixed step 1. Unit check U11.
- B28 CONFIRMED. The table contradicts "everywhere else" and "all unlocked periods" (p50 at 97.5 and 400.3). Reworded
  to what the table shows.
- B29 CONFIRMED. `mask.rs:170,181-187,225` clamp every count to 1023, so `cols=33220` cannot set bit 15. Fixed step 4.
- B30 PARTLY (S3). Applied a short scope note at the top of trigger.md (ticks at the nominal clock, conversion by
  `clock::to_nominal_fs`, scaling of TPU timers assumed). la.md not changed.
- B31 CONFIRMED (S3). Fixed la.md section 0 items 2 and 5 (no 0x10e4 doubling in roll, 2.5x at 156.25 MSa/s).
- B32 CONFIRMED (S3). `counter.rs:115-124`. Fixed counter.md intro.
- B33 CONFIRMED (S3). `hw.rs:89-95`, `dvm.rs:75-79`. Fixed the per-lane procedure.
- B34 CONFIRMED (S3). Phase count is j, not k (for j, k coprime). Fixed trig-gate.md.
- B35 CONFIRMED (S3). `hw.rs:45-49`. Fixed trigger.md section 3.
- B36 CONFIRMED (S3). `record.rs:37-44`: stride floor(len/4096), 4096..8191 samples. Fixed ac-trig.md item 3.

Counts: S1/S2 (B01-B29): 21 CONFIRMED (all 7 S1 among them), 2 PARTLY, 6 REJECTED. S3 (B30-B36): 6 CONFIRMED,
1 PARTLY.

## Code findings

- C1 (S2) `ui/src/trig.rs:223-240`: the software I2C Data matcher ignores the Direction setting for Bytes = Any and
  Bytes = 1..5. The menu offers Direction for Data (`serial-ui.md`), and the hardware receiver gets it
  (`hw.rs:918-947`). With `trig.serial_hw` off, Direction has no effect on Data; with it on, the record path can place
  the software point on a data match of the other direction. Fix: apply `read`/`s.dir` in the Data branch as `addr_ok`
  does.

## Fixes needed outside part b

- `docs/fork/specs/07-serial-triggers.md` sections 1.2-1.4: mark superseded by hwuart.md / hwserial.md (netlist
  simulation): UART field widths, relation vs data bits, truncation; SPI A = clock, R = data; I2C byte layout and
  0x21a0 [23:16].
- `docs/fork/specs/06-mask-unit.md:20`: 0x119c is width [23:0], source 29, idle level 30, event 31 (auxout.md).
- `docs/fork/specs/08-counter-dvm.md:52`: lane 0 is a literal lane, not "the source's own lane".
- `docs/models.md:219`: the LA UI is implemented and gated by the LA capability; the line is obsolete.

## Unit checks

U1. Serial bypass bit 12. Run hwuart.md steps 1-2 and hwserial.md steps 1, 2, 4, 5 with `trig.serial_hw = on`
    (OOS writes 0x205c bit 12 = 0). Expected: triggers as described. If none fire, repeat with bit 12 = 1 (needs a
    debug switch; 0x205c is write-only, use the shadow). Record which value works.
U2. SPI routing. hwserial.md steps 1-2: `Data = 0xa5` fires only on the second word; with the scope's clock and data
    channels swapped in the decoder, no stable trigger. Confirms A = clock, R = data.
U3. UART bit time high part. Baud 50 (bit time 3,125,000 clocks > 2^21): `:SYSTem:DEBug:UART?` shows r221c = 1;
    Frame triggers stable. Also 9600 and 1000000 (step 4).
U4. I2C five-byte data. Write 01 02 03 04 05 after the address; Bytes = 5, value 0x0102030405: fires. Change only the
    first byte to 0x11: no trigger; mask 0x00ffffffff: fires again. Checks 0x21a0 [7:0]/[23:16].
U5. AUX pass/fail (hwmask.md step 7). BNC AUX OUT to CH2. Fail frames: one high pulse of 1 us per failed frame;
    set idle-high and pass-event and check level and event. Also confirms the pin reaches the BNC.
U6. I2C Data direction in the receiver. Bytes = 1, Direction = Write against a read transfer of a matching byte:
    expected no hardware trigger if condition 5 applies [10:9]. Not known from the simulation.
U7. DVM lanes with four channels. Distinct DC on CH1..CH4, all on: source CH4 reads lane 3 and follows CH4; repeat
    with three channels (CH1, CH2, CH3) and CH3 as source.
U8. DVM absolute DC. After the core trim, a calibrated DC reference at 1 V/div and 100 mV/div, offsets 0 and +-2 div,
    two boots. Report lane spread and absolute error separately against a DMM.
U9. Mask strobe rule beyond the grid. 2 and 4 channels, 20 us/div and 5 ms/div, 10k: count = points/1000 runs, one
    more stalls (0x4030 read-back 0, state 5); clean frames pass, shifted frames fail as in software.
U10. Zone counter. Zone pass-all then reject-all box, 10 s each: latch 0x11a0/0x11a4 (0x1190 bit 28); compare with
     the accepted acquisitions in `:SYSTem:STATe?`. Expected per the netlist: counts equal accepted frames; observed
     so far: 0.
U11. LA pass-through bits. Before the first OOS LA read after a stock hand-over, note the stock values of 0x1298,
     0x1280, 0x1290 from a stock-side dump; compare a deep read with OOS's
     whole-word writes against one with those bits restored. Expected: identical words.
U12. Edge filter channel on a source change. AC coupling on CH1, then source CH3 with a signal only on CH3 (CH1 open):
     triggers on CH3. OOS rewrites 0x208c = 0x800b; confirms the channel field matters with bit 12 = 1.
U13. Zone skew. hwzone.md step 4 with the default skew 6.5: the narrow box at the trigger edge passes, 1 div right
     does not.
