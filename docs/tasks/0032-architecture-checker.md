---
id: 32
title: "Close architecture checker blind spots"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Close architecture checker blind spots

## Goal

Enforce each migrated boundary and document unsupported syntax.

## Plan

Reserved paths: none. Reserve paths when claimed.

Add boundary rules with each migration step. Check that migrated areas have rules. Cover re-exports, macros,
inline modules, globs, impl identity, path dependencies and index versus working tree. Use crate boundaries
where syntax checks cannot enforce the guarantee. Audit item: 16 and deferred gate holes.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
