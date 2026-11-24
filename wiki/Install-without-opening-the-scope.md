Both ways install OOS from the running stock system onto the card in the scope. Use the release GEL for the USB
route, or build your own GEL. The ADB route needs the scope build of `oos-update`. See [[Building OOS]].

## Read this before you start

- Both routes have run on one DHO924S with modified stock. The USB dry run and install were verified on
  2026-11-24, and the ADB route on 2026-11-07. Both routes are unverified with unmodified stock.
- The loader starts the stock copy if it refuses the OOS image. If the OOS kernel hangs before its first
  script runs, every power-on can hang. To recover, you then have to open the scope and remove the card.
- The installer accepts DHO804, DHO814, DHO824, DHO914S and DHO924S by default. See the [[Disclaimer]] and the
  [model checks](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md#4-models).

The installer puts OOS in `recovery`, saves a copy of the stock boot image in `backup`, and selects OOS as its
last step. A failure or power cut before that step leaves stock bootable. See the
[partition and rescue set details](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md).

## From a USB stick

1. Download `DHO800_DHO900_OOS_Install.GEL` and `SHA256SUMS` from the same
   [project release](https://github.com/yottabytesolutions/oos/releases/latest). Follow
   [Verify the download](Install-from-a-release#verify-the-download) before copying the GEL.
   For your own build, find the GEL in `target/image/`.
2. Use a clean USB stick with at least 200 MiB free, formatted as FAT32 or FAT16. Stock cannot mount exFAT.
   Copy the GEL into the top folder of the stick and keep its filename. Create an empty `oos-dry-run.txt` next
   to it. On macOS, copy from a terminal or run `dot_clean` with the stick's mount path to clear `._*` shadow files.
   Eject the stick before unplugging it.
3. Wait for stock to finish starting, then plug the stick into the front USB port. On one unit with a modified
   stock application, starting with the stick plugged in caused a “keeps stopping” message. Removing the stick
   and restarting the application cleared it.
4. Open **Start menu > Storage > Upgrade tab > File Path**. Open the disk selector and choose
   **Removable USB Disk (D)**. Select the GEL, tap **OK**, then tap **Upgrade**. The **Upgrade** icon in the
   Start menu opens the online updater, which reported “No firmware available” during the unit check.
   If Storage cannot see the stick, see
   [USB troubleshooting](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md#the-stock-system-does-not-see-the-stick).
5. Allow about a minute for the run. Open `oos-install.log` on the stick on your computer. The stock screen shows no
   lasting result. The dry run checks the model, signed package and card layout and saves the rescue set.
   Check the last run for `dry run: the card was not written` and `exit code 0` before continuing.
6. Copy `OOS-rescue` to your computer and keep it. It holds your calibration and licences in `rigol-data`,
   the start of the card and copies of its boot and recovery partitions. Check the saved partition files
   against the rescue folder's own `SHA256SUMS`.
7. Delete `oos-dry-run.txt`, eject the stick, and repeat the stock Storage upgrade with the same GEL.
   Check the latest run in `oos-install.log`. On a first install, look for `recovery: image written and read back`,
   `OOS selected for the next power-on` and `exit code 0`.
8. Remove the stick, switch the scope off at the mains, wait 30 seconds and switch it on. OOS starts.

If the card already holds the same or a newer OOS build, use an empty `oos-reinstall.txt` next to the GEL to
install the GEL's build. Otherwise the installer keeps the build on the card and selects it for the next power-on.
If the image is already byte-for-byte identical, the log says `recovery already holds this image`.

## Over ADB

You need `adb` on your computer and stock already offering root ADB on port 55555. Build `oos-update` for the
scope and prepare the signed package before starting. See [[Building OOS]].

1. Connect to stock, save calibration and licences, and copy the updater and package across. Replace
   `<scope-ip>` with the scope's IP address.

   ```sh
   adb connect <scope-ip>:55555
   adb root
   adb connect <scope-ip>:55555   # adb root drops the connection
   adb pull /rigol/data rigol-data
   adb push target/aarch64-unknown-linux-musl/release/oos-update /data/local/tmp/
   adb push target/image/oos-dev.oosupd /data/local/tmp/
   adb shell chmod 755 /data/local/tmp/oos-update
   ```

2. Run the dry run. Check for `dry run: the card was not written`, then copy the rescue set to your computer.

   ```sh
   adb shell /data/local/tmp/oos-update gel-install /data/local/tmp/oos-dev.oosupd --save /data/local/tmp/OOS-rescue
   adb pull /data/local/tmp/OOS-rescue
   ```

3. Keep the rescue set. Check the hashes in its folder with `sha256sum -c SHA256SUMS` on Linux or
   `shasum -a 256 -c SHA256SUMS` on macOS.
4. Run the real install. Look for `OOS selected for the next power-on`.

   ```sh
   adb shell /data/local/tmp/oos-update gel-install /data/local/tmp/oos-dev.oosupd --save /data/local/tmp/OOS-rescue --execute
   ```

5. Switch the scope off and on. OOS starts.

The rescue set takes 132 MiB on the scope. After saving and checking the copy on your computer, you can remove
the temporary files from stock with
`adb shell rm -rf /data/local/tmp/OOS-rescue /data/local/tmp/oos-update /data/local/tmp/oos-dev.oosupd`.

## After the install

On a fresh install, OOS starts in one-shot mode. To keep starting OOS, set **System > Boot mode > Persist** or
run `oos-update mode persist`. The card runs in High Speed (HS) after a GEL install. See
[[First start and choosing how OOS starts]] for the slow-mode notice and the SDR104 procedure.

OOS replaces the stock `recovery` partition. To restore stock recovery, use `recovery.bin` from the original rescue
set with `boot/card-install.py restore` and the card in a reader. See [[Going back to stock]].
