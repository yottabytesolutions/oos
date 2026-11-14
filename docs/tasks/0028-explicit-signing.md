---
id: 28
title: "Make package signing explicit"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Make package signing explicit

## Goal

Keep ordinary builds from silently selecting a private signing identity.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration, separate build, package and sign. Remove automatic private-key discovery.
Require explicit key selection and document public development fixtures and the bench trust policy.
A signature does not authorize installation on the shared unit. Audit item: 19.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
