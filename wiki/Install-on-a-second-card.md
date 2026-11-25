OOS goes on a second SD card. Your original card stays untouched in a drawer. This opens the scope.

The steps below use macOS, where they were developed. `tools/sdcard-clone.sh` also runs on Linux, where the disk is
`sdX` or `mmcblkN` instead of `diskN`; that path has not yet been run against a scope's card. The backup and the clone
on Windows and Linux, with the
tools to use and what to watch out for: [docs/sdcard-backup.md](https://github.com/yottabytesolutions/oos/blob/main/docs/sdcard-backup.md).

## You will need

- A DHO800/DHO900. The boot SD card is inside, so **you have to open the scope, which voids the warranty**
- A **second SD card**, best of the same size or larger. OOS is only ever installed on this card. A smaller card
  works too ([write the clone](#write-the-clone-to-the-second-card)), down to about 8 GB on a stock card.
- A card reader
- Python 3 and this repository (`git clone https://github.com/yottabytesolutions/oos.git`)
- To build the image yourself, not with a file from a release: [Rust](https://rustup.rs) with
  `rustup target add aarch64-unknown-linux-musl`, and Docker, running. A Rust from the distribution's packages usually
  lacks that target and `rust-lld`. Hosts: macOS on Apple silicon, Linux on arm64 or x86-64. On Linux, run the scripts
  as yourself, not with `sudo`: put yourself in the `docker` group (`sudo usermod -aG docker $USER`, then log in again).
  On x86-64 the kernel is cross-compiled; the image's busybox, wpa_supplicant and e2fsck come from arm64 containers,
  which need QEMU once: `docker run --privileged --rm tonistiigi/binfmt --install arm64`. See
  [Building on Linux](Building-OOS#building-on-linux).

The whole process takes about an hour, mostly for copying the cards.

## Back up the original card

This step protects your factory calibration. **Do not skip it.**

1. Switch the scope off, unplug the mains cable and all probes, open the instrument and take out the boot SD card.
2. Put the card in the reader and find its disk number:
   ```sh
   tools/sdcard-clone.sh list
   ```
3. Read the whole card into an image. The tool also writes a checksum and a layout report:
   ```sh
   tools/sdcard-clone.sh read diskN ~/oos-backup/sd-original.img
   ```
4. Keep the image in at least two places, then **put the original card away**. From here on you only use the second
   card. On Windows or Linux: [docs/sdcard-backup.md](https://github.com/yottabytesolutions/oos/blob/main/docs/sdcard-backup.md). Always the whole card, never a
   partition, and never let the computer format or initialise it.

## Write the clone to the second card

```sh
tools/sdcard-clone.sh list
tools/sdcard-clone.sh write ~/oos-backup/sd-original.img diskM    # asks you to type the disk id, then verifies
```

Cards of the same nominal size differ by a few MB. If the second card is smaller than the image, `write` refuses and
names the way out: `fit` makes a copy of the image for that card. The last partition, `userdata` (the stock `/data`,
about 1.4 GB used on a stock card), runs to the end of the card; `fit` checks its ext4 and shrinks it in Docker, as far
as the card needs. The original image is only read.

```sh
tools/sdcard-clone.sh fit ~/oos-backup/sd-original.img diskM ~/oos-backup/sd-fit.img
tools/sdcard-clone.sh write ~/oos-backup/sd-fit.img diskM
```

On Linux, give the card's size in bytes instead of `diskM` (`lsblk -bdno SIZE /dev/sdX`), and write `sd-fit.img` with
`dd`.

Put the second card into the scope, close the instrument and start it once. The stock system must work as before, with
its calibration and licences. Then unplug the scope and take the card out again.

## Build the image

With a file from a release, skip this step. Otherwise build the kernel and the image as described in [[Building OOS]].

## Install OOS on the second card

Put the second card in the reader. Unmount it, then do a dry run to check the card and print the plan. With a
release image, verify it against `SHA256SUMS` and substitute its path for `target/image/oos-dev.img` in both
commands. The 0.2.1 release image is `oos-v0.2.1.img`.

```sh
diskutil unmountDisk /dev/diskM
sudo python3 boot/card-install.py install /dev/rdiskM --image target/image/oos-dev.img --fallback-stock
```

The tool refuses cards that do not have the stock layout. If the plan looks right, write it:

```sh
sudo python3 boot/card-install.py install /dev/rdiskM --image target/image/oos-dev.img --fallback-stock --execute
```

This saves the current `recovery` partition to `backups/`, writes OOS into `recovery`, reads it back, and copies the
stock boot image into the empty `backup` partition. That copy is the fallback if the loader refuses OOS.

Now select OOS for the next start:

```sh
sudo python3 boot/switch.py oos /dev/rdiskM --execute
sudo python3 boot/switch.py show /dev/rdiskM
```

## Next

Put the card into the scope: [[First start and choosing how OOS starts]].
