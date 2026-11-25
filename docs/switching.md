# Switch between upstream OOS and this fork

Both directions were verified on a DHO924S on 2026-11-24, using upstream OOS 0.1.15
and this fork's release candidate. The two versions share the card layout, including the `/oos` data area.
For the user guide, see [Switching between OOS versions](../wiki/Switching-between-OOS-versions.md).

## This fork to upstream OOS

This fork trusts only its own release key, so its updater refuses upstream-signed packages by design.
Install upstream OOS from stock with its GEL.

1. Finish any update in progress. Run `oos-update status`, then `oos-update leave`.
   The first line reads `Update tracking cleared. Current mode: persist.` in persist mode and reports
   `oneshot` in one-shot mode. Run `oos-update status` again to see
   `untracked (tracking cleared by oos-update leave)`.
   This clears the build stamp and rollback tracking. The upstream updater compares uploads with that stamp.
   Its update page has no `--downgrade` option. The next power-on still follows the current boot selection.
2. [Return to stock](#return-to-stock). Verify upstream's GEL against its release `SHA256SUMS`.
   Put the GEL and empty `oos-reinstall.txt` and `oos-dry-run.txt` files on a clean FAT32 USB stick.
   The installer code keeps the newer image without `oos-reinstall.txt` and only selects it for the next power-on.
3. Insert the stick after stock starts. Open **Storage > Upgrade** from the Start menu.
   Under **File Path**, select the GEL on **Removable USB Disk (D)**. Select **OK**, then **Upgrade**.
   Check `oos-install.log` on the stick for `exit code 0`.
4. Remove `oos-dry-run.txt`, keep `oos-reinstall.txt` and repeat **Storage > Upgrade**.
   Check `oos-install.log` for `exit code 0` and power-cycle to start upstream OOS.
5. Check that `/oos` mounted and that settings, the calibration copy, the address and SSH host keys are intact.
   On the unit, `/oos` mounted as ext4 with the journal in ordered mode. `system.conf` and the calibration copy
   were byte-identical. The address and SSH host keys stayed the same.
   A dry run of `oos-update install FILE.oosupd` with upstream's own package reported `nothing to do`
   because recovery already held the build.

## Upstream OOS to this fork

1. Get the labelled bridge package for this fork. For 0.2.1 it is
   `oos-v0.2.1-bridge-for-upstream-verify-with-SHA256SUMS.oosupd` (44,857,783 bytes). It is signed with
   upstream's community key so that upstream's update page accepts it. Verify it against the same release's
   `SHA256SUMS`.
2. Upload the bridge through upstream's update page at `http://<scope-ip>:8080/update`. To use `curl`, send the
   package as the raw body of `POST /update`. The upload limit is 73,400,320 bytes. Use the scope's IP address.
   The upstream code returns HTTP 403 for a fully qualified host name outside `.local`.
   The unit upload used `curl` and the IP address.

   ```sh
   curl --fail-with-body -H 'Expect:' -H 'Content-Type: application/octet-stream' \
     --data-binary @BRIDGE.oosupd 'http://<scope-ip>:8080/update'
   ```

   Substitute the bridge filename and scope IP address. The 2026-11-24 candidate run answered HTTP 200 with this text.

   ```text
   signature ok, build 20261124-200342: the scope restarts into it by kexec and installs it from there
   ```

3. Wait for the trial to finish. The startup log reported `on trial (start 1)` and the watcher logged
   `confirmed (trial passed)`. The measured time from upload to confirmation was about 100 to 120 seconds.
4. Run `oos-update status`. After confirmation, `trial` reads `none` and `last event` begins with `confirmed`.
   `recovery` holds this fork's build and `rollback` lists upstream's image as `untracked-...`
   (`untracked-015ca17b3e8a` on the unit). `trusted keys` lists only this fork's release key.
   Settings, the calibration copy and network identity stayed intact on the unit.

## Failed trial

When the trial failed on the unit, the watcher reported this message.

```text
rolled back from 20261124-200342 to untracked-015ca17b3e8a: oos-ui crashed 3 times
```

The updater automatically restored upstream's image to `recovery` and selected OOS for the next start.
The next start ran upstream OOS with the same settings and calibration copy.

## Return to stock

1. In the running OOS version, select **System > Boot mode > One-shot** or run `oos-update mode oneshot`.
2. Power-cycle the scope to start stock. The unit session exercised this return from this fork only.

To keep the running OOS version as the default, select **Persist** or run `oos-update mode persist`.
Every confirmed start selects OOS again. An unconfirmed ordinary start leaves stock selected for the next power-on.

## Calibration and card mode

Skip self-calibration under the other system to preserve the existing calibration copy.
The release GEL writes the default High Speed (HS) boot image when it replaces `recovery`. During an SDR104
trial, the card can negotiate HS on a warm restart and show `SD card is in slow mode.` A cold start restored
SDR104 on the unit when SDR104 was already selected. If the updater reports a fallback to HS, see
[Device boot card mode](boot.md#device-boot-card-mode) for the restage procedure.
