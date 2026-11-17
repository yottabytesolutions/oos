Ready-made files from a GitHub release, without building anything.

The [latest release](https://github.com/yottabytesolutions/oos/releases/latest) has ready-made files. You need no Rust and no
Docker for them:

| File | What it is for |
|---|---|
| `DHO800_DHO900_OOS_Install.GEL` | install from a USB stick through the stock system, without opening the scope: go on at [From a USB stick](Install-without-opening-the-scope#from-a-usb-stick) |
| `oos-<version>.img` | install on a second card: follow [[Install on a second card]] with `--image oos-<version>.img`, and skip the build |
| `oos-<version>.oosupd` | update a scope that already runs OOS: open `http://<scope>:8080/update` and choose the file, then switch off at the mains, wait 30 seconds and switch on. From 0.1.6 or older, first `oos-0.1.7.oosupd` from the [0.1.7 release](https://github.com/yottabytesolutions/oos/releases/tag/v0.1.7): later builds bring another device tree, which only 0.1.7 and later can try before they install it |
| `SHA256SUMS` | the checksums of all of them |

Check what you downloaded. The number has to be the same as in the file's line of `SHA256SUMS`:

```sh
shasum -a 256 DHO800_DHO900_OOS_Install.GEL      # macOS
sha256sum DHO800_DHO900_OOS_Install.GEL          # Linux
certutil -hashfile DHO800_DHO900_OOS_Install.GEL SHA256      # Windows
```

The ADB install needs your own build: [[Building OOS]].
