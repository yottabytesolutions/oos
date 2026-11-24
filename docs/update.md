# Signed updates with rollback

Built and tested on the host (unit tests, a real image on a card-shaped file, the start-up watcher in an arm64 Linux
container: stable start, crash loop, install during the watch, hand-installed image). Run on the unit (DHO924S,
2026-11-07/08): installs from the web page and the shell, the kexec trial start, the trial start at the next power-on
and its confirmation. **Not yet verified on hardware: the automatic rollback of a bad update.**

The kexec trial start (below) ran on the unit (2026-11-08): from `install --execute` to the installed build about 20 s,
then the power-on as trial start, confirmed. Its first run had shown that the network links and the display must stop
first (step 3): the package arrived damaged and the display stayed dark. In the kexec start the kernel warns twice
while the PCIe driver loads (`its_build_mapti_cmd`, `its_build_inv_cmd`: the interrupt translation tables of the
previous kernel are still there); the display and the acquisition work, and the power-on after it has no warning.
That held with `poll_mode=1`. With MSI (`xdma.params = poll_mode=0`) every C2H read in a kexec start timed out
(0x4034 = 0x21c0, descriptor engine stopped, nothing acquired): the completion interrupts go through those tables and
never arrive. A real restart fixed it. So `oos-fpgad` loads xdma with `poll_mode=1` in a kexec start (`oos.kexec=` on
the command line) whatever the setting says, and logs the override (`oos-fpga-up` does the same). After a successful
install the kexec start restarts the scope at once (step 4). rcS runs that restart before the acquisition bring-up, so
on this path the kexec session never loads xdma. The override only matters when the restart is refused or fails. It
is covered by host tests only: no real install path leaves a kexec session running, so it is untested on the unit.

Code: `hal/src/update/` (hashes, Ed25519, package, card, state, kexec), `fpgad/src/bin/oos-update.rs` (command line),
`ui/src/ui/update.rs` and `ui/src/http.rs` (GUI, web upload), `rootfs/initramfs/etc/rcS` (start count, watcher).

Boot card mode is device-local: `sd.mode` in `/oos/system.conf` defaults to `hs`. Package verification precedes
DTB selection; the signed package stays unchanged while the selected image's loader id and local update hash
are recomputed. Kexec stages the selected tree too. `oos-update sd-mode [--execute]` restages a confirmed active
image for a mode-only trial with an HS rollback image. See [boot mode](boot.md#device-boot-card-mode) for the retry
rule and the pre-rcS safety boundary. This boot path is host-tested only and still needs a unit run.

## What it protects against

| Failure | What happens |
|---|---|
| package damaged in transfer, or not signed by a trusted key | refused before anything is written |
| detected model is absent from the package model list | refused before card writes or kexec unless explicitly overridden with `--any-model`; the signed board target must always match |
| image the stock loader would refuse (wrong SHA-1 id, section length not a multiple of 4) | refused before anything is written |
| power lost while `recovery` is written | the boot magic is set to `KRNL` before the write and back to `OOSK` only after the read-back, so the next power-on starts the intact stock image in `boot`; the previous OOS image is still in the rollback slot. The stock copy in `backup` is not needed for this |
| new kernel or initramfs hangs **before** rcS (panics, cannot find the card) | the new image is first started by kexec, with `recovery` untouched (below): power-cycle, and the previous OOS build starts again (stock in one-shot mode). The next OOS start reports it. A new device tree is tried too, from 0.1.7 on (below). Not covered for `--new-dt`, packages without `trial kexec` (older builds) and `--no-kexec` |
| new build hangs in rcS after `oos-oneshot` and the start count (the first three commands of rcS) | `KRNL` is back, so the next power-on boots stock; when OOS is started again, the trial image's second start rolls back |
| new oos-ui crashes in a loop (3 restarts) or never runs 20 s stable within 180 s | the watcher writes the previous image back right away; the notice is shown on the panel; power-cycle starts the previous build |
| cold boot ends in `abort` or hangs while oos-ui runs stable (hardware trouble) | counts as a good start (no automatic rollback); the screen names the stage, System > Roll back update is available |

## Trial start by kexec

OOS has no loader of its own, so nothing counts the starts before rcS. The loader does not consume `OOSK`;
stock selection is restored only by `oos-oneshot`. Boot arguments use `panic=0` until rcS confirms stock
selection, then enable a 10 s panic restart through `/proc/sys/kernel/panic`. An early cold-start panic halts
rather than resetting into the selected image indefinitely. Kexec command lines also force `panic=0`, even
when the running image's old command line had a panic restart timeout. The inherited watchdog can still
reset a failed kexec start into the installed fallback. An image written to `recovery` and selected with
`OOSK` that hangs in the kernel would start again at every power-on. So `install --execute` does not write it:

1. Verify the signature, image hash, loader limits, board and model, downgrade, card layout, and device tree.
   Wait for confirmation if an update is still on trial.
2. For a package with the signed `trial staged` line, reserve the rollback slot for the
   candidate. Clear the previous rollback entry and save `staged <build> <sha256> <len>` before the write.
   Write the original signed image bytes, sync and read them back while the UI and watchdog supervisor keep
   running. The boot magic and `recovery` stay as they were. Record `kexec <build>` and the payload byte count
   once staging succeeds.
3. Load the kernel and initramfs with `kexec_file_load`, or use kexec-tools (`kexec_load`) when supplying a
   changed tree. The initramfs gets a small archive with the signed header through its terminating empty line
   (`/oos-kexec.hdr`), the verifying key (`/oos-kexec.key`) and the booted tree section (`/oos-kexec.dt`).
   The command line gets `oos.kexec=install,staged` and `initramfs_async=0`. The archive unpacks before driver
   probes. Under the update lock the watcher and oos-ui stop, the network links go down and the display unbinds
   to stop their DMA. The updater syncs, attempts to remount `/oos` read only, takes over the watchdog and jumps.
4. The candidate's rcS runs `oos-oneshot`, mounts `/oos` and runs `oos-update boot`. The receiver verifies
   the header with its built-in release key, checks `trial staged` and the exact running build id, reads the
   signed length from the slot and verifies its SHA-256. The ordinary install checks board, model and loader
   again, saves the previous recovery image in the slot, then writes and verifies the candidate in `recovery`
   and arms `OOSK`. After a successful install, it warm-restarts into the trial. It checks for `OOSK` and
   the installed build on trial before restarting. The `kexec-restart` mark limits it to one restart per build
   and is cleared on the next ordinary start. Otherwise the panel asks for a power-cycle.

Staging gives up the older rollback image at step 2. `status` reports a staged candidate or no rollback image,
and `rollback` refuses until a verified rollback image is available. A failed read-back aborts before loading.
After an interrupted stage or a jump that never finishes, the next ordinary OOS start reports the unused
candidate once and clears the pending state. The updater retains slot ownership so the next install can reuse
it even if its header was torn.
A failed load or returned jump restores the running services. A refused handoff leaves the image regions as
they were at rcS and reports the reason. In **one-shot** mode the next power-on starts stock Android because
`oos-oneshot` selected `KRNL`. In **persist** mode the watcher re-arms `OOSK` for a valid recovery image with
no open trial, so the next power-on then starts that image. Until the re-arm succeeds, the next power-on starts
stock Android. If the install fails after it begins writing `recovery`, it attempts to restore the previous
image from the slot.

A package with `trial kexec` alone uses the legacy kexec path. Its full signed package rides in
`/oos-kexec.oosupd` in place of the header, and its command line gets `oos.kexec=install`. Both senders use the
same watchdog payload guard. The legacy receiver still reads the passed key and can leave a refused package in
RAM for inspection until restart. Older senders, including the bridge, use this path even for staged-capable
packages. The staged receiver trusts only the release keys compiled into the receiving build and ignores the
carried key. Ship a release that replaces the old key once over legacy kexec, signed with the old key and
announcing only `trial kexec`. Later releases can use staged kexec with the new key. A package with neither
capability takes the direct install path.

What it does not cover:

- A downgrade to the bridge receiver can discard new state fields when it saves a refusal. If the slot header
  is also damaged, a subsequent install can refuse the slot for inspection.
- A device tree change before 0.1.7: those builds have no kexec-tools and refuse an image with another tree for this
  path (`--new-dt` writes it directly, without a safety net before rcS). Install 0.1.7 first.
- A kernel that runs from kexec but hangs before rcS from a cold start (the kexec start runs on warm hardware) starts
  again at every power-on, as with a direct install.
- The install from 0.1.0 to a later build: 0.1.0 has no kexec start and writes the package directly.
- The command line of the first power-on comes from the device tree's `bootargs`, as the kexec start's did; a
  different command line in the image header is not used by this loader.
- A start whose device tree is unknown (`/tmp/oos-booted.dt` missing: started before a build with this, or
  recovery unreadable at start-up) takes the tree from `recovery` only while the state names the running build
  there; otherwise power-cycle once or use `--new-dt`.

### Another device tree

`kexec_file_load` gives the new kernel the running tree, so it cannot try an image with another one. From 0.1.7 the
image has kexec-tools (`rootfs/build-kexec.sh`, static), and `oos-update` loads such an image with `kexec_load`, which
takes a tree. Not the image's tree as it is: the stock loader adds to it at every power-on (reserved memory for the
ramdisk and the boot logo, `bootargs`, the display mode it set up for the logo, the logo's memory). The new tree gets
those changes: the difference between the tree in the image that started (`/tmp/oos-booted.dt`) and the tree the
kernel got (`/sys/firmware/fdt`), applied by node path (`hal/src/update/fdt.rs`). So the kexec start runs with the
tree a power-on of the new image would give it. The loader writes plain values; a change in a `phandle` (dtc numbers
them anew in every tree) or to a node the new tree does not have refuses the kexec start, and the install then needs
`--new-dt`. Left out on purpose: the running kernel's ramdisk (`linux,initrd-*` in `/chosen` and its reservation):
kexec-tools names the new one, and the kernel drops a ramdisk that lies in reserved memory (seen on the unit: the
55 MB of the kexec start fell into the 13 MB the loader had reserved for its own, and the kernel found no root; the
power-cycle brought the running build back, as it should). On the unit 33 changes of the loader carry over.

Where the segments go: above 1 GiB (`kexec --mem-min`), and above every GIC table the running kernel's log names. The
GICv3 keeps its LPI and ITS tables across kexec (it cannot be given new ones); the started kernel takes them over and
writes to them, and `/proc/iomem` does not show them. On the unit they lie at 0x2400000 and 0xa320000..0xa400000,
where kexec-tools put the 55 MB initramfs at first: the new kernel found it damaged ("rootfs image is not initramfs
(uncompression error)", read from pstore after a start with `panic=5`), with every other bus master stopped as well.

### Purgatory and the watchdog budget

The `kexec_load` path always passes `-i` (`--no-checks`) to kexec-tools. The signed package and its image hash
are verified before load; Linux copies the supplied segments into kernel-owned memory. Uncached SHA-256 in
purgatory after relocation adds no signature verification. `kexec_file_load` already starts without a post-copy
purgatory digest. A corrupt start that hangs is caught by the watchdog while the installed image is untouched,
and the later trial/rollback start checks the installed build. This is the fallback the four failed starts
exercised: the watchdog reset the SoC and the previous build returned. This does not prove arbitrary memory
corruption will always hang or be detected. Signature, image-hash and read-back checks remain required.

Owner-observed unit evidence, 2026-11-15 (task 0054): four changed-tree starts with digest checks reset about
26 s after the jump; these failures followed watchdog kicker supervision (`abd00ca`). D-to-T with a single
harmless tree property difference and `-i` succeeded, with the same kernel. The measured timeline from the jump:

| Event | Time |
|---|---:|
| Hardware watchdog period (maximum, fixed TOPs) | 21.5 s |
| New kernel first boot message, after uncached relocation | +6.88 s |
| `/init` starts | +11.77 s |
| Watchdog about to open, hardware running, timeleft 20 | +12.09 s |
| Back online, after install and warm trial restart | about +32 s |

Payload: kernel 27,159,040 bytes, staged initrd 57,611,256 bytes, merged DTB 94,618 bytes (about 85 MB total).

The measured legacy payload carries the compressed initramfs and the whole package. The kernel and initramfs
count twice on this path. The slot-based path carries the kernel and ramdisk once plus the small handoff
archive. The image regions limit its image size. A changed-tree start also supplies the merged DTB. Both paths
keep the ceiling in `hal/src/update/kexec/budget.rs` and log each start's payload size.

Ceiling history:

- 85 MiB, set about 5 % above the measured payload. Build 20261124-052052 reached 89,137,611 bytes, 8,651 over, and
  was refused.
- 90 MiB since 2026-11-24, by owner decision, without a new measurement of the timeline above. The margin rule below
  uses 18.1 s of the 21.5 s period at the measured payload and keeps 5 s in reserve, which leaves 3.4 s. The
  part of the start that grows with the payload is the uncached relocation inside the first 6.88 s, so a 6 %
  larger payload is expected to add less than 0.5 s. That is an inference, not a measurement.
- Measured 2026-11-24 on a DHO924S at a payload of 89.1 MB (legacy path, SDR104), without UART: the kexec'd
  kernel keeps the sender's clock and its console survives the trial restart in persistent RAM, so the sender's
  uptime at the last network reply before the jump can be compared with the new kernel's first console
  timestamp. Jump to first message: at most 7.7 s (an upper bound, the network stops before the jump; the UART
  figure at 85 MB was 6.88 s). From the first message: watchdog driver registered +4.25 s, `/init` +4.80 s,
  watchdog opened +5.13 s with the hardware running, install finished +15.8 s. With the margin rule below:
  1 + 7.7 + 4.25 + 5 = 17.95 s of 21.5 s. The 90 MiB ceiling adds about 6 % to the size-dependent part.
- If a kexec start is ever reset by the watchdog (the previous build returns and `oos-update status` reports the
  failed start), record the payload size here and lower the ceiling below it.
- Stripping the release binaries would free 0.7 MB of payload but cost 16 % record rate at 1 ch 5 ns/div on the
  unit. Do not strip.
Kernel, initrd, DTB and purgatory segments were at `0x40000000`, `0x41aa0000`, `0x45192000` and `0x451aa000`.
The working root cause is watchdog exhaustion during uncached hashing plus relocation, with MMU/D-cache off.
The successful bypass and the pre-supervision history support this; purgatory's checks-on duration was not
measured and a digest mismatch was excluded only indirectly.

Before stopping the watcher or UI, quiescing devices or remounting, both start paths check the supervisor pid file,
its exact `/bin/oos-watchdog` cmdline argument and the absence of a handoff request. Under the update lock, a
leftover `/tmp/oos-watchdog.handoff` is removed only if its recorded requester pid is dead (`kill(pid, 0)` reports
ESRCH); live, unknown or malformed requesters are refused with a specific message and left for inspection. Older
empty request files cannot be recovered automatically. The subsequent preflight is read only. If `/dev/watchdog`
is absent, the supervisor need not have published a pid: proceed without a watchdog ping and log that decision.
Counter survival is then unverified. The checks repeat at takeover to catch changes during teardown.

Both start paths transfer the single-open watchdog from `oos-watchdog` after the read-only remount attempt.
Supervisor crash backoff uses an interruptible background sleep and explicit wait, so USR1 is processed even
in a 60 s backoff. After reaping the kicker (or cancelling backoff with no kicker), the supervisor copies the
requester pid to `/tmp/oos-watchdog.ack`. Takeover waits for that exact acknowledgement before opening the
device, sharing a 2 s deadline with the open retries. An idle device alone is not proof of handoff. Read-only
preflight also rejects an acknowledgement left by a previous handoff. On cancellation the supervisor removes
its acknowledgement, resets backoff and resumes. These task 0067 changes are host-tested, not yet verified
on the unit.
The supervisor publishes its pid; an exclusive handoff request recording the updater pid plus USR1 makes it kill
and reap only its own BusyBox kicker with SIGKILL, avoiding magic close, then pause until the request is removed.
The updater waits at most 2 s for acknowledgement and the device, keeps its handle open, writes the immediate jump timestamp for a changed tree,
performs a final `sync()` and writes a non-`V` byte to ping hardware immediately before `reboot(KEXEC)`.
No sync, remount, logging or process launch comes between that ping and the syscall. This makes the final
userspace ping less than about 1 s before the syscall in normal scheduling. The syscall's shutdown-to-jump delay still needs unit measurement;
userspace cannot give a hard deadline across a stalled kernel. The normal kicker covers the earlier sync/remount;
the updater holds the device during the final sync, so a stuck flush remains bounded by the active watchdog.
On any acquisition/ping error or returned syscall, close the handle without magic close, remove the request,
and restart the supervised kicker immediately with backoff reset. The ordinary updater undo then restores the UI
and links. If the updater dies holding the request, no new userspace keepalive occurs and the active watchdog
expires; it is never deliberately handed to an indefinitely feeding kernel worker.

Source inspection of the pinned 6.12.115 base and `kernel/config` establishes why this matters:

- `CONFIG_WATCHDOG_NOWAYOUT` is off (dw_wdt's `nowayout` parameter defaults to it). With no reset line,
  `dw_wdt_stop` marks hardware running and returns success. Magic close clears the core's active bit and lets
  its worker feed indefinitely (`CONFIG_WATCHDOG_OPEN_TIMEOUT=0`). It gives neither a final immediate ping
  nor a watched failure path, so the handoff uses no magic close.
- A non-`V` write calls `watchdog_ping`; dw_wdt has no minimum hardware heartbeat interval, so the core calls
  its register ping synchronously. A nonmagic close leaves it active; worker pings stop at the userspace deadline.
- Ordinary kexec calls `kernel_restart_prepare`, which invokes the reboot notifier chain with `SYS_RESTART`.
  **`SYS_RESTART == SYS_DOWN`** (`include/linux/reboot.h`), so the stop-on-reboot notifier registered by dw_wdt
  does call `dw_wdt_stop`. The counter survives only because there is **no `resets` on the watchdog DT node**:
  without a reset line, `dw_wdt_stop` only sets `WDOG_HW_RUNNING` and returns success. This is load-bearing;
  adding a reset line would stop the watchdog across kexec. The DTS regression test guards board/unit overrides.
  Once registered in the new kernel, boot-enabled handling feeds it until userspace opens it. Nothing can feed
  it during relocation and the unregistered part of boot.

See the upstream [watchdog core](https://github.com/torvalds/linux/blob/v6.12/drivers/watchdog/watchdog_dev.c),
[reboot notifier](https://github.com/torvalds/linux/blob/v6.12/drivers/watchdog/watchdog_core.c),
[dw_wdt](https://github.com/torvalds/linux/blob/v6.12/drivers/watchdog/dw_wdt.c),
[restart codes](https://github.com/torvalds/linux/blob/v6.12/include/linux/reboot.h),
[kexec shutdown](https://github.com/torvalds/linux/blob/v6.12/kernel/kexec_core.c) and
[BusyBox kicker](https://github.com/mirror/busybox/blob/1_35_stable/miscutils/watchdog.c).
The local pinned kernel sources were checked, rather than assuming the upstream files are identical.

Margin rule: keep at least 5 s between new-kernel watchdog takeover and hardware expiry. Allowing 1 s for the
last ping's age gives `1 + 12.1 + 5 = 18.1 s < 21.5 s` for the measured payload; the registration ping actually
precedes the observed userspace open, so using open is conservative. Both loaders refuse payloads above
**90 MiB (94,371,840 bytes)** before loading: kernel file plus staged initrd plus the supplied DTB, if any.
Legacy image headroom at the measured payload is about **4.7 MB** (2 MB at the earlier 85 MiB ceiling).
The boundary and overflow tests pin the guard. The measurement above used the changed-tree path.
Any kernel, initramfs, DTB, segment memory-size/padding, CPU-clock
or shutdown-path change requires checking the 5 s margin on the unit before release, even below the ceiling. The raise to
90 MiB was made without that measurement (ceiling history above). Do not raise it further without one. The explicit `--no-kexec` route skips the trial start: a hanging kernel restarts at
every power-on until the card is pulled. It must not be treated as an equivalent safe fallback for a larger image.

Slot staging is host-tested. Unit acceptance needs same-tree and changed-tree staged installs, a cut during
staging followed by a successful retry, and a measured watchdog margin of at least 5 s. The bridge's legacy
jump margin also needs measurement.

`oos-update kexec-check FILE` loads a package's kernel and initramfs and unloads them again, nothing written, no
restart: it shows that the running kernel takes the image for a kexec start.

Only opened regular card files may use a simulated `--model`, extra `--keys` or the direct card-file install
route. Device aliases are canonicalized; the opened block device number is resolved through sysfs and
must name a whole disk on the board's SD controller. This also covers gel-install on stock Android,
where the card can be named `/dev/block/mmcblk1`. Symlinks and paths containing `.` cannot turn hardware
into a test file. Model comparison and `--any-model` retain their normal meaning for hardware.

## Card layout used

| LBA | size | content | written by the update |
|---:|---:|---|---|
| 122880 | 1 sector | `boot` magic `KRNL` / `OOSK` | yes (to `KRNL` before `recovery` is written, to `OOSK` after) |
| 188416 | 64 MiB | `recovery`: running OOS image | yes |
| 319488 | 32 MiB | `backup` start: stock boot copy (the loader's last fallback) | never |
| 385024 | 64 MiB | `backup` + 32 MiB: **rollback slot**, a previous OOS image or a staged candidate | yes |
| 516096 | 16 MiB | `backup` end: /oos data partition | never (state file through the file system) |

The slot is all zero on an original card (the whole `backup` is, see `boot.md`). Before the first write the tool
checks that the slot is either empty or holds an Android boot image. An incomplete slot the updater already
owns can also be overwritten. Every image write is padded to whole sectors and synced. The updater drops
the cached pages and reads the region back from the card.

The state is written before the `recovery` write, after a write test of the state file before any card write. The
rollback entry is cleared before a slot write. `slot.incomplete yes` records that the slot belongs to the updater
even if a cut damages its header, until a verified rollback image is saved. A rollback changes the state after
its write. The updater reads the card to verify "already holds", so an interrupted rollback can be retried.

## Package `.oosupd`

```
OOS-UPDATE 1
target dho900-recovery
build 20260101-120000 kernel 6.12.115+
kernel 6.12.115+
size 31141888
sha256 <image>
unit none
key <key id, 16 hex digits>
sig <Ed25519 over every byte above this line>

<image>
```

The header is readable text. The signature covers the header, the header names the image's SHA-256. The `build`
starts with the UTC build time; an older build than the installed one is refused unless `--downgrade`.

The signed `models` extension lists the models supported by the generic image. The signed board target must match
this board family, and the detected hardware model must appear in that list. Detection uses the HAL reader without
experimental overrides or an old image's unit-name file. A DTB change does not change hardware model identity.
Legacy packages without a model list require an explicit `--any-model` override on a detected device.

## Keys

- Builds from this repository trust only this project's release key: Ed25519 key id `5b293adb4389dc84`,
  public key `f5f316e2b278b4c29a3724e711493ebb748f7f22d6c9eff265822110f4a4343d`.
  `keys/update.pub` is compiled into every build (`oos_hal::update::TRUSTED_KEYS`). The release signing secret
  is kept outside the tree. The upstream maintainer and former public community keys are not trusted.
- Signing requires `--key FILE` or `OOS_SIGNING_KEY`. `--key` takes precedence. There is no built-in signing seed
  or home-directory discovery.
- A trusted public key cannot be used as a signing seed. The derived public key must be trusted by this build.
  For a deliberate key transition or host test, direct `oos-update pack` accepts `--untrusted-ok`.
  This flag does not change which keys an installed build trusts. Test-only signing fixtures are trusted by no build.
- A valid signature does not authorize installation on a shared unit. Installation needs the scope lease with
  install capability and the owner's go.
- `oos-update keygen --out FILE` creates a secret seed, mode 0600. Keep it outside the repository, back it up
  (password manager or an offline copy) and never publish it. A replacement release key requires an update to
  `keys/update.pub` and a build that trusts it. If the sole trusted signing key is lost, recovery requires a
  hand-installed image.
- Ed25519 and SHA-2 are implemented in the tree (no crates beyond libc): RFC 8032 test vectors, FIPS examples, and a
  cross-check against Python `cryptography` in both directions (Rust signs / Python verifies and the reverse).
- The signature guards the install path only. The root shell over SSH takes the scope's password or a trusted key
  ([network.md](network.md) 7); SCPI and the web remote retain their existing unauthenticated bench-LAN interfaces.

## Use

Build, sign and package are separate host steps. The paths below use the default
`target` directory; an overridden `CARGO_TARGET_DIR` selects its own `image/` directory
([output contract](../tools/OUTPUTS.md)). Receipts reject stale or replaced inputs.

```sh
boot/build-dev-image.sh                         # generic image, checksum, build id and models; never signs
boot/sign-dev-image.sh --key /path/to/signing.key # target/image/oos-dev.oosupd
boot/build-gel.py                               # target/image/DHO800_DHO900_OOS_Install.GEL
```

The build embeds `/etc/oos-models` and saves `target/image/oos-dev.models` with an output receipt. Signing checks
the saved list against the image payload. Missing, stale or replaced sidecars refuse signing; rebuild the image.
No device name, MAC, LAN prefix or SSH trust input participates in the build.

For signing, `OOS_SIGNING_KEY=/path/to/signing.key boot/sign-dev-image.sh` also works.
The signer uses `target/image/oos-dev.build`, saved with the image. It builds the host tool offline.
`oos-update pack --image IMG --build ID [--models LIST] [--key FILE] --out FILE.oosupd` is the direct signing step.
It also accepts `OOS_SIGNING_KEY`. Direct `pack` reads `etc/oos-models` from the image's initramfs and uses it when
`--models` is absent; a missing identity means `none`. An explicit `--models` must match that embedded list or
signing is refused. The package version and output names are unchanged.

Install on a running OOS:

- browser: `http://<scope>:8080/update`, choose the file, Install; or
  `curl --max-time 300 --data-binary @target/image/oos-dev.oosupd http://<scope>:8080/update`

The web server answers the upload once the whole package has arrived and its header (signature, target, length)
checked out: `accepted, build ...`, or `REFUSED: ...`. The image hash and the copy for the install job run after
that, off the screen's loop; their result and the install's show in System > Update and on `GET /update`. Every
request ends in an answer within bounded time (fork, `ui/src/http.rs`):

| case | answer |
|---|---|
| `Expect: 100-continue` (curl sends it for large bodies) | `100 Continue`, or the refusal at once without reading the body |
| POST without `Content-Length` (also chunked) | 411 |
| body over the limit (70 MiB for `/update`, 1 MiB otherwise) | 413, before the body |
| foreign page, or an address outside `net.allow` for `/update` and `/mcp` | 403, before the body |
| no data for 10 s, or the whole request over 120 s | 408 |
| request head over 16 KiB | 431 |
| more than 8 requests being read, or the screen's loop does not take a complete request within 10 s | 503 |

`--max-time` on the client is still worth giving: it bounds the network path too.
In a script, do not put a plain `nc` SCPI query (port 5555) before the curl: the SCPI server keeps the connection
open, and macOS `nc -w` does not exit on it, so the curl never runs (`nc -G 3 -w 3` or a script with a timeout does).
- shell: upload the file to /tmp, `oos-update install /tmp/oos-dev.oosupd` (dry run), then `--execute`

On `/update`, an incompatible model first asks for separate confirmation naming the package model list and detected
model. HTTP 409 JSON contains `"refused": true`, `package_models` and `running_model`. Cancel leaves nothing staged;
confirm uploads the same file with `?any-model=1`. The reply, job log and CLI report the explicit override.

The CLI equivalent is `oos-update install FILE --any-model`, then `--execute`. The warning names both models and
survives the kexec handoff. It never bypasses trusted keys, signatures, hashes, the board target, downgrade or loader
checks. Card-file tests may select a simulated `--model`; actual devices always use hardware detection.

**Migration from a unit-named image.** Save any MAC that exists only in image defaults into `/oos/system.conf` before
installing, as in [network.md](network.md) section 6. If the old image supplied capability overrides for hardware
that differs from its stored model, preserve those keys in `/oos/limits.conf` before installation (docs/models.md 4.2).
Keep `/oos/net/ipv6.secret` unchanged. The new updater ignores
`/etc/oos-unit` and accepts the generic package for the detected model without an override. An updater still running
the old unit-profile guard cannot interpret the new model list: use its explicit logged `--any-unit` once for this
migration, after verifying the generic image's signed board target and supported model list. That legacy flag is not
accepted by the new updater; subsequent updates use model matching. Builds older than the unit guard need no such
flag. Scope release, lease and install/reboot authorization still apply.

For host card-file tests, `install --card FILE --model NAME` supplies the simulated running identity; without
`--model` it is `none`, independent of host identity files. Hardware installs always detect the model and refuse this simulated-identity option.

The scope restarts into the new build by kexec, which installs itself (about a minute), then restarts once more into
the trial start; System > Update shows the result. Do not restart by hand within the next minute: the trial start
re-arms `OOSK` only after its confirmation, and a restart before that starts stock. If the screen stays dark after the
kexec start, the next power-on follows the boot magic. `OOSK` starts the image in `recovery`, which is still the
previous build if the updater never wrote `recovery`. `KRNL` starts stock Android. A package without a kexec
capability, `--no-kexec` and `--new-dt` write the image directly. Power-cycle to start that image on trial.

| command | |
|---|---|
| `oos-update status` | boot magic, recovery and slot (build, SHA-256, loader id), trial, mode, last event |
| `oos-update install FILE [--execute] [--downgrade] [--force] [--any-model] [--no-kexec \| --new-dt]` | see above |
| `oos-update kexec-check FILE` | load and unload the package's kernel for a kexec start; nothing written |
| `oos-update rollback [--execute]` | slot -> recovery (checked against its SHA-256 first), arm OOS |
| `oos-update confirm` | end a trial by hand (after stopping oos-ui for development, `/tmp/oos-ui.off`) |
| `oos-update mode persist` / `oneshot` | persist: OOS arms itself again after each confirmed start, and is selected for the next power-on at once; oneshot: stock is selected at once. The GUI has this as System > Boot mode |
| `oos-update verify FILE` | signature and image check, anywhere (`--keys` only here and for card-file tests) |
| `oos-update pack ...`, `keygen` | host side |

Logs: `/tmp/oos-update.log` (start count, watcher), `/tmp/oos-update.job` (GUI / web job), state `/oos/update/state`.

## Trial rules (`oos_hal::update::judge`)

- good: oos-ui has run 20 s without a restart and the start-up wrote `end` or `abort` (no acquisition hardware in the
  image: oos-ui alone); at 180 s also with a start-up still running, if oos-ui runs stable.
- bad: oos-ui restarted 3 times, or no oos-ui that ran 20 s stable by 180 s. The oos-ui process is followed by pid, so a
  forked child does not count as a restart.
- The trial is about the running image: the watcher only judges when `/etc/oos-build` equals the build on trial
  (`pack --build` must get the image's own build id, passed by sign-dev-image.sh via `target/image/oos-dev.build`),
  and applies its verdict only if the state still names the same image (an install from the web page during the
  watch stays on trial).
- Powering off before "confirmed" (about 20 s after the start-up) counts as an unconfirmed start: the next OOS start
  rolls back. System > Update shows "on trial" until then.
- `/tmp/oos-ui.off` signals a deliberate UI stop. The watcher leaves the trial open, skips the persist arm and
  publishes completion with the same Power restart hint as an unverified boot selection.
- rcS `oos-update boot`: a trial image that starts a second time without confirmation is rolled back at that start, with
  a notice on the panel first; a running image that is not the one on trial (recovery written by hand) drops the trial
  instead.
- All state changes take a lock (`/oos/update/state.lock`), so rcS, the watcher, the GUI job and a shell call never
  interleave.
- Power off, restart and a rollback tap are refused while an update job runs. The panel's own power key cannot be refused.
- The watcher writes `/tmp/oos-update.judged` after applying its verdict and completing any persist arm. A confirmed
  SDR104 trial records the negotiated mode from the startup check in the update event. Confirmation still follows
  the software-health rule when the card is slow.
- If the watcher cannot open the card for a persist arm, it logs the failure and publishes completion with boot
  selection unverified. If it cannot open the card for a failed trial's rollback, it issues the same rollback-failed
  notice as a restore error. A failed open leaves boot selection and trial state as they were before the attempt.
- The Power dialog offers Restart after the watcher finishes. If the marker is missing, Restart becomes available
  190 seconds after the UI starts, with a warning that the stock system may start. The active-update lock still
  gates the action. The watcher completes the power gate after any kexec persist arm and skips the trial verdict.

Install requests wait for the current trial to be confirmed. The receiver keeps the confirmed or hand-installed
image in `recovery` as the rollback image (`untracked-<sha>` for a hand-installed image). Slot staging needs a
different, valid image in `recovery` to keep for rollback. A forced rewrite of identical recovery bytes is
available through direct install and keeps the existing rollback image.

## Boot mode

Default stays **one-shot**: every OOS start puts `KRNL` back. `persist` makes OOS the normal system: after every
confirmed start the watcher sets `OOSK` again, checked and under the lock. An unconfirmed start leaves `KRNL`, so the
next power-on boots stock: a boot counter with one try, without an own U-Boot.

## Not covered

- Rolling forward after a rollback: the failed image is not kept; install the package again.
- Updating the stock loader, `trust`, the FPGA flash or the partition table: never part of an update.
- A power cut during the slot write before `recovery` is written leaves the old image in `recovery`: harmless, run
  the install again.
- The downgrade check only knows the state's `current`: with no state, an untracked image or after a rollback, any signed
  build can be installed from the LAN without authentication. It still runs on trial.
- **`--new-dt` has no safety net**, and neither has a direct install (`--no-kexec`, packages without `trial kexec`, the
  USB stick): a kernel that panics, hangs or cannot reach the SD card before rcS starts again with OOS selected on
  every power-on, and the card has to come out. Test such a change on a card you can take out first.
- The watcher judges a start by oos-ui staying up and the start-up checklist finishing. A build that runs but measures
  wrongly is confirmed and becomes the rollback image at the next install.
- A full `/oos` stops the trial bookkeeping: an update then neither confirms nor rolls back. The next power-on in one-shot
  mode starts stock.

Selecting OOS by hand (`oos-boot-oos-again`, `oos-update arm`) checks that `recovery` holds an image the loader takes and
that no install or rollback runs; after a cut job the next start stays stock. `oos-update confirm` ends a trial only when
the image on trial is the one running. Images whose load addresses or sizes are outside what the loader handles
(docs/boot.md) are refused at install.

## Web upload limits (ui/src/http.rs)

At most 8 connections; bodies up to 70 MiB only for `POST /update` (128 MiB for `POST /waveform`, 1 MiB elsewhere),
an announced larger `Content-Length` gets 413 at once; 10 s idle and 120 s per request. A body over 1 MiB is taken one
at a time: while one is being read, the next gets 503 when its head is in. `POST /update` and `/mcp` from outside
`net.allow` get 403 when the head is in, before the body is read. A `POST /waveform` is parsed on a thread of its own
and put into its slot by the frame loop, which answers then; while one is parsed, another gets 503. The page runs
`oos-update status --quick` (headers only, no hashing of 2 x 31 MB).

## To check on the unit

1. Install `target/image/oos-dev.img` once by hand (it brings `oos-update`), power-cycle.
2. `oos-update status`: layout ok, recovery untracked, slot empty, trusted key listed.
3. Build again, install the `.oosupd` from the web page, power-cycle: trial, then "confirmed" in
   `/tmp/oos-update.log` after ~20 s past the start-up.
4. Rollback test: `oos-update rollback --execute`, power-cycle, check the build in About.
5. Time of `install` (two 31 MB writes plus read-back on the SD card) and of the upload over 100 Mbit.
