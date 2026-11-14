# 0008: Ports and adapters where they pay

Status: proposed. 2026-11-14.

## Context

`Board`, `Protocol`, hal's `Bus` and usbtmc's `Bulk` already show the pattern works here: each has a fake, and code
behind it is tested on the host. Other IO (DRM, panel, sockets, files, clock) is called directly from `ui.rs`, so the
step cannot run in a test.

## Decision

- Ports for IO the step depends on: `Board`, `display::Present`, `input::Source`, `remote::Transport`, `store::Files`,
  `clock::Clock`, `instruments::Bulk`, `Protocol`.
- Each port has a fake next to it.
- No port for pure helpers (`font`, `layout`, `json`, `gfx`). A trait with one implementation and no test need is
  ceremony.

## Consequences

- `Session` runs on the host end to end with fakes, which golden pictures and change-storm tests need.
- Adapters do not call each other. The boundary checker enforces it.
