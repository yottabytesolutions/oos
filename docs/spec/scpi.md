# Legacy SCPI reference

Source baseline: merged `overhaul` c59b71d5, 2026-11-16. This is a source-derived contract
for the M15 rebuild, not a claim of hardware validation. No unit was accessed.
[ADR 0017](../architecture/adr/0017-hybrid-after-m12.md) requires the legacy parser
as a host differential oracle. Dispositions are **keep** by default; **change**
for the cited defects, with coordinator decisions and remaining proposals
labelled. Current **changed** dispositions identify merged behavior relative to
the old fork; they do not mean a proposed implementation change in this task.
The merged inventory preserves scpi-1..153 for existing headers and retires
scpi-154..162 as accepted side-effect aliases (their rejected-query contracts
remain as tombstones). New paths start at scpi-500; scpi-300..325 are
cross-cutting and catalogue requirements and scpi-400..408 remain RPC requirements. Counts and
validator differences are recorded below. A both row covers command and query;
numeric channel/slot expansions are not counted separately.

Mixed case encodes both spellings: `:CHANnel<n>:SCALe` means long
`:CHANNEL<n>:SCALE`, short `:CHAN<n>:SCAL`. Each node independently accepts its
long or short spelling, in any letter case. `[?]` is represented by kind **both**;
**C** means command, **Q** query. A both row covers the setter and its query.
Finite alias headers have their own rows; arbitrary ignored-node aliases use
the acceptance predicates below. `<n>` expands a numeric suffix, not a new
requirement per physical channel. Parameter-value catalogues follow the headers.
Evidence uses repository-relative `file:line` anchors at the merged baseline.

Table conventions apply to every row:

- **Command/Query mapping** names the future typed boundary, separately from the
  current model/action column. `C: tbd at M14`, `Q: tbd at M14` or
  `C/Q: tbd at M14` means the command,
  query or both variants still need names at M14; it does not assert a current
  typed parser mapping. M15 must fill those entries before parser acceptance
  (`docs/architecture/extending.md`, Add an SCPI command).
- **Errors** for every header row follow scpi-320's range/override map. Successful
  rows add no error unless that map or the row explicitly says otherwise.
- **T/V/M**: TCP SCPI (default 5555), VXI-11 and MCP `scpi` tool.
  All call the same parser; MCP is a text wrapper and does not preserve arbitrary binary
  replies. See transport rules below.
- **B**: `ON`/`1`, `OFF`/`0`, reply `1` or `0`. **F**: finite Rust `f32` decimal
  or exponent without a unit suffix. **D**: finite `f64`. **E**: seven significant
  digit exponential reply, e.g. `2.000000E-04`; unavailable/nonfinite `9.9E37`.
- **none**: no reply for a command. Query errors follow the error rule below.
  Unless specified, query parameters are ignored. Settings have no independent
  SCPI defaults: they use the current model/setup. Parser-owned initial defaults
  are waveform source CH1, BYTE format, NORM mode, STARt=1, STOP=0
  (through end), zero event status, no OPC events, empty error queue
  (`ui/src/scpi.rs:712`).
- Normal setters mutate the legacy model and the intake/change bridge applies
  normalization; this can invalidate records, restart hardware, or clear derived
  state. `ui/src/scpi.rs:870`, `ui/src/ui/change.rs:154` and
  `ui/src/ui/change/acquisition.rs:32` locate that boundary. Rows name extra effects.
  No setter implies waiting for hardware completion. The early model dispatch
  now calls `mark_changed` only for writes; successful queries preserve state (`ui/src/scpi.rs:1695`).
- **Subset** means covered by the file's Rigol-compatible subset claim, not
  verified against stock firmware. **OOS** and **fork debug** mean extensions
  identified in code. Availability is compile-time NCH (4) unless a row says
  installed channels, generator or LA capability. `ui/src/board/dho/mod.rs:78`,
  `ui/src/model.rs:21` and `ui/src/scpi.rs:475` distinguish these limits.


## Common commands and acquisition actions

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-1 | `*IDN` | both | none | OOS,model,serial-or-0,firmware | C/Q: tbd at M14 | platform + cached firmware; accepts without ? too | T/V/M | Subset; keep | `ui/src/scpi.rs:992` |
| scpi-2 | `*OPC` | both | none | Q: 1 on completion; C: none | C/Q: tbd at M14 | Q captures finite-operation fence and defers on T/V/H; C latches ESR bit 0 on completion, up to 64 pending events; see scpi-311 | T/V/M | changed: upstream operation tracking | `ui/src/scpi.rs:888`, `ui/src/ui/scpi_operations.rs:19` |
| scpi-3 | `*CLS` | C | none | none | C: tbd at M14 | Clear shared errors, event status and pending OPC events | T/V/M | changed: also clears ESR/OPC | `ui/src/scpi.rs:1013` |
| scpi-4 | `*RST` | C | none | none | C: tbd at M14 | Clear event status/OPC events, then DEFAULT key; reset setup | T/V/M | changed: also clears ESR/OPC | `ui/src/scpi.rs:1019` |
| scpi-5 | `:AUToscale` | both | none | Q: B | C/Q: tbd at M14 | AUTO key starts asynchronous autoset; Q reads auto_running | T/V/M | Subset; keep | `ui/src/scpi.rs:1073` |
| scpi-6 | `:CLEar` | C | none | none | C: tbd at M14 | CLEAR key clears picture/history according to UI action | T/V/M | Subset; keep | `ui/src/scpi.rs:1081` |
| scpi-7 | `:TFORce` | C | none | none | C: tbd at M14 | ui.force_trigger; route physical/reference/mixed acquisition and release stopped run | T/V/M | changed: shared force routing | `ui/src/scpi.rs:1041` |
| scpi-8 | `:RUN` | C | none | none | C: tbd at M14 | Physical: run_request if stopped, Run, Single to Auto. Reference: idempotent record_key RUN, Single to Normal; mixed-bus router takes precedence | T/V/M | changed: source-aware RUN | `ui/src/scpi.rs:1027`, `ui/src/scpi.rs:1103`, `ui/src/scpi/actions.rs:7` |
| scpi-9 | `:STOP` | C | none | none | C: tbd at M14 | Physical: run=Stop. Reference/mixed/SIM: routed STOP also stops replay or cancels generation; repeated STOP is idempotent | T/V/M | changed: source-aware STOP | `ui/src/scpi.rs:1027`, `ui/src/scpi.rs:1098` |
| scpi-10 | `:SINGle` | C | none | none | C: tbd at M14 | Arm exact operation identity; physical change::single or reference/mixed router; trig_mode=Single, Run; completion asynchronous | T/V/M | changed: operation fence and reference routing | `ui/src/scpi.rs:1027`, `ui/src/scpi.rs:1223` |

## Channels and time base

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-11 | `:CHANnel<n>:DISPlay` | both | n=1..4; B | B | C/Q: tbd at M14 | ch[n-1].on | T/V/M | Subset; keep | `ui/src/scpi.rs:1317` |
| scpi-12 | `:CHANnel<n>:SCALe` | both | n=1..4; F > 0 V/div at probe; nearest log-distance ladder 100 uV..10 V/div BNC, times probe; fine rounds up calibrated row | E V/div | C/Q: tbd at M14 | ch.vscale,vfine; divides by probe; with global v_expand_gnd scales offset by new/old V/div | T/V/M | changed: ground expansion | `ui/src/scpi.rs:1322` |
| scpi-13 | `:CHANnel<n>:VERNier` | both | n=1..4; B | B | C/Q: tbd at M14 | m.fine_v global switch; disabling retains current fine scale until a coarse step | T/V/M | changed: retain fine scale on OFF | `ui/src/scpi.rs:1267` |
| scpi-14 | `:CHANnel<n>:OFFSet` | both | n=1..4; F V, no parser magnitude limit | E V | C/Q: tbd at M14 | ch.offset | T/V/M | Subset; keep | `ui/src/scpi.rs:1333` |
| scpi-15 | `:CHANnel<n>:COUPling` | both | n=1..4; DC, AC, GND; DC50 only when board offers it | DC / AC / GND / DC50 | C/Q: tbd at M14 | ch.coupling | T/V/M | DC50 capability extension; keep | `ui/src/scpi.rs:1357` |
| scpi-16 | `:CHANnel<n>:BWLimit` | both | n=1..4; OFF/FULL, AUTO, GND, 20M/20MHZ; BW catalogue below | OFF, AUTO, 20M or uppercase table label without spaces | C/Q: tbd at M14 | ch.bw | T/V/M | Extended filter choices; keep | `ui/src/scpi.rs:1370` |
| scpi-17 | `:CHANnel<n>:PROBe` | both | n=1..4; F > 0, dimensionless attenuation, no upper parser limit | E | C/Q: tbd at M14 | ch.probe | T/V/M | Subset; keep | `ui/src/scpi.rs:1399` |
| scpi-18 | `:CHANnel<n>:INVert` | both | n=1..4; B | B | C/Q: tbd at M14 | ch.invert | T/V/M | Subset; keep | `ui/src/scpi.rs:1404` |
| scpi-19 | `:TIMebase:SCALe` | both | F s/div; nearest log ladder 50 ps..1000 s/div; no positivity check | E s/div | C/Q: tbd at M14 | m.tscale; tfine=1 | T/V/M | changed: display ladder starts at 50 ps, hardware minimum 1 ns | `ui/src/scpi.rs:1443` |
| scpi-20 | `:TIMebase:OFFSet` | both | F s, no parser magnitude limit | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:1452` |
| scpi-21 | `:TIMebase:POSition` | both | F s, alias of OFFSet | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:1452` |
| scpi-22 | `:TIMebase:MAIN:SCALe` | both | F s/div; nearest log ladder 50 ps..1000 s/div; no positivity check | E s/div | C/Q: tbd at M14 | m.tscale; tfine=1 | T/V/M | changed: display ladder starts at 50 ps, hardware minimum 1 ns | `ui/src/scpi.rs:1443` |
| scpi-23 | `:TIMebase:MAIN:OFFSet` | both | F s, no parser magnitude limit | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:1452` |
| scpi-24 | `:TIMebase:MAIN:POSition` | both | F s, alias of OFFSet | E s | C/Q: tbd at M14 | m.delay | T/V/M | Subset / position alias; keep | `ui/src/scpi.rs:1452` |

## Trigger and counter

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-25 | `:TRIGger:MODE` | both | EDGE/PULSE/PULS/SLOPE/SLOP/VIDEO/VID/PATTERN/PATT/DURATION/DUR/TIMEOUT/TIM/RUNT/WINDOW/WIND/DELAY/DEL/SETUPHOLD/SETUP/SET/NTHEDGE/NTH/NEDGE/NEDG/BUS | EDGE/PULS/SLOP/VID/PATT/DUR/TIM/RUNT/WIND/DEL/SET/NEDG/BUS | C/Q: tbd at M14 | ui.trigger_type_set actual trig_type; invalid -224; does not select RS232/IIC/SPI protocol | T/V/M | changed: upstream actual type selection; D2 partly met | `ui/src/scpi.rs:1154` |
| scpi-26 | `:TRIGger:STATus` | both | ignored | STOP / TD / AUTO / WAIT | C/Q: tbd at M14 | m.run,status; replies even without ? | T/V/M | Subset; keep | `ui/src/scpi.rs:1467` |
| scpi-27 | `:TRIGger:COUPling` | both | DC / AC / LFR / LFREJECT / HFR / HFREJECT | DC / AC / LFR / HFR | C/Q: tbd at M14 | m.trig_coup | T/V/M | Subset; keep | `ui/src/scpi.rs:1480` |
| scpi-28 | `:TRIGger:NREJect` | both | B | B | C/Q: tbd at M14 | m.trig_nrej | T/V/M | Subset; keep | `ui/src/scpi.rs:1503` |
| scpi-29 | `:TRIGger:HOLDoff` | both | F 0..10 s | E s | C/Q: tbd at M14 | m.holdoff | T/V/M | Subset; keep | `ui/src/scpi.rs:1516` |
| scpi-30 | `:TRIGger:SWEep` | both | prefix AUTO / NORM / SING | AUTO / NORM / SING | C/Q: tbd at M14 | m.trig_mode; running SING prefix allocates new Single operation identity | T/V/M | changed: fresh running Single arm | `ui/src/scpi.rs:1223`, `ui/src/scpi.rs:1483` |
| scpi-31 | `:TRIGger:EDGE:SOURce` | both | CHANnel1..4 / R1..R10 / REF1..REF10; D0..D15 through LA fallback | CHANn / Rn / Dn | C/Q: tbd at M14 | ui.set_trigger_source; primary reference precedes stale LA selection; unavailable/invalid non-D source -224; D fallback failure -113 | T/V/M | changed: reference-aware trigger router | `ui/src/scpi.rs:1124` |
| scpi-32 | `:TRIGger:EDGE:LEVel` | both | F V, no parser magnitude limit | E V | C/Q: tbd at M14 | m.trig_level | T/V/M | Subset; keep | `ui/src/scpi.rs:1559` |
| scpi-33 | `:TRIGger:EDGE:SLOPe` | both | prefix POS / NEG / RFAL | POS / NEG / RFAL | C/Q: tbd at M14 | m.slope | T/V/M | Subset; keep | `ui/src/scpi.rs:1564` |
| scpi-34 | `:COUNter:ENABle` | both | B | B | C/Q: tbd at M14 | m.cnt_on | T/V/M | Subset; keep | `ui/src/scpi.rs:4011` |
| scpi-35 | `:COUNter:SOURce` | both | CHANnel1..4; invalid -224 | CHANn | C/Q: tbd at M14 | m.cnt_src | T/V/M | changed: fixed NCH source parsing and -224 errors | `ui/src/scpi.rs:4028` |
| scpi-36 | `:COUNter:MODE` | both | FREQuency / PERiod / TOTalize (exact long/short) | FREQ / PER / TOT | C/Q: tbd at M14 | m.cnt_mode | T/V/M | changed: exact long/short enums and -224 errors | `ui/src/scpi.rs:4044` |
| scpi-37 | `:COUNter:NDIGits` | both | decimal u32 3..7 digits; invalid -224 | decimal integer | C/Q: tbd at M14 | m.cnt_digits | T/V/M | changed: 7 digits accepted | `ui/src/scpi.rs:4074` |
| scpi-38 | `:COUNter:CURRent` | Q | none | E Hz / s / count according to mode; 9.9E37 unavailable | Q: tbd at M14 | ui.counter_value() | T/V/M | Subset; keep | `ui/src/scpi.rs:4086` |
| scpi-39 | `:COUNter:TOTalize:CLEar` | C | none | none | C: tbd at M14 | ui.counter_clear() | T/V/M | Subset; keep | `ui/src/scpi.rs:4090` |

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
| scpi-74 | `:ACQuire:MDEPth` | both | AUTO prefix or depth catalogue; decimal/exponent plus K/M, rounded samples; f64 parse has no finite/sign guard, NaN/negative cast to 0 (AUTO); unmatched depth -113 | E live depth in samples, not stored menu index | C/Q: tbd at M14 | m.mdepth; Q src.rate_depth(m) | T/V/M | Subset; keep | `ui/src/scpi.rs:1586` |
| scpi-75 | `:ACQuire:SRATe` | Q | none | E samples/s | Q: tbd at M14 | src.rate_depth(m) | T/V/M | Subset; keep | `ui/src/scpi.rs:1684` |
| scpi-76 | `:ACQuire:TYPE` | both | NORMal / PEAK / AVERages / HRESolution (exact long/short); invalid -224 | NORM / PEAK / AVER / HRES | C/Q: tbd at M14 | m.acq | T/V/M | changed: strict enums and -224 | `ui/src/scpi.rs:1642` |
| scpi-77 | `:ACQuire:AVERages` | both | decimal u32 >=2; floor to 2,4,8,16,32,64,128,256,512,1024; above 1024 saturates | decimal count | C/Q: tbd at M14 | m.avg_count | T/V/M | changed: floor instead of rejecting non-ladder values; bad parse/<2 -224 | `ui/src/scpi.rs:1667` |
| scpi-78 | `:ACQuire:AVERages:HARDware` | both | B | B | C/Q: tbd at M14 | m.avg_hw | T/V/M | OOS FPGA average switch; keep | `ui/src/scpi.rs:1630` |
| scpi-79 | `:ACQuire:SEGMents` | both | OFF/0 or usize, clamp to 100000 segments; 1 is numeric one, ON invalid | decimal count | C/Q: tbd at M14 | m.segments; 0 disables | T/V/M | OOS; keep | `ui/src/scpi.rs:1612` |
| scpi-80 | `:ACQuire:SEGMents:HARDware` | both | B | B | C/Q: tbd at M14 | m.seg_hw | T/V/M | OOS; keep | `ui/src/scpi.rs:1622` |
| scpi-81 | `:ACQuire:SEGMents:INTerval` | both | F clamped 0..10 s | E s | C/Q: tbd at M14 | m.seg_ivl | T/V/M | OOS; keep | `ui/src/scpi.rs:1624` |
| scpi-82 | `:ACQuire:SEGMents:TIMes` | Q | none | comma-separated lowercase-exponent seconds since first trigger; 9.9E37 missing; empty if none | Q: tbd at M14 | ui.seg_times_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:2403` |

## Measurements

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-83 | `:MEASure:ITEM` | both | item[,wave[,wave]]; scpi-300; default Ui meas_source (initial CH1); pair C defaults both sources, pair Q requires explicit B except SETHold | E in item units; invalid/unavailable 9.9E37 | C/Q: tbd at M14 | C meas_place/meas_add(_pair); duplicate no-op; combined single/pair/catalogue cap 1024; when full, evict the first screen row (standard, then pair, then catalogue) and reset statistics. Q fact_meas/pair_at; single Q ignores extra sources; invalid item/source -224 | T/V/M | changed: upstream readout replaces fork; task 0080 restores drop-oldest | `ui/src/scpi.rs` meas_place, `ui/src/ui/tools.rs` meas_add / meas_add_pair |
| scpi-84 | `:MEASure:CLEar` | C | no argument or ALL: whole list; ITEM<n>: 1-based current screen index (standard, then pair, then catalogue) | none | C: tbd at M14 | meas_clear_remote / remove_measurement; valid removal resets statistics; invalid or missing row -224 with no mutation, also on an empty list; clearing all is idempotent | T/V/M | changed: task 0080 restores ITEM<n> | `ui/src/scpi/actions.rs` meas_clear, `ui/src/ui/tools.rs` remove_measurement |
| scpi-85 | `:MEASure:STATistic:DISPlay` | both | B | B | C/Q: tbd at M14 | ui.set_stats_shown / stats_shown; invalid B -224 | T/V/M | changed: upstream statistics owner | `ui/src/scpi.rs:2520` |
| scpi-86 | `:MEASure:STATistic:RESet` | C | none | none | C: tbd at M14 | ui.stats_reset resets history, preserves selected rows | T/V/M | changed: upstream statistics owner | `ui/src/scpi.rs:2542` |
| scpi-87 | `:MEASure:STATistic:ITEM` | both | C: item[,wave[,wave]] adds row and enables stats. Q: MAXimum/MINimum/CURRent/AVERages/DEViation/CNT/COUNt,item[,wave[,wave]]; default primary/secondary sources | E in item units; CNT E count; missing history 0 count, unavailable 9.9E37 | C/Q: tbd at M14 | meas_place + set_stats_shown; Q meas_remote_stat/pair_remote_stat; current can exist without selected history; invalid -224, placement full errors as scpi-83 | T/V/M | changed: setter and reference statistics | `ui/src/scpi.rs:2520` |

## Waveforms and logic analyser

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-88 | `:WAVeform:SOURce` | both | CHANnel1..4 / R1..R10 / REF1..REF10; empty ignored, invalid nonempty -224 | CHANn / Rn | C/Q: tbd at M14 | Scpi.wav_src shared by all clients | T/V/M | changed: references and strict invalid source | `ui/src/scpi.rs:1899` |
| scpi-89 | `:WAVeform:MODE` | both | NORMal / MAXimum / RAW; invalid -224, preserve selection | NORM / MAX / RAW | C/Q: tbd at M14 | Scpi.wav_mode 0/1/2; RAW memory, MAX memory only while STOP, NORM screen; refs always stored samples | T/V/M | changed: MAX mode and strict enum | `ui/src/scpi.rs:1913`, `ui/src/scpi.rs:4262` |
| scpi-90 | `:WAVeform:FORMat` | both | BYTE / WORD / ASCii; invalid -224, preserve format | WORD / ASC / BYTE | C/Q: tbd at M14 | Scpi.wav_fmt | T/V/M | changed: strict enum | `ui/src/scpi.rs:1930` |
| scpi-91 | `:WAVeform:POINts` | both | C u64 >=1 points starting at STARt; Q ignores arguments | decimal count | C/Q: tbd at M14 | page_count; setter wav_stop=saturating_add(wav_start,n-1) | T/V/M | changed: paging and count setter; bad count -224 | `ui/src/scpi.rs:1986` |
| scpi-92 | `:WAVeform:PREamble` | Q | none | 10 comma-separated fields; see waveform rule | Q: tbd at M14 | wave_preamble: page count/origin and selected mode; no sample copy | T/V/M | changed: page metadata | `ui/src/scpi.rs:4303` |
| scpi-93 | `:WAVeform:DATA` | Q | none | IEEE block: BYTE, LE WORD or ASCII volts; empty block if no data | Q: tbd at M14 | wave_data reads selected page, no implicit future record wait; pre-copy size check -223 when count*width+11 >64 MiB, ASCII estimate width=14 | T/V/M | changed: pages and bounded allocation; retain LE WORD | `ui/src/scpi.rs:4846` |
| scpi-94 | `:LA:STATe` | both | B | B | C/Q: tbd at M14 | m.la.on; mark_changed | T/V/M | Rigol names where present; LA capability required; keep | `ui/src/scpi.rs:4371` |
| scpi-95 | `:LA:ENABle` | both | B | B | C/Q: tbd at M14 | alias of LA:STATe | T/V/M | LA capability required; keep | `ui/src/scpi.rs:4371` |
| scpi-96 | `:LA:DIGital<n>:DISPlay` | both | n=0..15; B | B | C/Q: tbd at M14 | m.la.show bit n; no mark_changed here | T/V/M | LA capability required; keep | `ui/src/scpi.rs:4384` |
| scpi-97 | `:LA:POD<n>:THReshold` | both | n=1..2; f32 abs<=15 V; nan rejected by comparison | lowercase-exponent V | C/Q: tbd at M14 | m.la.preset=USER, la.user; mark_changed | T/V/M | LA capability required; keep | `ui/src/scpi.rs:4398` |
| scpi-98 | `:LA:POINts` | Q | none | decimal sample count; 0 and -230 if no record | Q: tbd at M14 | ui.la_record.words.len() | T/V/M | LA capability required; keep | `ui/src/scpi.rs:4412` |
| scpi-99 | `:LA:PREamble` | Q | none | points,dt,t0 with lowercase exponents (s); 0 and -230 if no record | Q: tbd at M14 | ui.la_record() | T/V/M | OOS data contract; LA capability required; keep | `ui/src/scpi.rs:4412` |
| scpi-100 | `:LA:DATA` | Q | none | IEEE block of LE u16, bit n=Dn; empty block and -230 if no record | Q: tbd at M14 | ui.la_record() | T/V/M | LA capability required; keep | `ui/src/scpi.rs:4412` |

## Display, views, calibration and Bode

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-101 | `:DISPlay:DATA` | Q | first word: empty/BMP prefix or PNG prefix; invalid -224 | IEEE #9 block: BMP (default) or PNG canvas bytes | Q: tbd at M14 | bmp(cv) / store::png_of(cv), synchronous encoding | T/V/M | changed: PNG and format validation | `ui/src/scpi.rs:2019` |
| scpi-102 | `:DISPlay:TYPE` | both | VECTors / DOTS / AUTO | VECT / DOTS / AUTO | C/Q: tbd at M14 | ui.set_dots(m.dots) | T/V/M | Stock VECT/DOTS spellings, OOS AUTO; keep | `ui/src/scpi.rs:3088` |
| scpi-103 | `:DISPlay:VIEW` | both | decimal usize 1..m.views | decimal active view, 1-based | C/Q: tbd at M14 | view_go; switch saved view | T/V/M | OOS; keep | `ui/src/scpi.rs:2032` |
| scpi-104 | `:DISPlay:VIEW:COUNt` | both | decimal usize 1..4 | decimal number of views | C/Q: tbd at M14 | views_set(m.views) | T/V/M | OOS; keep | `ui/src/scpi.rs:2032` |
| scpi-105 | `:VIEW` | both | SCOPe / SPECtrum / EYE / POWer / BODE / MAGnetic / INVerter / DPT; BANDwidth and STEP rejected -224 | uppercase Rust AView Debug, including MAGNETIC, INVERTER, DPT and internal STEP | C/Q: tbd at M14 | scpi_views registry + set_analysis_view; STEP absent from registry | T/V/M | changed: expanded registry; D5 still pending | `ui/src/scpi.rs:2059`, `ui/src/ui/analysers.rs:81` |
| scpi-106 | `:IDENtify` | Q | none | semicolon-separated CHn[+CHm] summary (confidence %); empty if no findings | Q: tbd at M14 | identify_text runs analysis synchronously | T/V/M | OOS; keep | `ui/src/scpi.rs:2071` |
| scpi-107 | `:CALibrate:STATus` | Q | none | cal_status human status string | Q: tbd at M14 | ui.cal_status_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:2075` |
| scpi-108 | `:CALibrate:STARt` | C | 0..4[,ZERO]; 0=all; parser actually invalid=>0, >4=>4 | none | C: tbd at M14 | selfcal_remote; starts asynchronous calibration without dialog; inputs must be disconnected; can persist calibration | T/V/M | OOS; documentation only, no authorization to run; keep | `ui/src/scpi.rs:2081` |
| scpi-109 | `:CALibrate:CANCel` | C | none | none | C: tbd at M14 | selfcal_remote(cancel,false) | T/V/M | OOS; keep | `ui/src/scpi.rs:2087` |
| scpi-110 | `:BODE:RUN` | C | B; invalid/missing treated ON | none | C: tbd at M14 | ui.bode_run; starts/stops asynchronous sweep, controls external generator | T/V/M | OOS; keep | `ui/src/scpi.rs:2093` |
| scpi-111 | `:BODE:DATA` | Q | none | f_Hz,gain_dB(3 decimals),phase_deg(2 decimals); rows separated by semicolon; empty if none | Q: tbd at M14 | ui.bode_text() | T/V/M | OOS; keep | `ui/src/scpi.rs:2097` |
| scpi-112 | `:BODE:GENerator` | both | usb or optionally quoted single-word endpoint; empty accepted; whitespace -224 | usb or endpoint (may be empty) | C/Q: tbd at M14 | m.bode_usb,bode_addr; retains LAN address on USB | T/V/M | OOS; keep | `ui/src/scpi.rs:2125` |

## Built-in generator

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-113 | `:SOURce[<n>]:OUTPut` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | afg_set(on); generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:2169` |
| scpi-114 | `:SOURce[<n>]:OUTPut:STATe` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | afg_set(on); generator capability required | T/V/M | Alias; keep | `ui/src/scpi.rs:2169` |
| scpi-115 | `:SOURce[<n>]:FUNCtion` | both | n defaults 1; 1..installed generators; SINusoid/SQUare/RAMP/PULSe/NOISe/DC/ARB | SIN/SQU/RAMP/PULS/NOIS/DC/ARB | C/Q: tbd at M14 | m.afg_wave; afg_set clamps; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:2271` |
| scpi-116 | `:SOURce[<n>]:FUNCtion:SQUare:DUTY` | both | n defaults 1; 1..installed generators; D clamped 5..95 % | E % | C/Q: tbd at M14 | m.afg_duty; generator capability required | T/V/M | keep | `ui/src/scpi.rs:2259` |
| scpi-117 | `:SOURce[<n>]:FUNCtion:PULSe:DUTY` | both | n defaults 1; 1..installed generators; D clamped 5..95 % | E % | C/Q: tbd at M14 | m.afg_duty, same as square; generator capability required | T/V/M | keep | `ui/src/scpi.rs:2259` |
| scpi-118 | `:SOURce[<n>]:FUNCtion:RAMP:SYMMetry` | both | n defaults 1; 1..installed generators; D clamped 0..100 % | E % | C/Q: tbd at M14 | m.afg_symmetry; generator capability required | T/V/M | keep | `ui/src/scpi.rs:2259` |
| scpi-119 | `:SOURce[<n>]:FREQuency` | both | n defaults 1; 1..installed generators; D Hz clamped 0.01..wave/platform maximum; generator limits below | E Hz | C/Q: tbd at M14 | afg_set(freq); may clamp amplitude, offset, FM deviation; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:2286` |
| scpi-120 | `:SOURce[<n>]:VOLTage` | both | n defaults 1; 1..installed generators; D Vpp; clamp by frequency, 0.002..10/5/2.5 Vpp | E Vpp | C/Q: tbd at M14 | afg_set(ampl); may clamp offset; generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:2288` |
| scpi-121 | `:SOURce[<n>]:VOLTage:AMPLitude` | both | n defaults 1; 1..installed generators; D Vpp, same limits | E Vpp | C/Q: tbd at M14 | afg_set(ampl); generator capability required | T/V/M | Alias; keep | `ui/src/scpi.rs:2288` |
| scpi-122 | `:SOURce[<n>]:VOLTage:OFFSet` | both | n defaults 1; 1..installed generators; D V; DC +/-5 V, otherwise +/-(Vmax-amplitude)/2 | E V | C/Q: tbd at M14 | afg_set(offset); generator capability required | T/V/M | Subset; keep | `ui/src/scpi.rs:2288` |
| scpi-123 | `:SOURce[<n>]:MOD` | both | n defaults 1; 1..installed generators; B; ON restores at least AM, OFF loses previous type | B | C/Q: tbd at M14 | m.afg_mod; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:2220` |
| scpi-124 | `:SOURce[<n>]:MOD:STATe` | both | n defaults 1; 1..installed generators; B | B | C/Q: tbd at M14 | m.afg_mod; generator capability required | T/V/M | Rigol DG style alias; keep | `ui/src/scpi.rs:2220` |
| scpi-125 | `:SOURce[<n>]:MOD:TYPe` | both | n defaults 1; 1..installed generators; AM/FM/PM | NONE/AM/FM/PM | C/Q: tbd at M14 | m.afg_mod; afg_set clamps; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:2209` |
| scpi-126 | `:SOURce[<n>]:MOD:AM:DEPTh` | both | n defaults 1; 1..installed generators; D clamped 0..120 % | E % | C/Q: tbd at M14 | m.afg_mod_depth; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:2203` |
| scpi-127 | `:SOURce[<n>]:MOD:FM:DEViation` | both | n defaults 1; 1..installed generators; D min 0.01 Hz, then min(carrier,max-carrier), floor 0.01 | E Hz | C/Q: tbd at M14 | m.afg_fm_dev; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:2207` |
| scpi-128 | `:SOURce[<n>]:MOD:PM:DEViation` | both | n defaults 1; 1..installed generators; D clamped 0..360 degrees | E degrees | C/Q: tbd at M14 | m.afg_pm_dev; afg_set; generator capability required | T/V/M | Rigol DG style; keep | `ui/src/scpi.rs:2205` |
| scpi-129 | `:SOURce[<n>]:MOD:AM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2201` |
| scpi-130 | `:SOURce[<n>]:MOD:AM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2188` |
| scpi-131 | `:SOURce[<n>]:MOD:FM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2201` |
| scpi-132 | `:SOURce[<n>]:MOD:FM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2188` |
| scpi-133 | `:SOURce[<n>]:MOD:PM:INTernal:FREQuency` | both | D clamped 0.002..1e6 Hz | E Hz | C/Q: tbd at M14 | m.afg_mod_rate shared across types; afg_set clamps | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2201` |
| scpi-134 | `:SOURce[<n>]:MOD:PM:INTernal:FUNCtion` | both | SINusoid/SQUare/TRIangle/RAMP/NRAMp/NOISe | SIN/SQU/TRI/RAMP/NRAM/NOIS | C/Q: tbd at M14 | m.afg_mod_shape shared across types | T/V/M | Rigol DG style; generator capability required; keep | `ui/src/scpi.rs:2188` |
| scpi-135 | `:SOURce[<n>]:ARB:DATA` | C | comma-separated F samples, arbitrary scale; at least 2 finite parsed samples; line size is practical bound | none | C: tbd at M14 | afg_arb_load selects ARB, resamples to 8192 s16 codes; bad tokens silently skipped | T/V/M | OOS; generator capability required; keep | `ui/src/scpi.rs:2243` |
| scpi-136 | `:SOURce[<n>]:ARB:CAPTure` | C | CHANnel1..4; current channel record required | none | C: tbd at M14 | afg_arb_capture selects ARB and writes arb.csv through store | T/V/M | OOS; generator capability required; keep | `ui/src/scpi.rs:2248` |

## System, setup access and debug

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-137 | `:SYSTem:SET` | C | key=value via store::set; emitted-key validation with optional suffixes vfine/unit/bias/skew/label/tfine, math_expr prefix and cat exception | none | C: tbd at M14 | store::set(&mut m), mark_changed; errors -224 | T/V/M | changed: cat accepted even empty/omitted | `ui/src/scpi.rs:2302` |
| scpi-138 | `:SYSTem:GET` | Q | setup key, exact case | serialized value; empty when omitted/unknown | Q: tbd at M14 | store::setup_text lookup; no error for missing key | T/V/M | OOS; keep with empty-reply fix D1 | `ui/src/scpi.rs:2328` |
| scpi-139 | `:SYSTem:STATe` | Q | none | diagnostic line; state-query.md | Q: tbd at M14 | cmd::stat_text | T/V/M | OOS; keep | `ui/src/scpi.rs:2336` |
| scpi-140 | `:SYSTem:GRAPhics` | both | CPU/GPU | CPU/GPU (requested, not necessarily active) | C/Q: tbd at M14 | ui.graphics_set; changes renderer preference | T/V/M | OOS; keep | `ui/src/scpi.rs:2359` |
| scpi-141 | `:SYSTem:AOUTput` | both | TOUT/OFF/PFA/PFAIL; PFA only with mask/zone HW offered | TOUT/OFF/PFA | C/Q: tbd at M14 | m.aux_out; OFF constant low | T/V/M | Stock TOUT/PFAil; OFF is OOS; keep | `ui/src/scpi.rs:2371` |
| scpi-142 | `:SYSTem:LOG` | C | B | none | C: tbd at M14 | ui.log_set; logging IO | T/V/M | OOS; keep | `ui/src/scpi.rs:2394` |
| scpi-143 | `:SYSTem:ERRor` | Q | none | code,"message" plus optional context; 0,"No error" when empty | Q: tbd at M14 | self.err.remove(0), consumes oldest error | T/V/M | Subset; keep | `ui/src/scpi.rs:2460` |
| scpi-144 | `:DECode:EVENts` | Q | none | newline-separated t_s,decoder,content,ok/error; empty if none; BUSY while decoding; -230 text on failed result | Q: tbd at M14 | decode_events_result; earlier decode_cmd intercepts old events_text route | T/V/M | changed: background reference decoding | `ui/src/scpi.rs:4097` |
| scpi-145 | `:DEBug:SLOTs` | Q | none | raw measurement slot diagnostic text | Q: tbd at M14 | ui.debug_slots(); schedules raw measure-unit run, returns previous result | T/V/M | fork debug; keep | `ui/src/scpi.rs:1070` |
| scpi-146 | `:SYSTem:DEBug:UART` | Q | none | board UART/protocol diagnostic text; off,enabled=0 when off | Q: tbd at M14 | board().uart_debug | T/V/M | fork debug; keep | `ui/src/scpi.rs:2410` |
| scpi-147 | `:SYSTem:DEBug:SERial` | Q | none | same as UART | Q: tbd at M14 | board().uart_debug | T/V/M | fork debug alias; keep | `ui/src/scpi.rs:2410` |
| scpi-148 | `:SYSTem:DEBug:ZONE` | Q | none | zone diagnostic text | Q: tbd at M14 | ui.zone_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:2420` |
| scpi-149 | `:SYSTem:DEBug:MASK` | Q | none | mask diagnostic text | Q: tbd at M14 | ui.mask_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:2449` |
| scpi-150 | `:SYSTem:DEBug:MASK:MAP` | both | one case-sensitive key=value; catalogue below | space-separated mapping key=value pairs | C/Q: tbd at M14 | board.mask_map; process-wide mapping; marks changed on set | T/V/M | fork debug; keep | `ui/src/scpi.rs:2424` |
| scpi-151 | `:SYSTem:DEBug:MASK:COLumns` | Q | optional a[,b] usize columns; one valid a gives (a,a); two sort endpoints; malformed defaults 0,9; extras ignored | column diagnostic text or off | Q: tbd at M14 | ui.mask_columns_text(a,b) | T/V/M | fork debug; keep | `ui/src/scpi.rs:2432` |
| scpi-152 | `:SYSTem:DEBug:DVM` | Q | none | DVM raw register diagnostic text | Q: tbd at M14 | ui.dvm_debug_text() | T/V/M | fork debug; keep | `ui/src/scpi.rs:2453` |
| scpi-153 | `:SYSTem:DEBug:DVM:LANE` | both | AUTO or decimal u32 0..3 | AUTO or decimal lane 0..3 | C/Q: tbd at M14 | ui.dvm_lane_set; forces single-channel ADC lane | T/V/M | fork debug; keep | `ui/src/scpi.rs:2457` |


## Additional merged command paths

New IDs describe upstream additions, except :ACQuire:CORes and
:CHANnel<n>:POSition, which correct omissions in the previous extraction.
All use T/V/M/H. Each both row supplies C and Q; command success is silent.
C/Q mappings remain unnamed until M14, as in the inherited table. Error cells
state overrides; invalid shapes/kinds default to -113, valid queries ignore
arguments unless explicitly stated. No header-relative inheritance is implied.
`slot` means integer 1..10 or R1..R10/REF1..REF10. Reference pair parsing and
acceptance predicates below are part of every affected row.

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-500 | `*ESR` | Q | none | decimal u8, destructive read | Q: tbd at M14 | poll_events, take event_status; bits 0/2/3/4/5 per scpi-307; errors: no parameter validation | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1008` |
| scpi-501 | `:TRIGger:FORCe` | C | none | none | C: tbd at M14 | ui.force_trigger, same as scpi-7; errors: arguments ignored | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1041` |
| scpi-502 | `:TRIGger:TYPe` | both | same exact type inputs as scpi-25 | Edge/Pulse/Slope/Video/Pattern/Duration/Timeout/Runt/Window/Delay/Setup/Hold/Nth edge/Bus | C/Q: tbd at M14 | ui.trigger_type_set; Q TrigType::name (title case); errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1154` |
| scpi-503 | `:TRIGger:SOURce` | both | wave; D handled only by EDGE fallback | CHANn/Rn/Dn | C/Q: tbd at M14 | set_trigger_source primary; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1124` |
| scpi-504 | `:TRIGger:SOURce2` | both | wave | CHANn/Rn | C/Q: tbd at M14 | set_trigger_source secondary; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1124` |
| scpi-505 | `:TRIGger:EDGE:SOURce2` | both | wave | CHANn/Rn | C/Q: tbd at M14 | set_trigger_source secondary; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1124` |
| scpi-506 | `:TRIGger:RUNT:UPPer` | both | F V | E V | C/Q: tbd at M14 | m.trig_level; errors: -224 invalid parameter or below the lower threshold (upstream b90facd5); -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1202` |
| scpi-507 | `:TRIGger:RUNT:ALEVel` | both | F V | E V | C/Q: tbd at M14 | m.trig_level; errors: -224 invalid parameter or below the lower threshold (upstream b90facd5); -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1202` |
| scpi-508 | `:TRIGger:RUNT:LOWer` | both | F V | E V | C/Q: tbd at M14 | m.trig_level2; errors: -224 invalid parameter or above the upper threshold (upstream b90facd5); -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1202` |
| scpi-509 | `:TRIGger:RUNT:BLEVel` | both | F V | E V | C/Q: tbd at M14 | m.trig_level2; errors: -224 invalid parameter or above the upper threshold (upstream b90facd5); -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1202` |
| scpi-510 | `:CHANnel<n>:POSition` | both | n=1..4; F V | E V | C/Q: tbd at M14 | ch.offset, alias of OFFSet; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-511 | `:CHANnel<n>:UNITs` | both | n=1..4; VOLTage/AMPere/WATT/UNKNown | VOLT/AMP/WATT/UNKN | C/Q: tbd at M14 | ch.unit; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-512 | `:CHANnel<n>:TCALibrate` | both | n=1..4; F seconds, abs<=SKEW_MAX=100 ns; time_per_div<=10 us required | E s | C/Q: tbd at M14 | ch.skew; -224 also for slow timebase; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-513 | `:CHANnel<n>:BIAS` | both | n=1..4; F V | E V | C/Q: tbd at M14 | ch.bias subtracted from reading; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-514 | `:CHANnel<n>:EXPand` | both | n=1..4; GND/CENTer | GND/CENT | C/Q: tbd at M14 | global m.v_expand_gnd; scales offset on later scale changes; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-515 | `:CHANnel<n>:LABel:SHOW` | both | n=1..4; B | B | C/Q: tbd at M14 | empty label means hidden; ON fills empty label with CHn, OFF erases label; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-516 | `:CHANnel<n>:LABel:CONTent` | both | n=1..4; trim outer double quotes, first 10 Unicode characters | unquoted text | C/Q: tbd at M14 | m.ch_label[n-1]; errors: -113 invalid channel/header; -224 invalid explicit value; POSition bad number -113 | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1242` |
| scpi-517 | `:TIMebase:VERNier` | both | B | B | C/Q: tbd at M14 | m.fine_t; OFF also tfine=1; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-518 | `:TIMebase:HREFerence:MODE` | both | CENTer/LB/LEFT/RB/RIGHt/TRIGger | CENT/LB/RB/TRIG | C/Q: tbd at M14 | m.h_expand; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-519 | `:TIMebase:DELay:ENABle` | both | B | B | C/Q: tbd at M14 | zoom_set_remote/m.zoom; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-520 | `:TIMebase:DELay:SCALe` | both | F s/div nearest time ladder, capped at main tscale | E s/div | C/Q: tbd at M14 | m.zscale; zoom_clamp; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-521 | `:TIMebase:DELay:OFFSet` | both | F seconds | E s | C/Q: tbd at M14 | m.zdelay; zoom_clamp; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-522 | `:TIMebase:DELay:POSition` | both | F seconds | E s | C/Q: tbd at M14 | m.zdelay; OFFSet alias; zoom_clamp; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-523 | `:TIMebase:ROLL` | both | B | B | C/Q: tbd at M14 | m.auto_roll; auto enters roll at 50 ms/div; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-524 | `:TIMebase:MODE` | both | MAIN/XY | MAIN/XY | C/Q: tbd at M14 | xy_set, not forced ROLL; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-525 | `:TIMebase:XY:ENABle` | both | B | B | C/Q: tbd at M14 | xy_set; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-526 | `:TIMebase:XY:X` | both | CHANnel1 only | CHAN1 | C/Q: tbd at M14 | fixed X source, matching setter no-op; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-527 | `:TIMebase:XY:Y` | both | CHANnel2 only | CHAN2 | C/Q: tbd at M14 | fixed Y source, matching setter no-op; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-528 | `:TIMebase:XY:Z` | both | CHANnel1..4/NONE/OFF | CHANn/NONE | C/Q: tbd at M14 | m.z_src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2605` |
| scpi-529 | `:ACQuire:ULTRa:MAXFrame` | both | usize; nearest ui::SEGMENTS ladder (0,10,50,100,200,500,1000,2000,5000,10000,20000,50000) | decimal count | C/Q: tbd at M14 | m.segments; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1573` |
| scpi-530 | `:ACQuire:CORes` | both | prefix MATC/RAW or B | MATC/RAW | C/Q: tbd at M14 | m.cores; errors: -113 invalid value | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1599` |
| scpi-531 | `:WAVeform:STARt` | both | u64 >=1, one-based inclusive; crossing the other endpoint moves it too | decimal position; STOP=source length when internal stop=0 | C/Q: tbd at M14 | Scpi.wav_start; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1955` |
| scpi-532 | `:WAVeform:STOP` | both | u64 >=1, one-based inclusive; crossing the other endpoint moves it too | decimal position; STOP=source length when internal stop=0 | C/Q: tbd at M14 | Scpi.wav_stop; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1955` |
| scpi-533 | `:MEASure:SOURce` | both | wave[,wave]; 1 or 2 nonempty words | primary CHANn/Rn | C/Q: tbd at M14 | Ui.meas_src, optional meas_src_b; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1771` |
| scpi-534 | `:MEASure:SETup:PSA` | both | wave | CHANn/Rn | C/Q: tbd at M14 | Ui.meas_src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1731` |
| scpi-535 | `:MEASure:SETup:DSA` | both | wave | CHANn/Rn | C/Q: tbd at M14 | Ui.meas_src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1731` |
| scpi-536 | `:MEASure:SETup:PSB` | both | wave | CHANn/Rn | C/Q: tbd at M14 | Ui.meas_src_b; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1731` |
| scpi-537 | `:MEASure:SETup:DSB` | both | wave | CHANn/Rn | C/Q: tbd at M14 | Ui.meas_src_b; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1731` |
| scpi-538 | `:MEASure:SETup:MAX` | both | D 0..100 percent; MIN<MAX | rounded integer percent | C/Q: tbd at M14 | m.meas_hpct / meas_lpct; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2492` |
| scpi-539 | `:MEASure:SETup:MIN` | both | D 0..100 percent; MIN<MAX | rounded integer percent | C/Q: tbd at M14 | m.meas_hpct / meas_lpct; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2492` |
| scpi-540 | `:MEASure:REFS` | both | 10/90,1090,10 or 20/80,2080,20; spaces removed | 10/90 if low<0.15 and high>0.85, else 20/80 | C/Q: tbd at M14 | m.meas_lpct/hpct; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1804` |
| scpi-541 | `:MEASure:TSETup` | both | finite D >=0 seconds | E s | C/Q: tbd at M14 | m.meas_tsetup; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1835` |
| scpi-542 | `:MEASure:THOLd` | both | finite D >=0 seconds | E s | C/Q: tbd at M14 | m.meas_thold; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1835` |
| scpi-543 | `:MEASure:BURSt:IDLE` | both | finite D >=0 seconds | E s | C/Q: tbd at M14 | m.meas_idle; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1709` |
| scpi-544 | `:MEASure:OUTSide` | both | ABOVe/BELow/BOTH | ABOV/BEL/BOTH | C/Q: tbd at M14 | m.meas_out; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3201` |
| scpi-545 | `:MEASure:NPERiods` | both | u32 1..32 | decimal count | C/Q: tbd at M14 | m.meas_n; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3201` |
| scpi-546 | `:MEASure:EDGes` | both | CLOCk/DATA | CLOC/DATA | C/Q: tbd at M14 | m.meas_data; measure_last; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3201` |
| scpi-547 | `:MEASure:INDicator` | both | B | B | C/Q: tbd at M14 | m.meas_labels; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3252` |
| scpi-548 | `:MEASure:GATing` | both | SCReen/NONE | SCREEN/NONE | C/Q: tbd at M14 | set_measurement_full_record; false=screen, true=full stored record; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1754` |
| scpi-549 | `:MEASure:STRip` | Q | ignored | semicolon records: item,wave,E; pair,wave,wave,E; CATid,a,b,c,d,E; DVM,mode,wave,E; COUNTER,mode,wave,E; empty allowed | Q: tbd at M14 | meas_strip of configured single/pair/catalogue rows, optional DVM and counter; errors: no parameter validation | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4963` |
| scpi-550 | `:MEASure:READout` | Q | ignored | STRip plus semicolon EYE_ITEM,E rows when view Eye; empty allowed | Q: tbd at M14 | meas_readout, EYE_ITEMS order; errors: no parameter validation | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4991` |
| scpi-551 | `:EYE:ITEM` | Q | figure from scpi-321; extra comma fields ignored | E in figure units; 9.9E37 invalid/unavailable | Q: tbd at M14 | ui.eye_readings selected figure; errors: -224 unknown figure; unavailable value has no error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4951` |
| scpi-552 | `:POWer:ITEM` | Q | item[,wave...] per scpi-322 | E; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_power; pw_v/pw_i/pw_vin/pw_clk defaults; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-553 | `:POWer:HARMonics` | Q | [wave], default pw_i | order,E_RMS_A pairs separated by semicolons; 9.9E37 if empty; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_harmonics; up to harmonic 40; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-554 | `:SPECtrum:ITEM` | Q | item[,wave], default spec_src | E; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_spectrum; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-555 | `:MAGnetic:ITEM` | Q | item[,V,I], default mag_v/mag_i | E; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_magnetic; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-556 | `:INVerter:ITEM` | Q | item[,wave...] per scpi-322 | E or DIR word FORWARD/REVERSE/STOPPED, sentinel if unavailable; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_inverter; inv_a/b/c/an; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-557 | `:DPT:ITEM` | Q | item[,V,I[,G]], default dpt_v/i/g | E; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_dpt; optional configured fourth input; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-558 | `:BANDwidth:ITEM` | Q | item[,wave], default Ch(bw_src) | E; nonfinite 9.9E37 | Q: tbd at M14 | ui.fact_bandwidth; errors: -224 invalid token/source/cardinality plus 9.9E37; missing valid waveform usually sentinel without error | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1842` |
| scpi-559 | `:BODE:MODE` | both | LOOP/PSRR/IMPedance | LOOP/PSRR/IMP | C/Q: tbd at M14 | m.bode_mode; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2100` |
| scpi-560 | `:CURSor:MODE` | both | OFF/MANual/TRACk/XY | OFF/MAN/TRAC/XY | C/Q: tbd at M14 | m.cursor; MAN retains Volt if already Volt, otherwise Time; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2822` |
| scpi-561 | `:CURSor:MANual:TYPE` | both | TIME/AMPLitude | AMPL only when Volt, otherwise TIME | C/Q: tbd at M14 | m.cursor Time/Volt unless already XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2861` |
| scpi-562 | `:CURSor:MEASure:INDicator` | both | B | B | C/Q: tbd at M14 | m.meas_labels alias; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2858` |
| scpi-563 | `:CURSor:MANual:SOURce` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-564 | `:CURSor:MANual:SOURce1` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-565 | `:CURSor:MANual:SOURce2` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-566 | `:CURSor:TRACk:SOURce` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-567 | `:CURSor:TRACk:SOURce1` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-568 | `:CURSor:TRACk:SOURce2` | both | wave; source must be supported/available | CHANn/Rn | C/Q: tbd at M14 | cursor_source; TRAC SOURce2 uses independent cursor_b_source; MAN SOURce2 accepts only current A; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2881` |
| scpi-569 | `:CURSor:MANual:CAX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-570 | `:CURSor:MANual:CBX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-571 | `:CURSor:MANual:CAY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-572 | `:CURSor:MANual:CBY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-573 | `:CURSor:MANual:AXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-574 | `:CURSor:MANual:BXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-575 | `:CURSor:MANual:AYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-576 | `:CURSor:MANual:BYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-577 | `:CURSor:MANual:XDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-578 | `:CURSor:MANual:IXDelta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-579 | `:CURSor:MANual:YDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-580 | `:CURSor:TRACk:CAX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-581 | `:CURSor:TRACk:CBX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-582 | `:CURSor:TRACk:CAY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-583 | `:CURSor:TRACk:CBY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-584 | `:CURSor:TRACk:AXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-585 | `:CURSor:TRACk:BXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-586 | `:CURSor:TRACk:AYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-587 | `:CURSor:TRACk:BYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-588 | `:CURSor:TRACk:XDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-589 | `:CURSor:TRACk:IXDelta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-590 | `:CURSor:TRACk:YDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-591 | `:CURSor:XY:CAX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-592 | `:CURSor:XY:CBX` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-593 | `:CURSor:XY:CAY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-594 | `:CURSor:XY:CBY` | both | finite D seconds for X, source voltage for Y | E or 9.9E37 for inactive axis | C/Q: tbd at M14 | cursor_axis converts to viewport fraction; adjusts cur_a/b/c/d; Off enables Time/Volt, crossing axis creates XY; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2985` |
| scpi-595 | `:CURSor:XY:AXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-596 | `:CURSor:XY:BXValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-597 | `:CURSor:XY:AYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-598 | `:CURSor:XY:BYValue` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-599 | `:CURSor:XY:XDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-600 | `:CURSor:XY:IXDelta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-601 | `:CURSor:XY:YDELta` | Q | ignored | E; IXDelta Hz = 1/abs(tb-ta); XDELta signed seconds, YDELta signed source units; 9.9E37 unavailable | Q: tbd at M14 | cursor_time_at / cursor_volt_at / wave_at_source; Track B uses B source; mixed-unit YDELta unavailable; errors: no parameter errors | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2920` |
| scpi-602 | `:CURSor:XY:AX` | both | finite D; X seconds, Y source voltage | E or 9.9E37 | C/Q: tbd at M14 | cursor_axis alias of CA/CB X/Y; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2912` |
| scpi-603 | `:CURSor:XY:BX` | both | finite D; X seconds, Y source voltage | E or 9.9E37 | C/Q: tbd at M14 | cursor_axis alias of CA/CB X/Y; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2912` |
| scpi-604 | `:CURSor:XY:AY` | both | finite D; X seconds, Y source voltage | E or 9.9E37 | C/Q: tbd at M14 | cursor_axis alias of CA/CB X/Y; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2912` |
| scpi-605 | `:CURSor:XY:BY` | both | finite D; X seconds, Y source voltage | E or 9.9E37 | C/Q: tbd at M14 | cursor_axis alias of CA/CB X/Y; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2912` |
| scpi-606 | `:DISPlay:GRADing:TIME` | both | MIN/INFinite or finite D >=0 seconds; nearest 0,0.1,0.5,1,5,10; INFinite selects last | MIN/0.1/0.5/1/5/10/INF | C/Q: tbd at M14 | m.persist; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-607 | `:DISPlay:WBRightness` | both | u32 1..100 percent, clamp floor 10 | decimal percent | C/Q: tbd at M14 | m.intensity; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-608 | `:DISPlay:GBRightness` | both | u32 0..100 percent, clamp floor 10 | decimal percent | C/Q: tbd at M14 | m.grid_bright; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-609 | `:DISPlay:GRID` | both | FULL/HALF/NONE | FULL/HALF/NONE | C/Q: tbd at M14 | m.grid; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-610 | `:DISPlay:RULers` | both | B | B | C/Q: tbd at M14 | m.grid_frame; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-611 | `:DISPlay:COLor` | both | B | B | C/Q: tbd at M14 | m.color_grade; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3084` |
| scpi-612 | `:DISPlay:VIEW:MONitor` | both | usize 0..views; nonzero requires views>=2 | decimal index | C/Q: tbd at M14 | m.hdmi_view, 0 follows panel in HDMI layout; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2035` |
| scpi-613 | `:REFerence:SLOT` | both | slot | Rn | C/Q: tbd at M14 | ui.ref_slot_set; selection; errors: -224 invalid slot | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-614 | `:REFerence:CURRent` | both | slot | decimal one-based slot | C/Q: tbd at M14 | ui.ref_slot_set; selection; errors: -224 invalid slot | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-615 | `:REFerence:SHOW` | both | [slot,]B; Q [slot] | B | C/Q: tbd at M14 | ui.ref_show_slot; errors: -224 invalid/empty/unavailable | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-616 | `:REFerence:DISPlay` | both | [slot,]B; Q [slot] | B | C/Q: tbd at M14 | ui.alias ref_show_slot; errors: -224 invalid/empty/unavailable | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-617 | `:REFerence:SOURce` | both | [slot,]CHANnel1..4; Q [slot] | CHANn | C/Q: tbd at M14 | ui.ref_source_set, per-slot capture source; errors: -224 invalid | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-618 | `:REFerence:VSCale` | both | [slot,]F V/div >0; Q [slot] | E V/div | C/Q: tbd at M14 | ui.ref_vscale_set; errors: -224 parse/slot; -230 empty or rejected nonpositive scale | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-619 | `:REFerence:VOFFset` | both | [slot,]F V; Q [slot] | E V | C/Q: tbd at M14 | ui.ref_voffset_set; errors: -224 parse/slot; -230 empty | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-620 | `:REFerence:LABel:CONTent` | both | [slot,]text, trim outer double quotes; Q [slot] | unquoted text | C/Q: tbd at M14 | ui.ref_label_set; errors: -224 invalid slot; -230 empty | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-621 | `:REFerence:COLor` | both | [slot,]GRAY/WHITe/RED/GREen/BLUE/ORANge/VIOLet; Q [slot] | GRAY/RED/GRE/BLUE/ORAN | C/Q: tbd at M14 | ui.ref_color_set; ORAN maps violet; errors: -224 slot/color; -230 empty | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4494` |
| scpi-622 | `:REFerence:RESet` | C | [slot] | none | C: tbd at M14 | ui.ref_reset_scale, V/div=1 offset=0; errors: -224 slot; -230 empty | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4611` |
| scpi-623 | `:REFerence:CLEar` | C | ignored | none | C: tbd at M14 | ui.ref_clear_all; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4611` |
| scpi-624 | `:REFerence:SAVE` | C | [slot] | none | C: tbd at M14 | ui.optional selection then ref_save_slot captures source waveform; errors: -224 slot or capture failure | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4611` |
| scpi-625 | `:REFerence:LOAD` | C | file required | none | C: tbd at M14 | ui.ref_load_named; errors: -256 missing file; -224 missing argument/unsupported/malformed file | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4611` |
| scpi-626 | `:REFerence:IMPort` | C | [file] | none | C: tbd at M14 | ui.ref_import_slot if empty else ref_load_named; errors: -256 missing file; -224 unsupported/malformed/import failure | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4611` |
| scpi-627 | `:SIM:PRESet` | both | case-insensitive serial::LANES name, optional double quotes; scpi-323 | preset name | C/Q: tbd at M14 | sim_preset_set cancels generation and applies preset; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-628 | `:SIM:SLOT` | both | slot | Rn | C/Q: tbd at M14 | sim_slot_set; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-629 | `:SIM:PLAY` | both | B; empty defaults ON | B | C/Q: tbd at M14 | sim_play_set; applies generation configuration on ON, generates/replays reference; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-630 | `:SIM:SHOW` | both | B | B | C/Q: tbd at M14 | sim_show_set; empty slot fails; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-631 | `:SIM:SSC:MODE` | both | OFF/DOWN/CENTER/CENT | OFF/DOWN/CENTER | C/Q: tbd at M14 | m.sim_ssc, sim_ssc_center; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-632 | `:SIM:SSC:DEViation` | both | D 0..0.05 fraction | E fraction | C/Q: tbd at M14 | m.sim_ssc_deviation; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-633 | `:SIM:SSC:FREQuency` | both | D 1e3..1e6 Hz | E Hz | C/Q: tbd at M14 | m.sim_ssc_hz; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-634 | `:SIM:JITTer:DCD` | both | D 0..100e-12 seconds peak-to-peak | E s | C/Q: tbd at M14 | m.sim_dcd_pp; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-635 | `:SIM:JITTer:CORRelated` | both | D 0..50e-12 seconds RMS | E s | C/Q: tbd at M14 | m.sim_corr_rj; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-636 | `:SIM:JITTer:CORNer` | both | D 1e3..100e6 Hz | E Hz | C/Q: tbd at M14 | m.sim_corr_hz; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-637 | `:SIM:PATTern` | both | PRBS7/9/13/15/23/31 or corresponding number; leading hyphens after optional PRBS prefix are stripped | PRBSn | C/Q: tbd at M14 | m.sim_prbs; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-638 | `:SIM:PAT` | both | same as PATTern | PRBSn | C/Q: tbd at M14 | m.sim_prbs alias; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4699` |
| scpi-639 | `:SIM:BUSY` | Q | ignored | B | Q: tbd at M14 | ui.sim_generating; configuration edits do not by themselves regenerate; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4702` |
| scpi-640 | `:MASK:ENABle` | both | B | B | C/Q: tbd at M14 | m.mask_on; tolerance writes rebuild existing mask; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3834` |
| scpi-641 | `:MASK:SOURce` | both | CHANnel1..4 | CHANn | C/Q: tbd at M14 | m.mask_src; tolerance writes rebuild existing mask; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3834` |
| scpi-642 | `:MASK:X` | both | F 0..2 horizontal divisions | E div | C/Q: tbd at M14 | m.mask_x; tolerance writes rebuild existing mask; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3834` |
| scpi-643 | `:MASK:Y` | both | F 0.04..4 vertical divisions | E div | C/Q: tbd at M14 | m.mask_y; tolerance writes rebuild existing mask; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3834` |
| scpi-644 | `:MASK:OPERate` | both | RUN/STOP, mask_on required | RUN only when mask_on and Run, else STOP | C/Q: tbd at M14 | m.run; RUN changes Single to Auto; tolerance writes rebuild existing mask; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3834` |
| scpi-645 | `:MASK:CREate` | C | ignored | none | C: tbd at M14 | ui.mask_create; capture mask from waveform; errors: -230 source has no waveform | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3887` |
| scpi-646 | `:MASK:RESet` | C | ignored | none | C: tbd at M14 | ui.mask_counts_reset; mark_changed; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3896` |
| scpi-647 | `:MASK:FAILed` | Q | ignored | decimal count | Q: tbd at M14 | ui.mask_counts fail; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3921` |
| scpi-648 | `:MASK:PASSed` | Q | ignored | decimal count | Q: tbd at M14 | ui.mask_counts pass; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3921` |
| scpi-649 | `:MASK:TOTal` | Q | ignored | decimal count | Q: tbd at M14 | ui.mask_counts pass+fail; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3921` |
| scpi-650 | `:HISTogram:ENABle` | both | B | B | C/Q: tbd at M14 | Off to Vertical on ON; OFF to Off; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3755` |
| scpi-651 | `:HISTogram:TYPE` | both | HORizontal/VERTical | HOR/VERT; VERT also if off | C/Q: tbd at M14 | m.hist; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3755` |
| scpi-652 | `:HISTogram:SOURce` | both | CHANnel1..4 | CHANn, default CH1 when no histogram source | C/Q: tbd at M14 | enable channel and set_meas_source; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3755` |
| scpi-653 | `:HISTogram:STATistics:RESult` | Q | ignored | Sum:N,Peaks:N,Max:E,Min:E,Pk_Pk:E,Mean:E,Median:E,Mode:E,Bin width:E,Sigma:E | Q: tbd at M14 | ui.hist_stat; units source volts for vertical, seconds for horizontal; blank sum/peaks=0 and other values 9.9E37; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3810` |
| scpi-654 | `:DVM:ENABle` | both | B | B | C/Q: tbd at M14 | m.dvm_on; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3939` |
| scpi-655 | `:DVM:SOURce` | both | CHANnel1..4 | CHANn | C/Q: tbd at M14 | m.dvm_src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3939` |
| scpi-656 | `:DVM:MODE` | both | DC/ACRMs/ACRMS/ACDC/ACDCrms | DC/ACRMS/ACDC | C/Q: tbd at M14 | m.dvm_mode; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3939` |
| scpi-657 | `:DVM:CURRent` | Q | ignored | E V or 9.9E37 | Q: tbd at M14 | ui.dvm_value; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4000` |
| scpi-658 | `:MATH<n>:DISPlay` | both | n=1..8; B | B | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; op Off on OFF; ON Off becomes Add, otherwise keeps operator; errors: -224 invalid B | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-659 | `:MATH<n>:OPERator` | both | n=1..8; catalogue scpi-324 | operator canonical token | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.op; errors: -224 invalid operator; OFF is not accepted | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-660 | `:MATH<n>:SOURce1` | both | n=1..8; channel, populated reference or earlier MATH trace | CHANn/Rn/MATHn | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.a; errors: -224 parse/self/later math; -230 empty reference | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-661 | `:MATH<n>:SOURce2` | both | n=1..8; channel, populated reference or earlier MATH trace | CHANn/Rn/MATHn | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.b; errors: -224 parse/self/later math; -230 empty reference | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-662 | `:MATH<n>:SCALe` | both | n=1..8; finite D >0; nearest math ladder or FFT_DB if FFT | E source units/div or dB/div | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.scale; errors: -224 invalid | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-663 | `:MATH<n>:OFFSet` | both | n=1..8; finite D clamped +/-1e9 | E | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.offset; errors: -224 invalid | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-664 | `:MATH<n>:EXPRession` | both | n=1..8; unquote matching single/double quotes; math::parse_expr language | unquoted expression | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; math_expr[k], op Expr; errors: -224 invalid expression | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-665 | `:MATH<n>:FFT:SOURce` | both | n=1..8; same source grammar as SOURce1 | CHANn/Rn/MATHn | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.a and op Fft; errors: -224 source/self/later math; -230 empty ref | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-666 | `:MATH<n>:FFT:WINDow` | both | n=1..8; RECTangle/BLACkman/HANNing/HAMMing/FLATtop | RECT/BLAC/HANN/HAMM/FLAT | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.win; does not switch op; errors: -224 invalid window | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-667 | `:MATH<n>:FFT:SCALe` | both | n=1..8; finite D >0 nearest FFT_DB | E dB/div | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.scale and op Fft; errors: -224 invalid | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-668 | `:MATH<n>:FILTer:TYPE` | both | n=1..8; LPASs/HPASs/BPASs/BSTop | LPAS/HPAS/BPAS/BST; LPAS if not filtering | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.op; errors: -224 invalid | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-669 | `:MATH<n>:FILTer:W1` | both | n=1..8; finite D >0, low cutoff Hz | E Hz or 9.9E37 if unsupported | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.fc or band center/bw; errors: -221 op lacks fc or low>=high; -224 invalid number | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-670 | `:MATH<n>:FILTer:W2` | both | n=1..8; finite D >0, high cutoff Hz, band filter required | E Hz or 9.9E37 if unsupported | C/Q: tbd at M14 | put_math c in maths[k], mirror selected fields; c.fc/c.bw; errors: -221 non-band filter or low>=high; -224 invalid number | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3512` |
| scpi-671 | `:BUS<n>:MODE` | both | n=1..4; protocol catalogue scpi-325 | proto_token: PAR/RS232/IIC/SPI/LIN/CAN or compact uppercase name | C/Q: tbd at M14 | with_bus/put_bus_wave; cfg.proto; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-672 | `:BUS<n>:DISPlay` | both | n=1..4; B | B | C/Q: tbd at M14 | with_bus/put_bus_wave; cfg.on; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-673 | `:BUS<n>:FORMat` | both | n=1..4; HEX/ASCii | HEX/ASC | C/Q: tbd at M14 | with_bus/put_bus_wave; cfg.ascii; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-674 | `:BUS<n>:EVENt` | both | n=1..4; B | B | C/Q: tbd at M14 | with_bus/put_bus_wave; global decode table enable; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-675 | `:BUS<n>:THReshold` | both | n=1..4; finite f64 final comma field, cast to f32 | E V | C/Q: tbd at M14 | with_bus/put_bus_wave; cfg.thr; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-676 | `:BUS<n>:RS232:TX` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; UART primary cfg.src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-677 | `:BUS<n>:RS232:RX` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; UART secondary cfg.src2; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-678 | `:BUS<n>:RS232:BAUD` | both | n=1..4; finite D >0, nearest BAUDS | decimal baud | C/Q: tbd at M14 | with_bus/put_bus_wave; UART cfg.baud; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-679 | `:BUS<n>:RS232:PARity` | both | n=1..4; NONE/ODD/EVEN | NONE/ODD/EVEN | C/Q: tbd at M14 | with_bus/put_bus_wave; UART cfg.parity; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-680 | `:BUS<n>:RS232:DBITs` | both | n=1..4; u32 5..8 | decimal bits | C/Q: tbd at M14 | with_bus/put_bus_wave; UART cfg.bits; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-681 | `:BUS<n>:IIC:SCLK:SOURce` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; I2C cfg.src2; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-682 | `:BUS<n>:IIC:SDA:SOURce` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; I2C cfg.src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-683 | `:BUS<n>:SPI:SCLK:SOURce` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; SPI cfg.src2; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-684 | `:BUS<n>:SPI:MOSI:SOURce` | both | n=1..4; available channel or populated reference | CHANn/Rn | C/Q: tbd at M14 | with_bus/put_bus_wave; SPI cfg.src; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-685 | `:BUS<n>:SPI:SS:SOURce` | both | n=1..4; available channel or populated reference/OFF/NONE | CHANn/Rn/OFF | C/Q: tbd at M14 | with_bus/put_bus_wave; SPI cfg.cs; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-686 | `:BUS<n>:SPI:DBITs` | both | n=1..4; u32 4..32 | decimal bits | C/Q: tbd at M14 | with_bus/put_bus_wave; SPI cfg.bits; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3271` |
| scpi-687 | `:BUS<n>:DATA` | Q | n=1..4; ignored | BUSY or #9 block of protocol token, LF, matching event rows separated by LF | Q: tbd at M14 | decode_events_result filter Dn rows; does not switch active decoder; errors: -230 decode failure; -113 invalid bus/header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:3347` |
| scpi-688 | `:DECode:SOURce` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 0 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-689 | `:DECode:SOURce1` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 0 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-690 | `:DECode:SOURce2` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 1 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-691 | `:DECode:CS` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 2 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-692 | `:DECode:SOURce4` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 3 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-693 | `:DECode:DIFFerential` | both | wave; optional roles accept NONE; in LA mode roles 0..3 use D0..15, optional NONE for roles>=2 | CHANn/Rn/Dn/NONE | C/Q: tbd at M14 | configure_decoder_source role 4 or m.dec_la[role]; empty refs/unsupported roles fail; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-694 | `:DECode:PROTocol` | both | compact protocol display name; scpi-325 | Proto::name human display spelling | C/Q: tbd at M14 | set_dec_proto_only; CPHY sets thr=0.02 and waveform input; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-695 | `:DECode:PROTO` | both | same as PROTocol | Proto::name | C/Q: tbd at M14 | PROTocol alias; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-696 | `:DECode:STATe` | both | B | B | C/Q: tbd at M14 | m.dec_on; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-697 | `:DECode:SELect` | both | usize 1..4 | decimal selected slot 1..4 | C/Q: tbd at M14 | m.dec_select; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-698 | `:DECode:THReshold` | both | F volts | E V | C/Q: tbd at M14 | m.dec_thr; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-699 | `:DECode:INPut` | both | LA when LA on and not CPHY; WAVEFORM/ANALOG | LA/WAVEFORM | C/Q: tbd at M14 | m.dec_la_in; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-700 | `:DECode:SRATe` | both | AUTO or finite D 0..1e12 symbols/s | AUTO if 0 else E sym/s | C/Q: tbd at M14 | m.dec_cphy_rate; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4097` |
| scpi-701 | `:DECode:CPHY:RATE` | both | AUTO or finite D 0..1e12 symbols/s | AUTO if 0 else E sym/s | C/Q: tbd at M14 | m.dec_cphy_rate; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1053` |
| scpi-702 | `:DECode:CPHY:SRATe` | both | AUTO or finite D 0..1e12 symbols/s | AUTO if 0 else E sym/s | C/Q: tbd at M14 | m.dec_cphy_rate alias; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:1053` |
| scpi-703 | `:DECode:BUSY` | Q | ignored | B | Q: tbd at M14 | ui.decode_busy; no acquisition/statistics effects; errors: none | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:4117` |
| scpi-704 | `:SYSTem:PIPeline` | both | prefix BATC/FRAM/STAN (case-insensitive) | BATC/FRAM | C/Q: tbd at M14 | m.draw_batch only; selected route can still be Frames for unsupported source; no mark_changed here; errors: -224 invalid parameter; -113 unsupported header | T/V/M/H | added; keep merged behavior pending cited target decisions | `ui/src/scpi.rs:2341` |

## Removed side-effect query aliases

scpi-154..162 retain their IDs as rejection requirements. At this merged base
all nine unsupported queries return -113 text, queue the error, and perform no
command effect. The old *CLS? queue-length defect D4 is gone. This removal is
implemented upstream, not a proposal to reintroduce command effects in M15.
The same rejection rule covers :TFORce?, :TRIGger:FORCe?, :MASK:CREate?,
:MASK:RESet?, :MEASure:CLEar? and :MEASure:STATistic:RESet?.

| Requirement | Long / short header | Kind | Parameters, units and limits | Reply | Command/Query mapping | Model field or action; side effects | Transport | Compatibility / disposition | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| scpi-154 | `*CLS?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-155 | `*RST?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-156 | `:CLEar?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-157 | `:RUN?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-158 | `:STOP?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-159 | `:SINGle?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-160 | `:CALibrate:STARt?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-161 | `:CALibrate:CANCel?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |
| scpi-162 | `:BODE:RUN?` | Q | any; ignored | -113 error text; error queued | Q: tbd at M14 | No command effect; unsupported query rejected | T/V/M | removed accepted side-effect alias; keep ID as rejection tombstone | `ui/src/scpi.rs:974`, `ui/src/scpi.rs:2487` |

## Parameter catalogues and delegated replies

**scpi-300 (changed): complete measurement catalogue.** Source tables
`ui/src/scpi.rs:525` and `ui/src/scpi.rs:599` define the following literal
spellings (long/uppercase short only), canonical export tokens and mappings.
`wave` means CHANnel1..4 or R1..R10/REF1..REF10. Default sources are Ui's
primary and secondary measurement selections, initially CH1/CH2. Syntactically
valid unavailable sources return 9.9E37 without a source parse error.
Single ITEM? ignores further source tokens. Pair ITEM? requires B, except
SETHold/SHOLD accepts zero or exactly two nonempty wave tokens and computes
setup/hold violations. Pair setters reject equal sources; queries may yield
unavailable instead. scpi-83/87 define setter cardinality and full-list errors.

Removed value spellings from the fork: MARea/MAR, MPARea/MPAR, PVRMs/PVRM,
PSLewrate, NSLewrate, TVM. Replacements are AREA, CARea/CAR,
CRMS, PSLew/PSL, NSLew/NSL, TVMAX and TVMIN. PSL/NSL survive, their old long
forms do not. TVMAX/TVMIN are now all-uppercase definitions with no TVM alias.
No requirement IDs are allocated per value token.

| Accepted definition | Canonical export | Model measurement | Unit |
|---|---|---|---|
| VPP | VPP | Vpp | V |
| VMAX | VMAX | Vmax | V |
| VMIN | VMIN | Vmin | V |
| VAVG | VAVG | Vavg | V |
| VRMS | VRMS | Vrms | V |
| VAMP | VAMP | Vamp | V |
| FREQuency | FREQUENCY | Freq | Hz |
| PERiod | PERIOD | Period | s |
| RTIMe | RTIME | Rise | s |
| FTIMe | FTIME | Fall | s |
| PWIDth | PWIDTH | PWidth | s |
| PDUTy | PDUTY | PDuty | % |
| NDUTy | NDUTY | NDuty | % |
| NWIDth | NWIDTH | NWidth | s |
| VTOP | VTOP | Vtop | V |
| VBASe | VBASE | Vbase | V |
| OVERshoot | OVERSHOOT | Overshoot | % |
| PREShoot | PRESHOOT | Preshoot | % |
| HTIMe | HTIME | HighTime | s |
| LTIMe | LTIME | LowTime | s |
| BWIDth | BWIDTH | Burst | s |
| TOUTside | TOUTSIDE | Outside | s |
| NPERiods | NPERIODS | NPeriods | s |
| CAMP | CAMP | CycAmp | V |
| CYCAmp | CAMP | CycAmp | V |
| CTOP | CTOP | CycTop | V |
| CBASe | CBASE | CycBase | V |
| CPP | CPP | CycPp | V |
| CYCPP | CPP | CycPp | V |
| CMAX | CMAX | CycMax | V |
| CMIN | CMIN | CycMin | V |
| VUPPer | VUPPER | Vupper | V |
| VMID | VMID | Vmid | V |
| VLOWer | VLOWER | Vlower | V |
| CAVG | CAVG | VavgCyc | V |
| CRMS | CRMS | VrmsCyc | V |
| ACRms | ACRMS | VrmsAc | V |
| SDEViation | SDEVIATION | StdDev | V |
| AREA | AREA | Area | V s |
| CARea | CAREA | AreaCyc | V s |
| PPULses | PPULSES | PPulses | count |
| NPULses | NPULSES | NPulses | count |
| PEDGes | PEDGES | PEdges | count |
| NEDGes | NEDGES | NEdges | count |
| TVMAX | TVMAX | TVmax | s |
| TVMIN | TVMIN | TVmin | s |
| PSLew | PSLEW | PSlew | V/s |
| NSLew | NSLEW | NSlew | V/s |
| ESTUI | ESTUI | EstUi | s |
| UI | ESTUI | EstUi | s |
| MEDian | MEDIAN | Median | V |
| MODE | MODE | Mode | V |
| LSLO | LSLO | LinSlope | V/s |
| LINSLOPE | LSLO | LinSlope | V/s |
| LINT | LINT | LinInt | V |
| LININT | LINT | LinInt | V |
| PAM0 | PAM0 | Pam0 | V |
| L0 | PAM0 | Pam0 | V |
| PAM1 | PAM1 | Pam1 | V |
| L1 | PAM1 | Pam1 | V |
| PAM2 | PAM2 | Pam2 | V |
| L2 | PAM2 | Pam2 | V |
| PAM3 | PAM3 | Pam3 | V |
| L3 | PAM3 | Pam3 | V |
| PSEP | PSEP | PamSep | V |
| BITS | BITS | Bits | count |
| ONES | BITS | Bits | count |
| PLLUI | PLLUI | PllUi | s |
| UIWTD | UIWTD | UiWtd | s |
| FOLD | FOLD | FoldRms | V |

| Accepted pair definition | Canonical export | Pair index | Unit |
|---|---|---|---|
| RRDelay | RRDELAY | 0 | s |
| FFDelay | FFDELAY | 1 | s |
| RFDelay | RFDELAY | 2 | s |
| FRDelay | FRDELAY | 3 | s |
| RRPHase | RRPHASE | 4 | degrees |
| FFPHase | FFPHASE | 5 | degrees |
| RFPHase | RFPHASE | 6 | degrees |
| FRPHase | FRPHASE | 7 | degrees |
| RRSkew | RRSKEW | 8 | s |
| FFSkew | FFSKEW | 9 | s |
| RFSkew | RFSKEW | 10 | s |
| FRSkew | FRSKEW | 11 | s |
| SETup | SETUP | 12 | s |
| HOLD | HOLD | 13 | s |
| SETHold | SETHOLD | 14 | count |
| FSETup | FSETUP | 15 | s |
| FHOLd | FHOLD | 16 | s |

SHOLD is a query-only spelling of SETHold; not accepted by meas_place or statistic ITEM.

**scpi-301 (changed timebase range): numeric ladders.** Channel coarse scale is
100/200/500 uV, 1/2/5/10/20/50/100/200/500 mV, 1/2/5/10 V at 1x.
Fine scale rounds up in the board's calibrated nanovolt rows, saturated at the
last row (`ui/src/scpi.rs:673`). Timebase is now the 41-entry 1-2-5 display ladder from 50 ps to 1000 s/div;
physical acquisition clamps to HW_MIN_TDIV=1 ns/div (`core/src/model/tables.rs:18`). Both ladders use
`nearest` log distance (`ui/src/scpi.rs:685`); nonpositive time requests can select
the first row through NaN comparisons. Memory depth values are AUTO (0), 1000,
10000, 100000, 1M, 10M, 25M, 50M, 62.5M, 100M, 125M samples
(`ui/src/board/dho/mod.rs:63`). Hardware may choose a different live depth/rate.
Bandwidth labels are AUTO, FULL/OFF, GND and 5,10,15,20,25,30,35,40,50,60,70,75,
80,90,100,125,150,175,200,250,300,310,400,600,800 MHz; 1,1.5,2 GHz.
Spaces are removed and the Hz suffix can be omitted (`core/src/model/tables.rs:43`,
`ui/src/scpi.rs:1375`). Baud rates are 1200,2400,4800,9600,19200,38400,57600,115200,460800,921600
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
hardware queues -241 (`ui/src/scpi.rs:2137`).

**scpi-303 (keep): setup and mask mapping.** SET delegates the complete setup
key/value language to `ui/src/store.rs:665`; this command accepts one key=value,
not a second catalogue of SCPI headers. It validates serialized keys after
mutation, except suffixes vfine/unit/bias/skew/label/tfine and prefix math_expr, plus exact key cat
(`ui/src/scpi.rs:2313`). GET reads only emitted setup keys, so an optional default
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
owned by the following producers: slots `ui/src/ui/autoset.rs:116` (`debug_slots`),
DVM `ui/src/ui/tools.rs:641`, lane `ui/src/ui/tools.rs:635` (`dvm_lane_text`),
mask `ui/src/ui/hwmask.rs:96`, columns `ui/src/ui/hwmask.rs:133`, zone
`ui/src/ui/hwzone.rs:48`, serial `ui/src/board/dho/mod.rs:160` (`uart_debug`).
The reply layouts below complete the response-format column. Literal key
order is significant for legacy comparison; unmarked integer values are decimal,
raw registers are `0x` plus eight hex digits, booleans 0/1. Producer-defined
condition/parity names use Rust Debug. These are diagnostic shapes, not portable
hardware guarantees.

| Header | Reply layout and units | Evidence |
|---|---|---|
| DEB:SLOTs? | `no hardware`, `pending`, `request;none`, or `request;min,max,mean` for each of eight slots; min/max 12-bit codes, mean one decimal on 16-bit scale. Schedules new run but returns previous one; query twice about 0.3 s apart | `ui/src/ui/autoset.rs:112` |
| SYST:DEB:DVM:LANE? | `AUTO` or decimal forced lane 0..3 | `ui/src/ui/tools.rs:635` |
| SYST:DEB:DVM? | `off,sw=<.6e V>` or `ch,lane,exp,n,cfg,sum_lo,sum_hi,sq_lo,sq_hi,hw,mode,sw` key=value pairs. hw=`none` or three `.6e` V readings joined by semicolons (DC/AC RMS/AC+DC RMS); mode decimal 0..2 | `ui/src/ui/tools.rs:641`, `ui/src/board/dho/hw.rs:1073` |
| SYST:DEB:MASK? | `off,enabled,active,blocked,sw_pass,sw_fail,acquired,used,records` or `ch,slot,ctrl,frames_lo,frames_hi,fails_lo,fails_hi,frames,fails,n,sw_pass,sw_fail,acquired,used,records` key=value pairs. Counts in frames/events as named | `ui/src/ui/hwmask.rs:96` |
| SYST:DEB:ZONE? | `off,enabled,active,sw_drops,acquired,used,records` or `ch,slot,ctrl,s4030,s4038,frames_lo,frames_hi,fails_lo,fails_hi,armed,pass,cycle,state,frames,n,batch,batch_ms,sw_drops,acquired,used,records` key=value pairs. batch is actual/requested; batch_ms one decimal ms | `ui/src/ui/hwzone.rs:48` |
| SYST:DEB:MASK:COLumns? | `off`, `no table`, or optional `pre=<sample>,trig=<offset with 2 decimals or none>,` followed by `cols=a-b,samples=N,per=<1 decimal>,fail=N,first=<up to 8 space-separated column indices>;` then semicolon-separated `c:lo-hi/<lower-enable><upper-enable>:min-max[!]`. Limits/codes dimensionless; per samples/column, ! marks failure; sample extrema can be inf/-inf without data | `ui/src/ui/hwmask.rs:133`, `ui/src/ui/hwmask.rs:159` |
| SYST:DEB:UART? and SERial? (UART) | `off,enabled=B` or `on/off,src=CHn,baud,bits,parity,cond,invert,clocks,r2100,r2104,r221c,mux,event_s,shift_s` key=value pairs. Baud symbols/s; clocks bit clocks; event_s/shift_s `.4e` seconds | `ui/src/board/dho/hw.rs:838` |
| SYST:DEB:UART? and SERial? (SPI/IIC) | SPI: `on/off,proto=SPI,clk=CHn,data=CHn,cs=CHn,clk_falling=B,width=N,<register pairs>,mux=N`; IIC: `on/off,proto=I2C,scl=CHn,sda=CHn,cond=<debug condition>,<register pairs>,mux=N`. Register pairs `r<4 hex digits>=<0x8hex>` come from receiver word generators | `ui/src/board/dho/hw.rs:974` |
| CAL:STATus? | `needs the hardware source`, `<rounded percent> % <progress>`, `starting ...`, last `done: ...`/`failed: ...` result, or `idle` | `ui/src/ui/selfcal.rs:124`, `ui/src/ui/selfcal.rs:226` |

Fallback strings, raw words and punctuation must be retained in corpus
fixtures. Register-word inventories are delegated to
`hal/src/dho/serialtrig.rs:58` (`spi_words`), `:131` (`i2c_words`) and
`hal/src/dho/uarttrig.rs:125` (`words`); their output is represented verbatim. The
parser does not define independent numeric register query headers. Bode serialization
is `ui/src/ui/bode.rs:863`; calibration status is `ui/src/ui/selfcal.rs:199`;
Identify is `ui/src/ui/analysis.rs:1414`; decoder rows `ui/src/ui/tables.rs:166`.

## Cross-cutting rules

**scpi-305 (changed): syntax, programs and ordering.** `Program::new`
(`ui/src/scpi.rs:77`) rejects text over 4 MiB or over 1024 nonempty commands
before any command executes. It splits CR/LF lines and literal semicolons,
without quote awareness. Empty segments/lines are ignored. Each header is
absolute; subsystem leading colons are optional and stripped; common * headers
require the leading * with no colon. Relative SCPI paths are
not inherited. First whitespace separates argument; header-ending ? selects
query, repeated trailing ? are stripped by exec_cmd. kw accepts only the long
or uppercase short definition, independently per node and case-insensitively.
Numeric suffixes split at the first digit and parse as usize, allowing leading
zeroes. Suffix acceptance beyond finite rows is specified in scpi-312.

Programs advance left to right in the intake closure, retaining prior replies
and remaining commands across a pending barrier without replaying writes.
Query replies are joined with semicolons within a source line and LF across
source lines. Embedded semicolons/LF in results (strip, Bode, decoder) are not
escaped. Successful commands have no reply. Bare *IDN and TRIG:STATus remain
exceptions that reply without ?. One final LF terminates a completed response
message, binary blocks included. Program completion and operation completion
are distinct. Cancellation and bounds can suppress an entire pending reply;
see scpi-311. Evidence: `ui/src/scpi.rs:888`, `ui/src/scpi/message.rs:50`.

**scpi-306 (changed; D1 implemented, earlier invalid-query target retained):
empty replies.** Successful empty text is Some(empty), a real response slot:
a lone query sends LF, and empty query then *IDN? begins with a semicolon.
TCP and VXI-11 preserve it; MCP returns empty text, reserving ok for
command-only messages (`ui/src/scpi/message.rs:33`, `ui/src/http.rs:1203`).
Examples include empty STRip/READout, BODE:DATA, IDENtify, DECode:EVENts,
ACQ:SEGM:TIMes and SYST:GET. This incorporates the fork's framing fix and
upstream's has_reply mechanism; it is no longer a later-branch-only fix.

Current invalid queries return their specific error text and queue it without
consumption. Earlier coordinator D1/D4 target asks for an empty terminated slot
and the queued error instead. This target is **not implemented** by the merged
base and conflicts with validator V1 below. Preserve both observations in the
oracle; a docs re-spec does not resolve that policy conflict. *CLS? no longer
clears errors, so D4's old queue-length/no-answer defect is removed. A cancelled
pending OPC query deliberately sends no reply and queues -410 (scpi-311).

**scpi-307 (changed): shared errors and event status.** One Scpi instance is
shared by clients/links/MCP; waveform preferences, errors, ESR and OPC events
are shared, not session-local. FIFO queue holds 20 errors. push_err truncates
to at most 256 bytes on a UTF-8 boundary, remembers last_error separately and
increments wrapping error_serial. At capacity it retains the first 19 errors
and replaces entry 20 with -350, while the failing query still reports its
specific last_error. Reading ERRor? removes the oldest or returns
`0,"No error"`; it does not clear ESR. *CLS clears queue, ESR and pending OPC
events. *RST clears ESR/events before DEFAULT, but has no explicit queue clear.
Fallback detects newly pushed errors by serial, not queue length, synthesizes
-113 if none, and returns last_error for an unanswered query. Successful empty
text never falls back (`ui/src/scpi.rs:739`, `ui/src/scpi.rs:940`).

push_err latches ESR bit 5 (32) for -199..-100, bit 4 (16) for -299..-200,
bit 3 (8) for -399..-300, bit 2 (4) for -499..-400, else bit 4. Queue overflow
also latches bit 3; OPC completion sets bit 0. *ESR? polls events and atomically
within the interpreter takes/zeros the u8 status. No *ESE, *SRE, *STB or *WAI.
Direct err.push remains in AOUTput and fork debug MASK:MAP / DVM:LANE setters:
these errors do not advance serial or latch ESR, and normalize_errors truncates
by 256 characters if over 256 bytes, then collapses >20 entries to first 19 plus
-350 through push_err. Do not promise uniform ESR for those setters. A MASK:MAP query whose board hook
returns Err uses direct err.push too: fallback then queues/returns -113, while
the earlier -224 remains in the queue. This can produce two errors for one query.

| Code | Trigger / reply behavior | Evidence |
|---|---|---|
| -113 | Undefined header, wrong query/command kind, many early-model parse failures; failed Q returns error text | `ui/src/scpi.rs:2487` |
| -221 | Settings conflict (math cutoff state/order); immediate adapter pending OPC, rejects tail | `ui/src/scpi.rs:3718`, `ui/src/scpi/message.rs:43` |
| -223 | Full single measurement selection; waveform page estimate above reply cap | `ui/src/scpi.rs:2809`, `ui/src/scpi.rs:4846` |
| -224 | Explicit invalid values, unavailable selections or sources, full/invalid pair row; numeric fact Q usually 9.9E37 | `ui/src/scpi.rs:2782`, `ui/src/scpi.rs:4440` |
| -230 | No LA record (0 or empty block); empty reference/math input; decode or mask data failure | `ui/src/scpi.rs:4412`, `ui/src/scpi.rs:4494` |
| -241 | Missing generator or LA capability, before argument checking | `ui/src/scpi.rs:2151`, `ui/src/scpi.rs:4352` |
| -256 | Missing waveform file on REF LOAD/IMPORT | `ui/src/scpi.rs:4682` |
| -350 | Error queue overflow or >64 pending OPC events | `ui/src/scpi.rs:739`, `ui/src/scpi.rs:1000` |
| -363 | Invalid oversized/over-command-count program, aggregate reply overflow; remaining commands dropped | `ui/src/scpi.rs:888`, `ui/src/scpi/message.rs:50` |
| -410 | Pending OPC fence cancelled: clear tail, buffered reply and has_reply, send nothing | `ui/src/scpi.rs:907` |

**scpi-308 (changed): pacing, bounds and transport.** TCP defaults to 5555,
eight clients, 4 MiB input buffer, reads at most 64 KiB per client per poll,
processes LF-complete lines, lossy UTF-8. Incoming bytes are checked before
append, including complete lines; over-limit disconnects. EOF allows immediate
complete replies but discards pending programs/tails. Output is bounded at
64 MiB including separators before append; no single-answer overshoot as in
the old spec. A socket write can still block UI for 2 s stall or 30 s total;
operation waiting itself uses nonblocking continuations. No rate guarantee.
MEASure queries call bounded measurement_settle before dispatch; this is not
an acquisition barrier (`ui/src/scpi.rs:786`, `ui/src/scpi/message.rs:64`).

VXI-11: portmapper TCP/UDP 111 and core 9009. device_write still ignores END,
flags and timeouts and treats each write as a complete program; no cross-write
fragment assembly. It queues at most 16 programs/link with combined input bytes
<=MAX_RECORD (1 MiB+4096); resource error 9 on overflow, error 4 invalid link.
Programs pump in order and retain pending OPC; device_read drains max(1,size)
bytes, END=4 if drained else REQCNT=1, immediate RPC timeout 15 if no bytes,
error 4 invalid link. Output pumping stops/clears queued programs once buffered
output is >=64 MiB; separate programs may take total link output past the cap.
RPC errors do not enter the SCPI queue (`ui/src/vxi11.rs:113`, `ui/src/vxi11.rs:223`).

MCP POST /mcp tools/call scpi arguments.command uses exec_message_checked.
Immediate completion returns UTF-8 text, invalid UTF-8 `(binary response)`,
empty successful query empty text, command-only ok. Pending OPC queues -221
and returns isError=true with text `Operation pending; use raw TCP
for *OPC?`; earlier writes remain, remaining program is dropped. VXI-11 also
supports pending programs despite that diagnostic mentioning only TCP.
The screenshot tool independently returns base64 PNG; HTTP envelope limits
also apply (`ui/src/http.rs:1203`). All command rows use this same language.

Network merge difference: upstream shared ACL rechecked existing clients after
changes; merged TCP uses Acl::permits at accept only and fork v6/host-port
binding. Service close shuts down all clients. These are adapter policies,
not per-header availability (`ui/src/scpi.rs:770`, `ui/src/scpi.rs:790`).

**scpi-309 (changed): numeric formats.** F/D are finite Rust f32/f64 decimal
or exponent; no engineering/unit suffix except MDEPth K/M. Protocol **U**
remains finite nonnegative f64 truncated/saturated to u64 or 0x/0X hex, with
per-field clamp/check (`ui/src/scpi/ser.rs:11`). B is exactly 1/0/ON/OFF.
General E is six fractional mantissa digits and signed >=2 digit exponent,
nonfinite 9.9E37 (`ui/src/scpi.rs:696`). Enum exact kw versus starts_with is
row-specific; no global prefix rule. LA/SPI/segment diagnostic times retain
lowercase exponent notation. Counts are decimal except statistic CNT uses E.

**scpi-310 (changed): waveform block and preamble.** Block is #9 followed by
nine decimal payload-length digits and payload. BYTE rounded/clamped 0..255,
WORD rounded/clamped 0..65535 as unsigned little-endian u16; ASCII comma E
volts in a block. Preamble is
`format,mode,page_points,1,xincrement,xorigin,0,yincrement,yorigin,yreference`.
format=0/1/2 BYTE/WORD/ASCII; mode=0/1/2 NORM/MAX/RAW (selected mode, even when
MAX currently uses screen data). dt and page origin are seconds E; origin adds
(STARt-1)*dt for a nonempty page. yinc is V/div*VDIV/250 for BYTE/ASCII or
/60000 WORD. yorg=round(offset/yinc), yref=128 or 32768, both plain Display.
Reconstruct volts `(code-yorigin-yreference)*yincrement`. RAW reads retained
memory, MAX uses memory only in STOP, refs use stored points, no data yields
empty block and zero count. Out-of-range page metadata keeps source dt/t0,
which can differ from empty wave_page's zero fallback. No automatic wait for a
new SINGLE record. Evidence: `ui/src/scpi.rs:4262` through `ui/src/scpi.rs:4890`.

**scpi-311 (changed current barrier; M15 target retained): query consistency.**
Exact case-insensitive segment `*OPC?` is intercepted by advance_program and
captures finite operations active at that point: autoset, SIM generation and
armed SINGLE, tied to workflow epoch and exact operation IDs. Continuous RUN
and future captures are not tracked. Complete sends 1; pending suspends this
program while other clients run. Cancellation/changed workflow or expired
receipt queues -410 and drops all prior buffered replies and the remaining
tail, with no response. Receipt deque holds 128 tokens. *OPC command registers
up to 64 such fences and latches ESR bit 0 on completion; cancellation does not
set bit 0. Parameters or repeated '?' defeat the exact *OPC? interception and
fall through as unsupported (-113), although other headers strip repeated '?'.
Immediate HTTP/in-process adapters reject pending with -221. Evidence:
`ui/src/ui/scpi_operations.rs:19`, `ui/src/scpi.rs:869`, `ui/src/scpi.rs:888`.

`:SINGle;:WAV:DATA?` without *OPC? still reads immediately; a tracking fence
is not the M15 `After { gen, epoch }` requirement. M15 still requires coherent
one-step answers and the future-record waveform barrier. The oracle must
record the existing finite-operation barrier, cancellation and approved M15
differences separately. No documentation-only change grants hardware evidence.

## Acceptance predicates and earlier decisions

**scpi-312 (changed): acceptance closure.** Finite header rows are supplemented
by these open predicates (`ui/src/scpi.rs:974` and delegated dispatch):

- TIMebase ignores intermediate nodes when the last is SCALe/OFFSet/POSition,
  except a second DELay/XY/HREFerence/VERNier node delegates to timebase_more.
- CHANnel<n> suffix 1..4 is required. SOURCE generator suffix defaults to 1;
  malformed suffix parses None and also defaults 1; valid numeric suffix must
  be 1..max(1,gen_count). Generator strips numeric suffix on every later node
  and matches presence, in priority OUTPUT, MOD, ARB, FUNC duty/symm, FUNC,
  FREQ, VOLT offset, VOLT; unrelated/trailing nodes are ignored. AM/FM/PM
  internal settings share storage, as in the inherited generator rows.
- TRIGger source intercept accepts exactly two or three nodes, any middle
  node, a final SOURce with any parsed suffix (only suffix 2 selects secondary;
  all others primary). Query always routes; a wave setter routes; invalid non-D
  value returns -224. D-prefixed setters fall through: only literal EDGE:SOURce
  has the legacy digital fallback, others -113. Thus :TRIG:BOGUS:SOUR2? is
  accepted. No additional trigger-family settings are inferred from MODE.
- IDENtify? ignores trailing nodes. REFerence ignores trailing nodes after
  SLOT/SHOW/DISPlay/CURRent/SOURce/VSCale/VOFFset/COLor and write-only
  RESet/CLEar/SAVE/LOAD/IMPort; LABel:CONTent allows further trailing nodes.
  SIM PATTern/PAT strips all leading hyphens after an optional PRBS prefix,
  so bare -7 and PRBS--7 select order 7. SIM PRESet/SLOT/PLAY/SHOW
  also ignore trailing nodes; other SIM paths check
  their exact length. REF write-only queries still reject without side effect.
- REF ref_pair: comma form requires a valid slot in first word, leaves rest
  as one string; empty arg uses current slot; bare valid slot selects that
  slot with empty rest; bare text beginning with a digit or R but not a slot
  is rejected; other bare text is the value for current slot. This makes bare
  RED fail COLor, despite [slot,] syntax; `1,RED` works. Bare numeric scale or
  offset may be parsed as slot, so use explicit slot,value. Extra query rest
  is ignored. See `ui/src/scpi.rs:4472`.
- CURSor MANual/TRACk/XY coordinate/value/delta namespace does not force the
  cursor mode. Query behavior depends on global m.cursor. Finite namespace
  aliases are enumerated individually above. SOURCE is not a valid alias for
  MATH SOURce1; BUS indexes require 1..4, MATH 1..8, LA digital 0..15.
- :SEGMents:HARDware/INTerval/TIMes? in the source comment remain unrouted;
  accepted paths require :ACQuire:SEGMents. :VIEW BANDwidth and STEP have no
  scpi registry entry and reject -224, despite internal views/fact queries.
- *IDN and TRIG:STATus reply without ?. Query forms of write-only actions now
  reject without effects, including the nine tombstone rows; no side-effect
  alias predicate from the pre-merge spec survives.

**scpi-313 (changed; D2 partly implemented): trigger type selection.** Merged
MODE selects and reports actual type in Rigol tokens, including generic BUS;
TYPe is an added alias with human display-name reply. RS232/IIC/SPI values
are not accepted. Earlier coordinator target still requires those three values
to select Bus plus protocol. Keep that as an explicit M15 divergence; no
coordinator approval is inferred for dropping it. Invalid writes now return
-224 and keep the selection. Current tokens are in scpi-25.

**scpi-314 (changed; D3 superseded by upstream): TVM.** Old TVMax and TVMin
shared the short TVM and selected maximum first. Upstream defines all-uppercase
TVMAX and TVMIN; TVM now returns sentinel plus -224. The old proposal to keep
TVM as max is unimplemented and conflicts with this base. Preserve long-form
fixtures and an explicit rejected-TVM case; do not silently map it back.

**scpi-321 (added): eye figure values.** kw definitions in
`ui/src/ui/eyepage.rs:26`, used by EYE:ITEM?; commas after figure ignored:
ONE, ZERO, AMPLitude, CROSsing, RISe, FALL, HEIGht, WIDTh, Q, HBER,
RECords, EDGes, UI, SAMPling, TPP, TRMS, DCD, DDJ, PJRms, PRMS, C2C,
F2, F4, F8, SRJ, PN, TPRE, TPOST, PPRE, PPOST, DCCM, ACCM, XOVer, TNT,
SSCPpm, SSCRate, TJ, RJ, DJ, OPENing, SEEN, TJBer, EWBer, J2, J9, NPJ.
Values come from eye_readings, not recomputation in the parser. Units:
ONE/ZERO/AMPLitude/HEIGht/HBER/DCCM/ACCM/XOVer V; CROSsing percent;
RECords/EDGes counts; Q dimensionless; SSCPpm ppm; SSCRate Hz; TNT dB;
OPENing/SEEN/EWBer UI fractions; all remaining figures seconds, including
SAMPling (sample interval dt, not sample rate) and PN (phase-noise jitter RMS).
See `ui/src/ui/eyepage.rs:606`, `ui/src/ui/eyepage.rs:679`,
`ui/src/ui/eyepage.rs:1785` and `dsp/src/eye.rs:621`.

**scpi-322 (added): analyser item tokens, cardinality, units and mapping.**
Source `ui/src/ui/facts.rs:66` through `ui/src/ui/facts.rs:434`. Item is the
uppercase first comma field, exact token, not kw abbreviation. waves() discards
empty comma fields; malformed wave names cause -224 plus 9.9E37. Sources use
the shared wave grammar. Unknown item/cardinality normally -224 plus sentinel;
missing valid data normally returns sentinel without error. Several producers
check data availability before matching item, so an unknown item with no usable
record can also return sentinel without an error. Preserve that ordering.

| Family / items | Explicit source count; defaults | Result field / units |
|---|---|---|
| POW: VRMS, IRMS, REAL, APP, REAC, PF, PHAS, FREQ, CREST, CRESTI | 0 or 2; pw_v,pw_i | quality vrms V, irms A, p_real W, s_apparent VA, q_reactive var, pf ratio, phase_deg degrees, freq Hz, crest_v/crest_i ratios |
| POW: EON, EOFF, ECOND, PAVG, DVDTR, DVDTF, DIDTR, DIDTF, RDSON | 0 or 2; pw_v,pw_i | switching e_on/e_off/e_cond J, p_avg W, dvdt_rise/fall V/s, didt_rise/fall A/s, rds_on ohm |
| POW: RIPPLE, ACRMS, LINE, LINER, SWITCH, SWRMS | 0 or 1; pw_v | peak-to-peak/rms V; LINE/LINER use pw_fline or detected line Hz, SWITCH/SWRMS use pw_fsw or detected switch Hz |
| POW: INRUSH, I2T | 0 or 1; pw_i | inrush peak A or current integral A^2 s |
| POW: TON, TOFF | 0 or 2; optional pw_vin,pw_v | turn_on/off_time s; absent configured vin returns unavailable |
| POW: CIN | 0 or 2; pw_v,pw_i | input_capacitance F |
| POW: ROVER, RUNDER, RDC, RMAX | 0,1,2,3; pw_v,pw_vin,pw_clk | rail overshoot/undershoot/DC V, RMAX maps turn_on seconds (despite its name); pw_ref supplies target voltage |
| POW:HARMonics? (no item) | 0 or 1; pw_i | order and RMS A, at most order 40 |
| SPEC: F1, THD, THDPCT, THDN, SNR, SINAD, SFDR, ENOB | 0 or 1; spec_src | distortion f1 Hz, thd_db dB, thd_pct %, thdn_db dB, snr_db dB, sinad_db dB, sfdr_dbc dBc, enob bits |
| SPEC: ACPR, ACPRLO, ACPRHI, FLOOR | 0 or 1; spec_src | configured spectrum/acpr; ratio dimensionless, lower_db/upper_db dB, noise_floor dBV; spec_center/span/rbw/win/ref/acpr_w/acpr_s settings |
| MAG: L, B, H, MUR, LOSS | 0 or 2; mag_v,mag_i | inductance H, flux T, field A/m, relative permeability ratio, loss W; mag_n/ae/le/rcu geometry |
| INV: D, Q, ZERO, FREQ, SPEED, ACCEL, DIR, ANGLE, TRAVEL, TORQUE, PMECH, PELEC, EFF | 0,3,4; also 1 only for Analog angle input; overrides rejected for Quad/Hall | inv_pack dq d/q/zero in source units, freq Hz, speed rpm, accel rpm/s, direction word, angle/travel degrees, torque N m, p_mech/p_elec W, eff signed mechanical/electrical ratio; inv_a/b/c/an defaults |
| DPT: VPEAK, IPEAK, EON, EOFF, TDON, TR, TON, TDOFF, TF, TOFF, DVDT, DIDT, TDT, TRR, QRR, ERR, IRRM, DDIDT, QOSS | 0,2,3; dpt_v/i/g; configured fourth input retained | peak V/A, energies J, times s, slew V/s or A/s, charges C, reverse current A; dpt_pct thresholds |
| BAND: RISE, BW, OVER | 0 or 1; Ch(bw_src) | step_response rise s, bw_from_rise Hz, overshoot %; bw_hint/bw_bins determine fold |

Spectrum display tracking (task 0087) is UI-only. There is no numbered
peak-table query in this command inventory. `:SPECtrum:ITEM? F1` and the
distortion, ACPR and floor items above continue to calculate from the selected
raw waveform; they do not report held or tracked display values. Auto-centre
also uses the current raw ranked lines. The peak table and trace markers
reserve eight rows, qualify a new line for 1 s and retain an absent line for
1 s, dimmed at its last value. Matching uses max(2 display bins, 0.5 actual RBW).
Frequency order changes only when the qualified set changes or the sort is
explicitly changed; frequency-mode numbers are reassigned only on a set change.
Level order swaps adjacent rows only when the lower row has a 1 dB advantage,
and preserves marker identities, including across explicit sort changes.
Displayed values are unsmoothed so real amplitude changes remain immediate. The
UI clock drives both holds. Tracking resets on source, window, trace or
frequency-grid/RBW changes, analyser lifecycle reset, and when no spectrum is
available. Tracking uses fixed storage for eight visible tracks plus eight
candidates; it adds no per-frame allocation to the existing FFT/measurement
pipeline.

**scpi-323 (added): simulation presets.** All 12 serial::LANES names are
case-insensitive whole-name matches, optional outer double quotes:
PCIe 1.0, PCIe 2.0, PCIe 3.0, PCIe 4.0, PCIe 5.0, USB 2.0 HS,
USB 3.2 Gen 1, USB 3.2 Gen 2, SATA I, SATA II, SATA III, 10GBASE-KR.
`dsp/src/serial.rs:48`, `ui/src/ui/sim.rs:794`. Advanced SSC/jitter/pattern
writes update configuration; generation occurs together on next SIM:PLAY ON.
STOP cancels generation; failures surface via sim_play_set or deferred take_sim_error
as -224 on the next command. Capacity or unavailable slot/show failures are
also -224. No preset implies hardware capability or hardware measurement.

**scpi-324 (added): math operators and sources.** Accepted definitions and
canonical replies are identical uppercase-short tokens except as shown:
ADD, SUBTract->SUBT, MULTiply->MULT, DIVision->DIV, AND, OR, XOR, NOT, FFT,
INTG, DIFF, SQRT, LG, LN, EXP, ABS, LPASs->LPAS, HPASs->HPAS, BPASs->BPAS,
BSTop->BST, AXB, NAND, NOR, CONV, FIR, EXPR, A2D, D2A, UNNRZI.
OFF is a query token only, selected by DISPlay OFF. Sources are installed live
channels, populated references or earlier numbered math traces; self/later
math references fail -224 and empty references -230. Selected trace fields are
mirrored by put_math, other traces retain their own MathCh. Expression grammar
is crate::math::parse_expr, including CHn/Rn; syntactic acceptance does not
promise every referenced record is available (`ui/src/scpi.rs:388`,
`ui/src/scpi.rs:450`, `ui/src/scpi.rs:3512`).

**scpi-325 (added): decoder catalogue and ownership.** BUS mode accepts
PARallel, RS232/UART, IIC/I2C, LIN, SPI, CAN, and each compiled protocol's
proto_token (compact alphanumeric uppercase name). DECode:PROTocol/PROTO
compacts both the input and human display name and compares whole strings;
it does not grant RS232 as an alias for the UART display name. Seven core
protocols are always compiled: UART, I2C, SPI, CAN, LIN, Parallel, C-PHY.
Optional compiled protocols follow ui/src/protocols/mod.rs MODULES and the
Cargo feature flags; a missing compiled module cannot be selected. Full names
are listed below from each Protocol::name; query emits display name for DECode
and proto_token for BUS. Sources are bound to decoder slots; inactive BUS slot
writes preserve selected slot and acquisition identity. Active primary source
configuration may route the trigger; only selected Bus-trigger writes mark
acquisition changes (`ui/src/scpi.rs:225`, `ui/src/scpi.rs:257`).

| Protocol display name | Compiled source (core always; others optional feature) |
|---|---|
| ARINC 429 | `ui/src/protocols/avionics.rs` |
| 1553 | `ui/src/protocols/avionics.rs` |
| UART | `ui/src/protocols/core.rs` |
| I2C | `ui/src/protocols/core.rs` |
| SPI | `ui/src/protocols/core.rs` |
| CAN | `ui/src/protocols/core.rs` |
| LIN | `ui/src/protocols/core.rs` |
| Parallel | `ui/src/protocols/core.rs` |
| C-PHY | `ui/src/protocols/core.rs` |
| DCC | `ui/src/protocols/dcc.rs` |
| SWD | `ui/src/protocols/debug.rs` |
| JTAG | `ui/src/protocols/debug.rs` |
| DMX512 | `ui/src/protocols/dmx512.rs` |
| I2S | `ui/src/protocols/i2s.rs` |
| I3C | `ui/src/protocols/i3c.rs` |
| Manchester | `ui/src/protocols/linecode.rs` |
| MLT-3 | `ui/src/protocols/linecode.rs` |
| 1-Wire | `ui/src/protocols/onewire.rs` |
| DS2482 | `ui/src/protocols/onewire.rs` |
| Quad-SPI | `ui/src/protocols/qspi.rs` |
| UART 9-bit | `ui/src/protocols/serial.rs` |
| Modbus RTU | `ui/src/protocols/serial.rs` |
| Modbus ASCII | `ui/src/protocols/serial.rs` |
| S/PDIF | `ui/src/protocols/spdif.rs` |
| USB | `ui/src/protocols/usb.rs` |
| USB PD | `ui/src/protocols/usbpd.rs` |
| CAN-FD | `ui/src/protocols/vehicle.rs` |
| FlexRay | `ui/src/protocols/vehicle.rs` |
| SENT | `ui/src/protocols/vehicle.rs` |

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
coordinator-approved disposition (D1 invalid-query target, remaining D2, D5 and the M15 barrier;
TVM proposal/D6/D7 only after approval). Store first differing event, replay seed and minimal reproducer.
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


**scpi-319 (change still pending; D5): STEP view selection.** The new analyser
registry still excludes STEP; its setter returns -224, while an internal Step
view can be printed as STEP. The coordinator's target to accept STEP remains
unimplemented. New MAGnetic/INVerter/DPT selections do not resolve D5.

**scpi-320 (changed): complete row error map.** The default for unknown
headers, wrong kinds and unmatched early model paths is -113, with error text
returned for a failed query. New rows embed explicit error overrides. This map
covers every inherited row and supplements scpi-307's queue/ESR distinctions.
Successful queries generally ignore arguments; commands do not reply.

| Rows (scpi-) | Invalid argument / availability behavior |
|---|---|
| 1..10, 26, 38..39, 82, 86, 91..93, 101, 106..107, 109, 111, 139, 143..149, 152 | Valid kinds ignore arguments except 91 setter/page size -224, 93 oversized page -223, 101 format -224, 144 failed decode -230. OPC bounds/pending/cancellation per scpi-311. 154..162 reject -113 with no action |
| 11..25, 27..33, 40..81 | Early closure parse/value failure -113 except 13 B -224, 25 type/source override -224, 31 reference router -224 (D fallback -113), 76 strict enum and 77 count -224. Protocol trigger ser.rs keeps -113 failures |
| 34..37 | -224 invalid boolean/channel/mode/digits; channel accepts 1..4; digits 3..7 |
| 83..85, 87 | -224 invalid measurement/source/statistic, excessive setter sources or pair placement; full single selection -223; single and stat queries return 9.9E37 for invalid/unavailable values; valid absent data does not itself queue an error |
| 88..90 | Invalid nonempty wave source or invalid mode/format -224; empty wave source silently ignored; no old coercion to NORM/BYTE |
| 94..100 | LA missing -241 before validation; bad suffix/B/threshold -224; missing record -230 plus 0 or empty block |
| 102..105, 112 | Invalid enum/index/view/whitespace endpoint -224; STEP still invalid |
| 108, 110 | Calibration keeps old coercion invalid to 0, >4 to 4; Bode invalid B defaults ON; no query effects |
| 113..136 | Invalid generator suffix -113, missing hardware -241, explicit B/enum/finite number/ARB errors -224, wrong ARB kind -113, finite out-of-range values clamp, ARB skips bad samples |
| 137..138 | SET -224 invalid setup key/value; unknown GET returns successful empty text |
| 140..142, 150..151, 153 | Explicit invalid values -224; mask columns parse fallback (0,9) is success. 141/150/153 use direct err.push, with no normal ESR latch |
| 154..162 | Removed accepted aliases; -113, queued and returned, no effects |

No full gate, unit or running-server validator is claimed by this source audit.

## VXI-11 procedures beyond read/write

These are RPC operations, separate from the SCPI header-path rows. All
listed baseline procedures return error 0 except resource exhaustion on create;
RPC errors are distinct from the shared SCPI error queue. The link buffer holds
response bytes, not device acquisition state. The Command/Query column records
target mappings; link lifecycle remains transport-owned.

| Requirement | Procedure / number | Current result and effect | Command/Query mapping | Disposition | Evidence |
|---|---|---|---|---|---|
| scpi-400 | create_link / 10 | Reuse first free slot or allocate up to 16 links; full returns error 9 with other reply fields 0; success: error=0, lid, abortPort=0, maxRecvSize=1048576 (1 MiB). Ignores clientId, lockDevice, lock_timeout and device | Transport-owned link allocation | keep limits and allocation behavior | `ui/src/vxi11.rs:111`, `:120`, `:223` |
| scpi-401 | device_readstb / 13 | Always error=0, stb=0, even for an invalid lid; no status register model | Q: transport constant 0 | keep stub; do not infer *STB support | `ui/src/vxi11.rs:195` |
| scpi-402 | device_trigger / 14 | Always error=0; no-op, does not call :TFORce | C: same as scpi-7, tbd at M14 | change proposal D6: map to :TFORce, including RUN/force-request effects | `ui/src/vxi11.rs:200`, `ui/src/scpi.rs:1076` |
| scpi-403 | device_clear / 15 | Clears selected link's output buffer and queued/pending programs if present; always error=0. Does not clear SCPI errors or picture/history | Transport buffer clear + C: same queue clear as scpi-3, tbd at M14 | change proposal D7: also clear shared SCPI error queue; preserve acquisition/picture state | `ui/src/vxi11.rs:200`, `ui/src/scpi.rs:998` |
| scpi-404 | device_remote / 16 | Always error=0; no-op | Transport-owned no-op | keep; no remote-control lockout | `ui/src/vxi11.rs:200` |
| scpi-405 | device_local / 17 | Always error=0; no-op | Transport-owned no-op | keep | `ui/src/vxi11.rs:200` |
| scpi-406 | device_lock / 18 | Always error=0; no-op | Transport-owned no-op | keep; no exclusive ownership is granted | `ui/src/vxi11.rs:200` |
| scpi-407 | device_unlock / 19 | Always error=0; no-op | Transport-owned no-op | keep | `ui/src/vxi11.rs:200` |
| scpi-408 | destroy_link / 23 | Sets selected slot to None, discarding its output; invalid/missing lid is ignored; always error=0 | Transport-owned link destruction | keep | `ui/src/vxi11.rs:212` |

D6 makes the transport trigger useful to clients expecting a device trigger and
shares the SCPI action rather than introducing a second acquisition writer.
D7 makes device clear reset pending protocol replies and stale command errors
together, so a client can recover its protocol session without an extra *CLS.
Because the SCPI queue is shared, D7 also clears errors visible to other clients;
that client-visible change is intentional in the proposal. Neither proposal is
implemented or hardware-verified in this documentation task.

## Fork, upstream and merged disposition ledger

Compared pre-merge fork c6f1f98, merged upstream main 2bd51214 and merged
c59b71d5 source. upstream/main may advance; this ledger refers to the merge's
fixed upstream input, not an evolving remote ref. Structural moves/formatting
alone are not wire differences. All expanded upstream families above are
**added** and retain upstream behavior unless an exception below says otherwise.
No implementation or validator behavior was changed for this re-spec.

| Surface / requirements | Pre-merge fork versus upstream | Merged c59b71d5 and disposition |
|---|---|---|
| MEAS ITEM/CLEar/STATistic, scpi-83..87 | Fork channel-only list: drop oldest, CLEar ALL/ITEMn; upstream WaveId/reference/catalogue selection, source defaults, full-list refusal, clear all | Upstream wins: cap 1024 shared rows, duplicate no-op, full single -223/pair -224, CLEar ignores args and clears singles/pairs/catalogue. 0080 restores selective clear/drop-oldest later; not in this tree |
| Measurement value names, scpi-300/314 | Fork 40 item definitions, TVM collision; upstream 70 single definitions (60 kinds), 17 pair definitions and revised long names | Upstream wins; removed aliases explicitly listed; TVM -224. D3's proposed TVM compatibility alias remains unimplemented |
| Empty success and compound replies, scpi-305/306 | Fork base lost empty slots; fork M2.3c fix retained Option and LF. Upstream program has_reply solves the same issue | Unified Option/has_reply implementation; leading empty slot retained, MCP empty text distinguished from ok; no remaining successful-empty disagreement |
| Invalid queries/action aliases, scpi-154..162, 306/307 | Fork fallback based on queue length, side effects then error/sometimes no answer; upstream serial/last_error, guards command effects | Upstream wins: no effects and queued/returned -113 even for *CLS? with existing errors. D4 old defect gone; earlier empty-invalid-slot target still differs |
| Completion/status, scpi-2/3/4/500/311 | Fork immediate OPC, no ESR; upstream bounded Program, pending operation fences and ESR | Upstream wins, adapted to change intake; explicit cancellation and immediate-adapter -221. M15 gen/epoch waveform barrier still a separate target |
| Live RUN/STOP/SINGLE, scpi-8..10 | Fork accepted transition requests/outbox/generation filtering; upstream direct model assignments plus operation IDs | Fork actions::run/change::single/run_request retained inside upstream program intake; upstream exact operation arm allocation retained. Reference/SIM/mixed routing remains upstream and runs first |
| Mixed bus commands | No equivalent WaveId router in old fork; upstream directly publishes mixed-bus hardware wants | Upstream router remains; it can write Shared.want outside fork publication. Task 0079 owns alignment; no new ownership is claimed by this spec |
| Trigger MODE/source, scpi-25/30/31 | Fork MODE stub, channel/digital source and no operation rearm; upstream real types, reference sources, RUNT thresholds, TYPE/FORCe aliases | Upstream wins; MODE BUS not RS232/IIC/SPI. D2 protocol-selection target outstanding |
| Model queries | Fork successful early queries called mark_changed; upstream writes-only guard | Upstream wins: no query mark_changed/reset/rearm for model reads. Explicit diagnostic query effects (slots) and bounded MEAS settle still exist |
| Timebase ladder, scpi-19/22/301 | Fork display ladder started at 1 ns; upstream adds stored-record zoom below the hardware minimum | Upstream 50 ps..1000 s/div ladder retained; physical HW_MIN_TDIV stays 1 ns. STEP/view policy is unrelated |
| Channels, scpi-12/13 and added rows | Fork fine OFF rounded/cleared fine scale; upstream retains it, adds global ground expansion, unit/label/skew/bias paths | Upstream wins; ground expansion scales offset, deskew slow-base rejection retained |
| Counter/acquisition, scpi-35..37/76/77 | Fork prefixes, digits 3..6, exact average ladder; upstream strict tokens, digits 3..7, floor average count | Upstream wins; invalid explicit values now -224 in delegated handlers. Existing fork segment/hardware-average/CORes/serial trigger extensions retained |
| Waveforms, scpi-88..93 and paging additions | Fork channel source, RAW/NORM coercions, whole current window, fixed preamble type 0; upstream refs, MAX, strict enums and pages | Upstream wins; mode in preamble, bounded count before copy, page origin. Unsigned LE WORD policy agrees and remains |
| Display/views | Fork BMP with ignored args and five analyser choices; upstream PNG validation and analyser registry | Merged keeps fork DotsMode VECT/DOTS/AUTO (upstream was boolean VECT/DOTS), plus upstream PNG and MAG/INV/DPT. STEP target D5 still unimplemented; BAND fact query does not grant VIEW BANDwidth |
| SYST SET/GET and PIPEline | Fork lacked cat key exception and pipeline header; upstream adds catalogue setup and draw path preference | Upstream wins for keys/header; both draw paths retained by merge decision. PIPEline query reports m.draw_batch preference; diagnostic pipeline prints startup switches.pipeline, not the effective draw_switches route |
| IDN | Fork build/cached firmware identity versus upstream fixed package-version text | Fork firmware_version() retained; runtime build identity can differ from package version. Model/serial placeholders remain producer-owned |
| Debug and transport adapters | Fork board/debug/capability APIs, IPv6 and accept ACL; upstream newer protocol handlers and program transports | Fork board abstraction/debug extensions retained. VXI queue retained; immediate HTTP/MCP text transport uses checked adapter. AOUT/MASK map/DVM lane retain direct err.push ESR exception |
| State diagnostics | Fork shadow/generation/loss/acquisition diagnostics versus upstream hardware fast-path suffix and HTTP workflow JSON | Both retained. Diagnostic 130 old positions +10 suffix fields; JSON has 8 separate keys. Global frames regex gets a third match when hardware suffix is present |
| Measurement implementation | Fork generation-checked live measure_rec; upstream full-record/reference/deskew/native-HiRes paths | Both currently coexist for live channels (task 0070 ports to one worker path); references/SIM use upstream behavior. Two SIM tests differ on when SINGLE statistics reset (merge task 0066). This spec documents source, not unit acceptance |
| Stopped settings and record identity | Fork identity/filter contract versus upstream deferred physical reconfigure while stopped and replay | Merge keeps upstream wait-until-RUN policy and fork captured-pair record tagging; no setter promises immediate hardware application. Do not substitute current settings for a frame's metadata |

## Validator cross-check

Data audited: `tools/scpi_validate.py` (578 lines) and
`tools/tests/test_scpi_validate.py` (48 lines), at c59b71d5. The four unittest
methods cover empty reply boundaries, binary LF/fragmentation, bytes in JSON
results and malformed/oversized/truncated blocks. They are client-framing tests,
not a parser command catalogue. The validator itself uses analytic 50,000-point,
1 ns references: offset 0.5 V, amplitude 2 V sine; -0.5..1.5 V trapezoid with
100 ns ramps/400 ns plateaus; and 1/3 V amplitude sine before/after 25 us.
Numeric expectations include 4/2 Vpp source isolation, sine RMS sqrt(2.25),
trapezoid RMS sqrt(0.25+1-4*100ns/(3*1us)), 1 MHz/1 us, 80/60 ns rise/fall,
2/6 Vpp screen windows, stopped captures and one triggered SINGLE.
These are analytic desktop expectations, not Rigol/physical-unit validation.

Static literal audit: 251 SCPI header occurrences, 200 distinct spellings,
119 inventory row IDs matched; sole unmapped literal :REFerence:NOPE? is an
intentional negative test. These are AST literal counts including helpers,
compound segments and conditional paths, not execution/test counts. Dynamic
item/source arguments, transport traffic and runtime assertions are not proven
by this static match. The inventory has 358 live header templates, covering
628 C/Q paths (270 both, 25 C, 63 Q), plus 9 retired alias tombstones and 9
separate RPC rows. Added rows scpi-500..704 total 205, including 2 prior-extraction
omissions; all 153 inherited headers retain IDs. Of those, 34 have explicit
changed dispositions and 119 retain the earlier contract, subject to revised
cross-cutting rules. There are 70 single-value definitions for 60 kinds and 17
pair definitions. This is an exhaustive source walk plus a validator subset
cross-check, not a claim that the validator covers the remaining 239 rows.

| Finding | Earlier spec / target versus validator data | Current merged disposition |
|---|---|---|
| V1, policy conflict | scpi-306 coordinator target: invalid query empty slot, error queued. Validator lines 309..316 requires unknown query response starts -113, *ESR?=32, queued -113 | Merged code agrees with validator, disagrees with that target. Preserve conflict for coordinator/M15; do not change validator or target silently |
| V2, qualification | Validator comment says every query answers. Pending OPC cancellation suppresses all reply bytes with -410; oversized programs/replies can also omit answers, and immediate adapter rejects pending with -221 | Narrow comment's universality in this spec (scpi-311); validator does not exercise cancellation, failed continuation or immediate adapters |
| V3, schema mismatch corrected | Validator state() reads /ui-state JSON captures/running/status; old state-query spec described only diagnostic SCPI line | Added state-query-300..307 for separate JSON, preserved diagnostic schema. Validator is not evidence for SYST:STATe? fields |
| V4, framing bound/coverage | Validator text reader readline cap 1 MiB, ASCII decoding and strip; server can send <=64 MiB aggregate, UTF-8 and embedded LF. Binary reader accepts #1..#9 widths, server emits only #9 | Client intentionally covers a subset; valid large or multiline textual replies can fail its reader, UTF-8 can fail decoding and trim masks whitespace. Do not infer server limits or exact-byte oracle from client limits |
| V5, fork behavior lost | Old scpi-83/84 promised drop-oldest and CLEar ITEMn; validator only adds one row and issues bare clear, then expects empty STRip | Merged code matches validator's tested case but lacks fork item-clear/eviction. Task 0080 follow-up; validator has no full-list/selective-clear assertion |
| V6, changed numerical contracts | Old spec average=exact ladder, counter max=6, no reference sources, no MAX/paging or PNG. Validator expects 100 averages ->64, references, RAW WORD page start10 stop12=3 points, PNG, threshold/source aliases | Updated rows match parser and validator; these supersede old spec entries. Validator checks six digits, so seven-digit acceptance still needs an oracle case |
| V7, operation semantics | Old spec OPC immediate; validator now uses *IDN? as write acknowledgement and explicitly avoids waiting on an impossible SINGLE | Consistent with deferred OPC. No validator assertion proves OPC completion/cancellation, *OPC ESR bit 0 or M15 gen/epoch waveform barrier |
| V8, measurement availability | Old stat row claimed not-selected always unavailable; validator asks CNT after reset (0) and current value for a reference | Updated scpi-87: current may be available without history, absent/reset count 0, aggregates unavailable. Invalid source/item sentinel -224 is explicitly asserted; disabled live source unavailable without parameter error |

No current parser-versus-validator contradiction is established by the literal
and assertion audit. V1 is a retained target conflict; V2 is an overbroad comment;
V3/V5/V6/V8 correct old spec assumptions; V4/V7 and untested rows are explicit
coverage limits. Unittests can be run without a server. The analytic validator
requires a running loopback desktop and replaces reference slots; it was not
run in this docs-only source task. No unit recordings or hardware evidence were
fabricated. All listed differences need differential fixtures at M15, including
clear-item/overflow, TVM, trigger protocol MODE, empty/error slots and STEP.
