# ADR 0036: The desktop view is the browser UI; the native Windows window is its own project

- Status: accepted
- Date: 2026-11-17
- Category: code (touches process: ADR 0033)

## Context

ADR 0033 left one question open: the tree held a native Windows desktop window of the screen (`ui/src/desktop/`,
`ui/src/announce/windows.rs`, `build-windows.ps1`, `run-windows.ps1` and the `cfg(windows)` branches of `oos-ui`).
It is a product feature, a simulator and viewer on a computer, not only a development path. Nothing in the gate or CI
built it, and it was the only user of a desktop build's platform crates (`minifb`, `windows-sys`).

The same screen already runs on every computer behind the host server, `oos-ui --serve`: the demo source with the web
remote, SCPI, HiSLIP and VXI-11 on loopback ports. A browser shows the web remote.

## Decision

The desktop view in this tree has one code path on every computer: the browser UI behind `oos-ui --serve`. On Windows,
`oos-ui` runs in WSL and the browser on Windows. The native Windows window moved with its history to its own project,
[yottabytesolutions/oos-desktop](https://github.com/yottabytesolutions/oos-desktop), and leaves this tree.

## Alternatives

- **Keep the window as an unsupported build:** code that nothing builds or tests rots, and every change near it pays
  for review of a path no one checks.
- **Port the window to Linux and macOS:** a second view of the same screen, kept pixel-equal with the browser UI, for
  no user that the browser does not already serve.

## Consequences

- `oos-ui --serve` started by hand prints where to connect. README and CONTRIBUTING name it as the way to run OOS on a
  computer.
- Code that only the window used goes with it: the host DNS-SD announcer, the slot choice for the native file dialog,
  the front-panel button. The licence lists drop `minifb` and `windows-sys`.
- A feature of the window that the browser UI lacks (the native file dialog, the front-panel window) is rebuilt in the
  browser UI when it is wanted, not kept as a second path.
- ADR 0037 governs any crate a desktop path needs later; no desktop path uses a platform crate today.

## References

ADR 0024, ADR 0033, ADR 0037, [oos-desktop](https://github.com/yottabytesolutions/oos-desktop),
discussion [#9](https://github.com/yottabytesolutions/oos/discussions/9), `ui/src/main.rs` (`serve`), `ui/src/net6.rs`.
