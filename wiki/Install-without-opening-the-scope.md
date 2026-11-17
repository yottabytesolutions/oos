Both ways install OOS from the running stock system onto the card that is in the scope. The USB stick takes the update
file of the release or of your build ([[Building OOS]]); ADB needs the build.

## Read this before you start

- **Both ways have run on one DHO924S only, whose stock system is a modified one.** Try them on a card you can take out
  if you can.
- **There is no second card to fall back on.** A refused image is harmless, the loader starts the stock copy. But if the
  kernel hangs before the first script of the image runs, every power-on hangs the same way. The card then has to come
  out, so **the scope has to be opened after all.**
- The install goes ahead on its own only on a model whose hardware OOS has run on: DHO804, DHO814, DHO824, DHO914S
  and DHO924S ([docs/install-usb.md](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md#4-models)
  section 4, which also says how to install on another model at your own risk). See the [[Disclaimer]].

The install writes `recovery`, the first 32 MiB of `backup` and, as its very last step, the 4 bytes of `boot` that select
OOS. A failure or power cut before that step leaves a card that starts stock as before, and running the same install
again finishes the job. No stock update step runs, and the FPGA flash and `/rigol` are not touched. Details:
[docs/install-usb.md](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md).

## From a USB stick

You need a FAT32 USB stick with at least 200 MiB free, and `DHO800_DHO900_OOS_Install.GEL`: download it from the
[latest release](https://github.com/yottabytesolutions/oos/releases/latest) and check it against `SHA256SUMS`, or take it from
`target/image/` of your own build.

1. Copy the file to the top folder of the stick. Keep the name: the scope ignores a file
   whose name does not end in `.GEL` and contain `DHO800_DHO900`.
2. For a dry run, put an empty file `oos-dry-run.txt` next to it. This runs all checks and saves the rescue set, without
   writing the card.
3. Start the stock system, put the stick into the front USB port, open **Storage > Upgrade**, choose the file and start
   it. This takes about a minute. The screen says at the end whether it worked. If Storage does not show the stick,
   switch the scope off and on and look again ([docs/install-usb.md](https://github.com/yottabytesolutions/oos/blob/main/docs/install-usb.md#the-stock-system-does-not-see-the-stick), section 5). If the list also
   has a small file starting with `._`, that one comes from macOS: choose the real file of about 16 MB.
4. Read `oos-install.log` on the stick on a computer. It should end with `exit code 0`.
5. Delete `oos-dry-run.txt` and run the update again. The log should end with `OOS selected for the next power-on`
   and `exit code 0`.
6. Copy the folder `OOS-rescue` from the stick to a computer and keep it. It holds your calibration (`rigol-data`), the
   stock `recovery` and the start of the card.
7. Switch the scope off at the mains, wait 30 seconds and switch it on. OOS starts.

The scope does not restart by itself after this update. OOS starts once: the power-on after it starts stock again. To stay on
OOS, set **System > Boot mode** to **Persist**. Later versions go in through the `.oosupd` file of a release.

## Over ADB

You need `adb` on your computer and a scope that answers on its network port 55555 with root access. This is known from a
modified stock system; it has not been checked on an unmodified one.

```sh
adb connect <scope>:55555
adb root
adb connect <scope>:55555        # adb root drops the connection

# your calibration and licences, onto your computer
adb pull /rigol/data rigol-data

# the tool for the scope, not the one in target/release
adb push target/aarch64-unknown-linux-musl/release/oos-update /data/local/tmp/
adb push target/image/oos-dev.oosupd /data/local/tmp/
adb shell chmod 755 /data/local/tmp/oos-update

# dry run: all checks, the rescue set, the card is not written
adb shell /data/local/tmp/oos-update gel-install /data/local/tmp/oos-dev.oosupd --save /data/local/tmp/OOS-rescue

# the rescue set onto your computer: keep it
adb pull /data/local/tmp/OOS-rescue

# the install: last line "OOS selected for the next power-on"
adb shell /data/local/tmp/oos-update gel-install /data/local/tmp/oos-dev.oosupd --save /data/local/tmp/OOS-rescue --execute
```

Before the last command, check the rescue set on your computer: run `sha256sum -c SHA256SUMS` in `OOS-rescue`
(`shasum -a 256 -c SHA256SUMS` on macOS). After the install, switch the scope off and on. OOS starts.

The rescue set takes 132 MiB on the scope. Afterwards you can remove it with
`adb shell rm -rf /data/local/tmp/OOS-rescue /data/local/tmp/oos-update /data/local/tmp/oos-dev.oosupd`.

## After the install

OOS starts once, then the next power-on starts stock again ([one-shot](First-start-and-choosing-how-OOS-starts#choose-how-oos-starts)). To start OOS once more,
run the same install from the stock system: it writes nothing but the selection. To stay on OOS, use
System > Boot mode > Persist on OOS.

The stock `recovery` is replaced by OOS. Everyday stock use does not need it, but anything that starts into it no longer
works. Putting it back needs the card in a reader, with `recovery.bin` from the rescue set and `boot/card-install.py restore`.
