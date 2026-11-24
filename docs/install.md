# Install OOS

The USB install puts OOS alongside stock on the card already in the scope. The installer can save a rescue set.
If OOS hangs before it can restore the stock boot selection, recovery requires opening the scope and removing the card.
Check the [supported models and recovery details](install-usb.md#4-models) before starting.

## First install from stock

The dry run, install and first OOS start were verified on a DHO924S with a modified stock application on 2026-11-24.
The stock USB route has yet to be verified with an unmodified stock application.

1. Download `DHO800_DHO900_OOS_Install.GEL` and `SHA256SUMS` from the same
   [project release](https://github.com/yottabytesolutions/oos/releases/latest). In the download folder, run the
   command for your computer. Compare the full hash to the line for the GEL in `SHA256SUMS`. Stop if they differ.

   | Computer | Command |
   |---|---|
   | macOS | `shasum -a 256 DHO800_DHO900_OOS_Install.GEL` |
   | Linux | `sha256sum DHO800_DHO900_OOS_Install.GEL` |
   | Windows | `certutil -hashfile DHO800_DHO900_OOS_Install.GEL SHA256` |

2. Use a clean USB stick with at least 200 MiB free. Format it as FAT32 or FAT16 if needed. Stock cannot mount exFAT.
   Copy the GEL unchanged into the top folder, and create an empty `oos-dry-run.txt` next to it.
   On macOS, copy from a terminal or run `dot_clean` with the stick's mount path to remove `._*` shadow files.
   Eject the stick before unplugging it.
3. Wait for stock to finish starting, then plug in the stick. On one unit with a modified stock application,
   starting with the stick plugged in caused a “keeps stopping” message. Removing the stick and restarting the
   application cleared it.
4. Open **Start menu > Storage > Upgrade tab > File Path**. Open the disk selector and choose
   **Removable USB Disk (D)**. Select the GEL, tap **OK**, then tap **Upgrade**. The **Upgrade** icon in the Start
   menu opens the online updater, which reported “No firmware available” during the unit check.
5. Allow about a minute for the run. Open `oos-install.log` on the stick on your computer. Use the last run
   in the log, since the stock screen shows no lasting result. The dry run checks the model, signed package and
   card layout and saves `OOS-rescue`. Look for `dry run: the card was not written` and `exit code 0` before
   continuing.
6. Copy `OOS-rescue` to your computer and keep it. It holds the calibration and licences, the start of the card
   and backups of its boot and recovery partitions. Check the saved partition files against the rescue folder's
   own `SHA256SUMS`.
7. Delete `oos-dry-run.txt`, eject the stick, and repeat the stock Storage upgrade with the same GEL. Read the
   latest run in `oos-install.log`. On a first install, look for `recovery: image written and read back`,
   `OOS selected for the next power-on` and `exit code 0`.
8. Remove the stick and power-cycle the scope. Check that OOS starts and that your calibration and settings are
   available.

If the card already holds the same or a newer OOS build, use an empty `oos-reinstall.txt` next to the GEL to
install the GEL's build. Without it, the installer keeps the build on the card and selects it for the next power-on.
If the image is already byte-for-byte identical, the log says `recovery already holds this image`.

### Over ADB

Use the [ADB procedure](https://github.com/yottabytesolutions/oos/wiki/Install-without-opening-the-scope#over-adb)
if stock already offers root ADB over the network. It uses the scope build of `oos-update` and a signed
`.oosupd` package. Back up the calibration and copy the rescue set off the scope before the real run.
The ADB route ran on a DHO924S with modified stock on 2026-11-07.

## After the install

1. A fresh OOS install starts in one-shot mode. The next power-on starts stock. To keep starting OOS, choose
   **System > Boot mode > Persist** or run `oos-update mode persist` while OOS is running.
2. Expect the card to run in High Speed (HS) after a GEL install. If an image requests SDR104 but the card
   negotiates HS, OOS shows “SD card is in slow mode”. A full power-off followed by a cold start restored SDR104
   on the test unit.
3. To select SDR104, use the OOS shell. Check `oos-update status` for a tracked, confirmed image. If the GEL
   install left it untracked, install a newer signed `.oosupd` and wait for confirmation.
4. Set `sd.mode = sdr104` in `/oos/system.conf`. Run `oos-update sd-mode` to preview the change, then
   `oos-update sd-mode --execute` to stage it. Power-cycle to try the new mode. See [SSH setup](network.md#7-ssh),
   [SD mode and trial behavior](update.md) and [boot card mode](boot.md#device-boot-card-mode).

## Update a running OOS installation

1. Download the project-signed `.oosupd` and verify it against the same release's `SHA256SUMS`. Use the hash
   command above with the package filename.
2. For a preview from the OOS shell, run `oos-update install FILE.oosupd` with the path to the package on the scope.
3. Open `http://<scope-ip>:8080/update` in a browser, choose the verified package and click **Install**.
4. Let the update finish. Check the trial, confirmation and boot mode with `oos-update status`.
   See [Signed updates with rollback](update.md) for the update sequence.

## Return to stock

1. Select **System > Boot mode > One-shot**.
2. Power-cycle the scope. Stock starts and OOS stays on the card.
