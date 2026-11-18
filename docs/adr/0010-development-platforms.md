# Development supports Linux and macOS; BSDs are best effort; Windows uses WSL or a VM

- Number: 0010
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

The gate, hooks and helpers use POSIX shell conventions; the image checks need Docker and ARM emulation. Linux CI and
the maintainer's macOS host exercise those paths, while no maintainer or CI job exercises native Windows. Maintaining an
untested second tooling implementation would add review work to every change.

## Decision

Linux and macOS are supported development hosts. The BSDs are best effort: reported failures are fixed, but every change
need not be tested there. Windows development uses WSL 2 or a Linux VM. Native Windows support is outside the review
scope. Project checks are Rust in xtask; a helper that grows into a check moves there.

Check: GitHub-hosted Linux CI; review checks host portability and tooling placement. macOS and BSD verification is
reported separately.

## Alternatives

Native Windows tooling: rejected because it needs a second implementation or restricted portable subset with no
maintainer or CI coverage. Linux only: rejected because macOS is a maintainer host and POSIX tooling permits BSD use.

## Consequences

No native Windows compatibility layer is maintained.
