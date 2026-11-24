Switch between this fork and [upstream OOS](https://github.com/dekajoule/oos).
Both directions were tested with upstream OOS 0.1.15 on a DHO924S on 2026-11-24.

## This fork to upstream OOS

This fork trusts only its own release key, so its updater refuses upstream-signed packages by design.
Use stock and upstream's GEL to switch.

1. Finish any update in progress. Run `oos-update status`, then `oos-update leave` on the scope.
   Look for the line beginning `Update tracking cleared.` and run `oos-update status` again to see
   `untracked (tracking cleared by oos-update leave)`.
   Clearing the recorded build avoids a downgrade refusal. Upstream's update page has no `--downgrade` option.
2. [[Go back to stock|Going back to stock]]. Put upstream's GEL and empty `oos-reinstall.txt` and
   `oos-dry-run.txt` files on a clean FAT32 USB stick. Verify the GEL against upstream's `SHA256SUMS`.
   The installer code keeps the newer image without `oos-reinstall.txt` and only selects it for the next power-on.
3. Insert the stick after stock starts. Open **Storage > Upgrade** from the Start menu.
   Under **File Path**, select the GEL on **Removable USB Disk (D)**. Select **OK**, then **Upgrade**.
   Check `oos-install.log` on the stick for `exit code 0`.
4. Remove `oos-dry-run.txt`, keep `oos-reinstall.txt` and run the upgrade again.
   Check `oos-install.log` for `exit code 0`, then power-cycle to start upstream OOS.
5. Check the settings and calibration. On the unit, upstream OOS mounted `/oos` with its journal.
   The settings, calibration copy, address and SSH host keys stayed the same.

## Upstream OOS to this fork

1. Download this fork's labelled bridge package. It is signed with upstream's community key so that upstream's
   update page accepts it. Verify it against this release's `SHA256SUMS`.
2. Upload the bridge at `http://<scope-ip>:8080/update` in a browser or from a terminal with `curl`.
   Use the scope's IP address. A fully qualified host name outside `.local` gets HTTP 403, according to the
   upstream code. The unit upload used `curl` and the IP address. The upload limit is 73,400,320 bytes.
   The `curl` command sends the package as the raw body of `POST /update`. Substitute the bridge filename
   and scope IP address.

   ```sh
   curl --fail-with-body -H 'Expect:' -H 'Content-Type: application/octet-stream' \
     --data-binary @BRIDGE.oosupd 'http://<scope-ip>:8080/update'
   ```

   The upload on the unit returned HTTP 200 with this text.

   ```text
   signature ok, build 20261124-200342: the scope restarts into it by kexec and installs it from there
   ```

3. Wait for the trial to finish. This took about 100 to 120 seconds from upload on the unit.
   The logs showed `on trial (start 1)` followed by `confirmed (trial passed)`.
   Run `oos-update status` on the scope. After confirmation, `trial` reads `none` and `last event` begins with
   `confirmed`. `recovery` holds this fork's build. `rollback` lists the previous upstream image as
   `untracked-...`. `trusted keys` lists only this fork's release key.

## Failed trial

If the trial fails as it did on the unit, the updater restores the previous image and the next start runs it.
The watcher reported this message during the test.

```text
rolled back from 20261124-200342 to untracked-015ca17b3e8a: oos-ui crashed 3 times
```

## After switching

Skip self-calibration under the other system to preserve the existing calibration copy.
The card can come up in High Speed (HS) after a switch and show `SD card is in slow mode.`
It stays in HS until SDR104 is selected again. A cold start or a restage restored SDR104 on the unit.

To power on into stock from either version, use [[Going back to stock]].
