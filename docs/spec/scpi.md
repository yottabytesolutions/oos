# Legacy SCPI reference

Source baseline: `overhaul` c6f1f98, 2026-11-16. This is a source-derived contract
for the M15 rebuild, not a claim of hardware validation. No unit was accessed.
[ADR 0017](../architecture/adr/0017-hybrid-after-m12.md) requires the legacy parser
as a host differential oracle. Dispositions are **keep** by default; **change**
for the cited defects, with coordinator decisions and remaining proposals
labelled.
No command is proposed for removal. The 162 header-path rows (153 canonical/alias
and nine query-effect aliases) differ from the approximately 258 literal tokens
because there are 101 distinct header nodes, argument and reply literals live in
catalogues, and numeric suffixes are collapsed; the token estimate is not a
coverage target.

Mixed case encodes both spellings: `:CHANnel<n>:SCALe` means long
`:CHANNEL<n>:SCALE`, short `:CHAN<n>:SCAL`. Each node independently accepts its
long or short spelling, in any letter case. `[?]` is represented by kind **both**;
**C** means command, **Q** query. A both row covers the setter and its query.
Finite alias headers have their own rows; arbitrary ignored-node aliases use
the acceptance predicates below. `<n>` expands a numeric suffix, not a new
requirement per physical channel. Parameter-value catalogues follow the headers.
Evidence uses repository-relative `file:line` anchors at this baseline, except
explicitly labelled references to the later empty-reply fix.

Table conventions apply to every row:

- **Command/Query mapping** names the future typed boundary, separately from the
  current model/action column. `C: tbd at M14`, `Q: tbd at M14` or
  `C/Q: tbd at M14` means the command,
  query or both variants still need names at M14; it does not assert a current
  typed parser mapping. M15 must fill those entries before parser acceptance
  (`docs/architecture/extending.md`, Add an SCPI command).
- **Errors** for every header row follow scpi-320's range/override map. Successful
  rows add no error unless that map or the row explicitly says otherwise.
- **T/V/M**: TCP SCPI (default 5555), VXI-11 and MCP `scpi` tool. All call the
  same parser; MCP is a text wrapper and does not preserve arbitrary binary
  replies. See transport rules below.
- **B**: `ON`/`1`, `OFF`/`0`, reply `1` or `0`. **F**: finite Rust `f32` decimal
  or exponent without a unit suffix. **D**: finite `f64`. **E**: seven significant
  digit exponential reply, e.g. `2.000000E-04`; unavailable/nonfinite `9.9E37`.
- **none**: no reply for a command. Query errors follow the error rule below.
  Unless specified, query parameters are ignored. Settings have no independent
  SCPI defaults: they use the current model/setup. Parser-owned initial defaults
  are waveform source CH1, BYTE format, NORM mode, empty error queue
  (`ui/src/scpi.rs:245`).
- Normal setters mutate the legacy model and the intake/change bridge applies
  normalization; this can invalidate records, restart hardware, or clear derived
  state. `ui/src/scpi.rs:337`, `ui/src/ui/change.rs:152` and
  `ui/src/ui/change/acquisition.rs:32` locate that boundary. Rows name extra effects.
  No setter implies waiting for hardware completion. The early model dispatch
  even calls `mark_changed` for successful queries (`ui/src/scpi.rs:780`).
- **Subset** means covered by the file's Rigol-compatible subset claim, not
  verified against stock firmware. **OOS** and **fork debug** mean extensions
  identified in code. Availability is compile-time NCH (4) unless a row says
  installed channels, generator or LA capability. `ui/src/board/dho/mod.rs:77`,
  `ui/src/model.rs:21` and `ui/src/scpi.rs:181` distinguish these limits.


## Common commands and acquisition actions

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-1 | `*IDN` | both | none | OOS,model,serial-or-0,firmware | C/Q: tbd at M14 | platform + cached firmware; accepts without ? too | T/V/M | Subset; keep | `ui/src/scpi.rs:397` |
| scpi-2 | `*OPC` | both | none | Q: 1; C: none | C/Q: tbd at M14 | Immediate parser completion only; no acquisition barrier | T/V/M | Subset; keep | `ui/src/scpi.rs:406` |
| scpi-3 | `*CLS` | C | none | none | C: tbd at M14 | Clear shared SCPI error queue | T/V/M | Subset; keep | `ui/src/scpi.rs:407` |
| scpi-4 | `*RST` | C | none | none | C: tbd at M14 | Front-panel DEFAULT key; reset setup | T/V/M | Subset; keep | `ui/src/scpi.rs:411` |
| scpi-5 | `:AUToscale` | both | none | Q: B | C/Q: tbd at M14 | AUTO key starts asynchronous autoset; Q reads auto_running | T/V/M | Subset; keep | `ui/src/scpi.rs:425` |
| scpi-6 | `:CLEar` | C | none | none | C: tbd at M14 | CLEAR key clears picture/history according to UI action | T/V/M | Subset; keep | `ui/src/scpi.rs:432` |
| scpi-7 | `:TFORce` | C | none | none | C: tbd at M14 | force_trigger; starts RUN if stopped, requests one FPGA force pulse | T/V/M | Subset; keep | `ui/src/scpi.rs:436` |
| scpi-8 | `:RUN` | C | none | none | C: tbd at M14 | run_request when stopped; run=Run; Single sweep becomes Auto | T/V/M | Subset; keep | `ui/src/scpi/actions.rs:7` |
| scpi-9 | `:STOP` | C | none | none | C: tbd at M14 | run=Stop | T/V/M | Subset; keep | `ui/src/scpi/actions.rs:15` |
| scpi-10 | `:SINGle` | C | none | none | C: tbd at M14 | change::single; trig_mode=Single, run=Run; asynchronous acquisition | T/V/M | Subset; keep | `ui/src/scpi/actions.rs:17` |

## Channels and time base

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-11 | `:CHANnel<n>:DISPlay` | both | n=1..4; B | B | C/Q: tbd at M14 | ch[n-1].on | T/V/M | Subset; keep | `ui/src/scpi.rs:454` |
| scpi-12 | `:CHANnel<n>:SCALe` | both | n=1..4; F > 0 V/div at probe; nearest log-distance ladder 100 uV..10 V/div BNC, times probe; fine rounds up calibrated row | E V/div | C/Q: tbd at M14 | ch.vscale,vfine; divides request by probe | T/V/M | Subset; keep | `ui/src/scpi.rs:459` |
| scpi-13 | `:CHANnel<n>:VERNier` | both | n=1..4; B | B | C/Q: tbd at M14 | m.fine_v; global for all channels | T/V/M | OOS global switch unlike per-channel name; keep | `ui/src/scpi.rs:465` |
| scpi-14 | `:CHANnel<n>:OFFSet` | both | n=1..4; F V, no parser magnitude limit | E V | C/Q: tbd at M14 | ch.offset | T/V/M | Subset; keep | `ui/src/scpi.rs:471` |
| scpi-15 | `:CHANnel<n>:COUPling` | both | n=1..4; DC, AC, GND; DC50 only when board offers it | DC / AC / GND / DC50 | C/Q: tbd at M14 | ch.coupling | T/V/M | DC50 capability extension; keep | `ui/src/scpi.rs:476` |
| scpi-16 | `:CHANnel<n>:BWLimit` | both | n=1..4; OFF/FULL, AUTO, GND, 20M/20MHZ; BW catalogue below | OFF, AUTO, 20M or uppercase table label without spaces | C/Q: tbd at M14 | ch.bw | T/V/M | Extended filter choices; keep | `ui/src/scpi.rs:487` |
| scpi-17 | `:CHANnel<n>:PROBe` | both | n=1..4; F > 0, dimensionless attenuation, no upper parser limit | E | C/Q: tbd at M14 | ch.probe | T/V/M | Subset; keep | `ui/src/scpi.rs:509` |
| scpi-18 | `:CHANnel<n>:INVert` | both | n=1..4; B | B | C/Q: tbd at M14 | ch.invert | T/V/M | Subset; keep | `ui/src/scpi.rs:514` |
| scpi-19 | `:TIMebase:SCALe` | both | F s/div; nearest log ladder 1 ns..1000 s/div; no positivity check | E s/div | C/Q: tbd at M14 | m.tscale; tfine=1 | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:524` |
| scpi-20 | `:TIMebase:OFFSet` | both | F s, no parser magnitude limit | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:534` |
| scpi-21 | `:TIMebase:POSition` | both | F s, alias of OFFSet | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:534` |
| scpi-22 | `:TIMebase:MAIN:SCALe` | both | F s/div; nearest log ladder 1 ns..1000 s/div; no positivity check | E s/div | C/Q: tbd at M14 | m.tscale; tfine=1 | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:524` |
| scpi-23 | `:TIMebase:MAIN:OFFSet` | both | F s, no parser magnitude limit | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:534` |
| scpi-24 | `:TIMebase:MAIN:POSition` | both | F s, alias of OFFSet | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:534` |

## Trigger and counter

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-25 | `:TRIGger:MODE` | both | Current: any argument ignored; target: Rigol type tokens in scpi-313 | Current Q: EDGE; target Q: actual type in Rigol tokens; C: none | C/Q: tbd at M14 | Current: no trigger-type selection; target: switch type, including protocol selection | T/V/M | Subset stub; change: coordinator decision D2, scpi-313 | `ui/src/scpi.rs:548` |
| scpi-26 | `:TRIGger:STATus` | both | ignored | STOP / TD / AUTO / WAIT | C/Q: tbd at M14 | m.run,status; replies even without ? | T/V/M | Subset; keep | `ui/src/scpi.rs:551` |
| scpi-27 | `:TRIGger:COUPling` | both | DC / AC / LFR / LFREJECT / HFR / HFREJECT | DC / AC / LFR / HFR | C/Q: tbd at M14 | m.trig_coup | T/V/M | Subset; keep | `ui/src/scpi.rs:561` |
| scpi-28 | `:TRIGger:NREJect` | both | B | B | C/Q: tbd at M14 | m.trig_nrej | T/V/M | Subset; keep | `ui/src/scpi.rs:580` |
| scpi-29 | `:TRIGger:HOLDoff` | both | F 0..10 s | E s | C/Q: tbd at M14 | m.holdoff | T/V/M | Subset; keep | `ui/src/scpi.rs:592` |
| scpi-30 | `:TRIGger:SWEep` | both | prefix AUTO / NORM / SING | AUTO / NORM / SING | C/Q: tbd at M14 | m.trig_mode; does not itself request Single rearm | T/V/M | Subset; keep | `ui/src/scpi.rs:599` |
| scpi-31 | `:TRIGger:EDGE:SOURce` | both | CHANnel1..4; D0..D15 only when trig_la_ok | CHANn / Dn | C/Q: tbd at M14 | m.trig_src,trig_la; D names like Rigol | T/V/M | Rigol D sources with LA enabled; keep | `ui/src/scpi.rs:619` |
| scpi-32 | `:TRIGger:EDGE:LEVel` | both | F V, no parser magnitude limit | E V | C/Q: tbd at M14 | m.trig_level | T/V/M | Subset; keep | `ui/src/scpi.rs:637` |
| scpi-33 | `:TRIGger:EDGE:SLOPe` | both | prefix POS / NEG / RFAL | POS / NEG / RFAL | C/Q: tbd at M14 | m.slope | T/V/M | Subset; keep | `ui/src/scpi.rs:642` |
| scpi-34 | `:COUNter:ENABle` | both | B | B | C/Q: tbd at M14 | m.cnt_on | T/V/M | Subset; keep | `ui/src/scpi.rs:667` |
| scpi-35 | `:COUNter:SOURce` | both | CHANnel1..installed-channel-count | CHANn | C/Q: tbd at M14 | m.cnt_src | T/V/M | Subset; keep | `ui/src/scpi.rs:685` |
| scpi-36 | `:COUNter:MODE` | both | prefix FREQ / PER / TOT | FREQ / PER / TOT | C/Q: tbd at M14 | m.cnt_mode | T/V/M | Subset; keep | `ui/src/scpi.rs:687` |
| scpi-37 | `:COUNter:NDIGits` | both | decimal u32 3..6 digits | decimal integer | C/Q: tbd at M14 | m.cnt_digits | T/V/M | Subset; keep | `ui/src/scpi.rs:689` |
| scpi-38 | `:COUNter:CURRent` | Q | none | E Hz / s / count according to mode; 9.9E37 unavailable | Q: tbd at M14 | ui.counter_value() | T/V/M | Subset; keep | `ui/src/scpi.rs:786` |
| scpi-39 | `:COUNter:TOTalize:CLEar` | C | none | none | C: tbd at M14 | ui.counter_reset() | T/V/M | Subset; keep | `ui/src/scpi.rs:789` |

## Protocol triggers

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-40 | `:TRIGger:RS232:DATA` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.trig_bus_val; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:49` |
| scpi-41 | `:TRIGger:RS232:RELation` | both | EQ/EQUAL, NE/NEQUAL, GT/GREATER, LT/LESS, IN, OUT | EQ / NE / GT / LT / IN / OUT | C/Q: tbd at M14 | m.ser.rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-42 | `:TRIGger:RS232:ARELation` | both | same relations, even on RS232/SPI | relation short form | C/Q: tbd at M14 | m.ser.addr_rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-43 | `:TRIGger:RS232:DMAX` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.ser.max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-44 | `:TRIGger:RS232:AMAX` | both | U, clamp to 0x3ff | decimal u32 | C/Q: tbd at M14 | m.ser.addr_max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-45 | `:TRIGger:RS232:MASK` | both | U up to u64::MAX; stored unmasked, query masks to 40 bits | decimal u64 masked to 0xffffffffff | C/Q: tbd at M14 | m.ser.mask; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:83` |
| scpi-46 | `:TRIGger:SPI:DATA` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.trig_bus_val; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:49` |
| scpi-47 | `:TRIGger:SPI:RELation` | both | EQ/EQUAL, NE/NEQUAL, GT/GREATER, LT/LESS, IN, OUT | EQ / NE / GT / LT / IN / OUT | C/Q: tbd at M14 | m.ser.rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-48 | `:TRIGger:SPI:ARELation` | both | same relations, even on RS232/SPI | relation short form | C/Q: tbd at M14 | m.ser.addr_rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-49 | `:TRIGger:SPI:DMAX` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.ser.max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-50 | `:TRIGger:SPI:AMAX` | both | U, clamp to 0x3ff | decimal u32 | C/Q: tbd at M14 | m.ser.addr_max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-51 | `:TRIGger:SPI:MASK` | both | U up to u64::MAX; stored unmasked, query masks to 40 bits | decimal u64 masked to 0xffffffffff | C/Q: tbd at M14 | m.ser.mask; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:83` |
| scpi-52 | `:TRIGger:IIC:DATA` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.trig_bus_val; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:49` |
| scpi-53 | `:TRIGger:IIC:RELation` | both | EQ/EQUAL, NE/NEQUAL, GT/GREATER, LT/LESS, IN, OUT | EQ / NE / GT / LT / IN / OUT | C/Q: tbd at M14 | m.ser.rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-54 | `:TRIGger:IIC:ARELation` | both | same relations, even on RS232/SPI | relation short form | C/Q: tbd at M14 | m.ser.addr_rel; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:56` |
| scpi-55 | `:TRIGger:IIC:DMAX` | both | U, clamp to 0xffffffffff | decimal u64 | C/Q: tbd at M14 | m.ser.max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-56 | `:TRIGger:IIC:AMAX` | both | U, clamp to 0x3ff | decimal u32 | C/Q: tbd at M14 | m.ser.addr_max; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:69` |
| scpi-57 | `:TRIGger:IIC:MASK` | both | U up to u64::MAX; stored unmasked, query masks to 40 bits | decimal u64 masked to 0xffffffffff | C/Q: tbd at M14 | m.ser.mask; shared across buses | T/V/M | Stock DATA; other common keys fork additions; keep | `ui/src/scpi/ser.rs:83` |
| scpi-58 | `:TRIGger:RS232:WHEN` | both | STAR/START, DATA, ERR/ERROR, CERR/CERROR, EITH/EITHER | STAR / DATA / ERR / CERR / EITH | C/Q: tbd at M14 | m.trig_bus,ser.uart_err | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:91` |
| scpi-59 | `:TRIGger:RS232:POLarity` | both | POS/POSITIVE, NEG/NEGATIVE | POS / NEG | C/Q: tbd at M14 | m.ser.uart_inv | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:110` |
| scpi-60 | `:TRIGger:RS232:STOP` | both | 1 / 1.5 / 2 stop bits | same | C/Q: tbd at M14 | m.ser.uart_stop | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:119` |
| scpi-61 | `:TRIGger:RS232:PARity` | both | NONE / EVEN / ODD | same | C/Q: tbd at M14 | m.dec_parity | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:134` |
| scpi-62 | `:TRIGger:RS232:WIDTh` | both | U 5..8 bits | decimal integer | C/Q: tbd at M14 | m.dec_bits | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:150` |
| scpi-63 | `:TRIGger:RS232:BAUD` | both | U baud, nearest decode::BAUDS value (catalogue below) | decimal baud | C/Q: tbd at M14 | m.dec_baud | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:155` |
| scpi-64 | `:TRIGger:SPI:WHEN` | both | CS / TIM / TIMEOUT | CS / TIM | C/Q: tbd at M14 | m.dec_cs; CS defaults CH3; TIM clears CS, sets timeout to 1 us if <=0 | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:168` |
| scpi-65 | `:TRIGger:SPI:TIMeout` | both | finite f32 0..1 s | Rust lowercase exponent seconds | C/Q: tbd at M14 | m.ser.spi_timeout | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:183` |
| scpi-66 | `:TRIGger:SPI:SLOPe` | both | POS/POSITIVE, NEG/NEGATIVE | POS / NEG | C/Q: tbd at M14 | m.dec_mode=0 / 1 (Q treats mode 3 as POS) | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:188` |
| scpi-67 | `:TRIGger:SPI:MODE` | both | HIGH / LOW | same | C/Q: tbd at M14 | m.ser.spi_cs_high | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:197` |
| scpi-68 | `:TRIGger:SPI:WIDTh` | both | U 4..32 bits | decimal integer | C/Q: tbd at M14 | m.dec_bits | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:206` |
| scpi-69 | `:TRIGger:IIC:ADDRess` | both | U, clamp to 0xffffffffff (same as DATA, not 10 bits) | decimal u64 | C/Q: tbd at M14 | m.trig_bus_val | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:49` |
| scpi-70 | `:TRIGger:IIC:WHEN` | both | STAR/START, ADDR/ADDRESS, DATA, NACK/NACKNOWLEDGE | STAR / ADDR / DATA / NACK | C/Q: tbd at M14 | m.trig_bus; no repeated-start, stop or address+data condition | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:217` |
| scpi-71 | `:TRIGger:IIC:AWIDth` | both | 7 / 10 address bits | same | C/Q: tbd at M14 | m.ser.ten_bit | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:235` |
| scpi-72 | `:TRIGger:IIC:DIRection` | both | READ / WRIT / WRITE / RWR / RWRITE | READ / WRIT / RWR | C/Q: tbd at M14 | m.ser.dir | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:244` |
| scpi-73 | `:TRIGger:IIC:DBYTes` | both | U 0..5 bytes | decimal integer | C/Q: tbd at M14 | m.ser.bytes | T/V/M | Stock names where implemented; fork conditions as listed; keep | `ui/src/scpi/ser.rs:259` |

## Acquisition and segmented records

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-74 | `:ACQuire:MDEPth` | both | AUTO prefix or depth catalogue; decimal/exponent plus K/M, rounded samples | E live depth in samples, not stored menu index | C/Q: tbd at M14 | m.mdepth; Q src.rate_depth(m) | T/V/M | Subset; keep | `ui/src/scpi.rs:696` |
| scpi-75 | `:ACQuire:SRATe` | Q | none | E samples/s | Q: tbd at M14 | src.rate_depth(m) | T/V/M | Subset; keep | `ui/src/scpi.rs:767` |
| scpi-76 | `:ACQuire:TYPE` | both | prefix NORM / PEAK / AVER / HRES | NORM / PEAK / AVER / HRES | C/Q: tbd at M14 | m.acq | T/V/M | Subset; keep | `ui/src/scpi.rs:732` |
| scpi-77 | `:ACQuire:AVERages` | both | decimal u32 in 2,4,8,16,32,64,128,256,512,1024 | decimal count | C/Q: tbd at M14 | m.avg_count | T/V/M | Subset; keep | `ui/src/scpi.rs:755` |
| scpi-78 | `:ACQuire:AVERages:HARDware` | both | B | B | C/Q: tbd at M14 | m.avg_hw | T/V/M | OOS FPGA average switch; keep | `ui/src/scpi.rs:761` |
| scpi-79 | `:ACQuire:SEGMents` | both | OFF/0 or usize, clamp to 100000 segments; 1 is numeric one, ON invalid | decimal count | C/Q: tbd at M14 | m.segments; 0 disables | T/V/M | OOS; keep | `ui/src/scpi.rs:713` |
| scpi-80 | `:ACQuire:SEGMents:HARDware` | both | B | B | C/Q: tbd at M14 | m.seg_hw | T/V/M | OOS; keep | `ui/src/scpi.rs:723` |
| scpi-81 | `:ACQuire:SEGMents:INTerval` | both | F clamped 0..10 s | E s | C/Q: tbd at M14 | m.seg_ivl | T/V/M | OOS; keep | `ui/src/scpi.rs:725` |
| scpi-82 | `:ACQuire:SEGMents:TIMes` | Q | none | comma-separated lowercase-exponent seconds since first trigger; 9.9E37 missing; empty if none | Q: tbd at M14 | ui.seg_times_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:1182` |

## Measurements

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-83 | `:MEASure:ITEM` | both | item[,CHANnel<n>[,CHANnel<m>]]; catalogue below; single source defaults CH1; pair requires distinct channels | E in item units; 9.9E37 unavailable or invalid | C/Q: tbd at M14 | C: meas_scpi_add; no duplicates, oldest evicted when full; Q measures without adding | T/V/M | Subset; keep | `ui/src/scpi.rs:1281` |
| scpi-84 | `:MEASure:CLEar` | C | ALL or ITEM<n>, 1-based current list index | none | C: tbd at M14 | meas_scpi_clear; bad/missing row -224 | T/V/M | Subset; keep | `ui/src/scpi.rs:1295` |
| scpi-85 | `:MEASure:STATistic:DISPlay` | both | B | B | C/Q: tbd at M14 | meas_stats_show / meas_stats_shown | T/V/M | Subset; keep | `ui/src/scpi.rs:1308` |
| scpi-86 | `:MEASure:STATistic:RESet` | C | none | none | C: tbd at M14 | meas_stats_reset; reset statistics | T/V/M | Subset; keep | `ui/src/scpi.rs:1319` |
| scpi-87 | `:MEASure:STATistic:ITEM` | Q | MAXimum / MINimum / CURRent / AVERages / DEViation / CNT,item[,CHANn[,CHANm]] | E in item units; CNT count; 9.9E37 if not listed/unavailable; bad args also -224 | Q: tbd at M14 | ui.meas_stat | T/V/M | Subset; keep | `ui/src/scpi.rs:1323` |

## Waveforms and logic analyser

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-88 | `:WAVeform:SOURce` | both | CHANnel1..4; invalid setter silently ignored | CHANn | C/Q: tbd at M14 | Scpi.wav_src; shared by TCP clients and MCP | T/V/M | Subset; keep | `ui/src/scpi.rs:798` |
| scpi-89 | `:WAVeform:MODE` | both | prefix RAW; anything else selects NORM | RAW / NORM | C/Q: tbd at M14 | Scpi.wav_raw; RAW falls back to screen record | T/V/M | Subset; keep | `ui/src/scpi.rs:807` |
| scpi-90 | `:WAVeform:FORMat` | both | prefix WORD / ASC; anything else BYTE | WORD / ASC / BYTE | C/Q: tbd at M14 | Scpi.wav_fmt | T/V/M | Subset; keep | `ui/src/scpi.rs:814` |
| scpi-91 | `:WAVeform:POINts` | Q | none | decimal count | Q: tbd at M14 | wave_params; raw_info or record_info | T/V/M | Subset; keep | `ui/src/scpi.rs:828` |
| scpi-92 | `:WAVeform:PREamble` | Q | none | 10 comma-separated fields; see waveform rule | Q: tbd at M14 | wave_params | T/V/M | Rigol-shaped preamble; keep | `ui/src/scpi.rs:832` |
| scpi-93 | `:WAVeform:DATA` | Q | none | IEEE block: BYTE, LE WORD or ASCII volts; empty block if no data | Q: tbd at M14 | wave_data; reads current record, no wait for future Single | T/V/M | Subset; keep with M15 ordering rule | `ui/src/scpi.rs:835` |
| scpi-94 | `:LA:STATe` | both | B | B | C/Q: tbd at M14 | m.la.on; mark_changed | T/V/M | Rigol names where present; LA capability required; keep | `ui/src/scpi.rs:1362` |
| scpi-95 | `:LA:ENABle` | both | B | B | C/Q: tbd at M14 | alias of LA:STATe | T/V/M | LA capability required; keep | `ui/src/scpi.rs:1362` |
| scpi-96 | `:LA:DIGital<n>:DISPlay` | both | n=0..15; B | B | C/Q: tbd at M14 | m.la.show bit n; no mark_changed here | T/V/M | LA capability required; keep | `ui/src/scpi.rs:1377` |
| scpi-97 | `:LA:POD<n>:THReshold` | both | n=1..2; f32 abs<=15 V; nan rejected by comparison | lowercase-exponent V | C/Q: tbd at M14 | m.la.preset=USER, la.user; mark_changed | T/V/M | LA capability required; keep | `ui/src/scpi.rs:1390` |
| scpi-98 | `:LA:POINts` | Q | none | decimal sample count; 0 and -230 if no record | Q: tbd at M14 | ui.la_record.words.len() | T/V/M | LA capability required; keep | `ui/src/scpi.rs:1405` |
| scpi-99 | `:LA:PREamble` | Q | none | points,dt,t0 with lowercase exponents (s); 0 and -230 if no record | Q: tbd at M14 | ui.la_record() | T/V/M | OOS data contract; LA capability required; keep | `ui/src/scpi.rs:1405` |
| scpi-100 | `:LA:DATA` | Q | none | IEEE block of LE u16, bit n=Dn; empty block and -230 if no record | Q: tbd at M14 | ui.la_record() | T/V/M | LA capability required; keep | `ui/src/scpi.rs:1405` |

## Display, views, calibration and Bode

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-101 | `:DISPlay:DATA` | Q | ignored (including optional stock arguments) | IEEE block containing 24-bit bottom-up BGR BMP of canvas | Q: tbd at M14 | bmp(cv); synchronous allocation/encoding | T/V/M | Stock BMP shape; keep | `ui/src/scpi.rs:839` |
| scpi-102 | `:DISPlay:TYPE` | both | VECTors / DOTS / AUTO | VECT / DOTS / AUTO | C/Q: tbd at M14 | ui.set_dots(m.dots) | T/V/M | Stock VECT/DOTS spellings, OOS AUTO; keep | `ui/src/scpi.rs:842` |
| scpi-103 | `:DISPlay:VIEW` | both | decimal usize 1..m.views | decimal active view, 1-based | C/Q: tbd at M14 | view_go; switch saved view | T/V/M | OOS; keep | `ui/src/scpi.rs:860` |
| scpi-104 | `:DISPlay:VIEW:COUNt` | both | decimal usize 1..4 | decimal number of views | C/Q: tbd at M14 | views_set(m.views) | T/V/M | OOS; keep | `ui/src/scpi.rs:860` |
| scpi-105 | `:VIEW` | both | SCOPe / SPECtrum / EYE / POWer / BODE; target also STEP | SCOPE / SPECTRUM / EYE / POWER / BODE / STEP | C/Q: tbd at M14 | m.view; current setter cannot select AView::Step | T/V/M | OOS; change: accept STEP, coordinator decision D5, scpi-319 | `ui/src/scpi.rs:879`, `ui/src/scpi.rs:888`, `core/src/model/display.rs:13` |
| scpi-106 | `:IDENtify` | Q | none | semicolon-separated CHn[+CHm] summary (confidence %); empty if no findings | Q: tbd at M14 | identify_text runs analysis synchronously | T/V/M | OOS; keep | `ui/src/scpi.rs:896` |
| scpi-107 | `:CALibrate:STATus` | Q | none | cal_status human status string | Q: tbd at M14 | ui.cal_status_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:900` |
| scpi-108 | `:CALibrate:STARt` | C | 0..4[,ZERO]; 0=all; parser actually invalid=>0, >4=>4 | none | C: tbd at M14 | selfcal_remote; starts asynchronous calibration without dialog; inputs must be disconnected; can persist calibration | T/V/M | OOS; documentation only, no authorization to run; keep | `ui/src/scpi.rs:903` |
| scpi-109 | `:CALibrate:CANCel` | C | none | none | C: tbd at M14 | selfcal_remote(cancel,false) | T/V/M | OOS; keep | `ui/src/scpi.rs:910` |
| scpi-110 | `:BODE:RUN` | C | B; invalid/missing treated ON | none | C: tbd at M14 | ui.bode_run; starts/stops asynchronous sweep, controls external generator | T/V/M | OOS; keep | `ui/src/scpi.rs:915` |
| scpi-111 | `:BODE:DATA` | Q | none | f_Hz,gain_dB(3 decimals),phase_deg(2 decimals); rows separated by semicolon; empty if none | Q: tbd at M14 | ui.bode_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:920` |
| scpi-112 | `:BODE:GENerator` | both | usb or optionally quoted single-word endpoint; empty accepted; whitespace -224 | usb or endpoint (may be empty) | C/Q: tbd at M14 | m.bode_usb,bode_addr; retains LAN address on USB | T/V/M | OOS; keep | `ui/src/scpi.rs:923` |

## Built-in generator

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-113 | `:SOURce[<n>]:OUTPut` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | afg_set(on); generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:963` |
| scpi-114 | `:SOURce[<n>]:OUTPut:STATe` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | afg_set(on); generator capability required | T/V/M | Alias; keep | `ui/src/scpi.rs:963` |
| scpi-115 | `:SOURce[<n>]:FUNCtion` | both | n defaults 1; 1..installed generators; SINusoid/SQUare/RAMP/PULSe/NOISe/DC/ARB | SIN/SQU/RAMP/PULS/NOIS/DC/ARB | C/Q: tbd at M14 | m.afg_wave; afg_set clamps; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:1068` |
| scpi-116 | `:SOURce[<n>]:FUNCtion:SQUare:DUTY` | both | n defaults 1; 1..installed generators; D clamped 5..95 % | E % | C/Q: tbd at M14 | m.afg_duty; generator capability required | T/V/M | keep | `ui/src/scpi.rs:1056` |
| scpi-117 | `:SOURce[<n>]:FUNCtion:PULSe:DUTY` | both | n defaults 1; 1..installed generators; D clamped 5..95 % | E % | C/Q: tbd at M14 | m.afg_duty, same as square; generator capability required | T/V/M | keep | `ui/src/scpi.rs:1056` |
| scpi-118 | `:SOURce[<n>]:FUNCtion:RAMP:SYMMetry` | both | n defaults 1; 1..installed generators; D clamped 0..100 % | E % | C/Q: tbd at M14 | m.afg_symmetry; generator capability required | T/V/M | keep | `ui/src/scpi.rs:1056` |
| scpi-119 | `:SOURce[<n>]:FREQuency` | both | n defaults 1; 1..installed generators; D Hz clamped 0.01..wave/platform maximum; generator limits below | E Hz | C/Q: tbd at M14 | afg_set(freq); may clamp amplitude, offset, FM deviation; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:1083` |
| scpi-120 | `:SOURce[<n>]:VOLTage` | both | n defaults 1; 1..installed generators; D Vpp; clamp by frequency, 0.002..10/5/2.5 Vpp | E Vpp | C/Q: tbd at M14 | afg_set(ampl); may clamp offset; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:1085` |
| scpi-121 | `:SOURce[<n>]:VOLTage:AMPLitude` | both | n defaults 1; 1..installed generators; D Vpp, same limits | E Vpp | C/Q: tbd at M14 | afg_set(ampl); generator capability required | T/V/M | Alias; keep | `ui/src/scpi.rs:1085` |
| scpi-122 | `:SOURce[<n>]:VOLTage:OFFSet` | both | n defaults 1; 1..installed generators; D V; DC +/-5 V, otherwise +/-(Vmax-amplitude)/2 | E V | C/Q: tbd at M14 | afg_set(offset); generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:1085` |
| scpi-123 | `:SOURce[<n>]:MOD` | both | n defaults 1; 1..installed generators; B; ON restores at least AM, OFF loses previous type | B | C/Q: tbd at M14 | m.afg_mod; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:1009` |
| scpi-124 | `:SOURce[<n>]:MOD:STATe` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | m.afg_mod; generator capability required | T/V/M | Rigol DG style alias; keep | `ui/src/scpi.rs:1009` |
| scpi-125 | `:SOURce[<n>]:MOD:TYPe` | both | n defaults 1; 1..installed generators; AM/FM/PM | NONE/AM/FM/PM | C/Q: tbd at M14 | m.afg_mod; afg_set clamps; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:999` |
| scpi-126 | `:SOURce[<n>]:MOD:AM:DEPTh` | both | n defaults 1; 1..installed generators; D clamped 0..120 % | E % | C/Q: tbd at M14 | m.afg_mod_depth; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:993` |
| scpi-127 | `:SOURce[<n>]:MOD:FM:DEViation` | both | n defaults 1; 1..installed generators; D min 0.01 Hz, then min(carrier,max-carrier), floor 0.01 | E Hz | C/Q: tbd at M14 | m.afg_fm_dev; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:997` |
| scpi-128 | `:SOURce[<n>]:MOD:PM:DEViation` | both | n defaults 1; 1..installed generators; D clamped 0..360 degrees | E degrees | C/Q: tbd at M14 | m.afg_pm_dev; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:995` |
| scpi-129 | `:SOURce[<n>]:MOD:AM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:991` |
| scpi-130 | `:SOURce[<n>]:MOD:AM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:978` |
| scpi-131 | `:SOURce[<n>]:MOD:FM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:991` |
| scpi-132 | `:SOURce[<n>]:MOD:FM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:978` |
| scpi-133 | `:SOURce[<n>]:MOD:PM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:991` |
| scpi-134 | `:SOURce[<n>]:MOD:PM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:978` |
| scpi-135 | `:SOURce[<n>]:ARB:DATA` | C | comma-separated F samples, arbitrary scale; at least 2 finite parsed samples; line size is practical bound | none | C: tbd at M14 | afg_arb_load selects ARB, resamples to 8192 s16 codes; bad tokens silently skipped | T/V/M | OOS; generator capability required; keep | `ui/src/scpi.rs:1040` |
| scpi-136 | `:SOURce[<n>]:ARB:CAPTure` | C | CHANnel1..4; current channel record required | none | C: tbd at M14 | afg_arb_capture selects ARB and writes arb.csv through store | T/V/M | OOS; generator capability required; keep | `ui/src/scpi.rs:1046` |

## System, setup access and debug

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-137 | `:SYSTem:SET` | C | key=value, setup key catalogue via store::set; key validation exceptions below | none | C: tbd at M14 | store::set(&mut m), mark_changed; errors -224 | T/V/M | OOS test access; keep | `ui/src/scpi.rs:1107` |
| scpi-138 | `:SYSTem:GET` | Q | setup key, exact case | serialized value; empty when omitted/unknown | Q: tbd at M14 | store::setup_text lookup; no error for missing key | T/V/M | OOS; keep with empty-reply fix D1 | `ui/src/scpi.rs:1128` |
| scpi-139 | `:SYSTem:STATe` | Q | none | diagnostic line; state-query.md | Q: tbd at M14 | cmd::stat_text | T/V/M | OOS; keep | `ui/src/scpi.rs:1135` |
| scpi-140 | `:SYSTem:GRAPhics` | both | CPU/GPU | CPU/GPU (requested, not necessarily active) | C/Q: tbd at M14 | ui.graphics_set; changes renderer preference | T/V/M | OOS; keep | `ui/src/scpi.rs:1139` |
| scpi-141 | `:SYSTem:AOUTput` | both | TOUT/OFF/PFA/PFAIL; PFA only with mask/zone HW offered | TOUT/OFF/PFA | C/Q: tbd at M14 | m.aux_out; OFF constant low | T/V/M | Stock TOUT/PFAil; OFF is OOS; keep | `ui/src/scpi.rs:1151` |
| scpi-142 | `:SYSTem:LOG` | C | B | none | C: tbd at M14 | ui.log_set; logging IO | T/V/M | OOS; keep | `ui/src/scpi.rs:1174` |
| scpi-143 | `:SYSTem:ERRor` | Q | none | code,"message" plus optional context; 0,"No error" when empty | Q: tbd at M14 | self.err.remove(0), consumes oldest error | T/V/M | Subset; keep | `ui/src/scpi.rs:1249` |
| scpi-144 | `:DECode:EVENts` | Q | none | newline-separated t_s,decoder,content,ok/error; empty if no events | Q: tbd at M14 | ui.events_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:1185` |
| scpi-145 | `:DEBug:SLOTs` | Q | none | raw measurement slot diagnostic text | Q: tbd at M14 | ui.debug_slots(); schedules raw measure-unit run, returns previous result | T/V/M | fork debug; keep | `ui/src/scpi.rs:422` |
| scpi-146 | `:SYSTem:DEBug:UART` | Q | none | board UART/protocol diagnostic text; off,enabled=0 when off | Q: tbd at M14 | board().uart_debug | T/V/M | fork debug; keep | `ui/src/scpi.rs:1189` |
| scpi-147 | `:SYSTem:DEBug:SERial` | Q | none | same as UART | Q: tbd at M14 | board().uart_debug | T/V/M | fork debug alias; keep | `ui/src/scpi.rs:1189` |
| scpi-148 | `:SYSTem:DEBug:ZONE` | Q | none | zone diagnostic text | Q: tbd at M14 | ui.zone_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:1198` |
| scpi-149 | `:SYSTem:DEBug:MASK` | Q | none | mask diagnostic text | Q: tbd at M14 | ui.mask_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:1227` |
| scpi-150 | `:SYSTem:DEBug:MASK:MAP` | both | one case-sensitive key=value; catalogue below | space-separated mapping key=value pairs | C/Q: tbd at M14 | board.mask_map; process-wide mapping; marks changed on set | T/V/M | fork debug; keep | `ui/src/scpi.rs:1202` |
| scpi-151 | `:SYSTem:DEBug:MASK:COLumns` | Q | optional a[,b] usize columns; one valid a gives (a,a); two sort endpoints; malformed defaults 0,9; extras ignored | column diagnostic text or off | Q: tbd at M14 | ui.mask_columns_text(a,b) | T/V/M | fork debug; keep | `ui/src/scpi.rs:1211` |
| scpi-152 | `:SYSTem:DEBug:DVM` | Q | none | DVM raw register diagnostic text | Q: tbd at M14 | ui.dvm_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:1231` |
| scpi-153 | `:SYSTem:DEBug:DVM:LANE` | both | AUTO or decimal u32 0..3 | AUTO or decimal lane 0..3 | C/Q: tbd at M14 | ui.dvm_lane_set; forces single-channel ADC lane | T/V/M | fork debug; keep | `ui/src/scpi.rs:1235` |


## Query spellings that execute command effects

These are legacy acceptance defects to preserve in the oracle until reviewed.
They do not supply status. Baseline replies are fallback error text, except
scpi-154 with a non-empty queue, which returns no answer (D4). Target invalid
queries queue -113 and return an empty terminated reply under scpi-306, without
exception. Existing command effects remain recorded; this round does not remove
them. The shared fallback is `ui/src/scpi.rs:356`.

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-154 | `*CLS?` | Q | none | Current: -113 text only if queue was empty; no reply if non-empty. Target: empty terminated reply; queue -113 | Q: tbd at M14 | Clears error queue before fallback; target queues -113 after the clear regardless of previous length | T/V/M | Legacy defect; change: coordinator decision D4, scpi-320 | `ui/src/scpi.rs:355`, `ui/src/scpi.rs:407` |
| scpi-155 | `*RST?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | DEFAULT key reset before fallback | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi.rs:411` |
| scpi-156 | `:CLEar?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | CLEAR key before fallback | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi.rs:432` |
| scpi-157 | `:RUN?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | same RUN mutation and run request as command | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi/actions.rs:7` |
| scpi-158 | `:STOP?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | sets run=Stop | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi/actions.rs:15` |
| scpi-159 | `:SINGle?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | sets Single and Run but omits explicit single request | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi/actions.rs:17` |
| scpi-160 | `:CALibrate:STARt?` | Q | 0..4[,ZERO], same coercion as command | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | starts calibration before fallback | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi.rs:903` |
| scpi-161 | `:CALibrate:CANCel?` | Q | none | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | cancels calibration before fallback | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi.rs:910` |
| scpi-162 | `:BODE:RUN?` | Q | B; invalid/missing defaults ON | Current: -113 text; target: empty terminated reply, queue -113 | Q: tbd at M14 | starts/stops Bode before fallback | T/V/M | Keep legacy effect; change reply per scpi-306 | `ui/src/scpi.rs:915` |

## Parameter catalogues and delegated replies

**scpi-300 (keep): measurement items.** `ui/src/scpi.rs:108` and
`ui/src/scpi.rs:139` are the complete item tables; `ui/src/scpi.rs:150` defines
source cardinality. The following are value tokens, not additional headers.
Channel sources accept CHAN or CHANNEL plus 1..4; one-channel items default CH1.
Units describe the measurement, not suffixes accepted on the wire
(`dsp/src/measure.rs:126`, `ui/src/ui/tools.rs:94`).

| Long / short item | Model measurement | Unit |
|---|---|---|
| VMAX | Vmax | V |
| VMIN | Vmin | V |
| VPP | Vpp | V |
| VTOP | Vtop | V |
| VBASe / VBAS | Vbase | V |
| VAMP | Vamp | V |
| VAVG | Vavg | V |
| VRMS | Vrms | V |
| OVERshoot / OVER | Overshoot | % |
| PREShoot / PRES | Preshoot | % |
| MARea / MAR | Area | V s |
| MPARea / MPAR | AreaCyc | V s |
| PERiod / PER | Period | s |
| FREQuency / FREQ | Freq | Hz |
| RTIMe / RTIM | Rise | s |
| FTIMe / FTIM | Fall | s |
| PWIDth / PWID | PWidth | s |
| NWIDth / NWID | NWidth | s |
| PDUTy / PDUT | PDuty | % |
| NDUTy / NDUT | NDuty | % |
| TVMax / TVM | TVmax | s |
| TVMin / TVM | TVmin | s; ambiguous short form, see D3 |
| PSLewrate / PSL | PSlew | V/s |
| NSLewrate / NSL | NSlew | V/s |
| VUPPer / VUPP | Vupper | V |
| VMID | Vmid | V |
| VLOWer / VLOW | Vlower | V |
| PVRMs / PVRM | VrmsCyc | V |
| PPULses / PPUL | PPulses | count |
| NPULses / NPUL | NPulses | count |
| PEDGes / PEDG | PEdges | count |
| NEDGes / NEDG | NEdges | count |
| RRDelay / RRD | pair 0 | s |
| FFDelay / FFD | pair 1 | s |
| RFDelay / RFD | pair 2 | s |
| FRDelay / FRD | pair 3 | s |
| RRPHase / RRPH | pair 4 | degrees |
| FFPHase / FFPH | pair 5 | degrees |
| RFPHase / RFPH | pair 6 | degrees |
| FRPHase / FRPH | pair 7 | degrees |

**scpi-301 (keep): numeric ladders.** Channel coarse scale is
100/200/500 uV, 1/2/5/10/20/50/100/200/500 mV, 1/2/5/10 V at 1x.
Fine scale rounds up in the board's calibrated nanovolt rows, saturated at the
last row (`ui/src/scpi.rs:203`). Timebase actually includes 1 ns and 2 ns despite
the table comment saying 5 ns (`core/src/model/tables.rs:14`). Both ladders use
`nearest` log distance (`ui/src/scpi.rs:215`); nonpositive time requests can select
the first row through NaN comparisons. Memory depth values are AUTO (0), 1000,
10000, 100000, 1M, 10M, 25M, 50M, 62.5M, 100M, 125M samples
(`ui/src/board/dho/mod.rs:62`). Hardware may choose a different live depth/rate.
Bandwidth labels are AUTO, FULL/OFF, GND and 5,10,15,20,25,30,35,40,50,60,70,75,
80,90,100,125,150,175,200,250,300,310,400,600,800 MHz; 1,1.5,2 GHz.
Spaces are removed and the Hz suffix can be omitted (`core/src/model/tables.rs:35`,
`ui/src/scpi.rs:497`). Baud rates are 1200,2400,4800,9600,19200,38400,57600,115200,460800,921600
(`core/src/decode.rs:17`); selection minimizes absolute distance, not logarithmic
distance.

**scpi-302 (keep): generator limits.** `ui/src/ui/afg.rs:230` clamps carrier to
0.01 Hz..`gen_max_hz` of current waveform and platform limits; the maximum is
model/configuration dependent (`hal/src/dho/afg.rs:339`). Amplitude is clamped to
2 mVpp..10 Vpp through 10 MHz, 5 Vpp through 30 MHz, 2.5 Vpp above. Offset limit
is +/-(Vmax-A)/2 with Vmax=10 V through 10 MHz, else 5 V; DC uses +/-5 V and
noise uses the 1 Hz clamp path (`hal/src/dho/afg.rs:233`). Arb requires two finite
samples, normalizes extrema to +/-32767, linearly resamples to 8192, flat=>zero
(`hal/src/dho/afg.rs:285`). There is no SCPI sample-count ceiling independent of
message limits. Generator n defaults to 1 and uses `gen_count`; nonexistent
hardware queues -241 (`ui/src/scpi.rs:940`).

**scpi-303 (keep): setup and mask mapping.** SET delegates the complete setup
key/value language to `ui/src/store.rs:430`; this command accepts one key=value,
not a second catalogue of SCPI headers. It validates serialized keys after
mutation, except suffixes vfine/unit/bias/skew/label/tfine and prefix math_expr
(`ui/src/scpi.rs:1116`). GET reads only emitted setup keys, so an optional default
can return empty even when the setting exists. The setup-key specification is a
separate spec-gaps row 4 deliverable.
MASK:MAP keys/values are case-sensitive (`hal/src/dho/mask.rs:194`): domain
readout/pregain; cpd finite >=1 codes/div; mid finite code; shift finite clamped
0..16 bits cast to u32; offs finite 8-bit-code units; invert 1/on/true means true,
anything else false; cols record/clocks/stock, numeric clamped 1..0x0fffffff, or
raw:count:unit (count 1..1023, unit>0 samples/strobe); range a-b converts to u16
and sorts; skew finite samples. Query returns all nine keys, separated by spaces
(`hal/src/dho/mask.rs:242`). Unknown/malformed key queues -224.

**scpi-304 (keep): debug and action replies.** These extensions intentionally
return diagnostic text, not a stable register API. Exact field order/format is
owned by the following producers: slots `ui/src/ui/autoset.rs:113` (`debug_slots`),
DVM `ui/src/ui/tools.rs:369`, lane `ui/src/ui/tools.rs:363` (`dvm_lane_text`),
mask `ui/src/ui/hwmask.rs:96`, columns `ui/src/ui/hwmask.rs:133`, zone
`ui/src/ui/hwzone.rs:48`, serial `ui/src/board/dho/mod.rs:159` (`uart_debug`).
The reply layouts below complete the response-format column. Literal key
order is significant for legacy comparison; unmarked integer values are decimal,
raw registers are `0x` plus eight hex digits, booleans 0/1. Producer-defined
condition/parity names use Rust Debug. These are diagnostic shapes, not portable
hardware guarantees.

| Header | Reply layout and units | Evidence |
|---|---|---|
| DEB:SLOTs? | `no hardware`, `pending`, `request;none`, or `request;min,max,mean` for each of eight slots; min/max 12-bit codes, mean one decimal on 16-bit scale. Schedules new run but returns previous one; query twice about 0.3 s apart | `ui/src/ui/autoset.rs:109` |
| SYST:DEB:DVM:LANE? | `AUTO` or decimal forced lane 0..3 | `ui/src/ui/tools.rs:363` |
| SYST:DEB:DVM? | `off,sw=<.6e V>` or `ch,lane,exp,n,cfg,sum_lo,sum_hi,sq_lo,sq_hi,hw,mode,sw` key=value pairs. hw=`none` or three `.6e` V readings joined by semicolons (DC/AC RMS/AC+DC RMS); mode decimal 0..2 | `ui/src/ui/tools.rs:369`, `ui/src/board/dho/hw.rs:1056` |
| SYST:DEB:MASK? | `off,enabled,active,blocked,sw_pass,sw_fail,acquired,used,records` or `ch,slot,ctrl,frames_lo,frames_hi,fails_lo,fails_hi,frames,fails,n,sw_pass,sw_fail,acquired,used,records` key=value pairs. Counts in frames/events as named | `ui/src/ui/hwmask.rs:96` |
| SYST:DEB:ZONE? | `off,enabled,active,sw_drops,acquired,used,records` or `ch,slot,ctrl,s4030,s4038,frames_lo,frames_hi,fails_lo,fails_hi,armed,pass,cycle,state,frames,n,batch,batch_ms,sw_drops,acquired,used,records` key=value pairs. batch is actual/requested; batch_ms one decimal ms | `ui/src/ui/hwzone.rs:48` |
| SYST:DEB:MASK:COLumns? | `off`, `no table`, or optional `pre=<sample>,trig=<offset with 2 decimals or none>,` followed by `cols=a-b,samples=N,per=<1 decimal>,fail=N,first=<up to 8 space-separated column indices>;` then semicolon-separated `c:lo-hi/<lower-enable><upper-enable>:min-max[!]`. Limits/codes dimensionless; per samples/column, ! marks failure; sample extrema can be inf/-inf without data | `ui/src/ui/hwmask.rs:133`, `ui/src/ui/hwmask.rs:159` |
| SYST:DEB:UART? and SERial? (UART) | `off,enabled=B` or `on/off,src=CHn,baud,bits,parity,cond,invert,clocks,r2100,r2104,r221c,mux,event_s,shift_s` key=value pairs. Baud symbols/s; clocks bit clocks; event_s/shift_s `.4e` seconds | `ui/src/board/dho/hw.rs:821` |
| SYST:DEB:UART? and SERial? (SPI/IIC) | SPI: `on/off,proto=SPI,clk=CHn,data=CHn,cs=CHn,clk_falling=B,width=N,<register pairs>,mux=N`; IIC: `on/off,proto=I2C,scl=CHn,sda=CHn,cond=<debug condition>,<register pairs>,mux=N`. Register pairs `r<4 hex digits>=<0x8hex>` come from receiver word generators | `ui/src/board/dho/hw.rs:957` |
| CAL:STATus? | `needs the hardware source`, `<rounded percent> % <progress>`, `starting ...`, last `done: ...`/`failed: ...` result, or `idle` | `ui/src/ui/selfcal.rs:124`, `ui/src/ui/selfcal.rs:218` |

Fallback strings, raw words and punctuation must be retained in corpus
fixtures. Register-word inventories are delegated to
`hal/src/dho/serialtrig.rs:58` (`spi_words`), `:131` (`i2c_words`) and
`hal/src/dho/uarttrig.rs:125` (`words`); their output is represented verbatim. The
parser does not define independent numeric register query headers. Bode serialization
is `ui/src/ui/bode.rs:769`; calibration status is `ui/src/ui/selfcal.rs:191`;
Identify is `ui/src/ui/analysis.rs:370`; decoder rows `ui/src/ui/tables.rs:97`.

## Cross-cutting rules

**scpi-305 (keep): syntax and ordering.** The TCP reader splits at LF, decodes
invalid UTF-8 lossily, trims whitespace, and the parser splits literally at `;`
without quote awareness. Empty segments are ignored. Each segment is an absolute
header; leading colons are optional and stripped, SCPI relative path inheritance
is not implemented. Header/argument split is at the first whitespace; only a
header-ending `?` selects query mode, with repeated trailing `?` stripped.
`kw` accepts exactly long or short, not arbitrary intermediate abbreviations
(`ui/src/scpi.rs:169`, `ui/src/scpi.rs:313`, `ui/src/scpi.rs:383`).

Segments execute synchronously, left to right inside one legacy intake closure.
Responses are joined with semicolons; messages whose handlers all return no
answer produce no response. Bare *IDN and TRIG:STATus are legacy exceptions
that answer without a question mark.
A semicolon or newline in a producer's text is not escaped (Bode, Identify,
decoder rows), so clients cannot blindly split every response on semicolons.
One LF terminates the response message, including binary blocks. CRLF input is
accepted by trimming. The current baseline loses leading empty response slots
because it tests output length (`ui/src/scpi.rs:337`); D1 corrects this.

**scpi-306 (change, D1): every query has a terminated response, including an
empty value.** An empty list/value is a successful answer, distinct from no
query. TCP and VXI-11 must send LF for a solitary empty query, and `;1\n` for
an empty query followed by `*OPC?`. MCP must return empty text for the query and
reserve `ok` for a command-only message. At this baseline, empty
`:BODE:DATA?`, `:IDENtify?`, `:DECode:EVENts?`, `:ACQuire:SEGMents:TIMes?`,
`:SYSTem:GET?` and `:BODE:GENerator?` can be suppressed or turned into `ok`.
The current fix is already on `t0003-m2`, commit `0137955`:
`ui/src/scpi/message.rs:9` (`response`), `:22` (`exec_message`),
`ui/src/vxi11.rs:111`, `ui/src/http.rs:1063` at that revision. It preserves
`Option<Vec<u8>>` and adds LF independently of payload length. That fix is the
oracle reference for this rule, not present in c6f1f98. Its tests are
`ui/src/vxi11/empty_tests.rs:1` and `ui/src/http/tests/empty_tests.rs:1` at
0137955. Those tests cover successful empty answers. The coordinator's
target for invalid queries is an empty terminated reply, with their error retained
in the queue for `:SYSTem:ERRor?`; baseline error-text replies are an explicit
oracle divergence. This includes *CLS? regardless of pre-existing errors (D4),
and any invalid query in a compound message occupies its empty response slot.

**scpi-307 (keep queue semantics; change invalid-query reply per scpi-306/D4):
shared error queue.** TCP clients, VXI-11 links and MCP calls share one Scpi
object and therefore
share errors and waveform preferences (`ui/src/system/device/remote.rs:71`,
`ui/src/http.rs:1063`). FIFO order, at most 20
errors. Context is limited to 256 characters when a stored string exceeds 256
bytes; overflow keeps the first 19 and appends -350. `*CLS` clears it;
`:SYSTem:ERRor?` removes one or returns `0,"No error"`. At baseline, queries
with no handler
answer return the latest queued error without consuming it; if queue length is
unchanged, -113 is synthesized. This length test fails after *CLS? clears a
non-empty queue (D4). Target: an invalid query queues the appropriate error from
scpi-320, synthesizing -113 when no handler supplied one, and returns an empty
terminated reply. Do not infer whether an error was supplied from queue length.
An existing successful empty answer is not an unknown header
(`ui/src/scpi.rs:356`, `ui/src/scpi.rs:1249`).

| Code | Message / trigger | Evidence |
|---|---|---|
| -113 | Undefined header; also many invalid values in the early model dispatch fall through here | `ui/src/scpi.rs:1252` |
| -224 | Illegal parameter value in explicitly validating handlers (measure, LA, source, views, system) | `ui/src/scpi.rs:1280`, `ui/src/scpi.rs:969` |
| -230 | Data corrupt or stale, no LA record; data query still returns empty block | `ui/src/scpi.rs:1407` |
| -241 | Hardware missing, LA or generator capability unavailable | `ui/src/scpi.rs:944`, `ui/src/scpi.rs:1352` |
| -350 | Queue overflow | `ui/src/scpi.rs:373` |
| -363 | Input buffer overrun, response budget reached, rest of compound dropped | `ui/src/scpi.rs:341` |

No IEEE event/status register model, `*ESR`, `*ESE`, `*SRE`, `*STB`, `*WAI` or
real pending-operation `*OPC` latch is implemented. Do not infer these from
Rigol compatibility. Invalid setters are inconsistent: some queue -224, some
fall through to -113, waveform selection often silently coerces. Keep these
observable differences until a separately approved decision changes them.

**scpi-308 (keep): pacing, bounds and transport.** TCP defaults to 5555
(`ui/src/ui/remote.rs:214`), allows eight clients, reads without waiting from the
UI poll, and executes all buffered complete lines in that pass. An unterminated
buffer over 4 MiB disconnects; the baseline checks after processing complete
lines, so this is not a strict cap on an already completed line. Aggregate
response cap is checked before each segment at 64 MiB; one answer can overshoot.
A socket writer can block UI for a 2 s stall or 30 s total send limit
(`ui/src/scpi.rs:41`, `ui/src/scpi.rs:281`). There is no parser-level minimum
delay, query-rate guarantee, or acquisition-completion wait. A client should
pace on responses, not a fixed sleep. M3d/M15 remove UI blocking; that is already
specified in `docs/architecture/migration.md` and is not a legacy behavior.

VXI-11 exposes the same command language via portmapper TCP/UDP 111 and core
RPC (`ui/src/vxi11.rs:4`, `ui/src/vxi11.rs:110`). `device_write` ignores its flags and executes each write immediately,
splitting its payload on CR or LF; it does not assemble fragments across writes
or honor END. `device_read` drains max(1,requestSize) bytes up to available data,
returning END when drained or REQCNT when more remains; no-data returns RPC
I/O timeout 15 immediately (`ui/src/vxi11.rs:142`, `ui/src/vxi11.rs:169`). Its limits and errors
are RPC errors, separate from the SCPI error queue. MCP POST `/mcp`, tools/call
name `scpi`, arguments.command, calls `exec_line`; command-only returns text
`ok`, valid UTF-8 reply returns text and invalid UTF-8 returns `(binary response)`
(`ui/src/http.rs:1061`). It is not a binary waveform transport. Limits from the
HTTP request envelope also apply. The MCP `screenshot` tool returns the current
canvas as a base64 PNG image (`ui/src/http.rs:1066`), independently of SCPI. ACL/service switches are transport policy,
not per-command availability. See D1 for current framing corrections.

**scpi-309 (keep): numeric formats.** General setters use finite f32 or f64;
no engineering suffix or unit suffix is implied. Depth alone accepts K/M.
Protocol trigger **U** uses decimal f64 (including exponent), nonnegative and
finite, truncated/saturated to u64, or exact `0x`/`0X` hex; individual fields then
clamp/check (`ui/src/scpi/ser.rs:11`). Bare negative decimal is rejected there.
Boolean uses only 1/0/ON/OFF. Enum acceptance varies: the table distinguishes
exact `kw` matches from `starts_with` prefix acceptance. General E reply format
and unavailable sentinel come from `ui/src/scpi.rs:225`; SPI/LA/segment times
use Rust's lowercase exponent, not E. Count fields use decimal integers.

**scpi-310 (keep): waveform block and preamble.** IEEE definite length block
is `#9` then nine decimal length digits then payload (`ui/src/scpi.rs:1451`).
BYTE clamps rounded codes 0..255; WORD clamps 0..65535, LE u16; ASCII is
comma-separated E volts inside the same block (`ui/src/scpi.rs:1434`). Preamble:
`format,0,points,1,xincrement,xorigin,0,yincrement,yorigin,yreference`.
Format 0=BYTE,1=WORD,2=ASCII; the second field (type) is always 0.
xincrement/xorigin seconds and yincrement V/code use E formatting; xreference
is zero. yorigin is rounded offset/yincrement codes, yreference is 128 BYTE/ASCII
or 32768 WORD; both use Rust f64 Display rather than E formatting. yincrement is V/div times VDIV/250 or /60000.
Reconstruct volts `(code-yorigin-yreference)*yincrement`. No record gives zero
points and zero time coordinates; RAW uses raw when available and otherwise
screen record (`ui/src/scpi.rs:1257`). This is a screen-window raw read,
not a promise of the whole FPGA memory. There are no START/STOP range headers.

**scpi-311 (keep, M15 target): query consistency.** Baseline `*OPC?` is immediate
and `:SINGle;:WAVeform:DATA?` can read an earlier record. The accepted migration
M15 requires a future-record barrier `After { gen, epoch }` for SINGLE followed
by waveform data, coherent answers from one step, and slow clients off main.
Corpus fixtures must label current immediate behavior and this approved target
difference explicitly, never silently bless the baseline as a barrier.

## Accidental accepted syntax and proposed defect decisions

**scpi-312 (keep pending coordinator decision): acceptance closure.** A finite
header table cannot enumerate arbitrary ignored nodes. Preserve these predicates
in negative/alias corpus cases as well as all rows above:

- The header comment advertises `:SEGMents:HARDware`, `:SEGMents:INTerval` and
  `:SEGMents:TIMes?`; only the `:ACQuire:SEGMents:...` paths are actually routed
  (`ui/src/scpi.rs:713`, `ui/src/scpi.rs:1182`). Do not add the shorter family
  as an accepted alias on the strength of the comment.
- Timebase examines only first and last node, accepting arbitrary intervening
  nodes before SCALe/OFFSet/POSition (`ui/src/scpi.rs:524`).
- IDENtify accepts trailing nodes (`ui/src/scpi.rs:896`).
- Generator matches presence of keywords anywhere after SOURCE, with numeric
  suffix stripped from every node. Priority is OUTPUT, MOD, ARB, FUNC duty/symm,
  FUNC, FREQ, VOLT offset, VOLT. Thus `:SOUR:BOGUS:VOLT?` reads amplitude;
  `:SOUR:MOD:BOGUS?` reads modulation enable; AM/FM/PM internal settings share
  storage (`ui/src/scpi.rs:960`). These are not additional designed features.
- `*IDN` and trigger STATus answer without `?`. `*CLS?`, `*RST?`, `:CLEar?`,
  `:RUN?`, `:STOP?`, `:SINGle?`, `:CAL:STARt?`, `:CAL:CANCel?`, `:BODE:RUN?`
  execute command effects before the unanswered-query fallback normally queues/returns
  -113; *CLS? with a non-empty queue instead suppresses the reply (D4). SINGLE? omits the explicit single request but changes mode/run
  (`ui/src/scpi.rs:395`, `ui/src/scpi/actions.rs:6`, `ui/src/scpi.rs:903`).
  They are accepted side-effect aliases, not supported status queries.

**scpi-313 (change, D2; coordinator decision): Rigol trigger type selection.**
Baseline MODE? always returns EDGE and its setter accepts anything without effect
(`ui/src/scpi.rs:548`). Target: the setter switches the actual trigger type
through the same command as the menu; MODE? returns the active type in Rigol's
wire tokens. RS232/IIC/SPI select both `TrigType::Bus` and the corresponding
protocol, not merely a generic BUS type. This is a client-visible change: for
example, `:TRIG:MODE PULSe;:TRIG:MODE?` changes acquisition type and answers PULS,
where today it leaves the type unchanged and answers EDGE. Invalid or unsupported
type writes queue -224 and leave the selection unchanged.

Use Rigol's long/short type spellings, not Rust Debug names. The supported mapping
below is the M15 target derived from `core/src/model/trigger.rs:24` and
[Rigol DHO800/DHO900 Programming Guide, section 3.27.1](https://download.rigol.com/en/Manual/Digital%20Oscilloscope/DHO800/DHO800900_ProgrammingGuide_EN.pdf).
This is a source/manual contract, not stock-unit validation. Other Rigol types
require actual platform/protocol support; the manual alone does not grant it.

| Setter spelling | Model type / selection | Query token |
|---|---|---|
| EDGE | Edge | EDGE |
| PULSe | Pulse | PULS |
| SLOPe | Slope | SLOP |
| VIDeo | Video | VID |
| PATTern | Pattern | PATT |
| DURation | Duration | DUR |
| TIMEOUT / TIM | Timeout | TIM |
| RUNT | Runt | RUNT |
| WINDow | Window | WIND |
| DELay | Delay | DEL |
| SETup | SetupHold | SET |
| NEDGe | NthEdge | NEDG |
| RS232 | Bus + RS232 | RS232 |
| IIC | Bus + IIC | IIC |
| SPI | Bus + SPI | SPI |

**scpi-314 (change, D3): ambiguous TVM measurement abbreviation.** `kw` extracts
uppercase letters. Both TVMax and TVMin produce TVM; first match wins Vmax time
(`ui/src/scpi.rs:128`, `ui/src/scpi.rs:152`, `ui/src/scpi.rs:169`). Full TVMAX and TVMIN
already work; only short TVM cannot address the minimum. Propose preserving TVM
as the existing Vmax alias, retaining TVMAX/TVMIN as the unambiguous canonical
spellings and rejecting the claim that TVM abbreviates both. Record this as a
catalogue correction, with corpus cases for TVM, TVMAX and TVMIN.

## Differential-oracle corpus

**scpi-315 (keep): session envelope.** Each recorded unitlab, `ifload.py` and
openSciLab bridge session must contain a format version, producer revision and
build identity (sanitized), platform capabilities/limits, enabled feature
switches, transport and framing options, initial setup text and parser state
(waveform preferences, error queue), plus a deterministic seed and input fixture
identifiers. Record intended versus observed startup state. Do not retain real
serial numbers, hostnames, LAN addresses or local paths; use project privacy
placeholders. Hardware evidence remains labelled separately from host replay.

**scpi-316 (keep): event trace.** Record exact request bytes and boundaries,
encoding, terminators, connection/link/session id, monotonic relative send/read
times, write fragmentation (including VXI END flags and requested read sizes),
ordered response bytes including LF and binary payload length/hash or attached
payload, successful empty replies distinctly from absent replies, disconnects,
timeouts and SCPI/RPC/JSON errors. Preserve command-only silence. Record every
external input between requests: step index/duration, record BatchMeta and data,
acquisition identity gen/epoch/stats_gen, trigger/SINGLE completions, analysis
results, calibration/generator capability fakes and queued worker actions.
Include resulting normalized settings, action/request log and error queue
snapshots at checkpoints, not only response strings. A wire transcript alone
cannot reproduce stateful acquisition on a host.

**scpi-317 (keep): comparison protocol.** Replay identical initial state and
event schedule through isolated legacy and new parser instances on in-memory
streams/fake ports, without unit IO or real clocks. Compare responses byte for
byte for deterministic fields, command/query classification, errors, state and
action ordering. Dynamic diagnostics have explicit per-field normalizers or
seeded values; timing tolerance never excuses changed units, dropped responses
or acquisition identity. Each expected divergence names its requirement and
coordinator-approved disposition (D1, D2, D4, D5 and the M15 barrier;
D3/D6/D7 only after approval). Store first differing event, replay seed and minimal reproducer.
Cover every row in long/short/mixed-case forms, lower/upper bounds and invalid
neighbors, unavailable hardware, nonfinite numbers, shared-client state,
compound empty/unknown/binary replies, queue overflow, packet fragmentation,
reconnect, STOP/RUN/SINGLE sequences, slow readers and current-fix empty replies.
Do not run calibration or debug hardware writes to manufacture fixtures.

**scpi-318 (keep): provenance and retirement.** Maintain a manifest mapping
every header row and parameter catalogue to cases, source revision and sanitized
fixture hashes. Add captured traffic from the three named clients when available;
this task specifies the corpus, it does not claim those recordings exist here.
Run all cases against both parsers in M15; an unexplained difference fails.
Retain golden session fixtures and approved decisions after the legacy oracle
is removed with the old parser, as ADR 0017 requires.


**scpi-319 (change, D5; coordinator decision): STEP view selection.** The
baseline query already emits STEP for AView::Step but the setter's five-entry
catalogue rejects STEP with -224. Target: accept STEP in scpi-105 through the
view command, and round-trip it as STEP. Preserve all existing view spellings.

**scpi-320 (keep legacy error distinctions except scpi-306/D4): row error map.**
This map plus the row's acceptance limits defines the oracle error expectation
for every scpi-1..162 row. Success adds no error unless stated below; ignored
query parameters do not become validation errors. Unsupported header shapes or
command/query kinds fall through to -113, except accepted aliases/predicates in
scpi-154..162 and scpi-312. Clamping/coercion explicitly listed in a row succeeds.
The queue limits/overflow policy in scpi-307 applies to all cases. Baseline
unanswered queries return error text; target unanswered invalid queries return
an empty terminated slot while leaving the error queued (scpi-306).

| Rows (scpi-) | Invalid argument / availability behavior | Evidence |
|---|---|---|
| 1..10, 38..39, 82, 86, 91..93, 101, 106..107, 109, 111, 139, 143..149, 152 | No argument validation; arguments ignored. Wrong kind/header: -113 unless covered by a query-effect alias | `ui/src/scpi.rs:395`, `:786`, `:828`, `:896`, `:1135`, `:1182`, `:1249`, `:1319` |
| 11..37, 40..81 | Early model closure: invalid value, failed parse, unavailable choice or bad channel suffix abandons the closure and reaches final -113, not -224. scpi-25 currently ignores all values; D2 changes invalid/unsupported MODE writes to -224 | `ui/src/scpi.rs:443`, `:776`, `:1252`, `ui/src/scpi/ser.rs:44` |
| 83..85, 87 | Bad measurement/source/statistic/list index or boolean: -224. Queries 83/87 still return 9.9E37; no error for a valid unavailable value | `ui/src/scpi.rs:1280`, `:1295`, `:1308`, `:1323` |
| 88..90 | No parameter error: bad source silently ignored; non-RAW mode selects NORM; non-WORD/non-ASC format selects BYTE | `ui/src/scpi.rs:798` |
| 94..100 | Missing LA capability: -241 before dispatch. Invalid digital/pod suffix, boolean or threshold: -224. 98..100 without a record: -230 plus 0 (98/99) or empty block (100); valid queries otherwise ignore arguments | `ui/src/scpi.rs:1352`, `:1362`, `:1377`, `:1390`, `:1405` |
| 102..105, 112 | Invalid display enum, view/count or whitespace in Bode endpoint: -224. STEP setter currently -224; target D5 accepts it | `ui/src/scpi.rs:842`, `:860`, `:879`, `:923` |
| 108, 110 | No parameter error: calibration invalid input coerces to channel 0 and >4 to 4; Bode invalid boolean defaults ON | `ui/src/scpi.rs:903`, `:915` |
| 113..136 | Invalid SOURCE suffix outside installed range: final -113. Otherwise missing generator: -241 before parameter validation. Bad boolean/enum/finite numeric, insufficient ARB samples or failed capture: -224. ARB unknown/wrong-kind paths: -113. Valid finite out-of-range numbers clamp; ARB ignores individual bad sample tokens | `ui/src/scpi.rs:940`, `:969`, `:978`, `:1040`, `:1056`, `:1068`, `:1083` |
| 137, 140..142, 150, 153 | Explicit validation: -224 for invalid setup key/value, graphics/output choice (including unavailable PFA), log boolean, mask mapping or DVM lane | `ui/src/scpi.rs:1107`, `:1139`, `:1151`, `:1174`, `:1202`, `:1235` |
| 138 | Missing/unknown setup key returns empty, no error | `ui/src/scpi.rs:1128` |
| 151 | One valid a gives (a,a); malformed input falls back to (0,9), no error | `ui/src/scpi.rs:1211` |
| 154 | D4: current empty queue gives -113 text; non-empty queue is cleared and no error/reply results. Target always queues -113 after the clear and returns empty terminated reply | `ui/src/scpi.rs:355`, `:407` |
| 155..162 | Command effect followed by fallback -113; target keeps error queued and returns empty terminated reply | `ui/src/scpi.rs:355`, `:411`, `:432`, `:903`, `ui/src/scpi/actions.rs:7` |

D4 is a defect in the unanswered-query fallback: `before` captures nonzero queue
length, *CLS? clears it, the equality check fails to synthesize -113, and
`err.last()` is None (`ui/src/scpi.rs:355-372`). This violates the target even
after successful-empty framing is fixed. Corpus fixtures must cover empty,
one-error and full queues, a following `:SYST:ERR?`, and compound response slots.
The target rule has no queue-state exception.

## VXI-11 procedures beyond read/write

These are RPC operations, separate from the 162 SCPI header-path rows. All
listed baseline procedures return error 0 except resource exhaustion on create;
RPC errors are distinct from the shared SCPI error queue. The link buffer holds
response bytes, not device acquisition state. The Command/Query column records
target mappings; link lifecycle remains transport-owned.

| Requirement | Procedure / number | Current result and effect | Command/Query mapping | Disposition | Evidence |
|---|---|---|---|---|---|
| scpi-400 | create_link / 10 | Reuse first free slot or allocate up to 16 links; full returns error 9 with other reply fields 0; success: error=0, lid, abortPort=0, maxRecvSize=1048576 (1 MiB). Ignores clientId, lockDevice, lock_timeout and device | Transport-owned link allocation | keep limits and allocation behavior | `ui/src/vxi11.rs:110`, `:120`, `:223` |
| scpi-401 | device_readstb / 13 | Always error=0, stb=0, even for an invalid lid; no status register model | Q: transport constant 0 | keep stub; do not infer *STB support | `ui/src/vxi11.rs:193` |
| scpi-402 | device_trigger / 14 | Always error=0; no-op, does not call :TFORce | C: same as scpi-7, tbd at M14 | change proposal D6: map to :TFORce, including RUN/force-request effects | `ui/src/vxi11.rs:198`, `ui/src/scpi.rs:436` |
| scpi-403 | device_clear / 15 | Clears only selected link's output buffer if present; always error=0. Does not clear SCPI errors or picture/history | Transport buffer clear + C: same queue clear as scpi-3, tbd at M14 | change proposal D7: also clear shared SCPI error queue; preserve acquisition/picture state | `ui/src/vxi11.rs:198`, `ui/src/scpi.rs:407` |
| scpi-404 | device_remote / 16 | Always error=0; no-op | Transport-owned no-op | keep; no remote-control lockout | `ui/src/vxi11.rs:198` |
| scpi-405 | device_local / 17 | Always error=0; no-op | Transport-owned no-op | keep | `ui/src/vxi11.rs:198` |
| scpi-406 | device_lock / 18 | Always error=0; no-op | Transport-owned no-op | keep; no exclusive ownership is granted | `ui/src/vxi11.rs:198` |
| scpi-407 | device_unlock / 19 | Always error=0; no-op | Transport-owned no-op | keep | `ui/src/vxi11.rs:198` |
| scpi-408 | destroy_link / 23 | Sets selected slot to None, discarding its output; invalid/missing lid is ignored; always error=0 | Transport-owned link destruction | keep | `ui/src/vxi11.rs:209` |

D6 makes the transport trigger useful to clients expecting a device trigger and
shares the SCPI action rather than introducing a second acquisition writer.
D7 makes device clear reset pending protocol replies and stale command errors
together, so a client can recover its protocol session without an extra *CLS.
Because the SCPI queue is shared, D7 also clears errors visible to other clients;
that client-visible change is intentional in the proposal. Neither proposal is
implemented or hardware-verified in this documentation task.
