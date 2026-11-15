---
id: 33
title: "Make image and GPU builds reproducible"
kind: chore
status: planned
release: 0.3.0
owner: none
depends: []
---

# Make image and GPU builds reproducible

## Goal

Build the same payload from the same source regardless of build date or local container cache.

## Plan

Reserved paths: none. Reserve paths when claimed.

Pin container digests and package snapshots or versioned build images. Accept a fixed source timestamp.
Record compiler and container identities. Separate reproducible payloads from variable package metadata.
Complete the per-role package manifest locks and pin the binfmt installer identity moved from task 0025.
0025 supplies base/recipe pins and informational installed-package provenance only.
Audit item: 18.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-15: Coordinator moved complete per-role package manifest locks and the binfmt installer identity
  from task 0025 into this release 0.3.0 scope. No implementation claimed here.

- 2026-11-14: seeded from task 0001. Not yet claimed.
