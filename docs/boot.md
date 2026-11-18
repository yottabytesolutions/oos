# Boot medium and boot chain of the DHO924S

## Medium

- The RK3399 boots from the **SD card** on `dwmmc@fe320000` (Android fstab: `/dev/block/platform/fe320000.dwmmc/by-name/*`).
  `sdhci@fe330000` (eMMC controller) is enabled in the DT but not used for the system.
- Linux sees the card as `/dev/block/mmcblk0`; a tool that works on the card should also handle `mmcblk1`.

## Card layout (Rockchip legacy "parameter" layout)

Partition table: the stock PARM CMDLINE. Offsets there are relative to LBA 0x2000; the table below is absolute. Verified on
the unit against a card image: the IDB, vendor storage, `uboot` through `backup`, `verity_mode`, `baseparamer` and `frp`.

| LBA | Sectors | Size | Partition / content |
|---:|---:|---:|---|
| 0 | | | no MBR signature, no GPT |
| 64 | | | IDB: Rockchip TPL/SPL (DDR init + miniloader) |
| 7168 | 4 x 128 | 4 x 64 KiB | vendor storage: LAN MAC (id 3), BT MAC (id 4), id 26 (8 bytes) |
| 8192 | 0x2000 | 4 MiB | `parameter` (PARM, copies up to LBA 16383) |
| 16384 | 0x2000 | 4 MiB | `uboot` (LOADER, U-Boot 2014.10-RK3399, 2024-08-29) |
| 24576 | 0x2000 | 4 MiB | `trust` (BL3X, TF-A v1.3) |
| 32768 | 0x2000 | 4 MiB | `misc` (BCB, holds `boot-recovery`) |
| 40960 | 0x8000 | 16 MiB | `resource` (RSCE: `rk-kernel.dtb`, `logo.bmp`, `logo_kernel.bmp`) |
| 73728 | 0xC000 | 24 MiB | `kernel` (KRNL + raw arm64 Image, Linux 4.4.126) |
| 122880 | 0x10000 | 32 MiB | `boot` (KRNL + gzip ramdisk) |
| 188416 | 0x20000 | 64 MiB | `recovery` (KRNL + gzip ramdisk) |
| 319488 | 0x38000 | 112 MiB | `backup` (**all zero** on the original card) |
| 548864 | 0x40000 | 128 MiB | `cache` (ext4) |
| 811008 | 0x400000 | 2 GiB | `system` (ext4) |
| 5005312 | 0x8000 | 16 MiB | `metadata` (ext4) |
| 5038080 | 0x40 | 32 KiB | `verity_mode` |
| 5038144 | 0x2000 | 4 MiB | `baseparamer` |
| 5046336 | 0x400 | 512 KiB | `frp` |
| 5047360 | 0xFA000 | 500 MiB | `rigol` (ext4, `/rigol`: app files, FPGA images, calibration in `/rigol/data`) |
| 6299648 | rest | ~26.8 GiB on 32 GB | `userdata` (ext4, `/data`) |

Kernel command line words added by the stock loader: `storagemedia=sd androidboot.oem_unlocked=0
uboot_logo=0x02000000@0xf5c00000 loader.timestamp=<build time> SecureBootCheckOk=0`;
for a `recovery`/`backup` boot it prefixes the partition list with `0x00002000@0x00000000(parameter),`.

## Boot chain

1. BootROM -> IDB at LBA 64 (TPL/SPL) -> `uboot` partition (Rockchip U-Boot 2014.10) + `trust` (BL31).
2. U-Boot reads `parameter`, the reboot flag and boot keys; normally it boots `boot`, falling back to `recovery`, then `backup`.
   It loads the kernel from `kernel` and the DTB from `resource` (it can also load the FDT from the recovery image).
3. Rockchip "KRNL" images are `KRNL` + u32 length + payload (+ CRC). `boot` and `recovery` hold only ramdisks.
4. U-Boot 2014 RK also boots Android boot images (`ANDROID!` header).

### How the stock loader chooses

Verified on the unit: with `boot-recovery` at LBA 32800 plus the OOS image in `recovery`, power-on and `reboot recovery`
both booted Android from `boot` (kernel command line without the `(parameter)` prefix, `androidboot.mode=emmc`).
The `misc` boot request is not on the power-on path.

**Decision logic of the stock loader:**
- A reboot flag in a PMU register (0x5242C3xx: normal, rockusb, fastboot, recover, wipe data, charge) and boot keys
  ("recovery key pressed", "rockusb key pressed"). The stock Android restarts through the front-panel MCU (power
  cycle), so the flag from `reboot recovery` does not survive.
- When an image fails its check (magic and sha): `boot` failed -> "try to start recovery"; `recovery` failed -> "try
  to start backup"; else Rockusb.

**Switch used by OOS:** replace the magic `KRNL` at the start of `boot` (LBA 122880) with `OOSK`. The loader then
boots `recovery`, and if it refuses that image it boots `backup`, which holds a copy of the stock boot image
(`card-install.py --fallback-stock`), in its first 32 MiB. Back to stock: write `KRNL` again. `boot/switch.py` does this
on a card in a host computer. The OOS initramfs restores `KRNL` at start (one-shot), and `oos-boot-oos-again` sets `OOSK`.

### Ramdisk length must be a multiple of 4

Verified on the unit: the loader refuses an image whose ramdisk length is not a multiple of 4. It hashes with the RK3399
crypto engine (`init` mode 0xa0), which works on 32-bit words. Images built from stock parts (with `boot` = `OOSK`,
`backup` = `BAKK`, so a refusal shows as Rockusb 2207:330c) gave: stock kernel + stock ramdisk + stock DTB boots (command
line then starts with `0x00002000@0x00000000(parameter)`); the same padded to 29.5 MB boots; the stock kernel padded to
the 6.12 size boots; stock kernel + stock DTB + an initramfs of 1 181 602 bytes is refused. `oos-bootimg.py` pads the
ramdisk to 4 bytes; the padded image boots Linux 6.12.115+ on the unit (panel, SD card as mmcblk0, Ethernet, one-shot).
The switch (`OOSK` in `boot` -> loader boots `recovery`) is verified on the unit.

### Image refusal

With `boot` = `OOSK` and the fallback in `backup` disabled (`BAKK`), a refused image leaves the Rigol logo on screen and
the scope enumerates on USB as Rockchip 2207:330c (Rockusb): the loader refused `recovery` and `backup` and went to "try
to start rockusb". With a stock copy in `backup`, a refusal of `recovery` boots Android instead.

The loader's check:
- secure mode flag set -> needs `ANDROID!` and "SIGN" at +0x400 plus an RSA signature ("signature mismatch!");
- otherwise `ANDROID!` magic, then SHA-1 over kernel, le32 size, ramdisk, size, second, size, header[32:576] compared
  with the id at +0x240 ("boot or recovery image sha mismatch!"). `oos-bootimg.py` computes exactly this.

The loader's messages go to UART2 (115200 8N1).

## Image format the stock loader accepts in `recovery`

- Android boot image, header version 0, page size 2048: `ANDROID!`, sizes and load addresses of kernel, ramdisk and
  `second`, `name[16]`, `cmdline[512]`, `id[32]`.
- kernel = uncompressed arm64 `Image`, loaded and entered at 0x00280000 (limit 0x00200000 + 0x3800000).
- ramdisk = gzip newc cpio, loaded at 0x04BF0000 (ramdisk + second together < 0x3000000).
- second = Rockchip RSCE container with `rk-kernel.dtb`.
- id = SHA-1 over kernel + le32 size, ramdisk + le32 size, second + le32 size, then header bytes 32..575; the loader
  refuses an image whose id does not match.
- The kernel command line is built by the loader: DTB `/chosen/bootargs` + the PARM CMDLINE + the loader's own words.
- Whole image must fit `recovery` (64 MiB).

## Consequences for OOS

- `boot/oos-bootimg.py` is OOS's own packer for this format (from scratch, no base image needed).
- The stock `kernel` partition stays untouched; OOS carries its kernel in the `recovery` boot image.
- The switch between stock and OOS is the magic of `boot`: `OOSK` = OOS (via the fallback to `recovery`), `KRNL` = stock.
- The PARM table stays as it is. Assumed, not yet verified on hardware: U-Boot checks a CRC over the first 942 bytes of
  the PARM copy, and a changed table drops into Rockusb.
- The boot card defaults to 3.3 V high speed (`sd.mode = hs`). Device configuration and trial details follow.

### Device boot card mode

`/oos/system.conf` accepts `sd.mode = hs` (default) or `sd.mode = sdr104`. The generic package ships one base
DTB. After verifying the package signature and image hash, the updater applies the small property delta from
`kernel/dts/rk3399-rigol-dho900-uhs.dts`. This reuses the existing FDT codec and is smaller than shipping a second
DTB. SDR104 enables `sd-uhs-sdr104` and `sd-uhs-sdr50`, caps the SD controller at 150 MHz, removes
`regulator-init-microvolt` from `vccio_sd`, and sets `regulator-settling-time-down-us` to 30000.
The regulator's 1.8-3.3 V range and its connection to the SD IO domain remain intact.

The setting affects the next kernel, never the running SD driver. Installs select the DTB both for the Android
boot image's RSCE `second` and for kexec's staged tree, before loader changes are merged. The package stays signed
and unchanged; the locally derived image gets a recomputed Android loader id and a SHA-256 in the update state.
No device build input is needed. Nothing writes stock `resource`, `kernel` or U-Boot.

To change an already installed image, edit `sd.mode` in `/oos/system.conf`, then run `oos-update sd-mode` to inspect
the dry run, followed by `oos-update sd-mode --execute`. This requires a tracked, confirmed active image and refuses
an update already on trial. The existing verified slot-write path retains an HS rollback image and writes the selected
DTB into OOS recovery, on trial. Reboot through the authorized unit procedure. There is no kexec on every boot.
The running session cannot confirm a same-build DTB trial until it has booted. The ordinary start-up watcher confirms
it after a stable start, or restores HS on failure. An unconfirmed second start also restores HS and logs the fallback.
After fallback, SDR104 is suppressed while `system.conf` still requests it. Select `hs` and run `sd-mode --execute`
before explicitly retrying `sdr104`; updates do not rewrite the device setting.

**Safety boundary, not yet verified on hardware:** the stock loader cannot read the setting or update state.
Rollback runs in rcS and therefore cannot run when a voltage-switch failure wedges the card before userspace.
An initial kexec trial from HS leaves that installed HS image untouched; remove power before the next start to reset the card.
A directly restaged cold trial that hangs before rcS still requires the owner's card-reader recovery procedure.
The existing trial mechanism does not guarantee automatic HS selection in that case. Unit acceptance remains open.

## OOS start-up after the kernel (rcS)

Not yet verified on hardware in this order.

Order in `rootfs/initramfs/etc/rcS`, each step logged to `/tmp/oos-boot.status` (`oos_hal::bootstatus`: uptime,
state, stage, detail; the boot-time profile of every start). What needs the hardware rcS asks of the board
(`rootfs/board/dho/bin/oos-board`); the commands named below are what that script runs on the DHO800/DHO900. Addresses come
from `/etc/oos-board` ([porting.md](porting.md)):

1. `Linux` (uptime at the start of rcS).
2. `oos-oneshot` (boot magic back to `KRNL`), as the very first program, so a hanging new build on trial is followed
   by stock at the next power-on (`update.md`), then the hardware watchdog (`oos-watchdog`, see below), then `/oos`,
   the crash records of the last start in the background (`oos-crash-save`, below) and the start count of 4c.
   Then **`oos-fpgad fpga-boot`**: only the
   BOOT command (spidev1.0, slot 0x400000), its uptime to `/tmp/oos-fpga-booted`. The bitstream loads while the rest of
   rcS runs; the fixed 3 s FPGA wait is counted from this command.
3. Network in the background (`udhcpc -t 5` would block the start-up up to 15 s without a DHCP answer), SSH when
   switched on (password or key, [network.md](network.md) 7), banner, `/oos`, touch and beeper modules.
4. **oos-ui** with `--hw --afe --start-up /tmp/oos-boot.status`: the start-up screen (`ui/src/ui/startup.rs`; logo or
   `/oos/splash.ppm`, checklist with times) while its hardware thread waits for the `end` line. No demo signals and
   no restart of oos-ui.
4c. `oos-update boot` (`update.md`), early in rcS: counts the start of an image on trial; one that already started once
   without confirmation is rolled back here. After oos-ui: `oos-update watch` in the background confirms the trial
   (oos-ui 20 s stable + start-up finished) or writes the previous image back.
4d. `oos-wlan` in the background: waits for the end of the start-up, then WLAN through a USB stick (`wlan.md`).
5. In the background: `oos-mount-rigol`, then **`oos-fpgad cold-boot --execute --power-on --booted --progress`**:
   calibration files check (warn if one is missing or damaged), sample clock and AFEs during the FPGA wait (GPIO
   only, no FPGA needed), the rest of the wait, PCIe polled every 20 ms, the cold-boot steps grouped into the screen's stages (`boot::stage`: FPGA image, sample clock, analog front ends,
   ADC and memory set-up, ADC data lanes, signal path; line-open errors as "GPIO lines", cold-boot warnings as
   "Cold-boot checks"), then the temperatures (SoC, FPGA XADC; warn above the guard limits). rcS writes `end` or `abort`
   only after oos-fpgad exited, so its GPIO lines are free for oos-ui.

The start-up screen goes away 2 s after a clean start (5 s with warnings); a failed start stays until a tap and names
the stage; System > About shows the last result. After 60 s without a new status line a tap leaves the screen (hung
mount or driver load). The power key and its dialog work over the start-up screen. The first status read happens at once.

The FPGA self-checks (0x1008 bit 29, 0x1210) are part of the cold boot; the ADC self-test stays out (never-do list in
`protocols/cold-boot.md`, not needed).

Time until acquisition, measured with the status file on a DHO924S: 8.1 s from the kernel's clock (Linux 3 s, calibration
files 0.9 s, sample clock 0.4 s, AFEs 2.7 s, the rest 1.1 s). Sample clock and AFEs run during the 3 s FPGA wait; `/oos/fpga-wait`
overrides it for trials.

## Restart after a panic or a hang

The scope runs unattended, so a kernel that stops must restart by itself. Verified on the unit with the `dho804`
implementation (abd00ca, 2026-11-15): a warm reboot clears the watchdog; a sysrq panic came back in 39 s with its
record saved; a killed kicker reset the unit after 41 s, and it was back after 56 s. That verifies the previous command-line `panic=10` behavior and the watchdog restart mechanism.
Task 0067 changes when panic restart and watchdog supervision are enabled, as described below. `oos-crash-save` on this branch is a rewrite (budget, naming, retry checks)
and is tested on the host only, not yet on the unit. The kexec path is not yet verified.

- `panic=0 softlockup_panic=1` in `OOS_BOOTARGS` (`rootfs/board/dho/etc/oos-board`) and the base DTS.
  Early panics halt while the selected recovery image may still be armed. After `oos-oneshot` confirms `KRNL`
  by read-back using `dd iflag=direct` (or finds it already set), rcS writes 10 to `/proc/sys/kernel/panic`: later panics restart after
  10 s. It also writes 1 to `kernel.softlockup_panic`, because a kexec start keeps the old build's command line. An unexpected card, layout, magic or failed restoration leaves automatic panic restart disabled.
  rcS records a warning in `/tmp/oos-boot.status` that watchdog and panic restart are off when restoration fails.
  A soft lockup panics; hard lockups and tasks hung for 120 s already did (kernel config).
  Task 0067 covers this timing change with host simulations only; it is not yet verified on the unit.
- Hardware watchdog: `dw_wdt` at `ff848000` (enabled in `rk3399-base.dtsi`). rcS starts `oos-watchdog` right after
  successful `oos-oneshot`: busybox `watchdog -F -T 30 -t 5`, nice -20, `oom_score_adj` -1000. If it exits it is started
  again for as long as the system runs: after 1 s, doubling up to 60 s, back to 1 s after 10 min of running; each
  restart is logged to the kernel log. It does not depend on oos-ui. A hang that never reaches a panic resets the SoC
  after 30 s (the kernel stretches the 21.5 s hardware maximum to the 30 s asked for). Log:
  `/tmp/oos-watchdog.log`. Before it opens the device it logs one `before open:` line (also in the kernel log): whether
  the hardware counter already runs (control register from the driver's debugfs), its period, the hardware maximum
  (2^31 cycles of `pclk_wdt`; 21.5 s on the unit), sysfs `state`/`timeout`/`timeleft`/`bootstatus` and whether
  this is a kexec start.
  `hw running` after a warm reboot that was not a kexec start means the system reset did not clear the watchdog.
- The device tree gives the watchdog no reset line, so the kernel cannot stop it once started. A reboot or panic
  restart (PSCI system reset) resets it with the SoC (verified on the unit), so a stock Android start after it sees
  it off. A kexec start
  keeps it running. Nothing services it between the last ping before the jump and the registration of `dw_wdt` in the
  new kernel, which comes after the initramfs is unpacked; from then on the kernel pings it
  (`CONFIG_WATCHDOG_HANDLE_BOOT_ENABLED`) until `oos-watchdog` opens it again. If the rest of the hardware period does
  not cover that gap, the SoC resets and the boot simply starts again. The `before open:` line measures the period.
- Which system comes back follows the boot magic: OOS when it was armed again (persist mode, about 20 s after oos-ui
  ran stable), else stock Android. A build on trial that panics is rolled back at its next start (`update.md`).
- The loader does not consume `OOSK`: it retries `recovery` on every reset until userspace restores `KRNL`.
  Previously `panic=10` could therefore loop an early kernel/initramfs panic forever, before either the trial
  count or rollback. With `panic=0`, a cold-start panic before restoration halts instead. Neither a halt nor
  a hang restores the selection: power-cycling can still retry the bad image, requiring a card reader
  (`boot/switch.py stock`). A kexec start carries an already-running hardware watchdog, which can still reset
  it before rcS; the installed image remains the fallback, as described in `update.md`.
- Crash records (ADR 0017, "Kernel panics" in `architecture/README.md`): ramoops keeps the oops/panic record and the
  console across a warm reset, not across a power cut. rcS starts `oos-crash-save` in the background, so a slow write
  never delays oos-ui. Only a dmesg record whose header says Panic or Oops starts an archive; other dmesg records,
  console and pmsg records (there after every warm reboot) leave pstore as it is. `/oos/crash` holds at most 1 MiB,
  and 4 MiB of `/oos` stay free: an archive that does not fit the reserve is skipped and nothing is pruned. An archive
  holds the dmesg records whole, the console's last 64 KiB, the saving boot's `boot_id` and the build, and is named
  `<n>-<record time>-<checksum>`. Order: copy into `.tmp` (every copy and the metadata checked), fsync each file and
  the directory, read everything back; rename, fsync `/oos/crash`; then clear the copied pstore entries; then prune
  the oldest archives beyond 1 MiB, never the new one. Any failure leaves pstore as it was. A start that finds an
  archive of the same records checks it against pstore and fsyncs it again before clearing; one that does not match
  is set aside and archived again. A `.tmp` from an interrupted save, or an archive that did not match, is kept as
  the one `.partial-<n>` (it replaces an older one). So `/oos/crash` exceeds 1 MiB by at most one archive and one
  partial. The start-up screen shows a `Crash record` warning; log `/tmp/oos-crash.log`; the last line names the
  reason (`panic` or `oops`). A watchdog reset leaves no dmesg record, so nothing is archived for it.
- M1b supervision (ADR 0017) includes the abnormal-exit counter, which counts a panic or oops archive once (durably,
  between the archive and the pstore clear, idempotent by the archive name), the `oos-ui.running` markers and safe
  start in oos-ui.

## Safe-start supervision details

ADR 0017 holds the exit and persistence contract. The supervisor supplies a fresh boot-id/process/sequence run
identity before each UI invocation. A same-boot stale running marker counts as an abort. Across boots it counts
only with archived panic/oops evidence or a positive watchdog reset indication. Panic and marker receipts deduplicate
one exit. A missing, zero or unreadable reset indication is no evidence; free-text warnings are not evidence.

A clean requested power-off saves the current setup before teardown. The first quarantined setup is kept; later quarantines retain the two newest timestamped copies, at most three
in total. Promotion retries after 1, 2, 4 seconds, doubling to 300. Candidate changes reset their own 60-second
interval. Log the first persistence error until a complete promotion succeeds. Show
`Persistence off: storage unavailable` on the waveform and System page. Requested power-off proceeds with clean
teardown even if saving fails; attempt the temporary supervisor stop flag.

Ledger mode reads filter without syncing. Explicit writes prune old entries. A positive parse failure selects safe
once, moves a damaged ledger aside and starts a new one; retain at most two damaged copies. Revalidate under the
writer lock. IO failures and timeouts preserve healthy history. Explicit clear does not require awk.
Writers use a BusyBox-compatible mkdir lock with stale-PID reclamation. The counter outlives a timed-out archive
parent. Run crash archiving under `timeout -s KILL 20` and each ledger operation under `timeout -s KILL 5`.
Archive counter failure preserves pstore and signals the supervisor with a temporary flag. These bound processes,
not blocked kernel IO. A failed operation selects safe once; update trials still belong to the update watcher.

The RSS watchdog forces STOP and releases buffer-holding stages at its hard limit, then exits 75 if memory does
not return. Shutdown is bounded; GPU helper death selects CPU rendering instead of exiting the UI.

Reset-evidence limits: not yet verified on hardware for RK3399. The captured dw_wdt TOPs warning describes
configuration, not a reset. The supervisor recognises WDIOF_CARDRESET (0x20) only if bootstatus supplies it.
Source inspection of upstream Linux 6.12 dw_wdt found no bootstatus assignment or CARDRESET capability;
that does not verify the pinned BSP or a live reset-reason interface. A prior watchdog reset left no panic dmesg.
Archive pstore durably and idempotently by record id before clearing it. The confirmed-image pre-rcS panic limit
remains documented in update.md.
