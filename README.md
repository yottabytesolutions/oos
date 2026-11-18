# OOS: an open operating system for the Rigol DHO800/DHO900

[![Main](https://github.com/yottabytesolutions/oos/actions/workflows/main.yml/badge.svg?branch=main)](https://github.com/yottabytesolutions/oos/actions/workflows/main.yml)
[![Release](https://img.shields.io/github/v/release/yottabytesolutions/oos?sort=semver)](https://github.com/yottabytesolutions/oos/releases/latest)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/yottabytesolutions/oos/badge)](https://scorecard.dev/viewer/?uri=github.com/yottabytesolutions/oos)
[![Licence: GPL-2.0-or-later](https://img.shields.io/badge/licence-GPL--2.0--or--later-blue)](LICENSE)

OOS replaces the software of a Rigol DHO800/DHO900 oscilloscope with a Linux 6.12 kernel and a new scope application
written in Rust. It runs from the scope's own SD card, next to the stock system, and you can switch back at any time.

![Two channels: a 10:1 probe on the calibration signal and the built-in generator](docs/images/channels.png)

## Status

Experimental. OOS has run on a DHO924S and, by a user report, on a DHO804; several functions have never run on real
hardware. Measurements made with OOS are not calibrated to any standard. What has run on a unit and what is tested on
the host only: [Supported models and features](https://github.com/yottabytesolutions/oos/wiki/Supported-models-and-features#status).

| Model | Runs OOS |
|---|---|
| DHO804 / DHO814 / DHO824 | a DHO804 runs it (user report) |
| DHO914S / DHO924S | a DHO924S runs it (developer's unit) |
| DHO802 / DHO812, DHO914 / DHO924, DHO914-EDU / DHO924-EDU | not yet |

## Safety

Use OOS entirely at your own risk. Installing it with a second card means opening the scope, which voids the warranty.
Back up the whole original card first: without that copy the factory calibration cannot be replaced. Connect the scope
to a trusted bench network only, never directly to the internet. Read the
[disclaimer](https://github.com/yottabytesolutions/oos/wiki/Disclaimer) before you install.

## User guide

The user guide is in the [wiki](https://github.com/yottabytesolutions/oos/wiki):

- [Home](https://github.com/yottabytesolutions/oos/wiki): what OOS is and an index of all pages
- [Installation](https://github.com/yottabytesolutions/oos/wiki/Installation): on a second card, from a USB stick or
  over ADB
- [Troubleshooting](https://github.com/yottabytesolutions/oos/wiki/Troubleshooting): what you see and what to do
- [Going back to stock](https://github.com/yottabytesolutions/oos/wiki/Going-back-to-stock): for one start or for good

The wiki is generated from [wiki/](wiki/Home.md) in this repository. Technical documentation is in
[docs/](docs/README.md), the changes per release in [CHANGELOG.md](CHANGELOG.md).

## Contributing

Reports, measurements and code are welcome. A test on a model that has not run OOS yet is the most useful thing right
now.

- [CONTRIBUTING.md](CONTRIBUTING.md): ground rules, quick start and the review gate
- [AGENTS.md](AGENTS.md): the rules for every change, by a person or a coding agent
- [SUPPORT.md](SUPPORT.md): where to ask questions and report bugs
- [SECURITY.md](SECURITY.md): how to report a vulnerability privately
- [GOVERNANCE.md](GOVERNANCE.md): who decides; everyone follows the [Code of Conduct](CODE_OF_CONDUCT.md)
- [Discussions](https://github.com/yottabytesolutions/oos/discussions) for questions,
  [issues](https://github.com/yottabytesolutions/oos/issues) for bugs and hardware reports

## Licence

OOS is free software under the GNU General Public License, version 2 or (at your option) any later version. See
[LICENSE](LICENSE). Third-party parts keep their own licences: [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md)
lists each with its version, licence and source. If you pass on an OOS image, the GPL requires you to provide the
corresponding source as well: pass on the source archives of its release and this tree at the release's tag, or point
to the release.

## Thanks

This tree is a hard fork of **[OOS](https://github.com/dekajoule/oos)** by Julian Decker and its contributors, at
upstream commit 15b819e2 ([ADR 0001](docs/adr/0001-hard-fork.md)). Their copyright and
the GPL-2.0-or-later licence stay with every file they wrote.

OOS also builds on [NovaOS](https://github.com/Bradyok/novaos-kernel-source) by Olympian LED, Inc., whose published
kernel source showed where to start; the findings of [Norbert Kiszka](https://github.com/norbertkiszka); and Linux,
Rockchip's board support, the Xilinx XDMA driver, BusyBox, e2fsprogs, wpa_supplicant, linux-firmware, Rust and Roboto.
The full list with what each contributed: [Thanks](https://github.com/yottabytesolutions/oos/wiki/Thanks-and-licences).
