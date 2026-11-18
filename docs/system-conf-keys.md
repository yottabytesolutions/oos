# Temporary system.conf keys

Removal backlog for [ADR 0012](adr/0012-hardware-verification.md).
Each row is a candidate for one coordinator-owned removal issue. Removal means choosing the supported path from
unit evidence and deleting the alternate path and its switch; this inventory does not change any defaults.
Hardware verification is limited to the evidence linked here, not a new unit run.

Defaults below are the parser defaults without environment overrides, image defaults or user configuration.
The [shared image defaults](../rootfs/initramfs/etc/oos/system.conf) enable `acq.pipeline = batch`, `acq.prearm = on`,
`acq.compress = on` and `perf.place = on`. The user's system.conf wins over image defaults.
`acq.frame0`, `trig.serial_hw`, `zone.batch` and `mask.map.*` are read only from the user's settings file;
the other rows use the combined settings and image text. These switches are read at startup.

| Key | Purpose | Default | Why temporary / removal issue scope |
|---|---|---|---|
| `acq.pipeline` | Choose Frames or Batch drawing. | `frames` | Two draw paths remain under docs/architecture/migration.md#m12; finish the [batch migration](architecture/migration.md) and remove the alternate path. |
| `acq.prearm` | Arm the next batch before decoding the current batch. | `off` | Hardware opt-in under ADR 0012; [transfer efficiency](fork/specs/10-transfer-efficiency.md) needs one supported arm order after unit verification. |
| `acq.compress` | Read live FPGA min/max pairs instead of every sample where possible. | `off` | Experimental acquisition alternative; use the [compression evidence](fork/compress.md) to settle the supported path and remove the switch. |
| `acq.interp` | Choose exact or fast sparse-frame interpolation. | `exact` | Fast alternative remains opt-in under ADR 0012; settle interpolation after validation and remove the alternate path. |
| `acq.frame0` | Keep frame 0 instead of dropping the suspect first frame of a batch and first short record after settings changes. | Drop; only `keep` disables it. | Diagnostic escape hatch for [stray frames](protocols/acquisition-start.md#frames-0-and-1-of-a-batch-issue-5); remove the override after the workaround is settled. |
| `meas.deep_raw` | Use raw deep reads for mean-type measurements instead of displayed min/max pairs. | `on` | Verified on the unit on 2026-11-15 per ADR 0012; the temporary `off` rollback and old path remain to be removed. |
| `perf.place` | Enable fixed thread and DMA interrupt placement. | `off` | Placement experiment under ADR 0012; consolidate the [placement policy](architecture/system.md) after unit acceptance. |
| `perf.cpu_floor` | Request a reversible 1,416,000 kHz policy4 minimum during hardware RUN. | `on` | Independent opt-out for the [RUN clock-floor rollout](architecture/system.md#run-clock-floors); remove the switch once the supported CPU policy is accepted. |
| `perf.gpu_floor` | Request a reversible 600,000,000 Hz GPU minimum during hardware RUN. | `on` | Independent opt-out for the same rollout; remove the switch once the supported GPU policy is accepted. |
| `trig.serial_hw` | Use the FPGA UART receiver for serial triggering. | `off` | Hardware opt-in under ADR 0012; settle the [UART trigger](fork/hwuart.md) after unit verification. |
| `cal.core_offset` | Match ADC core offsets at first configuration of each ADC mode. | Disabled; only `tight` enables it. | Experimental [core-offset trim](fork/core-offset.md); settle the verified correction policy and remove the opt-in path. |
| `dvm.hw` | Use the FPGA DVM block. | `off` | Experimental [hardware DVM](fork/dvm.md); verify or remove it with its switch under ADR 0012. |
| `mask.hw` | Run the mask test in the FPGA mask unit. | `off` | Experimental [hardware mask](fork/hwmask.md), with mapping and stall limits; settle support and remove the alternate path. |
| `zone.hw` | Qualify zone-trigger frames in the FPGA. | `off` | Experimental [hardware zone trigger](fork/hwzone.md); settle support and remove the opt-in path. |
| `zone.batch` | Limit frames per batch while the zone trigger runs. | `1` (clamped to 1..1000). | Manual [zone throughput tuning](fork/hwzone.md); replace the experimental override with the accepted batch policy. |
| `mask.map.domain` | Select read-out or pre-gain comparator codes for mask and zone tests. | `readout` | [Mapping sweep](fork/hwmask.md#sweep-of-the-mapping-unit-round-2) override; remove after the comparator domain is settled. |
| `mask.map.cpd` | Set comparator codes per division. | `7500` | Mapping sweep override; remove after the code scale is settled. |
| `mask.map.mid` | Set the comparator code at screen centre. | `32768` | Mapping sweep override; remove after the midpoint is settled. |
| `mask.map.shift` | Shift comparator codes into eight bits. | `8` | Mapping sweep override; remove after the code width is settled. |
| `mask.map.offs` | Add an offset to each eight-bit comparator code. | `0` | Mapping sweep override; remove after the code offset is settled. |
| `mask.map.invert` | Reverse the comparator code axis. | `0` (false) | Mapping sweep override; remove after the code polarity is settled. |
| `mask.map.cols` | Choose column strobe layout or an explicit experimental count. | `record` | [Strobe experiments](fork/hwmask.md#strobe-sweep-round-8) remain selectable; remove alternatives after the layout is settled. |
| `mask.map.range` | Restrict comparator testing to selected columns. | `0-999` | Diagnostic localisation of mapping failures; remove the range override once the mapping is accepted. |

The mask-map defaults above come from `oos_hal::dho::mask::Map::default()`; the parser accepts only the eight
`MASK_MAP_KEYS` in `ui/src/board/dho/hw.rs`. In particular, `mask.map.skew` is not a system.conf key.

The other system.conf keys reached through `ui/src/sysconf/switches.rs` were checked:
`net.ipv6` (default `off`) is the documented [IPv6 service setting](network.md#8-ipv6), and `mem.soft_mb` / `mem.hard_mb`
(default 768 / 1024 MiB, with soft capped at hard) configure the memory watchdog. They are operating settings,
not alternate experimental paths in this removal backlog. Environment-only switches such as `OOS_HWAVG` and
`OOS_FRAME_HEADER` are outside this system.conf inventory.

The brief's names `acq.frame`, `acq.wait` and `acq.held` are not system.conf keys in this tree. The frame override
is `acq.frame0`, listed above. `acq.wait` and `acq.held` are acquisition shared-lock timing probes, exposed as
`diag lock.acq.wait` and `diag lock.acq.held` in the [state query](spec/state-query.md). They have no configuration
default or key-removal issue; any removal of those metrics is a separate diagnostics decision.

## Removed temporary paths

The hardware-status trigger hint `trig.hw_status` and failed `autoset.levels = unit` measure-unit path were
removed on 2026-11-17 (#124). They are not opt-in functions and are not supported keys.
The current image selects Batch; the Standard-versus-Batch comparison and final deletion remain M12 work.
