What can go wrong, what it means and what to do.

Your scope can always be recovered as long as you have the image of your original card ([backup](Install-on-a-second-card#back-up-the-original-card)): the worst case is writing it to a card
again. Unless noted, the fixes below need the card in the reader, so you have to open the scope again
(`diskutil unmountDisk` first).

| What you see | Meaning | What to do |
|---|---|---|
| The stock system starts instead of OOS | one-shot mode, or the loader refused the OOS image and used the fallback | `boot/switch.py show`, then select OOS again. If that repeats, rebuild the image. |
| Rigol logo stays, nothing else, on every start | the kernel hangs before OOS runs | `sudo python3 boot/switch.py stock /dev/rdiskM --execute`, then install a working image |
| Black screen; a Rockchip USB device (2207:330c) appears | the loader refused `recovery` and has no fallback | `switch.py stock`. If needed, `sudo python3 boot/card-install.py restore /dev/rdiskM --backup backups/recovery-<date>.bin --execute` |
| Start-up screen stops at a step | a hardware step failed | Read the stage on the screen. Logs are at `/tmp/oos-boot.status` and `/tmp/oos-cold.log` over SSH. |
| A new build misbehaves | | System > Roll back update, or `oos-update rollback --execute` |
| Nothing helps | | Write `sd-original.img` to the second card again ([write the clone](Install-on-a-second-card#write-the-clone-to-the-second-card)) |

Serial console: UART2, 115200 8N1. The stock loader also reports there why it refuses an image.
