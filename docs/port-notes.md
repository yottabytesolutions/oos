# Port notes for OOS maintainers

The signal-processing kernels take sample slices and geometry from their callers. The pacing core takes measured
timings and a picture schedule. Start with these interfaces when porting to upstream OOS or another fork. Line
counts include comments and inline tests. Separate test modules are outside these counts.

## Code to reuse

| Piece | Source and lines | Purpose and dependencies |
|---|---|---|
| Envelope reducers | [neon.rs](../ui/src/picture/deposit/envelope/neon.rs) (171), [scalar.rs](../ui/src/picture/deposit/envelope/scalar.rs) (22), [envelope.rs](../ui/src/picture/deposit/envelope.rs) (170) | Reduce interleaved `u16` samples to per-column, per-lane minima and maxima. The NEON reducer handles phase-dependent code correction and uses `core::arch::aarch64`. Take the `Correction`, `Extrema` and join rules from `envelope.rs`. Keep the scalar reducer for other architectures. The envelope wrapper also uses the fork's column maps and row cache. |
| Interpolation | [interp.rs](../dsp/src/interp.rs) (318), [interpolation.rs](../ui/src/picture/deposit/interpolation.rs) (55) | The DSP file provides linear and windowed-sinc interpolation and a reusable Lanczos-8 grid for `f32` samples. It uses the Rust standard library. The deposit helper loads overlapping 16-sample windows through a callback and evaluates fixed-point taps using scalar Rust or AArch64 NEON. |
| Polyphase | [screen/polyphase.rs](../ui/src/picture/deposit/standard/screen/polyphase.rs) (338), [polyphase/average.rs](../ui/src/picture/deposit/standard/screen/polyphase/average.rs) (189), [zoom/polyphase.rs](../ui/src/fastdraw/difference/zoom/polyphase.rs) (265) | Resample corrected codes for sparse main and zoom views. The main path caches phase weights and its Average path reuses an aligned grid. These paths use the fixed-point interpolation helper, code and span types from `fastdraw`, and the caller's projection or zoom window. Supply equivalent code access, row scaling and clipping when porting them. |
| Trigger refinement | [trignear.rs](../ui/src/trignear.rs) (485), [pruned.rs](../ui/src/trignear/pruned.rs) (177), [wide.rs](../ui/src/trignear/wide.rs) (257) | Refine fractional trigger placement near the hardware anchor and use a wider search when required. The filtered search works through `Samples` and `EdgeSearch`; the code path also uses the board's ADC correction. It shares the `oos_dsp::edges::Edge` type and the deposit layer's borrowed `SamplesRef`. |
| Waveform placement | [kernel.rs](../ui/src/picture/deposit/kernel.rs) (374), [placement.rs](../ui/src/picture/deposit/envelope/placement.rs) (373), [edges.rs](../ui/src/picture/deposit/envelope/edges.rs) (267) | Turn an explicit projection and trigger position into spans or difference rows. The code adapter maps interleaved lanes to channels and reuses column edges and row scratch. It needs the envelope reducers, `fastdraw` sample and map types, the deposit count formats and the acquisition layout. Supply the local route and metrics hooks at the caller. |
| Pacing core | [fill.rs](../ui/src/pacing/fill.rs) (173), [frame_clock.rs](../ui/src/pacing/frame_clock.rs) (378) | `Fill` targets read completion ahead of a picture slot. `FrameCap` budgets the batch from measured read cost. The two use `std::time`, the picture period and a callback for the next slot. `FrameClock` supplies slots from timer or display flips and depends on the local presentation timing and view cost policy. |
| Data-area repair and journal | [data.rs](../hal/src/update/card/writer/data.rs) (352), [geometry.rs](../hal/src/update/card/writer/data/geometry.rs) (255), [journal.rs](../hal/src/update/card/writer/data/journal.rs) (222) | Check, replay or convert a scratch image before any repair write-back. The filesystem tools are `mke2fs`, `e2fsck` and `tune2fs`. The port needs a checked card writer with the local bounds, durable sync and read-back. Keep the write order for home blocks, journal and superblocks. [Startup mount script](../rootfs/initramfs/bin/oos-mount-data). |
| Staged update receiver | [kexec/receive.rs](../hal/src/update/kexec/receive.rs) (210), [oos_update/receive.rs](../fpgad/src/bin/oos_update/receive.rs) (171) | Verify the signed header passed at kexec and match its build to the running build. Read the signed length from the rollback slot and check the image hash. The receiver uses the package verifier, checked `Card` access, update lock and persistent state, and the model and image checks from the ordinary installer. The CLI wrapper handles refusal, rollback on write failure and reporting. Port the matching sender and startup path with it. [Handoff protocol](update.md#trial-start-by-kexec). |

## Switching compatibility

- **Package format.** A `.oosupd` starts with the `OOS-UPDATE 1` text header and carries a boot image. Ed25519 signs
  the header, which names the `dho900-recovery` target and gives the image size and SHA-256. This fork keeps the
  `trial kexec` handoff and adds signed `trial staged` and `staged-key` fields for slot staging. The running fork verifies
  ordinary updates with its release key. For the switch from upstream OOS, the bridge package is signed with
  upstream's community key so that upstream's update page accepts it. Verify it against this release's
  `SHA256SUMS`. [Package verifier](../hal/src/update/pkg.rs), [trusted keys](../hal/src/update/mod.rs),
  [release packer](../tools/build-release.py).
- **Card layout.** The stock PARM layout stays in place and uses 512-byte sectors. The selector at boot LBA 122880
  takes `KRNL` for stock or `OOSK` for the image in `recovery` (64 MiB at LBA 188416). The first 32 MiB of `backup`
  at LBA 319488 are reserved for the stock fallback. The next 64 MiB at LBA 385024 hold the rollback image or a
  staged candidate. The final 16 MiB at LBA 516096 hold `/oos`.
  [DHO target](../hal/src/dho/target.rs), [card access and boot selection](../hal/src/update/card.rs).
- **Persistent data.** The shared `/oos` area holds `system.conf`, the startup setup, saved setups and pictures,
  the OOS calibration copy, SSH state and host keys, and update state. The network address can be saved in
  `system.conf`. First format starts with ext2 using 1 KiB blocks. The conversion adds the ext3-compatible
  `has_journal` feature. Startup checks the area offline, then mounts it with the ext4 driver. On the
  test unit (a DHO924S, 2026-11-24), upstream 0.1.15 mounted the area with the journal in ordered mode.
  The settings, calibration copy, network address and SSH host keys survived the verified switch.
  `oos-update leave` clears update tracking before the switch and preserves the selected boot mode.
  [Startup scripts](../rootfs/initramfs/etc/rcS), [mount](../rootfs/initramfs/bin/oos-mount-data),
  [network address](../rootfs/initramfs/bin/oos-mac), [SSH state](../rootfs/initramfs/bin/oos-ssh),
  [data area format](update.md#data-area-after-an-unclean-shutdown), [tracking](../hal/src/update/reconcile.rs).
- **SCPI identity.** `*IDN?` keeps the four comma-separated fields `OOS,<model>,<serial>,<firmware>`. The model
  and serial come from the detected platform, with `0` for an unavailable serial. The firmware field is
  `oos-ui <version>` with ` build <id>` appended when the build stamp is available. The version and build can
  change across the switch. [SCPI response](../ui/src/scpi/common.rs), [firmware field](../ui/src/ui/host.rs).
