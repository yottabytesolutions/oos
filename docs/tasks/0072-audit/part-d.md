# Audit part d, second check

The audit verified every S1 and S2 finding against the cited documentation, the code, its tests and the unit evidence.

## Verdicts

- D01 CONFIRMED. `hal/src/dho/wpu.rs:30` uses lanes [lo, hi, lo, hi] for two channels; test `:63` and unit 2026-11-13
  pin it. afe-settings.md 4.1 and 6.8 corrected.
- D02 CONFIRMED. cold-boot.md 3.9 used the Q10 word as g and omitted the x2 split (`wpu.rs:24-35`). Row rewritten.
- D03 PARTLY. The pairing order[ch]/order[ch]+4 is the stock value and belongs to core-order mode. OOS self-cal never
  uses it (`selfcal.rs:446` averages 8 slots in channel-only mode). Normal-mode slots from `levels.rs:64`, CH1/CH2
  verified, rest assumed. The 14.3 "same cores as four-channel mode" sentence is now marked unverified for normal mode.
- D04 PARTLY. Section 8 is the stock driver flow. The finding holds when an offset change moves the gear (g1 and path gain
  change, `afg.rs:127-160`). OOS already recomputes both codes in `retune` (`afg.rs:625-645`). Section 8 now labelled
  stock and the 0x3070 rewrite added for a gear change.
- D05 PARTLY. A phase change writes 0x3040 (`afg.rs:76`), no table rebuild. OOS has no phase setting (start writes 0,
  `afg.rs:606`). Flow split into a phase line and a duty/symmetry line.
- D06 CONFIRMED. `boot/mod.rs:213` writes the table of `adc.clock_hz` on every boot; rcS only sets the file aside after
  an unfinished start. pll.md recovery text corrected.
- D07 CONFIRMED. Core-offset rounds 11/12 measured about 0.5-0.6 ADC LSB (9.0-10.6 codes) per register step in normal
  mode. self-cal.md 14.6, 14.9 and 14.10 now call 16 codes a loop coefficient. Core-order slope is a unit check.
- D08 CONFIRMED. `afe.rs:97` falls back to the last group; `afe.rs:547` substitutes group 0 for an empty block.
  afe-settings.md 1.3 and 6.7 corrected.
- D09 CONFIRMED. `adc.rs:306` writes e=15; `levels.rs:28` and fpga.md use 2^e. self-cal.md 2.4 now says 32768 samples,
  about 0.21 ms, assumed. Cross-part fix for self-cal.md:124-125 done.
- D10 CONFIRMED. `adc.rs:181-207` only updates shadows from the ADC-1 fields; fine blocks 4..10 unused. self-cal.md 14.10
  rewritten.
- D11 CONFIRMED. `boot/cal.rs:88-102` reads `/oos/cal` first for cal_adc. self-cal.md 14.10 corrected; cold-boot.md 1.3.1
  kept as stock with an OOS sentence added.
- D12 PARTLY. The lines describe the isolated per-setting writes, which is a valid description. OOS always runs the full
  set-up (`seq.rs:402`, `afe.rs:879`). Scope note added to afe-settings.md 3.2; "only hardware write" and "no AFE write"
  narrowed.
- D13 CONFIRMED. `selfcal.rs:182` has no hardware-version rule; UI All builds 0..NCH. self-cal.md 1.2 and 14.7 labelled
  stock, two-channel self-cal marked not yet verified.
- D14 CONFIRMED. `boot/io.rs:210` runs `early_start` during the FPGA wait with 1 ms gaps (`regs.rs:293`). pll.md,
  cold-clock.md 2.3 and 7, cold-boot.md section 4 rows 2/3 corrected.
- D15 CONFIRMED. models.md 4.1 records boots at 1.375/1.5/1.75/2 GHz. pll.md section title and intro corrected.
- D16 CONFIRMED. `lmx2582.rs:674` test selects /4 for 1.2 GHz (VCO 4800, N 24, integer). Both pll.md statements scoped
  to the listed frequencies.
- D17 PARTLY. The table is the stock family table; OOS has run it on DHO804 and DHO924S only (models.md 2.1). Reworded
  without the auditor's extra claims.
- D18 CONFIRMED. `boot/mod.rs:182` defaults to `sim::UNIT_IDELAY`. adc-spu-init.md corrected.
- D19 CONFIRMED. `boot/wnd.rs:73` writes none of the 14 delays of the call on a failure, and `boot/mod.rs:713` loads the
  full fallback. cold-boot.md 3.4 corrected.
- D20 CONFIRMED. `boot/mod.rs:508` calls `sync_seq` (with `spu_adc_sync`, `:436`) in core alignment. cold-clock.md 2.5
  corrected.
- D21 CONFIRMED as an unresolved conflict. No stock module evidence in the tree. Both cold-clock.md 2.3 and afg.md 4.1 now
  name the conflict and say not yet verified. Unit check U4.
- D22 PARTLY. The line describes stock. OOS uploads nothing at start; `afg::idle` (`afg.rs:663`). afg.md section 1 now
  gives both.
- D23 PARTLY. A ramp table does not depend on frequency (`afg.rs:385`), so the phrase was misleading; pulse was missing.
  afg.md 14 reworded.
- D24 CONFIRMED. models.md section 4 records square, ramp and pulse on the unit. afg.md scope line and section 15 mark
  corrected.
- D25 PARTLY. "keep previous values" is stock; OOS `gear` uses g1=7, g2=2, gain 1 (`afg.rs:127`). afg.md 5.1 gives both.
- D26 CONFIRMED. The doc's own line 21 says not measured. afe-offset.md now says assumed, not measured.
- D27 CONFIRMED. No test sets `cancelled` to true (`selfcal.rs:26`, model at `:1040`). self-cal.md 14.13 corrected.
- D28 CONFIRMED. `ui/src/ui/selfcal.rs:11,47,166` show an advisory uptime warning. self-cal.md 9 corrected.
- D29 CONFIRMED. `selfcal.rs:159-182` saves three files with early returns; each atomic, the set not. self-cal.md 2.2
  and 11.1 corrected.
- D30 CONFIRMED. DHO800 data sheet (offset range rows): +-0.5 V below 500 uV/div, +-1 V from 500 uV/div to 65 mV/div.
  afe-offset.md corrected.
- D31 CONFIRMED. `afe.rs:136` quantizes up to the next key. afe-settings.md 1.3 reworded.
- D32 CONFIRMED. `selfcal.rs:827-832` copies the base payload and indexes group 3 without extending it. self-cal.md 7.2
  note added. Code finding C1.

S3 applied (short and clearly right): D33 (units in afe-offset.md), D34 (container vs payload prefix in afg.md), D35
(rounding and clamp in cold-boot.md 3.10), D36 (min frequency per layer, stock vs OOS defaults in afg.md 3 and 7), D37
(clock note in afg.md, window at 1.25 GHz in pll.md and cold-boot.md row 12), D38 (first 1 -> 0 edge in
adc-spu-init.md).

Cross-part fixes applied: self-cal.md 2.4 (2^e), self-cal.md 14.6/14.9/14.10 (register step vs ADC LSB), cold-boot.md
3.9 (0x10a0 stock only; no write of 0x10a0 in hal, ui, core, fpgad or tools).

## Code findings

- C1 medium, `hal/src/dho/selfcal.rs:827-832`: `vertical_payload` indexes `p[o..o + 56]` for group 3 on a base payload
  that may have 12 blocks, which panics (slice out of range) in the save. `afe::Cal` accepts 12-block files, so the run
  completes and then panics at save time.
- C2 low, `hal/src/dho/selfcal.rs:381`: the ADC core offset loop corrects by (mean - 32768)/16 register steps per round,
  i.e. assumes 1 step = 1 ADC LSB; normal-mode unit data give about 0.6. It still contracts (about 40 % per round) but
  the model at `:1078` pins the 1:1 slope, and there is no settle between the offset write and the measure run, which
  round 11 identified as a cause of oscillation in `coretrim`. Path is host-tested only.
- C3 low, `hal/src/dho/selfcal.rs:21`: the `adc_mean` doc comment says 65536 samples and two cores averaged; the code uses
  e=15 (`adc.rs:306`) and `Unit::adc_mean` averages all 8 slots.
- C4 low, `fpgad/src/main.rs:833,845`: comment and error text say no other PLL table has run on a unit; models.md 4.1
  records four.
- C5 low, `ui/src/ui/selfcal.rs:156`: All builds `0..NCH` (compile-time 4) regardless of the model's channel count. On a
  2-channel model it requests CH3/CH4. Whether the board side refuses that is not checked (assumed not).
- C6 low, `hal/src/dho/selfcal.rs:159-182`: `save` is not transactional across its three files; a late error leaves a
  mixed calibration set and the UI reports a failed run (`ui/src/board/dho/hw.rs:1591`).

## Unit checks

- U1 (D09) Measure-unit run length. With acquisition stopped by the owning process (no hand writes while oos-ui runs),
  start 0x1040 with e = 12, 15, 18 and time start to bit 30. Expected 2^e / 156.25 MHz: 26 us, 0.21 ms, 1.68 ms. Double
  those means 2^(e+1).
- U2 (D07) Core offset slope in core-order mode (0x1004 bit 0 = 1) and at ADC x2. Nudge one core by +16 register steps,
  settle, read 4 runs of 0x1044 per slot, restore. Expected about 9-11 codes per step (as normal mode); 16 would match the
  self-cal coefficient.
- U3 (D03) Normal-mode slot grouping beyond CH1/CH2: a DC level on CH3 then CH4 in four-channel mode, and on each channel
  of a two-channel set; read slot means (`oos-fpgad adc slots`). Expected CH3 4/6, CH4 5/7; two channels 0..3 / 4..7.
- U4 (D21) Stock GPIO126 direction right after `afg_gpio` loads, before its first write (debugfs gpio, or the module
  disassembly). Expected one of: input (afg.md) or output low (cold-clock.md).
- U5 (D26) Stock offset sweep. Same CH3 scale/offset points as round 11, stock firmware, same warm-up and input. Expected:
  about -9 mV/V if stock shares the error.
- U6 (D13) Two-channel model: board/model survey, then self-cal applicability (index-3 AFE zero, EXT). Needs a unit of
  that variant.
- U7 (D17) PLL table on the untested variants: nominal bring-up, 0x1210 in 62494..62506, eye scan, clean known sine.
- U8 (D18) Eye-scan fallback effectiveness: already open in cold-boot.md row 12 ("fallback not yet").
- U9 (D04) OOS generator: 10 mVpp sine at +1.9 V offset, then +2.1 V. Expected the same measured amplitude on both sides
  of the gear change (full start).

## Files edited

docs/protocols/afe-settings.md, docs/protocols/self-cal.md, docs/protocols/pll.md, docs/protocols/cold-clock.md,
docs/protocols/cold-boot.md, docs/protocols/adc-spu-init.md, docs/protocols/afg.md, docs/fork/afe-offset.md.
docs/protocols/afe-spi.md had no S1/S2 finding and is unchanged.
