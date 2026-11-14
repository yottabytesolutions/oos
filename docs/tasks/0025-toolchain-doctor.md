---
id: 25
title: "Pin build tools and add a doctor command"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: []
---

# Pin build tools and add a doctor command

## Goal

Give contributors and CI the same tested toolchain and prerequisites.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration, add rust-toolchain.toml with components and target. Pin cargo-llvm-cov.
Add cargo xtask doctor for CI and humans to verify versions, linker, Python and optional GPU/image prerequisites.
Audit item: 9.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
