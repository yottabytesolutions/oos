What can go wrong, what it means and what to do.

Your scope can always be recovered as long as you have the image of your original card ([backup](Install-on-a-second-card#back-up-the-original-card)): the worst case is writing it to a card
again. The table includes fixes that need the card in a reader, so you have to open the scope again
(`diskutil unmountDisk` first). For the stock installer and OOS notices below, start with the card in the scope.

| What you see | Meaning | What to do |
|---|---|---|
| The stock system starts instead of OOS | one-shot mode, or the loader refused the OOS image and used the fallback | With the card in a reader, run `sudo python3 boot/switch.py show /dev/rdiskM`, substituting your card device. Select OOS again. |
| Rigol logo stays, nothing else, on every start | the kernel hangs before OOS runs | `sudo python3 boot/switch.py stock /dev/rdiskM --execute`, then install a working image |
| Black screen; a Rockchip USB device (2207:330c) appears | the loader refused `recovery` and has no fallback | Follow [Return to stock from the card](Going-back-to-stock#return-to-stock-from-the-card). If recovery needs restoring, run `sudo python3 boot/card-install.py restore /dev/rdiskM --backup RECOVERY.bin --execute` with your card device and saved recovery backup. |
| Start-up screen stops at a step | a hardware step failed | Read the stage on the screen. Logs are at `/tmp/oos-boot.status` and `/tmp/oos-cold.log` over SSH. |
| A new build misbehaves | | System > Roll back update, or `oos-update rollback --execute` |
| Nothing helps | | Write `sd-original.img` to the second card again ([write the clone](Install-on-a-second-card#write-the-clone-to-the-second-card)) |

Serial console: UART2, 115200 8N1. The stock loader also reports there why it refuses an image.

## No firmware available

The stock start menu's **Upgrade** icon opens the online updater. To install a GEL from USB, open
**Start menu > Storage > Upgrade**, choose **File Path > Removable USB Disk (D)**, select the GEL, then choose
**OK > Upgrade**. Check `oos-install.log` on the stick for `exit code 0`. See [[Install without opening the scope]].

## The stock system does not see the stick

1. Use a FAT32 stick. The stock system only mounts FAT filesystems.
2. Wait for the stock scope app to start, then insert the stick into the front USB port.
3. Open **Storage > Upgrade > File Path** and look for **Removable USB Disk (D)**. If both the GEL and a
   `._` copy appear, select the GEL whose name matches the release file.

## The stock scope app keeps stopping

A modified stock scope application can crash at start when a stick is already mounted.

1. Unplug the stick and let the stock app start.
2. Insert the stick after the app is up.

## SD card is in slow mode

The card negotiated a slower mode than expected. This can happen on a warm restart after a GEL or bridge install.
Choose **Open System** in the warning to see the requested and negotiated modes beside **Boot mode**.
Choose **Later** to keep using the scope in the current mode.

1. Choose **Restart** if offered, or **Power off** and remove power for 10 seconds before starting again.
2. In one-shot mode, expect stock at the next power-on. See [[First start and choosing how OOS starts]].
3. If status says the updater is falling back to HS, follow the [SDR104 restaging steps](#retry-sdr104-after-a-slow-trial).

## The data area was repaired

Startup found filesystem damage in `/oos`, checked the repair on a copy and wrote it back before mounting the area.
If startup reset a corrupt journal, recent changes may be lost. Check your most recent settings and saved files.
If anything is missing, save `/tmp/oos-data.log` and inspect `/oos/lost+found` for recovered files.
See [data area recovery](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md#data-area-after-an-unclean-shutdown).

## The data area is unavailable

Startup could not prepare or mount `/oos`. Saved settings and the calibration copy are unavailable, and the network
identity is temporary for this start.

1. Save `/tmp/oos-data.log` and contact support before changing the card.
2. If the notice says the next start will be stock Android, power-cycle the scope. If it says the next start is
   unknown, run `oos-oneshot` from a shell and power-cycle once it succeeds.

## An update did not finish after a power cut

If power went off while the new image was starting by kexec, `oos-update status` can report an update that
“did not finish when started by kexec” and was “not installed”. The previous image remains in `recovery`. A cut
while writing the staging slot can instead leave a “Staged update ... was never started” event. The next install
can reuse that slot.

One missed SDR104 start leaves SDR104 available. Two consecutive missed starts select HS. A cold start with
a known slower timing selects HS immediately when the running image requested SDR104.

## Retry SDR104 after a slow trial

Restaging requires a tracked, confirmed image in `recovery`. If the image is untracked after a GEL install, try a
cold start for a slow-mode warning. A successful signed update re-establishes tracking.

1. Select **SDR104** in **System > Boot card mode** and confirm **Restage**.
2. Start OOS again and check the negotiated mode under **Boot card mode**.

## Read the update status

Run `oos-update status` in a shell on OOS. Read these lines together.

| Line | What it tells you |
|---|---|
| `running` | The build in use now. |
| `boot magic` | The next power-on selection. `OOSK` selects OOS, and `KRNL` selects stock Android. |
| `mode` | `persist` reselects OOS after a healthy start. `oneshot` leaves stock selected. Read `boot magic` for the immediate next power-on. |
| `recovery` and `rollback` | The image in the OOS boot slot and the previous image, if available. The rollback slot may temporarily hold a staged candidate. Full status checks the loader IDs and compares image hashes with saved tracking entries. |
| `tracking` | Why the image is untracked, for example after `oos-update leave` or a GEL install of a different build. As of 0.2.1, reinstalling the same tracked build preserves tracking. |
| `trial` and `last event` | Whether an update still awaits confirmation, and the last recorded result or interruption. Look for `confirmed (trial passed)` to verify success. `trial none` also appears after a rollback. |
| `data journal` and `trusted keys` | Whether the data area has a journal and which signing keys the running updater trusts. |

If you see `state UNREADABLE`, check whether `/oos` is mounted. Until the state can be read, the trial and mode
lines show defaults. See [update commands and logs](https://github.com/yottabytesolutions/oos/blob/main/docs/update.md#use).
