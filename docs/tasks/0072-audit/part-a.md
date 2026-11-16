# Audit part a: second check

Input: `target/audit-a.md` (revision a451a3fa). Every S1 and S2 finding was checked against the cited doc lines, the
code and the unit logs (`docs/fork/unit-dho804.md`, `docs/fork/hwmask.md`, `docs/fork/flicker.md`). Corrections were
written from the code, not from the auditor's text. The upstream fix 4d43d3fa (TPU tick units, 0x10e4 unit) is
applied in `docs/hardware/fpga.md`; its "10 points per channel stream" wording was replaced per A02.

## Verdicts

- A01 CONFIRMED. `fpga-image.md` offered 0x800000 as spare; 2.5 of the same page and `FPGA_SLOTS` (`hal/src/dho/boot/regs.rs:323-326`) say it holds the self-test image. No free slot is established now.
- A02 CONFIRMED. `mem_size(memsel * ratio)` (`hal/src/dho/hz.rs:125,237`) counts bursts of 10 words over all channels together. Also applies to the 4d43d3fa wording.
- A03 CONFIRMED. Rounds 11 and 12: one register step moves a slot by about 0.5 to 0.62 ADC step (9.0-10.6 codes of 16). `coretrim::order` measures it.
- A04 CONFIRMED. `compressed_len` (`hal/src/dho/seq.rs:963`) asks floor(memsel/k/8)*8 points; ceil overruns the window (3912-point time-out).
- A05 CONFIRMED. Pairs are in time order (unit round 2, `unit-dho804.md:281`); "alternating low/high" removed.
- A06 CONFIRMED. `up_with_boot_retries` (`hal/src/dho/boot/io.rs:130-145`) repeats BOOT to the same slot at most twice; no fall-back to slot 0. Plan section rewritten as the current start-up.
- A07 CONFIRMED. `fpgad/src/main.rs:891-895` checks only `/sys/module/xdma`, `--force` skips it; `fpga_boot` has no check; only `pcie_down` (`:761-774`) checks both.
- A08 CONFIRMED. `la.md`: threshold sweep had no effect, under stock as well. Row now separates 0x12dc and the zero codes from threshold control.
- A09 CONFIRMED. No unit record of a force pulse in `unit-dho804.md` or the trigger docs. The bullet already said so in this revision; the register-table row did not.
- A10 CONFIRMED. The per-core rate is assumed (the Clocks table says so); the overview row no longer marks it verified.
- A11 CONFIRMED. `hwmask.md` round 20: state 5 survives a reconfiguration; the guard runs one batch without the unit. Stop alone is not a known recovery.
- A12 CONFIRMED. Round 5 (`unit-dho804.md:322`): k = 256 at 1 M without time-out after the TX-length fix. Updated in `fpga.md` and `compress.md`.
- A13 CONFIRMED. Parser default off (`ui/src/compress.rs:49-52`), unit image on (`rootfs/unit/dho804/system.conf:29`).
- A14 CONFIRMED. `compress::allowed` starts with `!m.dots_at(0.0)`; the dots-mode mixed display described the pre-fix state.
- A15 CONFIRMED. `framestats::needs_samples` excludes mean-type values on envelopes; `deepmeas` replays raw.
- A16 CONFIRMED. Rounds 13-16 ran on the unit in CH3-alone mode. Status now says verified for that mode only.
- A17 CONFIRMED. `ROUNDS = 12` (`hal/src/dho/coretrim.rs:48`); "up to 8 rounds" removed.
- A18 CONFIRMED. `models.md` 2.1 has variants with no run. Opening line now says "supports", with runs only as listed.
- A19 CONFIRMED. No EXT trigger source exists in `ui/src`; the column is the fitted input. Renamed "EXT input".
- A20 CONFIRMED. `ui/src/ui/la.rs` and `la.md` (probe run on a DHO924S). "Not implemented" replaced.
- A21 CONFIRMED. Section 4.1 of the same page records 1.5 and 2 GHz runs on one DHO924S.
- A22 PARTLY. The hdcode pins contradict `board.md` 7 (module not loaded); fixed. The auditor's wider claim (the table may not be one snapshot) has no evidence either way; the verified mark for the other pins stays. Unit check 4.
- A23 CONFIRMED. (size>>16)+1 is exact for an aligned addr unless size is a multiple of 64 KiB; for an unaligned addr it can be one sector short. The auditor's text missed the short case.
- A24 CONFIRMED. `switches.rs:51`: on unless `off`; verified 2026-11-13 on the same page.
- A25 PARTLY. "Shorter exponent" is wrong (18 against 15) and fixed in `trigger-lockin.md`. `fpga.md` already marks the run length assumed; no change there. The 2^e versus 65536-at-e=15 conflict sits in `self-cal.md` (outside part a). Unit check 6.
- A26 CONFIRMED. Single-channel timing 0.58-0.74 s and the six-case slot check (`unit-dho804.md`, round 8) are on the unit; multi-channel timing and the 5 V case are not.
- A27 PARTLY. Not a contradiction: RUN/STOP is one key, and the eye check saw it green with bit 16 (`panel.rs` test). Bit 17 alone was never observed, so STOP red is now marked assumed. Unit check 5.
- A28 CONFIRMED. `hz::prep_nominal`: 1 M at 2 ms/div gives 50 MSa/s, remapped to 31.25 MSa/s (`modify_rate`), 250 samples at 125 kHz. The multi-channel figure on the same page (4.2 MSa/s, 33 samples) was also wrong: 100 k gives 5 MSa/s, remapped to 2.5 MSa/s, 20 samples.
- A29 CONFIRMED. Header word 4 is a frame/chunk counter (`trigger.md`, `roll.rs:291-294`). Text now says no field identifies the settings.
- A30 CONFIRMED. `pcie_up` (`fpgad/src/main.rs:717-725`): six attempts, 20 ms poll for 2 s, unload, 2 s. No 500 ms / 15 s. `fpga.md` wording made precise too.
- A31 (S3) applied. Front-end and generator rows scoped to channel count and S models.
- A32 (S3) applied. Slot index s, field 1 << s (`hal/src/dho/mask.rs:12-14`).
- A33 (S3) applied. `avg_count_reg` writes max(N-1, 1).
- A34 (S3) applied. Cap is half the DDS clock in effect (`hal/src/model.rs:305`).
- A35 (S3) applied. `QUIET_RANGE` compares raw 12-bit codes after the x2 of 0x105c.

## Code findings (not changed)

- Low. `fpgad/src/main.rs:891-895`: the BOOT guard of `cold-boot` / `fpga-boot` checks only xdma, not `pcierockchiphost`, so BOOT can go out behind a bound host driver without xdma.
- Low. `ui/src/ui/autoset.rs:19-22,25-26`: comments give 41.7 MSa/s, 0.2 us and 4.2 MSa/s / 33 samples; `hz::prep` makes 31.25 MSa/s and 2.5 MSa/s (20 samples at 125 kHz).
- Low. `hal/src/dho/coretrim.rs:24`: `set_offset` doc says "-1024..1023 ADC steps"; these are register steps of about half an ADC step.
- Low. `hal/src/dho/boot/io.rs:18`: "Only the three image slots are accepted"; `boot_message` accepts two.

## Changes needed outside part a

- `docs/protocols/self-cal.md:124-125`: e = 15 described as 65536 samples (2^(e+1)), conflicting with 2^e in `levels.rs` and `fpga.md`. Mark the count assumed until unit check 6.
- `docs/protocols/self-cal.md:474,516`: one ADC LSB per offset register step is superseded by rounds 11/12 (about half).
- `docs/protocols/horizontal-config.md` 1.4 says an average-mode burst is 32 points; `hal/src/dho/hz.rs:138` calls `mem_size_avg` "10-point bursts". Not settled here.

## Unit checks

1. LA threshold (A08): DHO924S with a PLA2216, D0 at a measured DC level (for example 1.0 V). Sweep 0x12d4 [15:0] through codes for 0.5 to 2 V, read 0x12dc bit 0 at each step. Expected if the threshold acts: bit 0 flips where the threshold crosses the input. Repeat with a second probe to separate probe from port.
2. Force pulse (A09): oos-ui, NORMAL sweep, edge level above the signal (WAIT). Press Force once. Expected: one frame, 0x4034 bit 7 without bit 6, then WAIT again; with the level back inside the signal it triggers normally (mux kept). Do not read 0x205c.
3. Mask stall recovery (A11): in an authorized stall experiment, after the stall log 0x4030[3:0] after a stop sequence alone, then after one batch with the unit off. Expected per round 20: state 5 after the stop, cleared after the batch.
4. Stock GPIO claims (A22): under stock, in one shell session, `cat /sys/kernel/debug/gpio` and `lsmod`. Expected: every pin of `gpio-map.md` claimed except GPIO 4, 8, 11, 12 (hdcode not loaded).
5. STOP LED colour (A27): oos-ui stopped, front-panel frame cmd 3 with data 0x02 (bit 17 only), then 0x01 (bit 16 only). Record each colour by eye. Expected: red, green.
6. Measure-unit run length (A25, A10): oos-ui stopped, time start-to-done of 0x1040 for e = 12, 14, 16, 18 (poll bit 30). Expected: duration doubles per step; at 156.25 MHz 2^e samples gives 26 us, 105 us, 0.42 ms, 1.68 ms; double those means 2^(e+1).
7. Autoset band (A28): CH1 alone, generator sine at 125 Hz, 1 kHz, 10 kHz and 125 kHz, `:AUToscale`. Expected: correct period each time; the deep record's metadata shows 31.25 MSa/s and 1 M points.
8. Core trim in other modes (A16): `cal.core_offset = tight`, input open, CH1+CH2 and CH1..CH4. Expected: log line per mode with a result within 1.5 steps, or a refusal; cal_adc.hex sha256 unchanged.
9. Hardware variants (A18, A19, A31): a first boot on a two-channel DHO800, a non-S DHO900 and an EDU unit. Not possible on our unit; needs other hardware.

## Files edited

`docs/hardware/fpga.md`, `docs/hardware/gpio-map.md`, `docs/models.md`, `docs/protocols/fpga-boot.md`,
`docs/protocols/fpga-image.md`, `docs/protocols/frontpanel.md`, `docs/fork/core-offset.md`,
`docs/fork/trigger-lockin.md`, `docs/fork/compress.md`. Not edited: `docs/hardware/README.md`, `docs/protocols/board.md`
(no finding needed a change).
