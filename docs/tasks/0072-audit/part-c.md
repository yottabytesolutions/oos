# Audit part c, second check

Checked against a451a3f. Each S1/S2 finding re-derived from the cited doc lines, code, tests and unit logs.
No build, no hardware access.

## Verdicts

- C01 CONFIRMED. hal/src/dho/acq.rs:95-100 sets 0x1004 bit 2 for average OR rate < max/8; test acq.rs average_mode_writes
  asserts 0xc. unit-dho804.md:313: zero frames from 156.25 MSa/s without bit 2. Fixed horizontal-config.md 3 table.
- C02 CONFIRMED. seq.rs compressed_len = max(8, floor(memsel/k/8)*8), k <= memsel/8. unit-dho804.md:281 (timeout at
  k=256, rounded up) and :322 (pass). acquisition-start.md compression section corrected. horizontal-config.md 3.1
  roundup8(len) left as is: it is the stock screen-window TX length, not a whole-record read; a note in
  acquisition-start.md draws the line.
- C03 PARTLY. The stride rule (seq.rs frame_stride/read_len/unpad) is right. But the stock live-loop formulas are
  correct when len is the programmed 0x11f0 = roundup8(len): 16 + 2*ratio*len is then a multiple of 8. The real defect
  is the OOS window variant (acquisition-start.md variant 2, "2 x ratio x 0x11f0" with arbitrary 0x11f0). Fixed: len
  defined as the 0x11f0 value in acquisition-start.md and record-play.md 1.2/1.5/1.6; padded read for the window variant;
  OOS window row added to record-play.md 1.6.
- C04 CONFIRMED. acq.rs:63-66 SA_DELAY_FS = 800,000; tests pin 8 ns -> 10 / 5 steps; cold-boot.md 3.10 measured.
  Fixed horizontal-config.md 4.1 step 10 and 4.2 step 10.
- C05 CONFIRMED. scu.rs:75-106 status bits and test status_tells_tpu_trigger_from_forced; trigger-lockin.md:186-187
  says the old names are reversed. Fixed acquisition-start.md 0x4034 section.
- C06 CONFIRMED. scu.rs record_run/readout_play4 write first + n - 1 to 0x4018. Fixed horizontal-config.md section 6.
- C07 CONFIRMED. seq.rs deinterleave and roll.rs test three_channels_use_the_four_channel_slots keep physical slots.
  No unit evidence for 3 channels found, so the fix keeps "not yet verified on hardware" (U1).
- C08 CONFIRMED. la.rs prep halves only with analog_shown. Fixed horizontal-config.md 1.2.
- C09 CONFIRMED. la.rs config: 0x10e4 = 2*memsel (memsel in roll), 0x4050/0x4054 doubled in both. Fixed
  horizontal-config.md 0 and 1.6, roll-mode.md 3.1.
- C10 CONFIRMED. 216-byte chunk = 27 eight-byte units; roll.rs chunks = 4n/108; unit fill 270 for 10 chunks. Fixed
  roll-mode.md 2.2.
- C11 CONFIRMED. hz.rs mem_size_avg: >= 25 M points rounds to (n/2048+1)*2048, cap AVG_SEG_CAP = 7,864,320; tests at
  hz.rs average_segment_layout. Fixed horizontal-config.md 1.4; the deep average layout marked host-tested only (U6).
- C12 CONFIRMED. roll.rs config writes u32::MAX*den/num with f = num/den = expansion. 6250 -> 7000 needs 28/25;
  floor((2^32-1)*25/28) = 0xe4924923. Fixed horizontal-config.md 4.2, still "assumed" (U2).
- C13 PARTLY. The inconsistency is real: roll-mode.md 9 called tdiv/10 "one column" (a column is tdiv/100), and the
  stock rule in section 5 differs in threshold and direction. Nothing settles which the hardware needs. Rewrote both
  places to state code and stock rule exactly, marked not yet verified (U3). Code finding K3.
- C14 PARTLY. The stock frame request plays count = 0x4010 - 0x4014 + 1, the observed count, never the requested n; it
  is not wrong. The summary did not say that bit30 is first-frame-ready only. Added that sentence and OOS's wait.
- C15 CONFIRMED. One arm records n frames (scu.rs arm/record_run; acquisition-start.md batches on the unit). Fixed
  acquisition-start.md intro and run-sequence heading.
- C16 CONFIRMED. acquisition-start.md:20 and :117 contradicted each other; roll.rs stop and the STOP final replay add
  writes. Fixed :117.
- C17 PARTLY. adc.rs: r09 = 0x5020 is the kept value, the wire word is 0x7700; boot regs.rs:26 also writes 0x01097700.
  The "verified on the unit" mark on chip register values has no readback evidence in docs. 0x1058 kept as verified
  (separately reported). Rewrote adc-mode.md 2.2 and 5.4 as encoded write values (U4).
- C18 CONFIRMED. boot/cal.rs load_from passes OOS_CAL_DIR first; adc.rs load does the same. Fixed adc-mode.md 3.
- C19 CONFIRMED. No 0x1098/0x109c/0x10a0 write anywhere in hal/ui/core source (grep). Fixed adc-mode.md 4.1 and
  horizontal-config.md 4.1 step 47 (stock only).
- C20 CONFIRMED. specs/02-hires-fir.md 1.3 ends with 0x1098[1:0] = 0 (run). adc-mode.md 4.2 now gives the full
  sequence as a stock reconstruction, not yet verified on hardware. No unit check listed: OOS does not use the FIR.
- C21 CONFIRMED. hw.rs deep_raw_settings/window_with: screen + 4096 each side, trigger +-4096, 8-aligned, whole record
  fallback; budget over ratio lanes. Fixed deep-meas.md design, constants, host tests and step 5.
- C22 CONFIRMED. deepmeas.rs enabled_from: on unless "off"; test asserts "on by default". Fixed deep-meas.md
  on-unit baseline and the "Two lanes" paragraph.
- C23 CONFIRMED. framestats.rs fold_frame skips live pairs, on envelope frames skips only needs_samples kinds. Fixed
  deep-meas.md measured-counter paragraph.
- C24 CONFIRMED. compress.md evidence (round 2 replay) and unit-dho804.md:298 (round 3, 4 of 4) supersede "not shown"
  and "host only". Trigger alignment stays unverified (U7). Fixed evidence table and risks.
- C25 CONFIRMED. deep-meas.md "Cost on the unit" measured 11.41 -> 9.0/s. Fixed evidence row and step 3.
- C26 CONFIRMED. ui.rs stop_wants_final includes compress::live; hw.rs final_replay replays raw_newest without deep;
  prearm_keeps_replay excludes raw_newest batches. Fixed stop-exact.md header and three bullets.
- C27 PARTLY. roll-mode.md 2.4 is a stock reconstruction; section 10 already called it the stock buffer. The summary
  item 3 presented it unqualified. Labelled 2.4 and item 3 as stock, added the OOS rule from hw.rs (keep =
  round(10*tdiv/dt), pop_front).
- C28 CONFIRMED. hw.rs ROLL_TDIV = SCAN_TDIV = 0.2; roll-mode.md:52 claimed slow_mode follows the 50 ms table. Fixed
  roll-mode.md 0.6, 1.2 and horizontal-config.md 1.3. Code finding K1.
- C29 CONFIRMED. defaults.rs dots: DotsMode::Auto. Fixed zoomout.md (historical statement plus current default).
- C30 CONFIRMED. calfile.rs load_first takes the first CRC-valid container; adc.rs then fails on a size other than
  0x738 without trying the next location. Fixed adc-mode.md 5.1. Code finding K2.
- C31 CONFIRMED. unit-dho804.md:298 (equal at 2.3 s, not at 0.8 s); ui.rs STOP_FINAL_WAIT = 3 s; wave.rs hw_hold ends
  on final_ack and empty queue. Fixed stop-exact.md step 3 and the final-frame bullet.

S3 applied: C32 (one sentence in acquisition-start.md conventions: 6.4/1.6 ns ticks at the nominal clock; tpu.rs
converts via clock). C35 only for record-play.md 1.2 (RMS branch is stock, OOS has none). C33 and C34 not applied:
correct but not short, and they touch tables shared with other parts.

Counts: CONFIRMED 26 (C01 C02 C04-C12 C15 C16 C18-C26 C28-C31), PARTLY 5 (C03 C13 C14 C17 C27), REJECTED 0.

## Code findings

- K1 low. ui/src/board/dho/hw.rs:32: the doc comment of slow_mode says "roll from 50 ms/div"; ROLL_TDIV at :28 is
  200 ms/div. Stale comment.
- K2 medium. hal/src/dho/adc.rs:61-67: CalAdc::load stops at the first CRC-valid cal_adc.hex and returns InvalidData on
  a payload size other than 0x738, without trying /rigol/data or /rigol/data/default. Cold boot (boot/cal.rs:94-101)
  checks the size per location and goes on. A bad-size /oos/cal file blocks every runtime ADC mode switch while boot
  still works.
- K3 low, needs U3 first. hal/src/dho/roll.rs:509-518: scan_trig_pos enables 0x4058 from delay - 5 tdiv >= tdiv/10 and
  its comment calls tdiv/10 "one column"; a column is tdiv/100, and the stock rule reconstructed in roll-mode.md 5
  enables from > -tdiv/100. Default delay 0 gives 0 under both, so only delayed scans differ.

## Unit checks

- U1 three-channel slot layout. Masks 7, B, D, E, distinct signals per channel, OOS raw read (pass-through + window).
  Expect word 4i+k = CH(k+1) for enabled channels; record what the disabled channel's slot carries.
- U2 linear interpolation fraction. Two channels, 1 us/div, display chain with 0x1154 bit0 = 0, 0x115c = 0xe4924923,
  0x11f0 = 7000, 0x1170 = 0x40000001. Compare sample positions with a raw read of the same record; expect 7000 points
  per 6250 samples without a C2H timeout.
- U3 scan 0x4058. 200 ms/div, Auto Roll off, delays putting delay - 5 tdiv just below and above -tdiv/100, 0, tdiv/100
  and tdiv/10; write 0 and the computed value. Read 0x1204 position, 0x40d0 done and the streamed edge position. Settles
  K3.
- U4 ADC register readback. After mode-0 set-up with known cal: ADC SPI read of regs 0x09, 0x04, 0x05, 0x01 through
  0x1034 (0x0180xxxx read word, as the trim poll does) and 0x1038. Expect either the written data (0x7700, 0x8080,
  0x8080, 0x0000) or the kept values (0x5020, 0, 0, 0); record which.
- U5 2-channel stream padding. Ratio 2, OOS window with an odd L, batch of several frames. Expect a stride of
  roundup8(4L) bytes (acquisition-start.md C2H rule 1 marks this assumed).
- U6 average layout at deep records. Average mode, total points around 25,000,000 (below and above), a distinctive
  delayed event. Expect 0x10e0 per hz::mem_size_avg, no wrap, nonzero live ring. Owner approval needed.
- U7 deep raw replay alignment. 10 M, 1 ms/div, isolated edge; read pairs and the raw window of the same frame. Expect
  the edge at the same time in both; repeat at another depth, two channels and a nonzero delay.

## Files edited

docs/protocols/acquisition-start.md, docs/protocols/horizontal-config.md, docs/protocols/roll-mode.md,
docs/protocols/record-play.md, docs/protocols/adc-mode.md, docs/fork/deep-meas.md, docs/fork/stop-exact.md,
docs/fork/zoomout.md. docs/fork/upstream-wpu-lanes.md unchanged (no finding).

## For other parts

- docs/protocols/cold-boot.md 3.9 lists 0x10a0 = 16 in the horizontal block; same stock-only qualifier as C19 applies.
- docs/fork/specs/02-hires-fir.md:79 cites "adc-mode.md 190"; the sentence moved slightly but still sits in 4.1.
