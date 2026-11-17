# ADR 0030: This tree is a hard fork of OOS

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

This tree started as a fork of OOS by Julian Decker (upstream `github.com/dekajoule/oos`, formerly
`deckerjulian/oos`). The fork restructured the code on its own branch (`overhaul`) under tooling that upstream does not
use: `cargo xtask check` as the gate, shrink-only ratchets, per-crate lint tables, move-aware size limits.

Upstream was merged in regularly (task 0066), and changes went back as pull requests. Both directions cost more than
they returned:

- Upstream code does not follow the size and lint limits. Each intake needed a special ratchet rebaseline path in the
  gate ([architecture/0018](../../architecture/adr/0018-upstream-rebaseline.md)) and still failed the gate on a large
  intake (integration 30, aborted).
- Work sent upstream had to be ported back to upstream's layout and rules (ADR 0017, ADR 0019, ADR 0020) and lost the
  structure it was built for.
- Two record series with colliding numbers (`docs/adr/` and `docs/architecture/adr/`) had to be cited by path.

The owner decided on 2026-11-17 to fork hard.

## Decision

- This tree is a hard fork of OOS. The fork point is tag `hard-fork-20261117` (`overhaul` c654b1f0). The last
  upstream commit merged in is 15b819e2.
- `main` is the trunk. `overhaul` stays as a historical ref and receives no further commits.
- No upstream merges, no bridge merges and no upstream intake. Upstream is not tracked. A single upstream change may
  enter by cherry-pick: the commit keeps its author and carries the trailer `Cherry-picked-from: upstream <hash>`
  with the full upstream hash. It then passes the gate like any other change; nothing is rebaselined for it.
- No pull requests to upstream unless the owner asks for one.
- The licence stays GPL-2.0-or-later. Copyright lines, `LICENSE`, the authorship in the history and the attribution in
  `README.md` stay. Files written for this fork carry the SPDX line like the rest (ADR 0019).
- The tree's own rules are the official ones: the gate is `cargo xtask check` (ADR 0031), lints and ratchets follow
  ADR 0032. Where an upstream record conflicts with them, it is superseded or amended in place.

**Record series.** Both series stay where they are; they are not renumbered. A renumbering would rewrite about 150
references, many of them bare numbers like `adr/0011` in finished task files whose series is only clear from
context. That is not a mechanical change, and finished task files are kept as written.

- `docs/adr/<category>/NNNN-*.md` is the one series for new records. It continues at 0030. Numbers 0027 to 0029 are
  not used here: upstream used them after the fork point, and reusing them would make a cherry-picked reference
  ambiguous. Cite these records as `ADR NNNN`.
- `docs/architecture/adr/0000-0019` is closed to new records. Its records stay valid and are amended or superseded
  like any other. Cite them by path or as `architecture/NNNN`, never as a bare `ADR NNNN`.

## Alternatives

- **Keep merging upstream:** the rebaseline path, the double rules and the port-back cost stay, and every intake
  widens the ratchets that are meant to shrink.
- **Rebase onto upstream and send everything as pull requests:** the restructuring is too large to land upstream in
  reviewable pieces, and upstream decides on its own rules.
- **Renumber our records into one series:** see Record series above.

## Consequences

- The gate has no upstream exception any more: `xtask/src/rebaseline.rs` and the pending-rebaseline path of the fast
  gate are removed, and every ratchet only shrinks.
- Upstream fixes are found by reading upstream, not by merging it. A cherry-pick that does not fit the limits is
  reworked before it lands.
- Publication of `main` and the switch of the public default branch are separate steps with the owner's go.
- Worth a second look if upstream adopts compatible tooling and a shared trunk becomes cheaper than two.

## References

Tag `hard-fork-20261117`, [architecture/0018](../../architecture/adr/0018-upstream-rebaseline.md) (superseded),
ADR 0026, ADR 0031, ADR 0032, [README.md](../../../README.md), [AGENTS.md](../../../AGENTS.md).
