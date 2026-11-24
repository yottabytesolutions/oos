The projects OOS builds on, and the licences of OOS and its parts.

## Thanks

This tree is a hard fork of **[OOS](https://github.com/dekajoule/oos)** at
upstream commit 15b819e2 ([ADR 0001](https://github.com/yottabytesolutions/oos/blob/main/docs/adr/0001-hard-fork.md)). The original copyright notices and GPL-2.0-or-later
licence stay with the inherited files.

OOS builds on the work of these projects. Thank you to everyone behind them:

- **[NovaOS](https://github.com/Bradyok/novaos-kernel-source)** by Olympian LED, Inc., for publishing the GPL source of
  its Linux port to this hardware, which showed what a kernel needs here and where OOS started. The patches, device tree
  and configuration of the OOS kernel are written anew today. Its install notes also helped with the loader's image format.
- Published reverse-engineering findings and modified firmware demonstrated more of the hardware's capabilities.
- **[Linux](https://kernel.org)**, the kernel underneath everything, and **[Rockchip](https://github.com/rockchip-linux/kernel)**
  for the board support it runs on here.
- **[Xilinx XDMA driver](https://github.com/Xilinx/dma_ip_drivers)**, which connects the SoC to the FPGA over PCIe.
- **[BusyBox](https://busybox.net)**, the userland of the image.
- **[e2fsprogs](https://e2fsprogs.sourceforge.net)**, which checks the data partition at every start.
- **[wpa_supplicant](https://w1.fi/wpa_supplicant/)**, for WLAN.
- **[linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git)** and Realtek, for the
  firmware of the WLAN stick.
- **[Rust](https://www.rust-lang.org)** and the **[libc](https://github.com/rust-lang/libc)** crate, the only dependency
  of the OOS code on the scope. The Windows desktop build also uses **[minifb](https://github.com/emoon/rust_minifb)**
  and **[windows-sys](https://github.com/microsoft/windows-rs)** for its window and network.
- **[Roboto](https://github.com/googlefonts/roboto-classic)**, the typeface of the screen.

Sources and licences of the kernel parts are listed in [kernel/README.md](https://github.com/yottabytesolutions/oos/blob/main/kernel/README.md).

## Licences

OOS is free software under the GNU General Public License, version 2 or (at your option) any later version. See
[LICENSE](https://github.com/yottabytesolutions/oos/blob/main/LICENSE).

Third-party parts keep their own licences. [THIRD-PARTY-LICENSES.md](https://github.com/yottabytesolutions/oos/blob/main/THIRD-PARTY-LICENSES.md) lists every one of
them with its version, its licence, where its licence text is and which source files go with a release: the Linux
kernel and the XDMA driver, BusyBox, glibc and GCC's runtime, Mesa, libdrm and zlib (the GPU helper,
[gpu/README.md](https://github.com/yottabytesolutions/oos/blob/main/gpu/README.md)), e2fsck, kexec, wpa_supplicant with libnl, nftables with libmnl and libnftnl, Dropbear,
OpenSSH's `sftp-server`, musl, the Rust standard library, the Roboto font, the wireless regulatory database and the
RTL8188EU firmware. An image built from this tree carries the licence texts and that list under `/etc/licenses`.

Every release carries the source archives of the binaries in its image next to the image. If you pass on an OOS
image, the GPL requires you to provide the corresponding source as well: pass those archives and this tree at the
release's tag on with it, or point to the release.
