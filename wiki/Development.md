Where to start when you want to change OOS. The rules and the reference material live in the repository, not in
this wiki.

| Read | For |
|---|---|
| [CONTRIBUTING.md](https://github.com/yottabytesolutions/oos/blob/main/CONTRIBUTING.md) | Ground rules, quick start, branches, commits, pull requests, review and risk tiers |
| [AGENTS.md](https://github.com/yottabytesolutions/oos/blob/main/AGENTS.md) | The rules for every change, by a person or a coding agent |
| [docs/](https://github.com/yottabytesolutions/oos/blob/main/docs/README.md) | Technical documentation: architecture, boot, hardware, register-level protocols |
| [docs/development.md](https://github.com/yottabytesolutions/oos/blob/main/docs/development.md) | The gate (`cargo xtask check`), ratchets, hooks and CI |
| [SECURITY.md](https://github.com/yottabytesolutions/oos/blob/main/SECURITY.md) | How to report a vulnerability |
| [SUPPORT.md](https://github.com/yottabytesolutions/oos/blob/main/SUPPORT.md) | Where to ask questions and report bugs |
| [GOVERNANCE.md](https://github.com/yottabytesolutions/oos/blob/main/GOVERNANCE.md) | Who decides; the [Code of Conduct](https://github.com/yottabytesolutions/oos/blob/main/CODE_OF_CONDUCT.md) applies to everyone |

To build an image yourself, see [[Building OOS]].

## Repository layout

| Path | Content |
|---|---|
| `hal/` | Hardware layer: FPGA register sequences, front ends, ADC, PLL, cold boot, models, updates |
| `dsp/` | Signal processing: triggers, measurements, maths, decoders, analysers |
| `ui/` | `oos-ui`: scope screen, SCPI, VXI-11, HiSLIP, web remote, LAN announcement |
| `fpgad/` | `oos-fpgad` bring-up tool and `oos-update` |
| `boot/` | Boot image packer, image build, card installer, USB installer, stock/OOS switch |
| `rootfs/initramfs/` | Init scripts and firmware of the image |
| `rootfs/build-wlan-tools.sh`, `rootfs/build-fsck.sh` | Build wpa_supplicant and e2fsck in Docker |
| `keys/` | Public keys trusted for updates, community signing key |
| `tools/` | Card cloning and analysis, device inventory, Python models ([README](https://github.com/yottabytesolutions/oos/blob/main/tools/README.md)) |
| `kernel/` | Kernel source, own patches, device tree, configuration, build in Docker, licences ([README](https://github.com/yottabytesolutions/oos/blob/main/kernel/README.md)) |
| `CHANGELOG.md` | What changed between releases |
| `docs/` | Technical documentation: architecture, boot, hardware, register-level protocols ([index](https://github.com/yottabytesolutions/oos/blob/main/docs/README.md)) |

## Tests and tools

Each crate builds on its own. The hardware code is Linux-only, so it is checked for the target as well.
`cargo xtask check` is the gate: formatting, limits, lints, the tests of all crates in debug and release, the target
build, coverage and the script checks ([ADR 0031](https://github.com/yottabytesolutions/oos/blob/main/docs/adr/process/0031-the-gate-is-cargo-xtask-check.md),
[docs/development.md](https://github.com/yottabytesolutions/oos/blob/main/docs/development.md)). `cargo xtask check --fast` runs the seconds-long part:

```sh
cargo xtask check
(cd ui && cargo test)                                 # one crate
(cd ui && cargo check --target aarch64-unknown-linux-musl)
```

Coding agents, and people new to the tree, start with [AGENTS.md](https://github.com/yottabytesolutions/oos/blob/main/AGENTS.md).

Tests that need the calibration files of a real unit are skipped without them. To run them, copy `/rigol/data` of your
scope to `local/cal-full` and `local/cal` (the folder `local/` stays out of git). `OOS_CAL_DIR` overrides this for most of
them.

`tools/plan-matrix.sh` prints the register sequences of a fixed set of acquisitions without touching the hardware. Run
it with two builds and compare the output to prove that a change leaves the analog path alone.

The test `every_menu_fits` walks all menus and fails on text wider than its control or a dialog off the screen.
`OOS_GALLERY=<dir> cargo test gallery -- --ignored` in `ui/` writes every screen as a picture.

`tools/gel-sim.sh` runs the USB install on a file laid out like a stock card.

`tools/usb-sim.sh` tests the handling of USB sticks in a virtual machine, with disks that get pulled.

`tools/wlan-sim.sh` tests the WLAN path on simulated radios in a virtual machine, without a stick. It needs Docker,
`qemu-system-aarch64` and one finished image build.

Start with the [hardware overview](https://yottabytesolutions.github.io/oos/hardware/overview.html) and [docs/architecture/README.md](https://github.com/yottabytesolutions/oos/blob/main/docs/architecture/README.md).
