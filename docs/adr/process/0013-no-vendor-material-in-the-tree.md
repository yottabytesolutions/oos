# ADR 0013: No vendor material in the tree

- Status: accepted (rewritten 2026-11-17: the dependency rule it carried moved to ADR 0037)
- Date: 2026-11-07
- Category: process

## Context

OOS is published under GPL-2.0-or-later and replaces the software of a commercial instrument. Everything in the tree,
its history and the texts around it must be ours to publish. Material from the vendor's firmware is not.

## Decision

Nothing from the vendor's firmware enters the tree, its history, pull request texts or issues:

- no firmware images, binaries, libraries or bitstreams from a stock or modified firmware;
- no code from it, decompiled or not, and no listings, copied tables, strings, or the names of its functions,
  classes and files;
- no calibration files and no `vendor.bin`.

The hardware is described as behaviour (registers, sequences, formats) in the contributor's own words, with how it
is known (ADR 0015). What the vendor publishes for users may be used: the command names of the programming guide,
the manuals, and the data sheets of the parts.

## Consequences

- Contributions that copy from the vendor's firmware cannot be merged, however useful.
- A pull request or issue that breaks the rule is not merged, and its text is edited or removed.
- Reverse-engineered knowledge enters as documented behaviour, which also makes it reviewable.

## References

ADR 0015, ADR 0040, [CONTRIBUTING.md](../../../CONTRIBUTING.md).
