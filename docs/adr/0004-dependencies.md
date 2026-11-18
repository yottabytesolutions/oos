# Dependencies are listed, licence-checked and owner-approved

- Number: 0004
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

The former libc-only rule left hand-written protocol servers, repeated accept loops and socket writes on the frame
thread with a 2-second stall timeout. Maintained crates can replace that code; cargo-deny now checks GPL-compatible
licences and known advisories. This makes explicit dependency review more useful than a blanket ban.

## Decision

Every workspace crate can use crates.io dependencies. Each direct dependency has an entry in its
`xtask/data/architecture.txt` crate line and in `docs/fork/deps.md` explaining what it replaces. The maintainer approves
each new dependency. `cargo deny` accepts only the GPL-2.0-compatible licences in `deny.toml` and rejects known
advisories. ADR 0002 also applies to dependencies.

Check: `architecture`, the deny CI check, CODEOWNERS on xtask/data/architecture.txt, and review of docs/fork/deps.md.

## Alternatives

Keep the libc-only rule: rejected because it preserves duplicate protocol and event-loop code. Accept arbitrary crates
without review: rejected because licence/advisory checks do not establish quality or explain which local implementation
the dependency replaces.

## Consequences

Cargo.lock pins versions. Licence and advisory checks do not establish code quality.
