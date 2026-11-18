# SCPI follows stock names and identifies as OOS

- Number: 0025
- Category: code
- State: target: SCPI conformance follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Existing scripts use published stock names such as :CHANnel1:SCALe and :WAVeform:DATA?, while OOS has additional
analyser and reference functions. The programming guide leaves some wire details, including WORD byte order,
unspecified. Compatibility needs an explicit subset and identity rather than a claim to be stock firmware.

## Decision

Use published stock SCPI names and units for existing functions. Accept the shortest published and full case-insensitive
spellings, not intermediate abbreviations. Unsupported commands report SCPI errors. OOS-only functions use their own
keywords or extend the relevant stock subsystem; the reference marks them as extensions. *IDN? answers
`OOS,<model>,<serial>,oos-ui <version>` and never claims another vendor's identity. Where the guide is silent, publish
one stable wire policy: WORD samples are unsigned 16-bit little-endian. Released command names and wire formats do not
change. Each stock command cites its guide section; extension names do not collide with stock names. A command that is
not in docs/scpi.md is not promised. The command reference is docs/scpi.md, with the rebuild specification in
docs/spec/scpi.md.

Check: existing SCPI parser tests; review against the programming guide. Complete reference-to-parser and
no-undocumented-answer coverage is the target follow-up.

## Alternatives

The full stock command tree: rejected because answering for absent functions would lie. An OOS-only tree: rejected
because existing scripts would need rewriting. Stock *IDN? identity: rejected because it invites clients to depend on
unsupported stock behaviour and presents OOS as another vendor's product.

## Consequences

The naming and identity policy is in force. The reference and parser need a complete conformance check before the whole
record is in force.
