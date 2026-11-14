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
Audit item: 18.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
