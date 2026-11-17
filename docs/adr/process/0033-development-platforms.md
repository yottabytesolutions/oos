# ADR 0033: Development runs on Linux, the BSDs and macOS; Windows users develop in WSL or a Linux VM

- Status: accepted, amended by ADR 0036 (the open question: the window moved to oos-desktop)
- Date: 2026-11-17
- Category: process

## Context

OOS is a Unix program for a Unix platform: Linux on the scope. Developing it on a Unix host means fewer exceptions
to handle in the tooling, the tests and the review. That is the main reason for this record; the rest of the context
below is the detail.

The tree builds for one target, the scope (`aarch64-unknown-linux-musl`), and its tooling is written for a POSIX host:
the gate is `cargo xtask check` (ADR 0031), the helpers in `tools/` are shell and Python scripts, the image checks need
Docker with arm64 emulation, and the hooks in `.githooks/` are shell scripts. CI runs on Linux x86-64 runners; the
documented setup ([development.md](../../development.md)) is macOS arm64 or Linux x86-64.

Upstream also carried a native Windows path: `build-windows.ps1` and `run-windows.ps1` for a desktop window of the
screen (`ui/src/desktop.rs`, `cfg(windows)`, fe016e4). Nobody runs the gate on native Windows and CI does not build
it. Earlier records weighed tools against that platform: ADR 0020 rejected `make` and `xtask` partly because "`make`
is not on every Windows computer". Keeping a second, untested host family in step costs review time on every change
to the tooling, and it holds back tools that assume a POSIX shell.

WSL 2 and a Linux virtual machine give a Windows computer a full Linux host. The gate and the scripts run there as on
any Linux computer (assumed; not yet run by a maintainer in WSL).

## Decision

Development is supported on Linux and macOS, and on the BSDs as best effort. Windows users develop in WSL 2 or a Linux virtual machine,
which count as Linux. The project does not add native Windows code paths or build scripts, and does not hold back a
feature or a tool because native Windows lacks it.

- New project tooling and new checks written in Rust go into `xtask`, as a step or a subcommand of the gate
  (ADR 0031). This replaces ADR 0020's rejection of `xtask`. Small helpers stay shell or Python scripts in `tools/`,
  written for a POSIX shell; a script that grows into a check moves into `xtask`.
- Native Windows portability is not a review finding. A change may use POSIX paths, signals, sockets and shell tools.
- Windows users are welcome. Documentation points them to WSL or a VM in a friendly way and does not lecture.
- Linux and macOS are supported. Verified hosts are the ones CI and the setup document name: Linux x86-64 and macOS
  arm64.
- The BSDs are best effort. The project does not guarantee them and CI does not run there, but tooling stays POSIX
  and a reported BSD problem is picked up and fixed like any other bug.

## Alternatives

- **Keep native Windows as a development platform:** every tool and script would need a second implementation or a
  portable subset, tested on a host family that no maintainer and no CI job uses. The PowerShell scripts were already
  the only part of the tree outside the gate.
- **Linux only:** macOS is the main maintainer host, and the BSDs cost nothing as long as tooling stays POSIX.

## Consequences

- CONTRIBUTING points Windows users to WSL or a VM. Review findings about native Windows portability are out of
  scope ([AGENTS.md](../../../AGENTS.md)).
- ADR 0036 moves the native desktop window out of this tree; ADR 0037 governs dependencies.
- Worth a second look when a maintainer develops on native Windows, or when a tool needs a host this record does not
  cover.

## Open question

The tree still holds a native Windows desktop window of the screen: `ui/src/desktop/`, `ui/src/announce/windows.rs`,
`build-windows.ps1`, `run-windows.ps1` and the `cfg(windows)` branches of `oos-ui`. It is a product feature, not
only a development path, and its future is decided separately. Until then nothing in the gate or CI builds it.

## References

Discussion [#9](https://github.com/yottabytesolutions/oos/discussions/9) (dropping the PowerShell build),
ADR 0020 (superseded), ADR 0036, ADR 0037, ADR 0031, [CONTRIBUTING.md](../../../CONTRIBUTING.md),
[development.md](../../development.md), `ui/src/desktop.rs`.
