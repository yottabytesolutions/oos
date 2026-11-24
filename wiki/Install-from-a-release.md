Download ready-made files from the [latest release](https://github.com/yottabytesolutions/oos/releases/latest).
For the USB install from stock, download both the GEL and `SHA256SUMS` from that release.

| File | What it is for |
|---|---|
| `DHO800_DHO900_OOS_Install.GEL` | [Install from a USB stick](Install-without-opening-the-scope#from-a-usb-stick) through stock |
| `oos-<version>.img` | [[Install on a second card]] with `--image oos-<version>.img` and skip the build |
| `oos-<version>.oosupd` | [Update a running OOS installation](https://github.com/yottabytesolutions/oos/blob/main/docs/install.md#update-a-running-oos-installation) |
| `SHA256SUMS` | Check the files you downloaded from this release |

## Verify the download

1. Open a terminal in the download folder. Run the command for your computer. Replace the filename if you
   downloaded a different release file.

   | Computer | Command |
   |---|---|
   | macOS | `shasum -a 256 DHO800_DHO900_OOS_Install.GEL` |
   | Linux | `sha256sum DHO800_DHO900_OOS_Install.GEL` |
   | Windows | `certutil -hashfile DHO800_DHO900_OOS_Install.GEL SHA256` |

2. Open `SHA256SUMS` in a text editor. Find the line with that filename and compare the full hash.
   If it differs, stop and download the file again from the same release.

Continue with the install route in the table. For ADB, see [Over ADB](Install-without-opening-the-scope#over-adb).
