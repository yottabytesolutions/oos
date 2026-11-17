# OOS kernel

Everything the kernel of OOS is built from. `build-base.sh` uses the shared build-environment lock under `tools/`.

| Part | What |
|---|---|
| `src/linux-6.12.115-base.tar.xz` | Linux 6.12.115 with the SoC BSP: [`develop-6.12`](https://github.com/rockchip-linux/kernel) at `470f9dccbdc42e7b8a824d0a5c5640a10e9457d2` with the kernel.org 6.12 stable releases up to v6.12.115 merged in. Too large for git, `base.sha256` names it. Compared file by file with both: of 92,158 files 83,877 equal Rockchip's, 8,092 equal the stable tree, 189 are merges of the two, none is new. 37 lines in 20 of the merged files are in neither, mostly the merge following changed interfaces (syscore, reset controls) and build fixes in `bcmdhd`. 13 netfilter files whose names differ from others only in case are left out |
| `series`, `patches/` | 13 patches of ours on that tree, in this order: bug fixes the scope needs, RGB input for the video encoder's JPEG (the live picture of the web page), and a permanent MAC supplied before GMAC registration |
| `dts/` | device tree of the DHO800/DHO900 board |
| `config` | the kernel configuration: only what OOS uses, with 8 modules beside `xdma`; the Rockchip GMAC glue loads after device MAC resolution |
| `firmware/` | `regulatory.db` for the WLAN stack, built into the kernel |
| `modules/xdma/` | Xilinx' XDMA driver, unchanged: [dma_ip_drivers](https://github.com/Xilinx/dma_ip_drivers) at `f09dce768433b2809e7aa6b9d41779265357a2b4`, `XDMA/linux-kernel` |
| `tools/` | answers the `git log` queries of the Rockchip drivers, so the build needs no git tree |

`build-base.sh` fixes time stamps, build names and paths. The historical compiler was gcc 15.2.0;
the selected environment is identified by base/recipe pins, with installed packages recorded beside the output.
On an arm64 host it is Ubuntu's native gcc, on x86-64 Ubuntu's `gcc-aarch64-linux-gnu` of the same version; whether
the two give the same bits is not checked yet (the summaries of `.github/workflows/image.yml` show both checksums).

Licences: kernel and patches GPL-2.0, device tree GPL-2.0+ OR MIT, `xdma` as in its files, `firmware/` as in `firmware/LICENSE.wireless-regdb`.

The archive is too large for the repository: it is the asset of the pre-release
[`kernel-base-6.12.115`](https://github.com/yottabytesolutions/oos/releases/tag/kernel-base-6.12.115).
`tools/ci/kernel-base.sh download` fetches and checks it; `tools/ci/kernel-base.sh publish` makes the release for a
new `base.sha256`.

Whoever passes on an image must offer this source, the archive in `src/` included.


Run `tools/bootstrap.sh --image` after installing Docker/QEMU and supplying the checked source archive.
`cargo xtask doctor --image` verifies its checksum, QEMU and compiler/container prerequisites without building an
image or accessing a unit. Kernel and oos-hw compilers resolve through the same native-platform `kernel` role;
Linux x86-64 also requests the cross compiler packages. Build containers use a separate Docker volume
`oos-kbuild-<worktree>-<environment>` when base/recipe/package requests change. Old volumes remain for active builds. A completed build records `out/base/build-environment`;
image assembly rejects a missing/mismatched compiler environment instead of reusing a stale kernel.
The builder records sorted installed packages in `out/base/build-packages.txt` as informational provenance.
Missing package locks do not block builds; complete per-role locks and the binfmt installer identity move to
0033 (0.3.0). Base/recipe pins do not freeze apt package resolution. Real builds on this macOS arm64 host and
Linux x86-64 runner validation remain untested here; the latter does not block this scope. See `docs/development.md`.
