# No vendor material in the tree

- Number: 0002
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

OOS replaces a commercial instrument's firmware and is published under GPL-2.0-or-later. Firmware binaries, calibration
data and copied implementation details are not material the project has the right to republish. Hardware knowledge must
therefore enter as independently described behaviour, rather than copied vendor material.

## Decision

Nothing from stock or modified vendor firmware enters the tree, history, pull request texts or issues: firmware images,
binaries, libraries, bitstreams, code, decompiled listings, copied tables or strings, function/class/file names,
calibration files and `vendor.bin` are excluded.

Describe hardware behaviour in the contributor's own words and mark how it is known (ADR 0012). Published user material
is allowed: programming-guide command names, manuals and component data sheets.

Check: review only; the privacy scanner is a backstop, not a vendor-provenance check.

## Alternatives

Copy useful vendor implementation details: rejected because usefulness does not confer publication rights. Exclude only
binaries: rejected because copied code, data and surrounding text have the same provenance problem.

## Consequences

Contributions containing vendor material cannot merge. Their issue or pull request text is edited or removed.
